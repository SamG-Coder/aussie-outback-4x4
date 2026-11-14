//! The viewer on an Android phone: a game there is an app, and this is what the app runs.
//!
//! It is the desktop's viewer (`desktop.rs`), window, game and all, started another way and
//! with three things done through the phone's own system, which is reached in Java:
//!
//! * **Starting.** An app is not run with arguments. Its activity
//!   (`android/java/.../MainActivity.java`) unpacks the game's files, the same ones a
//!   download for Windows has beside its program, into the app's own folder; the library is
//!   loaded, and [`android_main`] is called on a thread of its own. It goes to that folder and
//!   does what the `game.txt` there says (`game.rs`).
//! * **The pages.** The menus and instruments are the same pages, in a web view of the
//!   system's laid over the game (`overlay_android.rs`). The activity opens it, runs scripts
//!   in it, and keeps what the page says until it is asked for.
//! * **Fetching the maps.** There is no `curl` on a phone: what `fetch.rs` asks of the site
//!   is asked by the activity, with the system's own web client and its certificates.
//!
//! What is said in the log goes to the system's (`adb logcat -s solstice`).
use crate::fetch::Wire;
use jni::objects::{JByteArray, JObject, JString, JValue};
use std::path::Path;
use std::sync::Mutex;
use winit::platform::android::activity::AndroidApp;

/// The app, as the system handed it over: where its activity and its folder are.
static APP: Mutex<Option<AndroidApp>> = Mutex::new(None);

/// The app this is running as.
pub fn app() -> AndroidApp {
    APP.lock()
        .expect("nothing panics holding it")
        .clone()
        .expect("the viewer is started by android_main")
}

/// The log, written to the system's.
struct Logcat;

impl log::Log for Logcat {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // The graphics layers say a great deal that is of no use here.
        let quiet = ["wgpu_core", "wgpu_hal", "naga"]
            .iter()
            .any(|layer| metadata.target().starts_with(layer));
        metadata.level() <= if quiet { log::Level::Warn } else { log::Level::Info }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        #[link(name = "log")]
        extern "C" {
            fn __android_log_write(priority: i32, tag: *const u8, text: *const u8) -> i32;
        }
        // The system's own numbers for how much a line matters.
        let priority = match record.level() {
            log::Level::Error => 6,
            log::Level::Warn => 5,
            log::Level::Info => 4,
            _ => 3,
        };
        let text = format!("{}: {}\0", record.target(), record.args()).replace('\0', " ") + "\0";
        // SAFETY: both are text that ends in a zero, and outlive the call.
        unsafe {
            __android_log_write(priority, b"solstice\0".as_ptr(), text.as_ptr());
        }
    }

    fn flush(&self) {}
}

/// Where the app starts: called by the activity's glue, on a thread of its own, once the
/// activity has unpacked the game's files.
#[no_mangle]
fn android_main(app: AndroidApp) {
    let _ = log::set_logger(&Logcat);
    log::set_max_level(log::LevelFilter::Info);
    std::panic::set_hook(Box::new(|panic| log::error!("{panic}")));
    // The game's files, and its maps and what it keeps, are in the app's own folder.
    let folder = app
        .external_data_path()
        .or_else(|| app.internal_data_path());
    *APP.lock().expect("nothing panics holding it") = Some(app);
    match &folder {
        Some(folder) => {
            if let Err(error) = std::env::set_current_dir(folder) {
                log::error!("{}: {error}", folder.display());
            }
        }
        None => log::error!("the app has no folder of its own"),
    }
    let game = crate::game::Game::parse(&std::fs::read_to_string("game.txt").unwrap_or_default());
    if game.run.is_empty() {
        log::error!("there is no game.txt in the app's folder: the game was not unpacked");
    }
    if let Err(error) = crate::desktop::run(game.arguments(Vec::new()), game.maps.clone()) {
        log::error!("{error}");
    }
    // The game is over: the activity is closed, and it ends the app's process as it goes, so
    // that the next time the app is opened is a fresh start. (Ending it from here, with other
    // threads of the system's still at work in it, brings it down untidily.)
    with_activity("the app could not be closed", |env, activity| {
        env.call_method(activity, "quit", "()V", &[])?;
        Ok(())
    });
}

