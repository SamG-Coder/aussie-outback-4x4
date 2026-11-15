//! The viewer on the desktop: an interactive window, or a headless capture for automated
//! checks. Two programs are this and nothing more: `viewer` (`main.rs`), which is run from a
//! console and says what it does there, and `viewer-windowed` (`windowed.rs`), the same with
//! a window and no console, which is what a game that is downloaded is given as its program.
//!
//! ```text
//! viewer [SCENE] [--capture out.png] [--frames N] [--size WxH]
//! viewer [SCENE] --capture out.png --frames N --film DIR [--film-from FRAME]   (frames for a film)
//!        [--camera x,y,z,yaw,pitch] [--debug MODE] [--sun x,y,z] [--unpack-textures]
//!        [--weather NAME|FILE [--weathered SECONDS]] [--strike-at FRAME]
//!        [--limits webgpu]
//! viewer [SCENE] --flyover PATH.txt --out DIR [--stability] [--no-taa] [--size WxH] [--quality Q] [--low-power]
//! viewer [SCENE] --play [--fly] [--scripts DIR] [--capture out.png --frames N
//!        [--press ACTION@FRAME]... [--hold KEY@FROM-TO]... [--mouse PIXELS@FRAME]...]
//! viewer [SCENE] --game DIR [the same as with --play]
//! viewer [SCENE] --instances-near x,y,z,radius
//! viewer --fetch-maps [--into FOLDER] SITE...
//! ```
//!
//! With a `game.txt` beside the program it is a game that was downloaded, and that file says
//! what it is run with, where its maps are fetched from and where its log goes (`game.rs`).
//!
//! `--play` runs the scene's game, if it has one: its actors move, the camera is the player's,
//! and keys and the mouse are its input. Its scripts are reloaded from `--scripts` (by default
//! `scenes`, where scene text files live) when they change on disk. **V** leaves the player
//! standing and flies the camera from where it is, and comes back; `--fly` starts that way. The
//! game goes on meanwhile, and still hears the keys: a door opens to the camera flown up to it.
//!
//! `--game DIR` plays the game in a folder in place of the one baked into the scene, if any:
//! its `scene.game`, and the actor files, scripts and input actions that names. So a scene
//! baked without a game, or one too big to bake again, is given one, or a new one, without a
//! bake. Its `actor` lines name the scene's instances themselves (`instances=FIRST+COUNT`),
//! which `--instances-near` lists: each instance placed within a radius of a point, with its
//! mesh, material, place and yaw.

#[cfg(not(target_os = "android"))]
use crate::{fetch, game};

/// Does what the arguments say: how a phone's app starts it (`android.rs`).
pub use native::run;

/// What either program does: what its arguments say, or, with a `game.txt` beside it, what
/// that says.
#[cfg(not(target_os = "android"))]
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `viewer --fetch-maps SITE...` fetches a game's maps and draws nothing (`fetch.rs`).
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|first| first == "--fetch-maps")
    {
        std::process::exit(fetch::run(&arguments[1..]));
    }
    // A game that was downloaded says beside the program what the program is to do.
    let game = game::Game::beside_program();
    // Opened with a double click and nothing beside it: a game's program taken out of its
    // folder, or opened from inside the zip it came in, which hands it over alone.
    if game.is_none() && arguments.is_empty() && !game::has_console() {
        game::alert(
            "This is a game's program, and the game's other files are not beside it.\n\n\
             Unzip the whole download first (right-click the zip, Extract All), then open the \
             program in the folder that makes.",
        );
        return Ok(());
    }
    let mut sites = Vec::new();
    if let Some(game) = &game {
        arguments = game.arguments(arguments);
        sites = game.maps.clone();
    }
    native::start_log(game.as_ref().and_then(|game| game.log_file()));
    let result = native::run(arguments, sites);
    if let Err(error) = &result {
        log::error!("{error}");
        game::alert(&error.to_string());
    }
    result
}

mod native {
    use glam::Vec3;
    use solstice_asset::paged::PagedScene;
    use solstice_asset::scene::Viewpoint;
    use solstice_asset::LoadedScene;
    use solstice_render::{
        Camera, DebugView, Effect, Environment, Gpu, Ocean, Pond, Quality, Renderer,
        RendererSettings, StreamRequest, StreamingBudget, TimeOfDay, Weather,
    };
    use solstice_viewer::bob::Bobbing;
    use solstice_viewer::play::PlayMode;
    use solstice_viewer::shell::{Order, Screen, Shell, Step};
    use solstice_viewer::{
        default_environment, initial_camera, is_dark, night_eyes, parse_time, CameraInput,
        DayClock, FlyCamera,
    };
    use std::collections::HashSet;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::dpi::PhysicalSize;
    use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::keyboard::{KeyCode, PhysicalKey};
    use winit::window::{CursorGrabMode, Window, WindowId};

    const CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Reads pages from a scene file on background threads.
    struct PageReader {
        path: Arc<PathBuf>,
        finished_sender: mpsc::Sender<(u32, Option<Vec<u8>>)>,
        finished: mpsc::Receiver<(u32, Option<Vec<u8>>)>,
        running: u32,
    }

    impl PageReader {
        /// Pages to read at once: enough to use the disk, few enough to stay out of the way.
        const MAX_FETCHING: u32 = 256;

        fn new(path: &Path) -> Self {
            let (finished_sender, finished) = mpsc::channel();
            Self {
                path: Arc::new(path.to_owned()),
                finished_sender,
                finished,
                running: 0,
            }
        }

        /// Hands over what has been read, and starts reading what's now wanted. Call per frame.
        fn pump(&mut self, renderer: &mut Renderer) {
            while let Ok((id, data)) = self.finished.try_recv() {
                self.running -= 1;
                match data {
                    Some(data) => {
                        renderer.deliver_fetched(id, data);
                    }
                    None => renderer.request_failed(id),
                }
            }
            for request in renderer.stream_requests(Self::MAX_FETCHING) {
                self.running += 1;
                let path = self.path.clone();
                let sender = self.finished_sender.clone();
                std::thread::spawn(move || {
                    let data = read_range(&path, &request)
                        .map_err(|error| {
                            log::warn!(
                                "bytes {} to {} of the scene could not be read: {error}",
                                request.start,
                                request.end
                            );
                        })
                        .ok();
                    let _ = sender.send((request.id, data));
                });
            }
        }

        fn is_idle(&self) -> bool {
            self.running == 0
        }
    }

    fn read_range(
        path: &Path,
        request: &StreamRequest,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut file = std::fs::File::open(path)?;
        // A map joined with what every map shares is two files, whose bytes are numbered
        // straight on from one to the other: past the end of the map's is the shared one.
        let length = file.metadata()?.len();
        if request.start >= length {
            file = std::fs::File::open(path.with_file_name(solstice_viewer::shared::FILE))?;
            file.seek(SeekFrom::Start(request.start - length))?;
        } else {
            file.seek(SeekFrom::Start(request.start))?;
        }
        let mut bytes = vec![0; (request.end - request.start) as usize];
        file.read_exact(&mut bytes)?;
        if request.deflated {
            bytes = solstice_asset::file::inflate(&bytes)?;
        }
        Ok(bytes)
    }

    /// Reads just the part of a scene file that stays resident.
    fn read_manifest(path: &Path) -> Result<PagedScene, Box<dyn std::error::Error>> {
        let mut file = std::fs::File::open(path)?;
        let mut head = [0; solstice_asset::file::HEADER_BYTES];
        file.read_exact(&mut head)?;
        let header = solstice_asset::file::decode_header(&head)?;
        file.seek(SeekFrom::Start(header.manifest_offset))?;
        let mut packed = vec![0; header.manifest_bytes as usize];
        file.read_exact(&mut packed)?;
        let manifest = solstice_asset::file::inflate(&packed)?;
        let scene = solstice_asset::file::decode_manifest(
            &manifest,
            header.manifest_offset + header.manifest_bytes,
        )?;
        if scene.external.is_some() {
            return Err("this is the file made for browsers: open the .solstice file".into());
        }
        Ok(scene)
    }

    /// A scene as this run holds it: all in memory, or read from disk as wanted.
    enum Source {
        Loaded(LoadedScene),
        Streamed(PagedScene),
    }

    impl Source {
        fn viewpoint(&self) -> Option<Viewpoint> {
            match self {
                Self::Loaded(loaded) => loaded.scene.viewpoint,
                Self::Streamed(paged) => paged.viewpoint,
            }
        }

        /// The scene's collision, whole, read from its file if it wasn't all loaded.
        fn collision(
            &self,
            path: &Path,
        ) -> Result<Option<solstice_asset::Collision>, Box<dyn std::error::Error>> {
            let paged = match self {
                Self::Loaded(loaded) => return Ok(loaded.collision.clone()),
                Self::Streamed(paged) => paged,
            };
            let Some(index) = &paged.collision else {
                return Ok(None);
            };
            let mut file = std::fs::File::open(path)?;
            let mut pieces = Vec::new();
            for ((offset, stored), raw) in index.ranges().into_iter().zip(index.raw_sizes()) {
                file.seek(SeekFrom::Start(paged.data_offset + offset))?;
                let mut packed = vec![0; stored as usize];
                file.read_exact(&mut packed)?;
                pieces.push(solstice_asset::file::inflate_limited(&packed, raw)?);
            }
            Ok(Some(index.decode(&pieces)?))
        }

        /// Prints the instances placed within a radius of a point, `near` being x, y, z and the
        /// radius: what a scene's game needs to say which instances an actor moves.
        fn list_instances(&self, near: [f32; 4]) {
            let paged = match self {
                Self::Loaded(loaded) => &loaded.scene,
                Self::Streamed(paged) => paged,
            };
            let [x, y, z, radius] = near;
            println!("instance  mesh  material  x y z  yaw");
            for (id, instance) in paged.instances.iter().enumerate() {
                let (_, rotation, at) = instance.transform.to_scale_rotation_translation();
                if at.distance(Vec3::new(x, y, z)) > radius {
                    continue;
                }
                let yaw = rotation.to_euler(glam::EulerRot::YXZ).0.to_degrees();
                println!(
                    "{id}  {}  {}  {:.3} {:.3} {:.3}  {yaw:.1}",
                    instance.mesh, instance.material, at.x, at.y, at.z
                );
            }
        }

        fn gameplay(&self) -> &[solstice_asset::GameFile] {
            match self {
                Self::Loaded(loaded) => &loaded.scene.gameplay,
                Self::Streamed(paged) => &paged.gameplay,
            }
        }

        fn animation(&self) -> solstice_game::animation::AnimationAssets<'_> {
            let paged = match self {
                Self::Loaded(loaded) => &loaded.scene,
                Self::Streamed(paged) => paged,
            };
            solstice_game::animation::AnimationAssets {
                skeletons: &paged.skeletons,
                clips: &paged.clips,
                poses: &paged.poses,
                instance_poses: &paged.instance_poses,
            }
        }

