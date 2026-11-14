# The game on Android

The Android app is the desktop game: the same viewer, the same maps, the same pages for its
menus and instruments, the same game underneath. What differs is how it is started, where its
pages are shown, how it is driven, and how its maps are fetched.

```bash
python scripts/package-android.py                # for phones (arm64)
python scripts/package-android.py arm64 x86_64   # and for the emulator on a PC
```

writes `target/android-pack/AussieOutback4x4.apk`, about 45 MB for phones alone. It is signed
with the Android SDK's debug key, which is enough to put it on a phone by hand
(`adb install -r target/android-pack/AussieOutback4x4.apk`, or copy the file to the phone and
open it); an app for a shop needs a key of its owner's.

It needs the Android SDK (build tools, a platform, the NDK), the Java that comes with Android
Studio, and Rust's Android targets (`rustup target add aarch64-linux-android
x86_64-linux-android`). There is no Gradle project: the script runs the SDK's own tools one
after another (`javac`, `d8`, `aapt2`, `zipalign`, `apksigner`).

## What it is made of

| Part | Where | What it does |
| --- | --- | --- |
| The game | `apps/viewer/src/desktop.rs` | The desktop's viewer, window and all: winit gives it the app's window as it gives it a desktop's. |
| The start | `apps/viewer/src/android.rs` | `android_main`: goes to the app's folder, reads the `game.txt` there, and runs the viewer with what it says. Calls the activity for what only Java can do. |
| The pages | `apps/viewer/src/overlay_android.rs` | The desktop's `Overlay` by another road: the system's web view, which the activity makes. |
| The activity | `android/java/.../MainActivity.java` | A `NativeActivity`. Unpacks the game's files, makes the web view, hears a controller, and fetches from the site. |
| The app's files | `android/AndroidManifest.xml`, `android/game.txt` | What the app is, and what the game is run with. |

**Starting.** The app's package holds what a Windows download has beside its program: the
pages (`ui/`), the sounds, `scenes/outback.world` and a `game.txt` (docs/ui.md). The activity
unpacks them into the app's own folder (`Android/data/com.rust4x4.outback/files`) the first
time each build is opened, before the game starts; the game then reads them as files, as it
does on a desktop. Its maps, its best laps and anything else it keeps are in the same folder.

**The window.** A `NativeActivity` hands its window to native code to draw on, and shows no
views of its own in it. So the pages are in a window of their own on top: a see-through,
full-screen dialog holding one web view. It is the window in front, so touches, keys and a
controller all come to it.

**Driving.** The pages draw the controls on a screen that is touched (`#touch` in
`ui/index.html`): a pad on the left that steers by how far the thumb is slid from where it
came down, the two pedals on the right, and buttons for the rest. They are the game's touch
stick, which the pages tell it of as `touch X Y`, and keys. A controller is heard by the
activity and told to the game a line at a time, as the pages tell it things (`pad button south
1`, `pad axis left_x 0.4`). With a controller plugged in, or used, the controls on the screen
are not drawn and the instruments are where they are on a desktop; touch the screen and they
are back. The phone's Back is Escape: it pauses, and goes back a menu.

**The maps.** Fetched as on Windows (docs/hosting.md), piece by piece and only the pieces
that are not there, with how far along it is on the loading screen. There is no `curl` on a
phone: `fetch.rs` asks the activity, which uses the system's web client. A map is read from
its file a part at a time as it is driven through (`--stream`), not loaded whole.

**Away and back.** When the app goes behind another, the system takes its surface away: the
game stops drawing and is quiet. Back in front, it makes a surface on the new window and goes
on. Closed, the activity ends the app's process, so that the next opening is a fresh start.

## Trying it without a phone

The emulator that comes with Android Studio runs it, with no window of its own:

```bash
emulator -avd NAME -no-window -no-audio -gpu host
adb install -r target/android-pack/AussieOutback4x4.apk
adb shell am start -n com.rust4x4.outback/.MainActivity
adb logcat -s solstice           # what the game says
adb exec-out screencap -p > screen.png
adb shell input tap X Y          # a touch; `input swipe X Y X Y 3000` holds one for 3 s
adb shell input gamepad keyevent 96    # a controller's bottom button
```

Give the emulated phone 6 GB of memory and 12 GB of storage. To skip the download, copy the
maps into the app's folder after it has been opened once (so that the folder is the app's own):
`adb push assets/hub.solstice /sdcard/Android/data/com.rust4x4.outback/files/assets/`, with its
`.solstice.chunks` beside it.

The pages can be looked at as a phone shows them with no phone at all: open `ui/index.html`
with `?screen=drive&touch` in a browser window the shape of a phone on its side.

## What is not known yet

It has been run on the emulator, which draws with the PC's own graphics card. Not yet on a
phone. Two things are different there:

- **Speed.** The renderer was made for a desktop card. A phone's is a fraction of one, and
  the game draws at the screen's full size. How fast it runs is to be measured on a phone, and
  the detail turned down to suit.
- **Textures.** The maps' textures are packed in a form most phones' cards cannot read. The
  viewer unpacks them where it has to, as it does in a browser, which costs memory.