/// Calls the activity, from whatever thread this is. `None`, with why in the log, if it
/// could not be done or the activity threw.
fn with_activity<R>(
    what: &str,
    call: impl FnOnce(&mut jni::JNIEnv, &JObject) -> jni::errors::Result<R>,
) -> Option<R> {
    let app = app();
    // SAFETY: the system's own machine and activity, which outlive the app's threads.
    let (machine, activity) = unsafe {
        (
            jni::JavaVM::from_raw(app.vm_as_ptr().cast()).ok()?,
            JObject::from_raw(app.activity_as_ptr().cast()),
        )
    };
    // A thread stays known to the machine once it has called it.
    let mut env = machine.attach_current_thread_permanently().ok()?;
    // What the call makes is let go when it is over: this thread never goes back to Java.
    let result = env.with_local_frame(32, |env| call(env, &activity));
    match result {
        Ok(result) => Some(result),
        Err(error) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
            }
            log::warn!("{what}: {error}");
            None
        }
    }
}

/// Lays the page at `address` over the game. Whether the activity took it.
pub fn overlay_open(address: &str) -> bool {
    with_activity("the pages could not be opened", |env, activity| {
        let address = env.new_string(address)?;
        env.call_method(
            activity,
            "overlayOpen",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&address)],
        )?;
        Ok(())
    })
    .is_some()
}

/// Runs a script in the pages.
pub fn overlay_run(script: &str) {
    with_activity("the pages could not be told", |env, activity| {
        let script = env.new_string(script)?;
        env.call_method(
            activity,
            "overlayRun",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&script)],
        )?;
        Ok(())
    });
}

/// What the pages have said since this was last called, a line each.
pub fn overlay_take() -> Vec<String> {
    with_activity("the pages could not be heard", |env, activity| {
        let said = env
            .call_method(activity, "overlayTake", "()Ljava/lang/String;", &[])?
            .l()?;
        if said.is_null() {
            return Ok(Vec::new());
        }
        let said: String = env.get_string(&JString::from(said))?.into();
        Ok(said.lines().map(str::to_owned).collect())
    })
    .unwrap_or_default()
}

/// Why the activity could not fetch what it was last asked for.
fn fetch_error() -> String {
    with_activity("the activity could not be asked", |env, activity| {
        let why = env
            .call_method(activity, "fetchError", "()Ljava/lang/String;", &[])?
            .l()?;
        if why.is_null() {
            return Ok(String::new());
        }
        Ok(env.get_string(&JString::from(why))?.into())
    })
    .unwrap_or_else(|| "the system's web client could not be reached".to_owned())
}

/// The system's web client, by way of the activity.
pub struct Http;

impl Wire for Http {
    fn get(&self, url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, String> {
        // First and one past last; with no range, both nothing.
        let (from, to) = range.unwrap_or((0, 0));
        let bytes = with_activity("fetching", |env, activity| {
            let url = env.new_string(url)?;
            let bytes = env
                .call_method(
                    activity,
                    "fetch",
                    "(Ljava/lang/String;JJ)[B",
                    &[
                        JValue::Object(&url),
                        JValue::Long(from as i64),
                        JValue::Long(to as i64),
                    ],
                )?
                .l()?;
            if bytes.is_null() {
                return Ok(None);
            }
            Ok(Some(env.convert_byte_array(JByteArray::from(bytes))?))
        })
        .flatten()
        .ok_or_else(fetch_error)?;
        match range {
            Some((from, to)) if bytes.len() as u64 != to - from => Err(format!(
                "asked for {} bytes and was sent {}",
                to - from,
                bytes.len()
            )),
            _ => Ok(bytes),
        }
    }

    fn download(&self, url: &str, to: &Path) -> Result<(), String> {
        let done = with_activity("downloading", |env, activity| {
            let url = env.new_string(url)?;
            let to = env.new_string(to.to_string_lossy())?;
            env.call_method(
                activity,
                "download",
                "(Ljava/lang/String;Ljava/lang/String;)Z",
                &[JValue::Object(&url), JValue::Object(&to)],
            )?
            .z()
        });
        match done {
            Some(true) => Ok(()),
            _ => Err(fetch_error()),
        }
    }
}