        fn renderer(
            &self,
            gpu: &Gpu,
            format: wgpu::TextureFormat,
            options: &Options,
        ) -> (Renderer, Option<PageReader>) {
            let (mut renderer, reader) = match self {
                Self::Loaded(loaded) => {
                    (Renderer::new(gpu, loaded, format, options.settings), None)
                }
                Self::Streamed(paged) => (
                    Renderer::streaming(gpu, paged, format, options.settings, options.budget),
                    Some(PageReader::new(&options.scene)),
                ),
            };
            renderer.set_weather(options.weather, 0.0);
            renderer.advance_weather(options.weathered);
            if options.ocean.is_some() {
                renderer.set_ocean(options.ocean, 0.0);
            }
            if let Some(level) = options.sea_level {
                renderer.set_sea_level(level);
            }
            // A scene's own ponds stand unless others are asked for.
            if !options.ponds.is_empty() {
                renderer.set_ponds(&options.ponds);
            }
            spawn_effects(&mut renderer, options);
            (renderer, reader)
        }
    }

    struct Options {
        /// Load pages from disk as they're wanted, rather than all at the start.
        stream: bool,
        /// GPU memory for the scene when streaming.
        budget: StreamingBudget,
        scene: PathBuf,
        capture: Option<PathBuf>,
        /// A folder to save a capture's frames into as it runs, every other one of them
        /// from a given frame on: thirty pictures for each second, to make a film of.
        film: Option<PathBuf>,
        film_from: u32,
        frames: u32,
        size: (u32, u32),
        /// Overrides the scene's own viewpoint.
        camera: Option<Camera>,
        environment: Environment,
        settings: RendererSettings,
        power: wgpu::PowerPreference,
        /// Acts as if the GPU had no block compression, to test that path.
        unpack_textures: bool,
        /// Asks for no more than these limits of the GPU, to see what a small one is given:
        /// `--limits webgpu`, the WebGPU defaults, which many phones are given.
        limits: Option<wgpu::Limits>,
        /// Metres per frame the capture camera slides sideways, for moving-view benchmarks.
        capture_motion: f32,
        /// What the truck wears, for a picture: `--coat paint=#1f4fa0,rust=0.4,dust=0.3,mud=0.7`.
        coat: Option<solstice_viewer::play::Wear>,
        /// What is fitted to the truck, for a picture: `--kit showroom`, `--kit bullbar=plate,tyres=mud`.
        kit: Option<String>,
        /// Damage to put on the truck, for a picture: `--damage bumper_front=dented,door_FL=loose`
        /// (each piece sound, scuffed, dented, loose or off), and blows to hit it with, each
        /// `--hit X,Y,Z,PX,PY,PZ,KN`: where on the truck, the way it pushes, and how hard
        /// (`solstice_viewer::damage`).
        damage: Option<String>,
        hits: Vec<solstice_viewer::damage::Hit>,
        /// A truck built of pieces to play in place of the map's own: its pack, made by
        /// `bake --kit` (`--truck mitta` is `trucks/mitta.truck` beside the map). It is put
        /// together as the map is read, of the pieces `kit` says and no others.
        truck: Option<String>,
        /// Trucks to stand in the scene to be looked at, in place of its own: each its name
        /// and how it is fitted (`all` is every piece of it). One is shown at a time.
        shown: Vec<(String, String)>,
        /// The day's used trucks, to be parked in a map that has a yard for them: each its
        /// place on the lot, its name, its colour, what is asked and how far it has been.
        /// And whether whichever is looked over is pulled out of its row.
        parked: Vec<solstice_viewer::shell::ForSale>,
        pulled: bool,
        /// Which of the kept games is being played: its number, from 1.
        slot: u32,
        /// Which of the truck's doors are open, for a picture: `--open all`, `--open door_FL,tailgate`.
        open: Option<String>,
        /// A camera path to fly, with results written to `flyover_out`.
        flyover: Option<PathBuf>,
        flyover_out: PathBuf,
        /// Measure frame-to-frame change during a flyover. Slows the flight, so not for timing.
        stability: bool,
        /// The weather to start with: a built-in state's name, or a weather file.
        weather: Weather,
        /// Seconds the weather has already gone on for, as if they had passed: how long it has
        /// rained, for one.
        weathered: f32,
        /// A frame of a capture on which lightning strikes, whatever the weather.
        strike_at: Option<u32>,
        /// When set, the sun and moon follow the time of day instead of `environment`.
        clock: Option<DayClock>,
        /// An open sea, by name or file, and the level of its calm surface. Scenes with a sea of
        /// their own bring both.
        ocean: Option<Ocean>,
        sea_level: Option<f32>,
        /// Still water to lay over the scene: each a rectangle and the water's height.
        ponds: Vec<Pond>,
        /// Particle effects to run, by name or file, and where.
        effects: Vec<(Effect, Vec3)>,
        /// Instances to rock as if afloat: those centred within a radius of a point across the
        /// ground, as x, z, radius, the height they rock about, and how far they rock.
        bob: Option<[f32; 5]>,
        /// Run the scene's game.
        play: bool,
        /// Start the game flying the camera, with whoever is played left standing.
        fly: bool,
        /// A folder holding the game to run, in place of the scene's own.
        game: Option<PathBuf>,
        /// List the instances placed within a radius of a point, as x, y, z, radius, and stop.
        instances_near: Option<[f32; 4]>,
        /// Where the scene's scripts are on disk, by the names the scene gives them, to reload
        /// them when they change.
        scripts: PathBuf,
        /// The folder of pages to lay over the window: the game's menus and instruments.
        ui: Option<PathBuf>,
        /// The game's maps and which is its hub (`solstice_viewer::shell::World`): with it
        /// the scene is one map of a game, with a title, a pause menu and trailheads.
        world: Option<PathBuf>,
        /// Whether night is shown as eyes used to the dark see it, so that it can be driven in.
        night_eyes: bool,
        /// Whether the truck is not drawn: for a picture of a map and nothing else.
        no_truck: bool,
        /// The share of its engine's pull the truck still has: less, overdue for its oil.
        pull: f32,
        /// Where in a showroom the trucks stand, if the map says where for what is looked at.
        stand: Option<String>,
        /// Whether a scene was named, rather than left to be the game's hub.
        scene_named: bool,
        /// For trying the menus with no hand on anything: scripts to run in the pages, and
        /// controls to press (`key:Enter`, `gamepad:south`), each with the second to do it at.
        ui_runs: Vec<(String, f32)>,
        /// What a trial does, and from when to when, in seconds: a key or a controller's
        /// button held (`key:KeyE`, `south`), the mouse moved (`mouse:DX,DY`), the truck put
        /// somewhere (`place:X,Y,Z,YAW`), a picture taken (`shot:NAME`), or the game closed
        /// (`quit`).
        taps: Vec<(String, f32, f32)>,
        /// A trial: the game played by `--tap` alone, where nobody sees or hears it and it
        /// touches nothing of a real game's. Its window is off the screen and is never the
        /// one typed into, it makes no sound, no real controller is read, what it keeps and
        /// the pictures it takes go in this folder, and it says what the pages are told
        /// once a second.
        trial: Option<PathBuf>,
        /// Actions to press in a capture, and the frame to press each in.
        presses: Vec<(String, u32)>,
        /// Keys to hold in a capture, by their `KeyboardEvent.code`, and from which frame to
        /// which.
        holds: Vec<(String, u32, u32)>,
        /// The mouse moved in a capture, by pixels across, at a frame.
        mouse_moves: Vec<(f32, u32)>,
        /// And down, the same way: `--mouse-down PIXELS@FRAME`.
        mouse_downs: Vec<(f32, u32)>,
    }

    /// Starts the effects the options name.
    fn spawn_effects(renderer: &mut Renderer, options: &Options) {
        for (effect, at) in &options.effects {
            renderer
                .particles()
                .spawn(effect, glam::Mat4::from_translation(*at));
        }
    }

    /// Makes the instances `--bob` names movable, ready to rock.
    fn bobbing(options: &Options, renderer: &mut Renderer) -> Option<Bobbing> {
        let [x, z, radius, height, amplitude] = options.bob?;
        let bobbing = Bobbing::around(renderer, [x, z], radius, height, amplitude);
        log::info!("{} instances bob around ({x}, {z})", bobbing.len());
        Some(bobbing)
    }

    /// Starts the scene's game, if `--play` asks and the scene has one, logging every mistake
    /// in its files if it can't start.
    fn start_play(options: &Options, scene: &Source, renderer: &mut Renderer) -> Option<PlayMode> {
        if !options.play {
            return None;
        }
        let side;
        let files = match &options.game {
            None => scene.gameplay(),
            Some(folder) => {
                let mut files = Vec::new();
                if let Err(error) = read_game_files(folder, "", &mut files) {
                    log::error!("--game {}: {error}", folder.display());
                    return None;
                }
                side = files;
                &side
            }
        };
        if files.is_empty() {
            log::warn!("--play: this scene has no gameplay, so there is nothing to play");
            return None;
        }
        let collision = match scene.collision(&options.scene) {
            Ok(collision) => collision,
            Err(error) => {
                log::error!("the scene's collision can't be read: {error}");
                None
            }
        };
        if collision.is_none() {
            log::warn!("this scene has no collision: rebake it to walk on it");
        }
        match PlayMode::start(renderer, files, collision.as_ref(), &scene.animation()) {
            Ok(mut play) => {
                // Which parts of what moves are lamps: whatever glows. The game lights them
                // as it is driven.
                if let Source::Loaded(loaded) = scene {
                    let (mut tail, mut head) = (Vec::new(), Vec::new());
                    for id in play.play.instances() {
                        let glow = loaded
                            .scene
                            .instances
                            .get(id as usize)
                            .and_then(|instance| {
                                loaded.scene.materials.get(instance.material as usize)
                            })
                            .map_or([0.0; 3], |material| material.emissive);
                        if !glow.iter().any(|&channel| channel > 0.0) {
                            continue;
                        }
                        // A lamp that is all behind the middle of the car is a tail lamp:
                        // a car's nose is along -z.
                        let instance = &loaded.scene.instances[id as usize];
                        let behind = loaded
                            .scene
                            .meshes
                            .get(instance.mesh as usize)
                            .is_some_and(|mesh| mesh.quant_origin.z > 1.0);
                        if behind {
                            tail.push(id);
                        } else {
                            head.push(id);
                        }
                    }
                    log::info!("lamps: {} behind, {} in front", tail.len(), head.len());
                    play.set_lamps(tail, head);
                    // A truck baked into its map is measured from it, for what a blow
                    // lands on (one put together from its pack comes with its measure).
                    play.measure_truck(loaded);
                }
                log::info!(
                    "playing: {} instances move, scripts {}",
                    play.play.instances().count(),
                    play.play.scripts().join(", ")
                );
                play.play.set_flying(options.fly);
                Some(play)
            }
            Err(errors) => {
                for error in errors {
                    log::error!("{error}");
                }
                None
            }
        }
    }

    /// Reads every text file in a folder, and the folders in it, as gameplay files named as a
    /// scene's are: from the folder, with `/` between folders.
    fn read_game_files(
        folder: &Path,
        prefix: &str,
        files: &mut Vec<solstice_asset::GameFile>,
    ) -> std::io::Result<()> {
        let mut entries = std::fs::read_dir(folder)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if entry.file_type()?.is_dir() {
                read_game_files(&entry.path(), &format!("{name}/"), files)?;
            } else if let Ok(text) = std::fs::read_to_string(entry.path()) {
                files.push(solstice_asset::GameFile { name, text });
            }
        }
        Ok(())
    }

    /// Logs what a game's scripts logged, and its errors.
    fn log_messages(messages: Vec<solstice_viewer::play::Message>) {
        for message in messages {
            if message.error {
                log::error!("{}", message.text);
            } else {
                log::info!("{}", message.text);
            }
        }
    }

    /// Reloads a game's scripts when they change on disk.
    struct ScriptWatch {
        /// Each script's name in the game, its file, and when that last changed.
        files: Vec<(String, PathBuf, Option<std::time::SystemTime>)>,
        since_check: f32,
    }

    impl ScriptWatch {
        /// How often to look, in seconds.
        const INTERVAL: f32 = 0.5;

        fn new(play: &PlayMode, folder: &Path) -> Self {
            let modified = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
            let files = play
                .play
                .scripts()
                .iter()
                .map(|name| {
                    let path = folder.join(name);
                    let time = modified(&path);
                    if time.is_none() {
                        log::warn!("{} is not there to watch", path.display());
                    }
                    (name.clone(), path, time)
                })
                .collect();
            Self {
                files,
                since_check: 0.0,
            }
        }

        fn check(&mut self, play: &mut PlayMode, seconds: f32) {
            self.since_check += seconds;
            if self.since_check < Self::INTERVAL {
                return;
            }
            self.since_check = 0.0;
            for (name, path, last) in &mut self.files {
                let Ok(now) = std::fs::metadata(&*path).and_then(|m| m.modified()) else {
                    continue;
                };
                if *last == Some(now) {
                    continue;
                }
                *last = Some(now);
                match std::fs::read_to_string(&*path) {
                    Ok(source) => match play.play.reload(name, &source) {
                        Ok(()) => log::info!("reloaded {name}"),
                        Err(error) => log::error!("{error}"),
                    },
                    Err(error) => log::error!("{}: {error}", path.display()),
                }
            }
        }
    }

    /// The light for the next frame, `seconds` after the last: the clock's, moving on, or the
    /// fixed environment when there is no clock.
    fn light(clock: &mut Option<DayClock>, fixed: Environment, seconds: f32) -> Environment {
        match clock {
            Some(clock) => clock.tick(seconds),
            None => fixed,
        }
    }

    /// A scene's gameplay files, and with them a trailhead wherever `SOLSTICE_PLACE` says
    /// (`MAP X Y Z RADIUS`): for trying what a trailhead does without driving to one.
    fn with_trial_place(files: &[solstice_asset::GameFile]) -> Vec<solstice_asset::GameFile> {
        let mut files = files.to_vec();
        if let Ok(place) = std::env::var("SOLSTICE_PLACE") {
            files.push(solstice_asset::GameFile {
                name: "trial.places".to_owned(),
                text: format!("place {place}\n"),
            });
        }
        files
    }

    fn parse_args(arguments: Vec<String>) -> Result<Options, String> {
        let mut options = Options {
            stream: false,
            budget: StreamingBudget::default(),
            scene: PathBuf::from("assets/temple.solstice"),
            capture: None,
            film: None,
            film_from: 0,
            frames: 60,
            size: (1600, 900),
            camera: None,
            environment: default_environment(),
            settings: RendererSettings::default(),
            power: wgpu::PowerPreference::HighPerformance,
            unpack_textures: false,
            limits: None,
            capture_motion: 0.0,
            coat: None,
            kit: None,
            damage: None,
            hits: Vec::new(),
            truck: None,
            shown: Vec::new(),
            parked: Vec::new(),
            pulled: false,
            slot: 1,
            open: None,
            flyover: None,
            flyover_out: PathBuf::from("screenshots/flyover"),
            stability: false,
            weather: Weather::default(),
            weathered: 0.0,
            strike_at: None,
            ocean: None,
            sea_level: None,
            ponds: Vec::new(),
            clock: None,
            effects: Vec::new(),
            bob: None,
            play: false,
            game: None,
            fly: false,
            instances_near: None,
            scripts: PathBuf::from("scenes"),
            ui: None,
            world: None,
            night_eyes: false,
            no_truck: false,
            pull: 1.0,
            stand: None,
            scene_named: false,
            ui_runs: Vec::new(),
            taps: Vec::new(),
            trial: None,
            presses: Vec::new(),
            holds: Vec::new(),
            mouse_moves: Vec::new(),
            mouse_downs: Vec::new(),
        };
        // The time of day, built up from its options and made at the end.
        let (mut time, mut day_seconds, mut latitude) = (None, None, None);
        let mut day = TimeOfDay::default().day;
        let floats = |text: &str| -> Result<Vec<f32>, String> {
            text.split(',')
                .map(|part| part.parse::<f32>().map_err(|e| format!("{part}: {e}")))
                .collect()
        };
        let mut args = arguments.into_iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
            match arg.as_str() {
                "--capture" => options.capture = Some(PathBuf::from(value()?)),
                "--film" => options.film = Some(PathBuf::from(value()?)),
                "--film-from" => {
                    options.film_from =
                        value()?.parse().map_err(|e| format!("--film-from: {e}"))?;
                }
                "--frames" => {
                    options.frames = value()?.parse().map_err(|e| format!("--frames: {e}"))?
                }
                "--size" => {
                    let text = value()?;
                    let (w, h) = text.split_once('x').ok_or("--size expects WxH")?;
                    options.size = (
                        w.parse().map_err(|e| format!("--size: {e}"))?,
                        h.parse().map_err(|e| format!("--size: {e}"))?,
                    );
                }
                "--camera" => {
                    let v = floats(&value()?)?;
                    let [x, y, z, yaw, pitch] = v[..] else {
                        return Err("--camera expects x,y,z,yaw,pitch".into());
                    };
                    options.camera = Some(Camera {
                        position: Vec3::new(x, y, z),
                        yaw,
                        pitch,
                        ..Default::default()
                    });
                }
                "--sun" => {
                    let v = floats(&value()?)?;
                    let [x, y, z] = v[..] else {
                        return Err("--sun expects x,y,z".into());
                    };
                    options.environment.sun_direction = Vec3::new(x, y, z).normalize();
                }
                "--debug" => {
                    let name = value()?;
                    options.settings.debug_view = DebugView::from_name(&name)
                        .ok_or_else(|| format!("unknown debug view: {name}"))?;
                }
                "--stability" => options.stability = true,
                "--no-taa" => options.settings.temporal_accumulation = false,
                "--no-clouds" => options.settings.clouds = false,
                "--time" => {
                    let text = value()?;
                    time =
                        Some(parse_time(&text).ok_or(format!("--time expects HH:MM, not {text}"))?);
                }
                "--day" => day = value()?.parse().map_err(|e| format!("--day: {e}"))?,
                "--latitude" => {
                    latitude = Some(value()?.parse().map_err(|e| format!("--latitude: {e}"))?);
                }
                "--day-length" => {
                    day_seconds = Some(value()?.parse().map_err(|e| format!("--day-length: {e}"))?);
                }
                "--ocean" => {
                    let name = value()?;
                    options.ocean = Some(match Ocean::by_name(&name) {
                        Some(ocean) => ocean,
                        None => {
                            let text = std::fs::read_to_string(&name).map_err(|e| {
                                let names: Vec<_> = Ocean::names().collect();
                                format!(
                                    "--ocean: {name} is neither {} nor a file: {e}",
                                    names.join(", ")
                                )
                            })?;
                            Ocean::parse(&text).map_err(|e| format!("{name}: {e}"))?
                        }
                    });
                }
                "--effect" => {
                    // NAME or FILE, then optionally @x,y,z for where it stands.
                    let text = value()?;
                    let (name, at) = match text.split_once('@') {
                        Some((name, at)) => {
                            let at = floats(at)?;
                            let [x, y, z] = at[..] else {
                                return Err("--effect expects NAME@x,y,z".into());
                            };
                            (name.to_owned(), Vec3::new(x, y, z))
                        }
                        None => (text, Vec3::ZERO),
                    };
                    let effect = match Effect::by_name(&name) {
                        Some(effect) => effect,
                        None => {
                            let text = std::fs::read_to_string(&name).map_err(|e| {
                                let names: Vec<_> = Effect::names().collect();
                                format!(
                                    "--effect: {name} is neither {} nor a file: {e}",
                                    names.join(", ")
                                )
                            })?;
                            Effect::parse(&text).map_err(|e| format!("{name}: {e}"))?
                        }
                    };
                    options.effects.push((effect, at));
                }
                "--sea-level" => {
                    options.sea_level =
                        Some(value()?.parse().map_err(|e| format!("--sea-level: {e}"))?);
                }
                "--pond" => {
                    let ponds = Pond::parse_list(&value()?).map_err(|e| format!("--pond: {e}"))?;
                    options.ponds.extend(ponds);
                }
                "--strike-at" => {
                    options.strike_at =
                        Some(value()?.parse().map_err(|e| format!("--strike-at: {e}"))?);
                }
                "--weathered" => {
                    options.weathered =
                        value()?.parse().map_err(|e| format!("--weathered: {e}"))?;
                }
                "--weather" => {
                    let name = value()?;
                    options.weather = match Weather::by_name(&name) {
                        Some(weather) => weather,
                        None => {
                            let text = std::fs::read_to_string(&name).map_err(|e| {
                                let names: Vec<_> = Weather::names().collect();
                                format!(
                                    "--weather: {name} is neither {} nor a file: {e}",
                                    names.join(", ")
                                )
                            })?;
                            Weather::parse(&text).map_err(|e| format!("{name}: {e}"))?
                        }
                    };
                }
                "--bob" => {
                    let v = floats(&value()?)?;
                    options.bob = Some(match v[..] {
                        [x, z, radius] => [x, z, radius, 0.0, 1.0],
                        [x, z, radius, height] => [x, z, radius, height, 1.0],
                        [x, z, radius, height, amplitude] => [x, z, radius, height, amplitude],
                        _ => return Err("--bob expects x,z,radius[,height[,amplitude]]".into()),
                    });
                }
                "--coat" => options.coat = Some(solstice_viewer::play::Wear::parse(&value()?)?),
                "--kit" => options.kit = Some(value()?),
                "--damage" => options.damage = Some(value()?),
                "--hit" => options
                    .hits
                    .push(solstice_viewer::damage::Hit::parse(&value()?)?),
                "--truck" => options.truck = Some(value()?),
                // For a picture: these trucks parked in the map's used-truck yard, in its
                // places in turn, each with a colour and a price made up from its place.
                "--park" => {
                    let colours = solstice_viewer::yard::COLOURS;
                    options.parked = value()?
                        .split(',')
                        .enumerate()
                        .map(|(place, truck)| solstice_viewer::shell::ForSale {
                            place,
                            truck: truck.to_owned(),
                            paint: Some(colours[(place * 5 + 3) % colours.len()].1),
                            price: 4_300 + 2_150 * place as u64,
                            km: 96_000.0 + 31_500.0 * place as f32,
                        })
                        .collect();
                }
                "--open" => options.open = Some(value()?),
                "--stream" => options.stream = true,
                "--play" => options.play = true,
                "--fly" => options.fly = true,
                "--game" => {
                    // Its scripts are watched where they were read from.
                    let folder = PathBuf::from(value()?);
                    options.play = true;
                    options.scripts.clone_from(&folder);
                    options.game = Some(folder);
                }
                "--instances-near" => {
                    options.instances_near = Some(
                        <[f32; 4]>::try_from(floats(&value()?)?)
                            .map_err(|_| "--instances-near expects x,y,z,radius")?,
                    );
                }
                "--scripts" => options.scripts = PathBuf::from(value()?),
                "--ui" => options.ui = Some(PathBuf::from(value()?)),
                "--ui-run" | "--tap" => {
                    let text = value()?;
                    // `WHAT@SECONDS`, or for a tap that is held, `WHAT@FROM-TO`.
                    let (what, from, to) = text
                        .rsplit_once('@')
                        .and_then(|(what, at)| {
                            let (from, to) = match at.split_once('-') {
                                Some((from, to)) => (from.parse().ok()?, to.parse().ok()?),
                                None => {
                                    let from: f32 = at.parse().ok()?;
                                    (from, from + 0.15)
                                }
                            };
                            Some((what.to_owned(), from, to))
                        })
                        .ok_or(format!("{arg} expects WHAT@SECONDS"))?;
                    if arg == "--tap" {
                        options.taps.push((what, from, to));
                    } else {
                        options.ui_runs.push((what, from));
                    }
                }
                "--trial" => options.trial = Some(PathBuf::from(value()?)),
                "--night-eyes" => options.night_eyes = true,
                "--no-truck" => options.no_truck = true,
                "--world" => {
                    options.world = Some(PathBuf::from(value()?));
                    options.play = true;
                    options.night_eyes = true;
                }
                "--press" => {
                    let text = value()?;
                    let (action, frame) = text
                        .split_once('@')
                        .and_then(|(a, f)| Some((a.to_owned(), f.parse().ok()?)))
                        .ok_or("--press expects ACTION@FRAME")?;
                    options.presses.push((action, frame));
                }
                "--hold" => {
                    let text = value()?;
                    let parsed = text.split_once('@').and_then(|(key, range)| {
                        let (from, to) = range.split_once('-')?;
                        Some((key.to_owned(), from.parse().ok()?, to.parse().ok()?))
                    });
                    options
                        .holds
                        .push(parsed.ok_or("--hold expects KEY@FROM-TO")?);
                }
                "--mouse" => {
                    let text = value()?;
                    let parsed = text
                        .split_once('@')
                        .and_then(|(x, frame)| Some((x.parse().ok()?, frame.parse().ok()?)));
                    options
                        .mouse_moves
                        .push(parsed.ok_or("--mouse expects PIXELS@FRAME")?);
                }
                "--mouse-down" => {
                    let text = value()?;
                    let parsed = text
                        .split_once('@')
                        .and_then(|(y, frame)| Some((y.parse().ok()?, frame.parse().ok()?)));
                    options
                        .mouse_downs
                        .push(parsed.ok_or("--mouse-down expects PIXELS@FRAME")?);
                }
                "--geometry-budget-mb" => {
                    let megabytes: u64 = value()?
                        .parse()
                        .map_err(|e| format!("--geometry-budget-mb: {e}"))?;
                    options.budget.geometry_bytes = megabytes << 20;
                }
                "--texture-budget-mb" => {
                    let megabytes: u64 = value()?
                        .parse()
                        .map_err(|e| format!("--texture-budget-mb: {e}"))?;
                    options.budget.texture_bytes = megabytes << 20;
                }
                "--flyover" => options.flyover = Some(PathBuf::from(value()?)),
                "--out" => options.flyover_out = PathBuf::from(value()?),
                "--prefetch" => {
                    options.settings.prefetch_radius =
                        value()?.parse().map_err(|e| format!("--prefetch: {e}"))?;
                }
                "--specks" => {
                    options.settings.speck_texels =
                        value()?.parse().map_err(|e| format!("--specks: {e}"))?;
                }
                "--move" => {
                    options.capture_motion =
                        value()?.parse().map_err(|e| format!("--move: {e}"))?;
                }
                "--quality" => {
                    let name = value()?;
                    let quality = Quality::from_name(&name)
                        .ok_or_else(|| format!("unknown quality: {name}"))?;
                    options.settings.apply_quality(quality);
                }
                "--low-power" => options.power = wgpu::PowerPreference::LowPower,
                "--unpack-textures" => options.unpack_textures = true,
                "--limits" => match value()?.as_str() {
                    "webgpu" => options.limits = Some(wgpu::Limits::defaults()),
                    other => return Err(format!("unknown limits: {other}")),
                },
                "--scale" => {
                    options.settings.render_scale =
                        value()?.parse().map_err(|e| format!("--scale: {e}"))?;
                }
                "--lod" => {
                    options.settings.lod_threshold =
                        value()?.parse().map_err(|e| format!("--lod: {e}"))?;
                }
                other if !other.starts_with('-') => {
                    options.scene = PathBuf::from(other);
                    options.scene_named = true;
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        // A game has a clock, and its time passes as it does, a minute in a minute: driven on
        // into the evening, it gets dark. Anything else stands still unless it is told how
        // long its day is.
        let game = options.world.is_some();
        let time = time.or(game.then_some(10.0));
        let day_seconds = day_seconds.unwrap_or(if game { 86_400.0 } else { 0.0 });
        if let Some(hours) = time {
            let mut clock = DayClock::new(TimeOfDay { day, hours }, day_seconds);
            if let Some(latitude) = latitude {
                clock.place.latitude = latitude;
            }
            clock.daylight = options.environment;
            options.clock = Some(clock);
        }
        Ok(options)
    }

    fn load_scene(path: &PathBuf) -> Result<LoadedScene, Box<dyn std::error::Error>> {
        let start = Instant::now();
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let scene = solstice_asset::file::decode(&bytes)?;
        log::info!(
            "loaded {} ({:.0} MB) in {:.2}s",
            path.display(),
            bytes.len() as f64 / 1.0e6,
            start.elapsed().as_secs_f32()
        );
        Ok(scene)
    }

    /// Starts the log: on the console, or in a game's own file, where what stops the program
    /// is written too, as there is then no console for it to be said on.
    #[cfg(not(target_os = "android"))]
    pub fn start_log(file: Option<std::fs::File>) {
        let mut log = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"),
        );
        if let Some(file) = file {
            log.target(env_logger::Target::Pipe(Box::new(file)));
            std::panic::set_hook(Box::new(|panic| {
                log::error!("{panic}");
                crate::game::alert(&format!("The game has stopped.\n\n{panic}"));
            }));
        }
        log.init();
    }

    /// Reads a scene: all of it, or what says where the rest of it is.
    /// Why a map that needs the file every map shares could not be read with it.
    fn missing_shared(file: &Path, error: &dyn std::error::Error) -> String {
        format!(
            "{}, which the game's maps are read with, could not be read: {error}",
            file.display()
        )
    }

    /// Where a truck's pack is: a path as it is given, or, for a name alone, `trucks/NAME.truck`
    /// beside the map.
    fn truck_pack(map: &Path, truck: &str) -> PathBuf {
        if truck.contains(['/', '\\', '.']) {
            PathBuf::from(truck)
        } else {
            map.with_file_name("trucks").join(format!("{truck}.truck"))
        }
    }

    fn read_source(
        options: &Options,
        path: &PathBuf,
    ) -> Result<Source, Box<dyn std::error::Error>> {
        Ok(if options.stream {
            let mut scene = read_manifest(path)?;
            if solstice_viewer::shared::wanted(&scene.gameplay) {
                let beside = path.with_file_name(solstice_viewer::shared::FILE);
                let shared =
                    read_manifest(&beside).map_err(|error| missing_shared(&beside, &*error))?;
                let files = shared.gameplay.clone();
                // The shared file's bytes are numbered on from the end of the map's.
                let after = std::fs::metadata(path)?.len();
                let joined = solstice_asset::join::join(&mut scene, shared, after)?;
                solstice_viewer::shared::adopt(&mut scene, &files, joined)?;
                // And what nuggets are shown with, which are made as they are found.
                solstice_viewer::nugget::join_streamed(&mut scene)?;
            }
            Source::Streamed(scene)
        } else {
            let mut scene = load_scene(path)?;
            // A game's maps share its truck, which is in a file of its own beside them: the
            // two are read together, and are one scene.
            if solstice_viewer::shared::wanted(&scene.scene.gameplay) {
                let beside = path.with_file_name(solstice_viewer::shared::FILE);
                let shared =
                    load_scene(&beside).map_err(|error| missing_shared(&beside, &*error))?;
                let files = shared.scene.gameplay.clone();
                let joined = scene.join(shared)?;
                solstice_viewer::shared::adopt(&mut scene.scene, &files, joined)?;
                solstice_viewer::nugget::join_loaded(&mut scene)?;
            }
            // A truck built of pieces, in place of the map's own: put together now, of the
            // pieces it is wearing. What is not fitted to it is not read.
            if !options.shown.is_empty() {
                // Trucks stood here to be looked at: every one of the page, each put together
                // now, so that another is shown with nothing more read.
                let mut trucks = Vec::new();
                for (truck, build) in &options.shown {
                    let pack = truck_pack(path, truck);
                    let mut file = std::fs::File::open(&pack)
                        .map_err(|error| format!("{}: {error}", pack.display()))?;
                    let truck = solstice_viewer::kit::assemble(&mut file, Some(build), 1.0)
                        .map_err(|error| format!("{}: {error}", pack.display()))?;
                    trucks.push(truck);
                }
                solstice_viewer::shared::show_trucks(&mut scene, trucks, options.stand.as_deref())?;
            } else if let Some(truck) = &options.truck {
                let pack = truck_pack(path, truck);
                let mut file = std::fs::File::open(&pack)
                    .map_err(|error| format!("{}: {error}", pack.display()))?;
                let truck =
                    solstice_viewer::kit::assemble(&mut file, options.kit.as_deref(), options.pull)
                        .map_err(|error| format!("{}: {error}", pack.display()))?;
                solstice_viewer::shared::swap_truck(&mut scene, truck)?;
            }
            // A map with a used-truck yard has the day's trucks parked in it, as they
            // were made.
            let spots = solstice_viewer::shared::lot(&scene.scene.gameplay);
            if !spots.is_empty() && !options.parked.is_empty() {
                let mut trucks = Vec::new();
                for sale in &options.parked {
                    let pack = truck_pack(path, &sale.truck);
                    let mut file = std::fs::File::open(&pack)
                        .map_err(|error| format!("{}: {error}", pack.display()))?;
                    let truck = solstice_viewer::kit::assemble(&mut file, Some("stock"), 1.0)
                        .map_err(|error| format!("{}: {error}", pack.display()))?;
                    trucks.push(solstice_viewer::shared::Parked {
                        place: sale.place,
                        truck,
                        paint: sale.paint,
                        price: sale.price,
                        km: sale.km,
                    });
                }
                solstice_viewer::shared::park_trucks(&mut scene, trucks, &spots, options.pulled)?;
            }
            Source::Loaded(scene)
        })
    }

    /// Does what the arguments say. `sites` are where a game's maps are fetched from before
    /// it is played, in its window: none, and they are taken to be here.
    pub fn run(
        arguments: Vec<String>,
        sites: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut options = parse_args(arguments)?;
        let shell = match &options.world {
            None => None,
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let world = solstice_viewer::shell::World::parse(&text)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                // At the title no game is being played: one is begun, or gone back to, from
                // there. (A trial plays the one in its own folder.)
                let kept = match &options.trial {
                    Some(_) => std::fs::read_to_string(save_file(&options)).unwrap_or_default(),
                    None => String::new(),
                };
                options.slot = 0;
                let mut shell = Shell::new(world, &kept);
                shell.set_trial(options.trial.is_some());
                if options.trial.is_none() {
                    shell.set_saves(&kept_games(), 0);
                    shell.set_options(&std::fs::read_to_string(OPTIONS_FILE).unwrap_or_default());
                }
                // A truck asked for is driven, whatever was kept; and whichever it is, the
                // first scene is read with it.
                if let Some(truck) = &options.truck {
                    let built = options.kit.as_deref() == Some("showroom");
                    shell.wear(
                        Some(truck)
                            .filter(|name| *name != "raptor")
                            .map(String::as_str),
                        built,
                    );
                }
                if options.truck.is_none()
                    || options.kit.is_none()
                    || options.kit.as_deref() == Some("showroom")
                {
                    let wearing = shell.wearing();
                    options.kit = wearing
                        .as_ref()
                        .map(|(_, fit)| (*fit).to_owned())
                        .or(options.kit.take());
                    options.truck = wearing.map(|(truck, _)| truck);
                }
                // A game starts in its hub, unless a scene of it was asked for.
                let named = shell
                    .world()
                    .maps
                    .iter()
                    .find(|map| Path::new(&map.scene) == options.scene)
                    .map(|map| map.name.clone());
                match (options.scene_named, named) {
                    (true, Some(name)) => shell.start_in(&name),
                    (true, None) => {}
                    (false, _) => {
                        let hub = shell.world().hub.clone();
                        if let Some(map) = shell.world().map(&hub) {
                            options.scene = PathBuf::from(&map.scene);
                        }
                    }
                }
                Some(shell)
            }
        };
        // A game whose maps are to be fetched opens its window first, and reads its scene
        // when it has come (`Lobby`).
        let windowed = options.instances_near.is_none()
            && options.flyover.is_none()
            && options.capture.is_none();
        let scene = if windowed && !sites.is_empty() {
            None
        } else {
            Some(read_source(&options, &options.scene)?)
        };
        if let (Some(near), Some(scene)) = (options.instances_near, &scene) {
            scene.list_instances(near);
            return Ok(());
        }
        if let (true, Some(scene)) = (options.flyover.is_some(), &scene) {
            let Source::Loaded(scene) = scene else {
                return Err("flyovers load the whole scene, so they can't use --stream".into());
            };
            flyover(&options, scene)
        } else if let (true, Some(scene)) = (options.capture.is_some(), &scene) {
            capture(&options, scene)
        } else {
            #[cfg(not(target_os = "android"))]
            let event_loop = EventLoop::new()?;
            // On a phone the window is the app's activity's, and its events are the app's.
            #[cfg(target_os = "android")]
            let event_loop = {
                use winit::platform::android::EventLoopBuilderExtAndroid;
                EventLoop::builder()
                    .with_android_app(crate::android::app())
                    .build()?
            };
            let started = (
                options.clock.as_ref().map(|clock| clock.time.hours),
                options.weather,
                options.weathered,
            );
            let mut app = App {
                options,
                scene,
                state: None,
                input: CameraInput::default(),
                keys: HashSet::new(),
                captured_mouse: false,
                weather_index: 0,
                // A machine with no way to read gamepads plays on with its keys.
                pads: match gilrs::Gilrs::new() {
                    Ok(pads) => Some(pads),
                    Err(error) => {
                        log::warn!("gamepads can't be read: {error}");
                        None
                    }
                },
                sony: crate::sony::SonyPad::new(),
                shell,
                sites,
                lobby: None,
                asleep: false,
                loading: None,
                look: false,
                cursor: (0.0, 0.0),
                cursor_told: true,
                quit: false,
                started,
            };
            event_loop.run_app(&mut app)?;
            Ok(())
        }
    }

    fn create_instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            // WGPU_BACKEND picks another, such as dx12, which is what Chrome draws with on Windows.
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        })
    }

    /// Renders offscreen, saves a PNG, and fails loudly on any GPU validation error.
    fn capture(options: &Options, scene: &Source) -> Result<(), Box<dyn std::error::Error>> {
        let path = options
            .capture
            .as_ref()
            .expect("capture mode requires a path");
        let mut gpu = pollster::block_on(Gpu::request_within(
            &create_instance(),
            None,
            options.power,
            options.limits.as_ref(),
        ))?;
        gpu.block_compressed_textures &= !options.unpack_textures;

        let errors: Arc<std::sync::Mutex<Vec<String>>> = Default::default();
        let sink = errors.clone();
        gpu.device
            .on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
                sink.lock().expect("error sink").push(error.to_string());
            }));

        let start = Instant::now();
        let (mut renderer, mut reader) = scene.renderer(&gpu, CAPTURE_FORMAT, options);
        log::info!("renderer created in {:.2}s", start.elapsed().as_secs_f32());
        let mut bob = bobbing(options, &mut renderer);
        let mut play = start_play(options, scene, &mut renderer);

        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d {
                width: options.size.0,
                height: options.size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CAPTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());

        let start = Instant::now();
        let mut slowest = 0.0_f32;
        // What each section of the frame took, in all and at most: a redraw of a cached view
        // costs one frame in many, which the last frame's times alone seldom show.
        let mut slowest_sections = [0.0_f32; solstice_render::profiler::SECTIONS.len()];
        let mut summed_sections = [0.0_f32; solstice_render::profiler::SECTIONS.len()];
        let mut timed_frames = 0u32;
        let mut last_time = Instant::now();
        let mut camera = options
            .camera
            .unwrap_or_else(|| initial_camera(scene.viewpoint()));
        if let Some(reader) = &mut reader {
            settle(&gpu, &mut renderer, reader, &camera, &view, options)?;
        }
        let mut clock = options.clock;
        for frame in 0..options.frames {
            if options.strike_at == Some(frame) {
                renderer.strike_lightning();
            }
            camera.position += camera.right() * options.capture_motion;
            if let Some(reader) = &mut reader {
                reader.pump(&mut renderer);
            }
            let environment = light(&mut clock, options.environment, 1.0 / 60.0);
            let dark = is_dark(&environment);
            let environment = night_eyes(environment, options.night_eyes);
            if let Some(bob) = &mut bob {
                bob.advance(&mut renderer, 1.0 / 60.0);
            }
            if let Some(play) = &mut play {
                play.lamps = dark;
                if let (0, Some(wear)) = (frame, options.coat) {
                    play.set_wear(wear);
                }
                if frame == 0 {
                    if let Some(words) = &options.damage {
                        play.stage_damage(words);
                    }
                    for hit in &options.hits {
                        play.strike(*hit);
                    }
                }
                if let (0, Some(build)) = (frame, &options.kit) {
                    play.fit(build)?;
                }
                if let (0, Some(doors)) = (frame, &options.open) {
                    for door in doors.split(',') {
                        play.swing(door.trim(), 1.0)?;
                    }
                }
                for (action, at) in &options.presses {
                    if *at == frame {
                        // `get_out` is not an action of the game's: it is what its shell
                        // does when the button is held, said here for a picture on foot.
                        if action == "get_out" {
                            play.play.get_out();
                        } else {
                            play.play.press(action);
                        }
                    }
                }
                for (key, from, to) in &options.holds {
                    if frame == *from {
                        play.devices.key(key, true);
                    } else if frame == *to {
                        play.devices.key(key, false);
                    }
                }
                for (pixels, at) in &options.mouse_moves {
                    if *at == frame {
                        play.devices.mouse_motion(*pixels, 0.0);
                    }
                }
                for (pixels, at) in &options.mouse_downs {
                    if *at == frame {
                        play.devices.mouse_motion(0.0, *pixels);
                    }
                }
                if options.no_truck {
                    play.set_unseen(&mut renderer, true);
                }
                log_messages(play.frame(&mut renderer, &mut camera, 1.0 / 60.0));
            }
            renderer.render(&gpu, &camera, &environment, &view, options.size, 1.0 / 60.0);
            gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
            if let Some(film) = options.film.as_ref().filter(|_| frame >= options.film_from) {
                let shot = frame - options.film_from;
                if shot % 2 == 0 {
                    std::fs::create_dir_all(film)?;
                    let name = format!("{:05}.png", shot / 2);
                    save_png(&gpu, &target, options.size, &film.join(name))?;
                }
            }
            if frame > 10 {
                slowest = slowest.max(last_time.elapsed().as_secs_f32());
                if let Some(times) = renderer.section_times() {
                    timed_frames += 1;
                    for (index, ms) in times.into_iter().enumerate() {
                        slowest_sections[index] = slowest_sections[index].max(ms);
                        summed_sections[index] += ms;
                    }
                }
            }
            last_time = Instant::now();
        }
        if play.is_some() {
            let p = camera.position;
            log::info!("the camera ended at {:.2}, {:.2}, {:.2}", p.x, p.y, p.z);
        }
        let average = start.elapsed().as_secs_f32() / options.frames.max(1) as f32;

        save_png(&gpu, &target, options.size, path)?;

        let stats = renderer.stats();
        if reader.is_some() {
            let streaming = stats.streaming;
            println!(
                "pages: {} of {} installed in a pool of {} ({} MiB), {} evicted, {} short of room",
                streaming.installed,
                streaming.pages,
                streaming.slots,
                streaming.pool_bytes >> 20,
                streaming.evictions,
                streaming.starved,
            );
            if let Some(bands) = streaming.bands {
                let [near, middle, far] = [bands[0], bands[1], bands[2]];
                let spare = bands[3..]
                    .iter()
                    .fold((0, 0), |sum, band| (sum.0 + band.0, sum.1 + band.1));
                println!(
                    "bands: near {}/{}, middle {}/{}, far {}/{}, spare {}/{}",
                    near.0, near.1, middle.0, middle.1, far.0, far.1, spare.0, spare.1
                );
            }
            let memory = renderer.scene_bytes();
            println!(
                "scene memory: {:.1} MiB of records, {:.1} MiB of pages, {:.1} MiB of textures | textures: {}, {} at middle, {} at top",
                memory.records as f64 / 1048576.0,
                memory.pages as f64 / 1048576.0,
                memory.textures as f64 / 1048576.0,
                renderer.texture_summary(),
                streaming.middle_layers,
                streaming.top_layers,
            );
        }
        println!(
            "avg {:.2} ms, worst {:.2} ms | {} clusters ({} software), {} triangles of {} source ({} unique), {} instances",
            average * 1000.0,
            slowest * 1000.0,
            stats.clusters,
            stats.software_clusters,
            stats.triangles,
            stats.source_triangles,
            stats.unique_triangles,
            stats.instances,
        );
        if stats.particles.emitters > 0 {
            let particles = stats.particles;
            println!(
                "particles: {} alive in {} emitters, {} of {} slots reserved, {} spawned, {} dropped",
                particles.alive,
                particles.emitters,
                particles.reserved,
                particles.capacity,
                particles.spawned,
                particles.dropped,
            );
        }
        println!(
            "last shadow cascade: {} clusters, {} triangles",
            stats.shadow_clusters, stats.shadow_triangles
        );
        if stats.dropped > 0 || stats.shadow_dropped > 0 {
            println!(
                "WARNING: the cull lists overflowed: {} dropped in the main view, {} in the last shadow cascade redrawn",
                stats.dropped, stats.shadow_dropped
            );
        }

        if let Some(times) = renderer.section_times() {
            let parts: Vec<String> = solstice_render::profiler::SECTIONS
                .iter()
                .zip(times)
                .map(|(name, ms)| format!("{name} {ms:.2}"))
                .collect();
            println!(
                "gpu ms: {} | total {:.2}",
                parts.join(", "),
                times.iter().sum::<f32>()
            );
            let listed = |times: [f32; solstice_render::profiler::SECTIONS.len()]| {
                let parts: Vec<String> = solstice_render::profiler::SECTIONS
                    .iter()
                    .zip(times)
                    .filter(|(_, ms)| *ms >= 0.005)
                    .map(|(name, ms)| format!("{name} {ms:.2}"))
                    .collect();
                format!(
                    "{} | total {:.2}",
                    parts.join(", "),
                    times.iter().sum::<f32>()
                )
            };
            let frames = timed_frames.max(1) as f32;
            println!(
                "gpu ms on average: {}",
                listed(summed_sections.map(|ms| ms / frames))
            );
            println!("gpu ms at most: {}", listed(slowest_sections));
        }

        let errors = errors.lock().expect("error sink");
        if errors.is_empty() {
            println!("saved {} with no GPU errors", path.display());
            Ok(())
        } else {
            Err(format!("{} GPU errors, first:\n{}", errors.len(), errors[0]).into())
        }
    }

    /// Renders from one spot until streaming has nothing left to do, so a capture shows what
    /// the view settles on rather than how far loading got.
    fn settle(
        gpu: &Gpu,
        renderer: &mut Renderer,
        reader: &mut PageReader,
        camera: &Camera,
        view: &wgpu::TextureView,
        options: &Options,
    ) -> Result<(), Box<dyn std::error::Error>> {
        const QUIET_FRAMES: u32 = 30;
        const MAX_FRAMES: u32 = 20000;
        let start = Instant::now();
        let mut quiet = 0;
        let mut frames = 0;
        while quiet < QUIET_FRAMES && frames < MAX_FRAMES {
            let before = renderer.stats().streaming.installed;
            reader.pump(renderer);
            renderer.render(
                gpu,
                camera,
                &options
                    .clock
                    .map_or(options.environment, |clock| clock.environment()),
                view,
                options.size,
                1.0 / 60.0,
            );
            gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
            let stats = renderer.stats().streaming;
            let textures_busy = stats.texture_fetches > 0;
            // Pages may be left waiting with nowhere to go, so those don't count as busy.
            if stats.installed == before && !textures_busy && reader.is_idle() {
                quiet += 1;
            } else {
                quiet = 0;
            }
            frames += 1;
        }
        let stats = renderer.stats().streaming;
        println!(
            "streaming settled after {frames} frames, {:.2}s: {} of {} pages",
            start.elapsed().as_secs_f32(),
            stats.installed,
            stats.pages,
        );
        Ok(())
    }

    /// Flies a camera path at a fixed 60 steps per second. Writes an image for each second, plus
    /// `frames.csv` with the time of every frame. Each frame waits for the GPU to finish, so its
    /// time is real GPU work, not hidden by queuing, and image saving is left out of the timing.
    fn flyover(options: &Options, scene: &LoadedScene) -> Result<(), Box<dyn std::error::Error>> {
        const STEP: f32 = 1.0 / 60.0;
        use solstice_render::profiler::SECTIONS;

        let text = std::fs::read_to_string(
            options
                .flyover
                .as_ref()
                .expect("flyover mode requires a path"),
        )?;
        let path = solstice_viewer::camera_path::CameraPath::parse(&text)?;
        std::fs::create_dir_all(&options.flyover_out)?;

        let mut gpu = pollster::block_on(Gpu::request_within(
            &create_instance(),
            None,
            options.power,
            options.limits.as_ref(),
        ))?;
        gpu.block_compressed_textures &= !options.unpack_textures;
        let errors: Arc<std::sync::Mutex<Vec<String>>> = Default::default();
        let sink = errors.clone();
        gpu.device
            .on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
                sink.lock().expect("error sink").push(error.to_string());
            }));
        log::info!("flying on {}", gpu.adapter_info.name);

        let mut renderer = Renderer::new(&gpu, scene, CAPTURE_FORMAT, options.settings);
        renderer.set_weather(options.weather, 0.0);
        renderer.advance_weather(options.weathered);
        if options.ocean.is_some() {
            renderer.set_ocean(options.ocean, 0.0);
        }
        if let Some(level) = options.sea_level {
            renderer.set_sea_level(level);
        }
        if !options.ponds.is_empty() {
            renderer.set_ponds(&options.ponds);
        }
        spawn_effects(&mut renderer, options);
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("flyover"),
            size: wgpu::Extent3d {
                width: options.size.0,
                height: options.size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CAPTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());

        let frames = (path.duration() / STEP) as u32;
        let mut records: Vec<(f32, f32, u32, [f32; SECTIONS.len()])> =
            Vec::with_capacity(frames as usize);
        let mut previous_grid: Option<Vec<f32>> = None;
        let mut changes: Vec<(f32, GridChange)> = Vec::new();
        let mut clock = options.clock;
        let (mut dropped, mut shadow_dropped) = (0, 0);
        for frame in 0..=frames {
            let time = frame as f32 * STEP;
            let start = Instant::now();
            let environment = light(&mut clock, options.environment, STEP);
            renderer.render(
                &gpu,
                &path.sample(time),
                &environment,
                &view,
                options.size,
                STEP,
            );
            gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
            let elapsed = start.elapsed().as_secs_f32() * 1000.0;
            let sections = renderer.section_times().unwrap_or_default();
            let stats = renderer.stats();
            records.push((time, elapsed, stats.triangles, sections));
            dropped = dropped.max(stats.dropped);
            shadow_dropped = shadow_dropped.max(stats.shadow_dropped);
            if options.stability {
                let pixels = read_pixels(&gpu, &target, options.size)?;
                let grid = luma_grid(&pixels, options.size);
                if let Some(previous) = &previous_grid {
                    changes.push((time, grid_change(previous, &grid)));
                }
                previous_grid = Some(grid);
            }
            if frame % 60 == 0 {
                let second = frame / 60;
                save_png(
                    &gpu,
                    &target,
                    options.size,
                    &options.flyover_out.join(format!("{second:03}.png")),
                )?;
            }
        }

        let mut csv = format!("time,ms,triangles,{}\n", SECTIONS.join(","));
        for (time, ms, triangles, sections) in &records {
            let parts: Vec<String> = sections.iter().map(|s| format!("{s:.3}")).collect();
            csv += &format!("{time:.3},{ms:.3},{triangles},{}\n", parts.join(","));
        }
        std::fs::write(options.flyover_out.join("frames.csv"), csv)?;

        // The first frames build everything from nothing, which is loading, not stutter.
        let flight = &records[records.len().min(30)..];
        let mut sorted: Vec<f32> = flight.iter().map(|r| r.1).collect();
        sorted.sort_by(f32::total_cmp);
        let percentile = |p: f32| sorted[((sorted.len() - 1) as f32 * p) as usize];
        let count_over = |limit: f32| flight.iter().filter(|r| r.1 > limit).count();
        println!(
            "{} frames | avg {:.2} ms | p50 {:.2} p95 {:.2} p99 {:.2} max {:.2} | over 16.7 ms: {}, over 33 ms: {} | startup {:.0} ms",
            flight.len(),
            sorted.iter().sum::<f32>() / sorted.len() as f32,
            percentile(0.5),
            percentile(0.95),
            percentile(0.99),
            sorted[sorted.len() - 1],
            count_over(16.7),
            count_over(33.3),
            records[0].1,
        );
        for (index, name) in SECTIONS.iter().enumerate() {
            let (sum, max) = flight.iter().fold((0.0, 0.0_f32), |(sum, max), r| {
                (sum + r.3[index], max.max(r.3[index]))
            });
            println!(
                "  {name:10} avg {:7.2} ms   max {:7.2} ms",
                sum / flight.len() as f32,
                max
            );
        }
        let mut worst: Vec<_> = flight.iter().collect();
        worst.sort_by(|a, b| b.1.total_cmp(&a.1));
        for record in worst.iter().take(4) {
            println!(
                "  slow frame at {:5.2}s: {:6.2} ms, {} triangles",
                record.0, record.1, record.2
            );
        }
        if options.stability {
            report_stability(&changes);
        }

        if dropped > 0 || shadow_dropped > 0 {
            println!(
                "WARNING: the cull lists overflowed: up to {dropped} dropped in the main view, {shadow_dropped} in the last shadow cascade redrawn each frame"
            );
        }
        let errors = errors.lock().expect("error sink");
        if errors.is_empty() {
            println!("no GPU errors");
            Ok(())
        } else {
            Err(format!("{} GPU errors, first:\n{}", errors.len(), errors[0]).into())
        }
    }

    /// How much a frame differs from the one before, on a coarse grid of average brightness.
    #[derive(Clone, Copy)]
    struct GridChange {
        mean: f32,
        worst_tile: f32,
        worst_at: (usize, usize),
    }

    const GRID: (usize, usize) = (48, 27);

    fn luma_grid(pixels: &[u8], size: (u32, u32)) -> Vec<f32> {
        let (width, height) = (size.0 as usize, size.1 as usize);
        let mut sums = vec![0.0_f32; GRID.0 * GRID.1];
        let mut counts = vec![0u32; GRID.0 * GRID.1];
        for y in 0..height {
            for x in 0..width {
                let p = &pixels[(y * width + x) * 4..][..3];
                let tile = (y * GRID.1 / height) * GRID.0 + x * GRID.0 / width;
                sums[tile] += 0.3 * f32::from(p[0]) + 0.6 * f32::from(p[1]) + 0.1 * f32::from(p[2]);
                counts[tile] += 1;
            }
        }
        sums.iter()
            .zip(&counts)
            .map(|(s, &c)| s / c.max(1) as f32)
            .collect()
    }

    fn grid_change(a: &[f32], b: &[f32]) -> GridChange {
        let mut sum = 0.0;
        let mut worst = 0.0_f32;
        let mut worst_at = (0, 0);
        for (index, (x, y)) in a.iter().zip(b).enumerate() {
            let d = (x - y).abs();
            sum += d;
            if d > worst {
                worst = d;
                worst_at = (index % GRID.0, index / GRID.0);
            }
        }
        GridChange {
            mean: sum / a.len() as f32,
            worst_tile: worst,
            worst_at,
        }
    }

    /// Camera motion changes every frame steadily. A flicker or pop is a jump well above the
    /// frames around it, so each frame is compared with the median of its neighbours.
    fn report_stability(changes: &[(f32, GridChange)]) {
        const WINDOW: usize = 15;
        const POP_RATIO: f32 = 3.0;
        const POP_MINIMUM: f32 = 3.0;
        let flight = &changes[changes.len().min(30)..];
        let mut pops: Vec<(f32, f32, f32, (usize, usize))> = Vec::new();
        for index in 0..flight.len() {
            let start = index.saturating_sub(WINDOW);
            let end = (index + WINDOW + 1).min(flight.len());
            let mut around: Vec<f32> = flight[start..end].iter().map(|c| c.1.worst_tile).collect();
            around.sort_by(f32::total_cmp);
            let median = around[around.len() / 2];
            let here = flight[index].1.worst_tile;
            if here > median * POP_RATIO && here > POP_MINIMUM {
                pops.push((flight[index].0, here, median, flight[index].1.worst_at));
            }
        }
        let mean = flight.iter().map(|c| c.1.mean).sum::<f32>() / flight.len().max(1) as f32;
        let worst = flight.iter().map(|c| c.1.worst_tile).fold(0.0, f32::max);
        let worst_time = flight
            .iter()
            .find(|c| c.1.worst_tile == worst)
            .map_or(0.0, |c| c.0);
        println!(
            "stability: mean change {mean:.2}/255 per frame, worst tile {worst:.1}/255 at {worst_time:.2}s, {} sudden jumps",
            pops.len()
        );
        pops.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (time, here, median, at) in pops.iter().take(12) {
            println!(
                "  jump at {time:5.2}s: tile {at:?} of {GRID:?} changed {here:.1}, typical there {median:.1}"
            );
        }
    }

    fn read_pixels(
        gpu: &Gpu,
        texture: &wgpu::Texture,
        size: (u32, u32),
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let row_bytes = size.0 * 4;
        let aligned = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture staging"),
            size: u64::from(aligned) * u64::from(size.1),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        staging.slice(..).map_async(wgpu::MapMode::Read, |result| {
            result.expect("mapping the capture buffer failed");
        });
        gpu.device.poll(wgpu::PollType::wait_indefinitely())?;

        let data = staging.slice(..).get_mapped_range()?;
        let mut pixels = Vec::with_capacity((row_bytes * size.1) as usize);
        for row in data.chunks_exact(aligned as usize) {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(data);
        Ok(pixels)
    }

    fn save_png(
        gpu: &Gpu,
        texture: &wgpu::Texture,
        size: (u32, u32),
        path: &PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pixels = read_pixels(gpu, texture, size)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, size.0, size.1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        Ok(())
    }

    struct WindowState {
        /// The menus and instruments, laid over the window. Before the window, so that it
        /// goes first when both go.
        overlay: Option<crate::overlay::Overlay>,
        window: Arc<Window>,
        /// What the surface was made by, and is made by again when a phone gives the app a
        /// new one to draw on.
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        gpu: Gpu,
        renderer: Renderer,
        reader: Option<PageReader>,
        camera: FlyCamera,
        bob: Option<Bobbing>,
        play: Option<(PlayMode, ScriptWatch)>,
        last_frame: Instant,
        title_timer: f32,
        frames_since_title: u32,
        /// Seconds since the window opened.
        age: f32,
        /// A picture a trial has asked for, to be taken of the next frame; and when a trial
        /// last said what the pages were told.
        shot: Option<PathBuf>,
        said: f32,
        /// Frames a second, as last measured, and how long since the dashboard was written.
        fps: f32,
        hud_timer: f32,
    }

    /// The hour, the weather's name and how many seconds of it there have been already, for a
    /// map to be played in.
    type Sky = (f32, &'static str, f32);

    /// Where what a game keeps from one run to the next is written.
    const SAVE_FILE: &str = "save/profile.txt";

    /// Where the game is kept: a trial keeps its own, in its own folder.
    fn save_file(options: &Options) -> PathBuf {
        match &options.trial {
            Some(folder) => folder.join("profile.txt"),
            None => slot_file(options.slot),
        }
    }

    /// The most games that are kept.
    const SLOTS: u32 = 3;
    /// Where the options are kept: they are the player's, whichever game is played.
    const OPTIONS_FILE: &str = "save/options.txt";
    /// The file a kept game is in: the first is the one there has always been.
    fn slot_file(slot: u32) -> PathBuf {
        if slot <= 1 {
            PathBuf::from(SAVE_FILE)
        } else {
            PathBuf::from(format!("save/profile{slot}.txt"))
        }
    }

    /// The places there are for kept games: each its number, and what is kept in it, if a
    /// game is.
    fn kept_games() -> Vec<(u32, Option<String>)> {
        (1..=SLOTS)
            .map(|slot| (slot, std::fs::read_to_string(slot_file(slot)).ok()))
            .collect()
    }

    /// Does what was asked with the kept games: begins a new one in a place, goes back to
    /// one, or deletes one. What is being played is kept first.
    fn change_game(
        playing: &mut u32,
        trial: bool,
        shell: &mut Shell,
        what: solstice_viewer::shell::Saves,
    ) {
        use solstice_viewer::shell::Saves;
        if trial {
            return;
        }
        let write = |file: &Path, text: &str| {
            let saved = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))
                .and_then(|()| std::fs::write(file, text));
            if let Err(error) = saved {
                log::warn!("{}: {error}", file.display());
            }
        };
        if *playing != 0 {
            write(&slot_file(*playing), &shell.keep_now());
        }
        let known = |slot: u32| (1..=SLOTS).contains(&slot);
        let play = match what {
            Saves::Close => {
                log::info!("games: the one in place {playing} is kept and closed");
                *playing = 0;
                shell.close();
                None
            }
            Saves::New(slot) if known(slot) => {
                write(&slot_file(slot), "");
                log::info!("games: a new one is begun in place {slot}");
                Some(slot)
            }
            Saves::Use(slot) if known(slot) && slot_file(slot).exists() => Some(slot),
            Saves::Delete(slot) if known(slot) => {
                if let Err(error) = std::fs::remove_file(slot_file(slot)) {
                    log::warn!("{}: {error}", slot_file(slot).display());
                }
                log::info!("games: the one in place {slot} is deleted");
                if slot == *playing {
                    *playing = 0;
                }
                None
            }
            _ => None,
        };
        if let Some(slot) = play {
            *playing = slot;
            let kept = std::fs::read_to_string(slot_file(slot)).unwrap_or_default();
            log::info!("games: playing the one in place {slot}");
            shell.load(&kept);
        }
        shell.set_saves(&kept_games(), *playing);
    }

    struct App {
        /// The game round the driving, if the scene is one map of a game.
        shell: Option<Shell>,
        /// The scene to read next, and when it was asked for: a moment is left for the pages
        /// to say so before the window stops answering to read it.
        loading: Option<(String, Option<Sky>, Instant)>,
        /// The page asked for the mouse to be taken hold of, to look round with.
        look: bool,
        /// Where the mouse is over the window, in its pixels, and whether the page has been
        /// told of it there.
        cursor: (f64, f64),
        cursor_told: bool,
        /// The hour and the weather the game was started in, which the hub is always in.
        started: (Option<f32>, Weather, f32),
        quit: bool,
        options: Options,
        /// The scene being shown. None only before a game's first map has come.
        scene: Option<Source>,
        /// The sites a game's maps are fetched from before it starts, if any.
        sites: Vec<String>,
        /// The window while they are fetched, before there is a scene to draw in it.
        lobby: Option<Lobby>,
        /// On a phone: the app is not the one in front, and has nothing to draw on.
        asleep: bool,
        state: Option<WindowState>,
        input: CameraInput,
        keys: HashSet<KeyCode>,
        captured_mouse: bool,
        /// Which built-in weather the C key moved to last.
        weather_index: usize,
        /// The gamepads plugged in, if they can be read.
        pads: Option<gilrs::Gilrs>,
        /// A PlayStation controller, which is read by itself.
        sony: Option<crate::sony::SonyPad>,
    }

    /// What the fetching of a game's maps says to its window, from a thread of its own.
    enum Fetched {
        /// How far along the download of the map called `title` is, in bytes.
        Progress { title: String, done: u64, of: u64 },
        /// It is over, and every map is here, or is not.
        Done(bool),
    }

    /// Tells the window how the fetching goes, and the log what is said of it.
    struct ToWindow(mpsc::Sender<Fetched>);

    impl crate::fetch::Tell for ToWindow {
        fn say(&mut self, line: &str) {
            log::info!("maps: {}", line.trim());
        }

        fn progress(&mut self, title: &str, done: u64, of: u64) {
            let _ = self.0.send(Fetched::Progress {
                title: title.to_owned(),
                done,
                of,
            });
        }
    }

    /// A game's window before there is a map to draw in it: its pages alone, on their loading
    /// screen, saying how far along the maps' download is. When the maps have come, the one
    /// the game starts in is read, and the game begins in the same window.
    struct Lobby {
        /// Before the window, so that it goes first when both go.
        overlay: Option<crate::overlay::Overlay>,
        window: Arc<Window>,
        said: mpsc::Receiver<Fetched>,
        /// What the pages show: the loading screen, for the map the game starts in.
        screen: String,
        /// What they say of the download, as their `ui.stream` takes it.
        stream: String,
        /// The pages have been told all of that as it now is.
        told: bool,
        /// The fetching is over, and since when: the map is read a moment after, so that
        /// the pages have said so before the window stops to read it.
        fetched: Option<Instant>,
        /// Why the game cannot start, if it cannot: said on the pages, which wait to be closed.
        stuck: Option<String>,
    }

    /// What `ui.stream` is told: what is coming, how much of it has, in words and as a share,
    /// and whether more is coming now.
    fn stream_told(what: &str, much: &str, share: f64, busy: bool) -> String {
        serde_json::json!({ "what": what, "much": much, "share": share, "busy": busy, "fades": false })
            .to_string()
    }

    /// Hands the game what the first gamepad plugged in is doing: each button by how far it is
    /// pressed, so that the triggers are pedals, and each stick, by the names the standard
    /// layout gives them (`solstice_game::input`).
    ///
    /// Returns whose controller it is, an Xbox's or a PlayStation's, for its buttons to be
    /// drawn as they are; or `None` with none plugged in.
    fn read_pads(
        pads: &mut gilrs::Gilrs,
        devices: &mut solstice_game::Devices,
    ) -> Option<solstice_viewer::shell::PadKind> {
        use gilrs::{Axis, Button};
        // Its events are what keeps its picture of each pad up to date.
        while pads.next_event().is_some() {}
        let (_, pad) = pads.gamepads().next()?;
        let kind = solstice_viewer::shell::PadKind::of(pad.name(), pad.vendor_id());
        const BUTTONS: [(Button, &str); 16] = [
            (Button::South, "south"),
            (Button::East, "east"),
            (Button::West, "west"),
            (Button::North, "north"),
            (Button::LeftTrigger, "l1"),
            (Button::RightTrigger, "r1"),
            (Button::LeftTrigger2, "l2"),
            (Button::RightTrigger2, "r2"),
            (Button::Select, "select"),
            (Button::Start, "start"),
            (Button::LeftThumb, "l3"),
            (Button::RightThumb, "r3"),
            (Button::DPadUp, "up"),
            (Button::DPadDown, "down"),
            (Button::DPadLeft, "left"),
            (Button::DPadRight, "right"),
        ];
        for (button, name) in BUTTONS {
            let value = pad.button_data(button).map_or(0.0, |data| data.value());
            devices.gamepad_button(name, value);
        }
        // A stick pushed away is above zero here, and below it in the standard layout.
        for (axis, name, sign) in [
            (Axis::LeftStickX, "left_x", 1.0),
            (Axis::LeftStickY, "left_y", -1.0),
            (Axis::RightStickX, "right_x", 1.0),
            (Axis::RightStickY, "right_y", -1.0),
        ] {
            devices.gamepad_axis(name, pad.value(axis) * sign);
        }
        Some(kind)
    }

    impl App {
        /// Opens the window, and lays the game's pages over it if it has any.
        fn open_window(
            &self,
            event_loop: &ActiveEventLoop,
        ) -> Result<(Arc<Window>, Option<crate::overlay::Overlay>), Box<dyn std::error::Error>>
        {
            let mut attributes = Window::default_attributes()
                .with_title("Solstice")
                .with_inner_size(PhysicalSize::new(self.options.size.0, self.options.size.1));
            // A trial's window is nowhere it can be seen, and is not the one typed into.
            if self.options.trial.is_some() {
                attributes = attributes
                    .with_position(winit::dpi::PhysicalPosition::new(-30000, -30000))
                    .with_active(false);
                #[cfg(windows)]
                {
                    use winit::platform::windows::WindowAttributesExtWindows;
                    attributes = attributes.with_skip_taskbar(true);
                }
            }
            // The menus are a window inside this one, and see-through: what is drawn here has
            // to be drawn under them too.
            #[cfg(windows)]
            let attributes = {
                use winit::platform::windows::WindowAttributesExtWindows;
                attributes.with_clip_children(self.options.ui.is_none())
            };
            let window = Arc::new(event_loop.create_window(attributes)?);
            let overlay = self
                .options
                .ui
                .as_ref()
                .and_then(|folder| crate::overlay::Overlay::new(&window, folder));
            Ok((window, overlay))
        }

        /// Opens a game's window before its maps are here, and starts fetching them.
        fn open_lobby(
            &mut self,
            event_loop: &ActiveEventLoop,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let (window, overlay) = self.open_window(event_loop)?;
            let (sender, said) = mpsc::channel();
            let sites = self.sites.clone();
            std::thread::spawn(move || {
                let whole = crate::fetch::fetch_maps(
                    &crate::fetch::wire(),
                    &sites,
                    Path::new("assets"),
                    &mut ToWindow(sender.clone()),
                );
                let _ = sender.send(Fetched::Done(whole));
            });
            let screen = match &self.shell {
                Some(shell) => shell.heading_out(),
                None => serde_json::json!({ "screen": "loading", "map": {} }).to_string(),
            };
            self.lobby = Some(Lobby {
                overlay,
                window,
                said,
                screen,
                stream: stream_told("Checking the maps", "", 0.0, true),
                told: false,
                fetched: None,
                stuck: None,
            });
            Ok(())
        }

        /// A step of the wait for a game's maps: hears how the fetching goes, tells the pages,
        /// and when it is over reads the map the game starts in and begins.
        fn wait_for_maps(&mut self, event_loop: &ActiveEventLoop) {
            let Some(lobby) = self.lobby.as_mut() else {
                return;
            };
            while let Ok(said) = lobby.said.try_recv() {
                lobby.told = false;
                match said {
                    Fetched::Progress { title, done, of } => {
                        let much =
                            format!("{:.0} of {:.0} MB", done as f64 / 1.0e6, of as f64 / 1.0e6);
                        let share = if of == 0 {
                            1.0
                        } else {
                            done as f64 / of as f64
                        };
                        // With no pages, the window's title is all there is to say it on.
                        lobby
                            .window
                            .set_title(&format!("Downloading {title}: {much}"));
                        lobby.stream =
                            stream_told(&format!("Downloading {title}"), &much, share, true);
                    }
                    Fetched::Done(whole) => {
                        if !whole {
                            log::warn!("maps: not all of the maps are here");
                        }
                        lobby.window.set_title("Loading");
                        lobby.stream = stream_told("Loading", "", 0.0, true);
                        lobby.fetched = Some(Instant::now());
                    }
                }
            }
            if let Some(overlay) = &mut lobby.overlay {
                // A page that has just loaded has been told nothing yet.
                if overlay.take().iter().any(|line| line == "hello") {
                    lobby.told = false;
                }
                if !lobby.told {
                    lobby.told = true;
                    overlay.show(&lobby.screen);
                    match &lobby.stuck {
                        None => overlay.run(&format!(
                            "window.ui&&window.ui.stream&&window.ui.stream({})",
                            lobby.stream
                        )),
                        Some(why) => overlay.run(&format!(
                            "window.ui&&window.ui.stuck&&window.ui.stuck({})",
                            serde_json::json!(why)
                        )),
                    }
                }
            }
            let ready = lobby
                .fetched
                .is_some_and(|since| since.elapsed().as_secs_f32() > 0.4);
            if !ready || lobby.stuck.is_some() {
                return;
            }
            // The map the game starts in. Without it there is nothing to play; with it, the
            // game is played with what there is, and a map that did not come is fetched the
            // next time.
            let path = self.options.scene.clone();
            match read_source(&self.options, &path) {
                Err(error) => {
                    log::error!("{error}");
                    lobby.window.set_title("The maps could not be downloaded");
                    lobby.stuck = Some(if path.exists() {
                        format!("{error}")
                    } else {
                        "The game downloads its maps the first time it is played, and could not get \
                         them. Check the internet connection and open the game again: it carries on \
                         where it stopped."
                            .to_owned()
                    });
                    lobby.told = false;
                    if lobby.overlay.is_none() {
                        crate::game::alert(lobby.stuck.as_deref().unwrap_or_default());
                        event_loop.exit();
                    }
                }
                Ok(scene) => {
                    self.scene = Some(scene);
                    let lobby = self.lobby.take().expect("just seen");
                    match self.create_state(lobby.window, lobby.overlay) {
                        Ok(state) => {
                            if let Some(overlay) = &state.overlay {
                                overlay.run("window.ui&&window.ui.stream&&window.ui.stream(null)");
                            }
                            state.window.request_redraw();
                            self.state = Some(state);
                            self.enter_scene();
                        }
                        Err(error) => {
                            log::error!("{error}");
                            crate::game::alert(&error.to_string());
                            event_loop.exit();
                        }
                    }
                }
            }
        }

        /// Tells the game's shell of the scene now shown: its trailheads, and where it starts.
        fn enter_scene(&mut self) {
            if let (Some(shell), Some(scene)) = (&mut self.shell, &self.scene) {
                shell.enter(&with_trial_place(scene.gameplay()));
            }
        }

        /// Starts drawing the scene in a window, and its game if it has one.
        fn create_state(
            &self,
            window: Arc<Window>,
            overlay: Option<crate::overlay::Overlay>,
        ) -> Result<WindowState, Box<dyn std::error::Error>> {
            let scene = self.scene.as_ref().ok_or("there is no scene to show")?;
            let instance = create_instance();
            let surface = instance.create_surface(window.clone())?;
            let gpu = pollster::block_on(Gpu::request_within(
                &instance,
                Some(&surface),
                self.options.power,
                self.options.limits.as_ref(),
            ))?;

            let size = window.inner_size();
            let mut config = surface
                .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
                .ok_or("this surface cannot be presented to")?;
            config.format = config.format.remove_srgb_suffix();
            config.present_mode = wgpu::PresentMode::AutoVsync;
            surface.configure(&gpu.device, &config);

            let (mut renderer, reader) = scene.renderer(&gpu, config.format, &self.options);
            let bob = bobbing(&self.options, &mut renderer);
            let play = start_play(&self.options, scene, &mut renderer).map(|mut play| {
                // A game in a window is heard: its sounds are beside the scenes. A trial
                // is not.
                if self.options.trial.is_none() {
                    play.with_sound(std::path::Path::new("assets/sounds"));
                }
                let watch = ScriptWatch::new(&play, &self.options.scripts);
                (play, watch)
            });
            Ok(WindowState {
                overlay,
                bob,
                play,
                reader,
                window,
                instance,
                surface,
                config,
                gpu,
                renderer,
                camera: FlyCamera::new(
                    self.options
                        .camera
                        .unwrap_or_else(|| initial_camera(scene.viewpoint())),
                ),
                last_frame: Instant::now(),
                title_timer: 0.0,
                frames_since_title: 0,
                age: 0.0,
                shot: None,
                said: 0.0,
                fps: 0.0,
                hud_timer: 0.0,
            })
        }

        /// Leaves the scene being played for another: reads it, and starts its game in the
        /// same window, with the same sound.
        fn go_to(
            &mut self,
            scene: &str,
            sky: Option<Sky>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // The hour and the weather asked for, or the ones the game was started in.
            let (hours, weather, weathered) = match sky {
                Some((hours, name, weathered)) => (
                    // (No hour: the clock goes on from where it has got to.)
                    Some(hours).filter(|hours| *hours >= 0.0),
                    Weather::by_name(name).unwrap_or(self.started.1),
                    weathered,
                ),
                None => self.started,
            };
            self.options.weather = weather;
            self.options.weathered = weathered;
            if let (Some(hours), Some(clock)) = (hours, &mut self.options.clock) {
                clock.time.hours = hours;
            }
            let path = PathBuf::from(scene);
            // The truck the game says is driven, fitted as it says: or the trucks it says
            // stand here to be looked at.
            if let Some(shell) = &mut self.shell {
                self.options.shown = shell.showing();
                (self.options.parked, self.options.pulled) = shell.parked();
                self.options.pull = shell.power();
                self.options.stand = shell.stand().map(str::to_owned);
                let wearing = shell.wearing();
                self.options.kit = wearing.as_ref().map(|(_, fit)| (*fit).to_owned());
                self.options.truck = wearing.map(|(truck, _)| truck);
            }
            let source = read_source(&self.options, &path)?;
            self.options.scene = path;
            let scene = self.scene.insert(source);
            let Some(state) = self.state.as_mut() else {
                return Ok(());
            };
            let sound = state.play.as_mut().and_then(|(play, _)| play.take_sound());
            state.play = None;
            state.bob = None;
            let (mut renderer, reader) =
                scene.renderer(&state.gpu, state.config.format, &self.options);
            state.bob = bobbing(&self.options, &mut renderer);
            state.play = start_play(&self.options, scene, &mut renderer).map(|mut play| {
                match sound {
                    Some(sound) => play.give_sound(sound),
                    None if self.options.trial.is_some() => {}
                    None => play.with_sound(Path::new("assets/sounds")),
                }
                let watch = ScriptWatch::new(&play, &self.options.scripts);
                (play, watch)
            });
            state.renderer = renderer;
            state.reader = reader;
            state.camera = FlyCamera::new(
                self.options
                    .camera
                    .unwrap_or_else(|| initial_camera(scene.viewpoint())),
            );
            // The time it took to read is not a frame's worth of driving.
            state.last_frame = Instant::now();
            self.enter_scene();
            Ok(())
        }

        fn axis(&self, positive: KeyCode, negative: KeyCode) -> f32 {
            f32::from(u8::from(self.keys.contains(&positive)))
                - f32::from(u8::from(self.keys.contains(&negative)))
        }

        fn draw(&mut self) {
            let forward = self.axis(KeyCode::KeyW, KeyCode::KeyS);
            let right = self.axis(KeyCode::KeyD, KeyCode::KeyA);
            let up = self.axis(KeyCode::KeyE, KeyCode::KeyQ);
            let fast = self.keys.contains(&KeyCode::ShiftLeft);
            let look = std::mem::take(&mut self.input);
            let night_eyes = self.options.night_eyes;
            let kept_in = save_file(&self.options);
            let (fixed, clock) = (self.options.environment, &mut self.options.clock);
            let Some(state) = self.state.as_mut() else {
                return;
            };

            if let Some(reader) = &mut state.reader {
                reader.pump(&mut state.renderer);
            }

            let now = Instant::now();
            let delta = (now - state.last_frame).as_secs_f32();
            state.last_frame = now;
            let environment = light(clock, fixed, delta);
            let dark = is_dark(&environment);
            let environment = self::night_eyes(environment, night_eyes);
            if let Some(bob) = &mut state.bob {
                bob.advance(&mut state.renderer, delta);
            }
            // Playing as someone, the keys and the mouse are theirs; otherwise, or having left
            // them with V, they fly the camera.
            let playing_as_someone = state
                .play
                .as_ref()
                .is_some_and(|(p, _)| p.play.has_camera());
            if !playing_as_someone {
                state.camera.update(
                    &CameraInput {
                        forward,
                        right,
                        up,
                        fast,
                        ..look
                    },
                    delta,
                );
            }
            let mut driving = false;
            // What the pages say: the keys and the mouse they were given, and what was chosen
            // in a menu.
            let said = state.overlay.as_mut().map(|o| o.take()).unwrap_or_default();
            // Where the mouse has got to, for the page to show what it is over: once a frame
            // at most, and not while the mouse is held to look round with.
            if !self.cursor_told && !self.captured_mouse {
                self.cursor_told = true;
                if let Some(overlay) = &state.overlay {
                    let scale = state.window.scale_factor();
                    overlay.pointer("move", self.cursor.0, self.cursor.1, scale);
                }
            }
            if let Some((play, watch)) = &mut state.play {
                watch.check(play, delta);
                play.lamps = dark;
                for line in &said {
                    match &mut self.shell {
                        Some(shell) => shell.hear(line, &mut play.devices),
                        None => {
                            let words: Vec<&str> = line.split_whitespace().collect();
                            if let ["key", code, down] = words[..] {
                                play.devices.key(code, down == "1");
                            }
                        }
                    }
                }
                // What a trial asks for: scripts run in the pages, and controls pressed for a
                // moment. While one is pressed the real pads are not read, which would let it
                // go again.
                let (before, now) = (state.age, state.age + delta);
                state.age = now;
                for (script, at) in &self.options.ui_runs {
                    if (before..now).contains(at) {
                        if let Some(overlay) = &state.overlay {
                            overlay.run(script);
                        }
                    }
                }
                let mut tapping = false;
                for (control, down, up) in &self.options.taps {
                    let (down, up) = (*down, *up);
                    let begun = (before..now).contains(&down);
                    let Some((device, control)) = control.split_once(':') else {
                        if control == "quit" && begun {
                            self.quit = true;
                        }
                        // A controller's button, by its name alone.
                        if control != "quit" {
                            let held = (down..up).contains(&now);
                            tapping |= held;
                            if held || (before..now).contains(&up) {
                                play.devices
                                    .gamepad_button(control, f32::from(u8::from(held)));
                            }
                        }
                        continue;
                    };
                    // What a trial does besides pressing things, each once, as it begins.
                    let numbers: Vec<f32> = control
                        .split(',')
                        .filter_map(|n| n.trim().parse().ok())
                        .collect();
                    match (device, &numbers[..]) {
                        ("mouse", [x, y]) => {
                            if begun {
                                play.devices.mouse_motion(*x, *y);
                            }
                            continue;
                        }
                        ("place", [x, y, z, yaw]) => {
                            if begun {
                                play.play.place_car(Vec3::new(*x, *y, *z), *yaw);
                            }
                            continue;
                        }
                        ("shot", _) => {
                            if begun {
                                let folder = self
                                    .options
                                    .trial
                                    .clone()
                                    .unwrap_or_else(|| PathBuf::from("save"));
                                state.shot = Some(folder.join(format!("{control}.png")));
                            }
                            continue;
                        }
                        _ => {}
                    }
                    let held = (down..up).contains(&now);
                    tapping |= held;
                    if held || (before..now).contains(&up) {
                        match device {
                            "key" => play.devices.key(control, held),
                            _ => play
                                .devices
                                .gamepad_button(control, f32::from(u8::from(held))),
                        }
                    }
                }
                if !tapping && self.options.trial.is_none() {
                    use solstice_viewer::shell::PadKind;
                    let mut kind = None;
                    if let Some(pads) = &mut self.pads {
                        kind = read_pads(pads, &mut play.devices);
                    }
                    // A PlayStation controller, if there is one, has the last word.
                    if let Some(sony) = &mut self.sony {
                        if sony.read(&mut play.devices, delta) {
                            kind = Some(PadKind::PlayStation);
                        }
                    }
                    // Whose buttons the pages draw: those of the controller there is.
                    if let (Some(kind), Some(shell)) = (kind, &mut self.shell) {
                        shell.pad_kind(kind);
                    }
                }
                let step = match &mut self.shell {
                    Some(shell) => {
                        // What hour it is, for what a sleep can be till; and how fast the
                        // right stick looks, from the next time it is read.
                        if let Some(clock) = clock.as_ref() {
                            shell.set_hour(clock.time.hours);
                        }
                        play.devices.look = shell.look_gain();
                        shell.before(&mut play.devices, delta)
                    }
                    None => Step::Play,
                };
                let camera = &mut state.camera.camera;
                match step {
                    Step::Play => log_messages(play.frame(&mut state.renderer, camera, delta)),
                    Step::Unattended(orbit) => log_messages(play.frame_unattended(
                        &mut state.renderer,
                        camera,
                        delta,
                        orbit,
                    )),
                    Step::Held => play.frame_held(&state.renderer, delta),
                }
                if let Some(shell) = &mut self.shell {
                    shell.see(play.things());
                    shell.after(play.play.hud());
                    shell.bound(play.play.input_map());
                    shell.see_dirt(play.dirt());
                    let blows = play.take_damage_changes();
                    if !blows.is_empty() {
                        shell.see_damage(play.damage().to_text(), &blows);
                    }
                    // While somebody else's run is watched, the camera is the shot's.
                    if let Some((position, yaw, pitch)) = shell.view() {
                        (camera.position, camera.yaw, camera.pitch) = (position, yaw, pitch);
                    }
                    // In the showroom it is before the truck that stands there.
                    // (Where that is is asked of the game: behind a menu the shell has not
                    // been told where the map's truck stands.)
                    let stands = play.play.car_pose();
                    if let Some((at, turned, seconds)) = shell.showroom() {
                        let (at, turned) = play.stand().unwrap_or((at, turned));
                        (camera.position, camera.yaw, camera.pitch) =
                            play.showroom_camera(at, turned, seconds, shell.focus());
                    }
                    // Behind the main menu there is no truck: only the bush, looked round.
                    let preview = shell.preview();
                    play.set_unseen(&mut state.renderer, preview.is_some());
                    if let Some((at, seconds)) = preview {
                        let (at, turned) = stands.unwrap_or((at, glam::Quat::IDENTITY));
                        (camera.position, camera.yaw, camera.pitch) =
                            play.preview_camera(at, turned, seconds);
                    }
                    play.set_detector(shell.detector());
                    // The options: how loud it is, and whether its window is the screen.
                    play.set_volume(shell.volume());
                    if self.options.trial.is_none() {
                        let full = shell.fullscreen();
                        if state.window.fullscreen().is_some() != full {
                            let whole = winit::window::Fullscreen::Borderless(None);
                            state.window.set_fullscreen(full.then_some(whole));
                        }
                    }
                    if let Some(told) = shell.tell(play.play.hud(), delta) {
                        if let Some(overlay) = &state.overlay {
                            overlay.show(&told);
                        }
                        // A trial says what the pages are told, once a second, to be read.
                        if self.options.trial.is_some() && state.age - state.said >= 1.0 {
                            state.said = state.age;
                            log::info!("told at {:.1}: {told}", state.age);
                        }
                    }
                    // (What is not a kept game, the title and a free drive, is not kept.)
                    let playing = self.options.trial.is_some() || self.options.slot != 0;
                    if let Some(kept) = shell.to_keep().filter(|_| playing) {
                        let file = kept_in;
                        let saved =
                            std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))
                                .and_then(|()| std::fs::write(&file, kept));
                        if let Err(error) = saved {
                            log::warn!("{}: {error}", file.display());
                        }
                    }
                    for order in shell.take_orders() {
                        match order {
                            Order::Look => self.look = true,
                            Order::Quit => self.quit = true,
                            // A night slept through: it is the next morning.
                            Order::Sleep { hours, days } => {
                                if let Some(clock) = clock.as_mut() {
                                    clock.time.hours = hours;
                                    clock.time.day += days as f32;
                                }
                                // The new day has its own weather, as it was forecast.
                                let (name, soaked) = shell.today_sky();
                                if let Some(weather) = Weather::by_name(name) {
                                    state.renderer.set_weather(weather, 0.0);
                                    state.renderer.advance_weather(soaked);
                                    self.options.weather = weather;
                                    self.options.weathered = soaked;
                                }
                            }
                            // The gold: a nugget shown or put away, its body put in the
                            // world or taken out, and what is heard.
                            Order::Item(shown) => play.show_item(&mut state.renderer, shown),
                            Order::Thing(body) => play.thing(body),
                            Order::Carry(carried) => play.set_carried(carried),
                            Order::Sound(name) => play.sound_once(name),
                            // A door of the truck's swings. (A truck with no kit has none.)
                            Order::Door { piece, open } => {
                                let _ = play.swing(&piece, if open { 1.0 } else { 0.0 });
                            }
                            Order::Paint(colour) => play.paint(colour),
                            Order::Dirt(dirt) => play.set_dirt(dirt),
                            Order::Damage(damage) => {
                                if let Err(error) = play.set_damage(&damage) {
                                    log::warn!("the truck's damage: {error}");
                                }
                            }
                            Order::Show { index, paint } => {
                                play.display(&mut state.renderer, index, paint);
                            }
                            Order::Unpark(place) => play.unpark(&mut state.renderer, place),
                            Order::Fit(build) => {
                                if let Err(error) = play.display_fit(&build) {
                                    log::warn!("fitting {build}: {error}");
                                }
                            }
                            Order::Options => {
                                if self.options.trial.is_none() {
                                    let kept = std::fs::create_dir_all("save").and_then(|()| {
                                        std::fs::write(OPTIONS_FILE, shell.options_text())
                                    });
                                    if let Err(error) = kept {
                                        log::warn!("{OPTIONS_FILE}: {error}");
                                    }
                                }
                            }
                            Order::Saves(what) => {
                                let trial = self.options.trial.is_some();
                                change_game(&mut self.options.slot, trial, shell, what);
                            }
                            Order::Afoot(at) => {
                                let stood = play.play.stand_at(at);
                                log::info!("driver: out at the door, {stood}");
                            }
                            // Out of the truck, or, beside it, back in.
                            Order::Swap => {
                                let done = if play.play.afoot() {
                                    play.play.get_in(solstice_viewer::shell::GET_IN + 1.0)
                                } else {
                                    play.play.get_out()
                                };
                                log::info!(
                                    "driver: {}",
                                    match (done, play.play.afoot()) {
                                        (false, _) => "stays where they are",
                                        (true, true) => "out of the truck",
                                        (true, false) => "back in the truck",
                                    }
                                );
                            }
                            Order::Place { position, yaw } => {
                                play.play.place_car(position, yaw);
                                log::info!("the truck is where it was left: {position}");
                            }
                            // A timed run, or somebody else's being watched.
                            Order::Stage(stage) => crate::cinema::stage(play, stage),
                            Order::Load { map, scene, sky } => {
                                log::info!("going to {map}: {scene}");
                                self.loading = Some((scene, sky, Instant::now()));
                            }
                        }
                    }
                }
                // Driving, the title is the dashboard: speed, gear, revs and lap times.
                if let Some(hud) = play.play.hud() {
                    driving = true;
                    state.hud_timer += delta;
                    if state.hud_timer > 0.1 {
                        state.hud_timer = 0.0;
                        // (A game has its instruments on its pages: its window has its name.)
                        if self.shell.is_some() {
                            state.window.set_title("Aussie Outback 4x4");
                        } else {
                            // (And what the truck last hit, for a while after it.)
                            state.window.set_title(&format!(
                                "{}{}   |   {:.0} fps",
                                hud.line(),
                                play.damage_line(),
                                state.fps
                            ));
                        }
                    }
                }
            }

            state.title_timer += delta;
            state.frames_since_title += 1;
            if state.title_timer > 0.5 {
                let stats = state.renderer.stats();
                state.fps = state.frames_since_title as f32 / state.title_timer;
                if !driving {
                    state.window.set_title(&format!(
                        "Solstice | {:.0} fps | {:.2}M tris of {:.0}M | {} clusters ({} sw)",
                        state.fps,
                        stats.triangles as f32 / 1.0e6,
                        stats.source_triangles as f32 / 1.0e6,
                        stats.clusters,
                        stats.software_clusters,
                    ));
                }
                state.title_timer = 0.0;
                state.frames_since_title = 0;
            }

            // A picture a trial asked for: this frame is drawn to be kept, not to be shown.
            if let Some(path) = state.shot.take() {
                let size = (state.config.width, state.config.height);
                let kept = state.gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("a trial's picture"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: state.config.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = kept.create_view(&Default::default());
                state.renderer.render(
                    &state.gpu,
                    &state.camera.camera,
                    &environment,
                    &view,
                    size,
                    delta,
                );
                let swapped = matches!(
                    state.config.format,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
                );
                let saved = read_pixels(&state.gpu, &kept, size).and_then(|mut pixels| {
                    if swapped {
                        for pixel in pixels.chunks_exact_mut(4) {
                            pixel.swap(0, 2);
                        }
                    }
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let file = std::io::BufWriter::new(std::fs::File::create(&path)?);
                    let mut encoder = png::Encoder::new(file, size.0, size.1);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder.write_header()?.write_image_data(&pixels)?;
                    Ok(())
                });
                match saved {
                    Ok(()) => log::info!("a picture: {}", path.display()),
                    Err(error) => log::warn!("{}: {error}", path.display()),
                }
                return;
            }
            let texture = match state.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture)
                | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    state.surface.configure(&state.gpu.device, &state.config);
                    return;
                }
                _ => return,
            };
            let view = texture.texture.create_view(&Default::default());
            state.renderer.render(
                &state.gpu,
                &state.camera.camera,
                &environment,
                &view,
                (state.config.width, state.config.height),
                delta,
            );
            state.gpu.queue.present(texture);
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            // A phone's app that was behind another is in front again, with a new surface to
            // draw on in place of the one it had.
            if std::mem::take(&mut self.asleep) {
                if let Some(state) = &mut self.state {
                    match state.instance.create_surface(state.window.clone()) {
                        Ok(surface) => {
                            let size = state.window.inner_size();
                            state.config.width = size.width.max(1);
                            state.config.height = size.height.max(1);
                            surface.configure(&state.gpu.device, &state.config);
                            state.surface = surface;
                        }
                        Err(error) => log::error!("no surface to draw on: {error}"),
                    }
                    // The time it was away is not a frame's worth of driving.
                    state.last_frame = Instant::now();
                    state.window.request_redraw();
                }
            }
            if self.state.is_some() || self.lobby.is_some() {
                return;
            }
            // With no scene yet, a game's maps are still to come: its window, and the wait.
            let opened = if self.scene.is_none() {
                self.open_lobby(event_loop)
            } else {
                self.open_window(event_loop)
                    .and_then(|(window, overlay)| self.create_state(window, overlay))
                    .map(|state| {
                        self.state = Some(state);
                        self.enter_scene();
                    })
            };
            if let Err(error) = opened {
                log::error!("{error}");
                crate::game::alert(&error.to_string());
                event_loop.exit();
            }
        }

        fn suspended(&mut self, _: &ActiveEventLoop) {
            // A phone has taken the app's surface away: nothing is drawn until it is back.
            self.asleep = true;
            self.keys.clear();
            if let Some((play, _)) = self.state.as_mut().and_then(|s| s.play.as_mut()) {
                play.devices.release_all();
                play.hush();
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            // Drawing a scene keeps itself going, a frame asking for the next. Waiting for
            // maps there is nothing drawn, so the wait is looked in on by the clock.
            if self.lobby.is_some() {
                self.wait_for_maps(event_loop);
            }
            event_loop.set_control_flow(if self.lobby.is_some() {
                ControlFlow::WaitUntil(Instant::now() + std::time::Duration::from_millis(50))
            } else {
                ControlFlow::Wait
            });
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => {
                    // Closed with no warning: what there is of the game is kept first.
                    let playing = self.options.trial.is_some() || self.options.slot != 0;
                    if let Some(shell) = self.shell.as_mut().filter(|_| playing) {
                        let file = save_file(&self.options);
                        let saved =
                            std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))
                                .and_then(|()| std::fs::write(&file, shell.keep_now()));
                        if let Err(error) = saved {
                            log::warn!("{}: {error}", file.display());
                        }
                    }
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    if let Some(lobby) = &self.lobby {
                        if let Some(overlay) = &lobby.overlay {
                            overlay.resize(&lobby.window);
                        }
                    }
                    if let Some(state) = self.state.as_mut() {
                        if size.width > 0 && size.height > 0 {
                            state.config.width = size.width;
                            state.config.height = size.height;
                            state.surface.configure(&state.gpu.device, &state.config);
                            if let Some(overlay) = &state.overlay {
                                overlay.resize(&state.window);
                            }
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        // A phone's Back is what Escape is on a keyboard: it pauses, and
                        // goes back a menu.
                        let code = if code == KeyCode::BrowserBack {
                            KeyCode::Escape
                        } else {
                            code
                        };
                        // winit's key names are the browser's `KeyboardEvent.code` names.
                        if let Some((play, _)) = self.state.as_mut().and_then(|s| s.play.as_mut()) {
                            if !event.repeat {
                                let down = event.state == ElementState::Pressed;
                                play.devices.key(&format!("{code:?}"), down);
                                // Leaves whoever is played to fly the camera, and comes back.
                                if down && code == KeyCode::KeyV && play.play.has_player() {
                                    let flying = !play.play.flying();
                                    play.play.set_flying(flying);
                                    log::info!("{}", if flying { "flying" } else { "playing" });
                                }
                            }
                        }
                        // While someone is played, and not left to fly the camera, the keys
                        // are the game's: C changes a car's view, so it is not the weather's.
                        let played = self
                            .state
                            .as_ref()
                            .and_then(|s| s.play.as_ref())
                            .is_some_and(|(play, _)| play.play.has_player() && !play.play.flying());
                        match event.state {
                            ElementState::Pressed => {
                                self.keys.insert(code);
                                if code == KeyCode::Escape {
                                    self.set_mouse_capture(false);
                                }
                                // Cycles through the built-in weather, easing into each.
                                if code == KeyCode::KeyC && !played {
                                    self.next_weather();
                                }
                                // Moves the clock on an hour, starting one if there is none.
                                if code == KeyCode::KeyT && !played {
                                    let fixed = self.options.environment;
                                    let clock = self.options.clock.get_or_insert_with(|| {
                                        let mut clock = DayClock::new(TimeOfDay::default(), 0.0);
                                        clock.daylight = fixed;
                                        clock
                                    });
                                    clock.time.advance(1.0);
                                    log::info!("time {:.0}:00", clock.time.hours.floor());
                                }
                            }
                            ElementState::Released => {
                                self.keys.remove(&code);
                            }
                        }
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some((play, _)) = self.state.as_mut().and_then(|s| s.play.as_mut()) {
                        let name = match button {
                            MouseButton::Left => Some("left"),
                            MouseButton::Right => Some("right"),
                            MouseButton::Middle => Some("middle"),
                            _ => None,
                        };
                        // The click that captures the mouse is not a click in the game.
                        if let (Some(name), true) = (name, self.captured_mouse) {
                            play.devices
                                .mouse_button(name, state == ElementState::Pressed);
                        }
                    }
                    // Over a game's pages the mouse is the pages' to be told of: they work out
                    // what was clicked, and ask for the mouse to be held if it was nothing,
                    // while driving. With no pages, a click takes hold of it.
                    let paged = self.state.as_ref().is_some_and(|s| s.overlay.is_some());
                    if paged && !self.captured_mouse && button == MouseButton::Left {
                        if let Some(s) = &self.state {
                            if let Some(overlay) = &s.overlay {
                                let what = if state == ElementState::Pressed {
                                    "down"
                                } else {
                                    "up"
                                };
                                overlay.pointer(
                                    what,
                                    self.cursor.0,
                                    self.cursor.1,
                                    s.window.scale_factor(),
                                );
                            }
                        }
                    } else if !paged
                        && state == ElementState::Pressed
                        && button == MouseButton::Left
                    {
                        self.set_mouse_capture(true);
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor = (position.x, position.y);
                    self.cursor_told = false;
                }
                WindowEvent::Focused(false) => {
                    self.keys.clear();
                    if let Some((play, _)) = self.state.as_mut().and_then(|s| s.play.as_mut()) {
                        play.devices.release_all();
                    }
                }
                WindowEvent::RedrawRequested => {
                    if self.asleep {
                        return;
                    }
                    self.draw();
                    if self.quit {
                        event_loop.exit();
                        return;
                    }
                    // The page asked for the mouse to be held, to look round with.
                    if std::mem::take(&mut self.look) {
                        self.set_mouse_capture(true);
                    }
                    // A menu has the mouse back.
                    if self.captured_mouse
                        && self
                            .shell
                            .as_ref()
                            .is_some_and(|shell| shell.screen() != Screen::Drive)
                    {
                        self.set_mouse_capture(false);
                    }
                    // A few frames after it was asked for, so that the pages are showing that
                    // a map is being read before the window stops to read it.
                    if self
                        .loading
                        .as_ref()
                        .is_some_and(|(_, _, since)| since.elapsed().as_secs_f32() > 0.4)
                    {
                        let (scene, sky, _) = self.loading.take().expect("just seen");
                        if let Err(error) = self.go_to(&scene, sky) {
                            log::error!("{scene}: {error}");
                            if let Some(shell) = &mut self.shell {
                                shell.stay();
                            }
                        }
                    }
                    if let Some(state) = &self.state {
                        state.window.request_redraw();
                    }
                }
                _ => {}
            }
        }

        fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
            if let (DeviceEvent::MouseMotion { delta }, true) = (event, self.captured_mouse) {
                self.input.look_yaw -= delta.0 as f32 * 0.002;
                self.input.look_pitch -= delta.1 as f32 * 0.002;
                if let Some((play, _)) = self.state.as_mut().and_then(|s| s.play.as_mut()) {
                    play.devices.mouse_motion(delta.0 as f32, delta.1 as f32);
                }
            }
        }
    }

    impl App {
        /// Eases into the next built-in weather, over half a minute, as a front would bring it.
        fn next_weather(&mut self) {
            let Some(state) = &mut self.state else {
                return;
            };
            let names: Vec<&str> = Weather::names().collect();
            self.weather_index = (self.weather_index + 1) % names.len();
            let name = names[self.weather_index];
            log::info!("weather: {name}");
            state
                .renderer
                .set_weather(Weather::by_name(name).expect("listed weather"), 30.0);
        }

        fn set_mouse_capture(&mut self, capture: bool) {
            let Some(state) = &self.state else { return };
            let mode = if capture {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
            if state.window.set_cursor_grab(mode).is_err() && capture {
                let _ = state.window.set_cursor_grab(CursorGrabMode::Confined);
            }
            state.window.set_cursor_visible(!capture);
            self.captured_mouse = capture;
        }
    }
}
