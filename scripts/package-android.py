"""Builds the game's Android app:

    python scripts/package-android.py [arm64] [x86_64]

into `target/android-pack/AussieOutback4x4.apk`. With no names it is built for phones (arm64);
`x86_64` is the emulator on a PC, and both may be named.

The app is the engine's viewer as a native library (apps/viewer/src/android.rs), one Java
activity that hosts it (android/java), and the game's files: its pages, its sounds and its
`game.txt`. The maps are not in it; the game fetches them the first time it is opened, as the
Windows download does (docs/hosting.md).

It is put together with the Android SDK's own tools and no build system: `javac` and `d8`
compile the activity, `aapt2` packs the manifest and the game's files, the libraries are added,
and `zipalign` and `apksigner` finish it. It is signed with the debug key every Android SDK
makes (~/.android/debug.keystore), which is enough to install it on a phone by hand; an app
for a shop is signed with a key of its owner's.

Needs the Android SDK (build tools, a platform, the NDK), Android Studio's Java, and Rust's
targets: `rustup target add aarch64-linux-android x86_64-linux-android`.
"""
import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "android-pack"
NAME = "AussieOutback4x4"
# The oldest Android it runs on (9), and the one it is written to.
MIN_SDK, TARGET_SDK = 28, 34
VERSION_CODE, VERSION_NAME = 1, "0.2.0"
# Each kind of machine: what Rust calls it, and what an app calls it.
ABIS = {"arm64": ("aarch64-linux-android", "arm64-v8a"), "x86_64": ("x86_64-linux-android", "x86_64")}


def newest(folder, what):
    found = sorted(p for p in folder.iterdir() if p.is_dir()) if folder.is_dir() else []
    if not found:
        sys.exit(f"no {what} in {folder}")
    return found[-1]


def run(*command, **more):
    command = [str(part) for part in command]
    done = subprocess.run(command, **more)
    if done.returncode != 0:
        sys.exit(f"{Path(command[0]).name} failed")


def main():
    wanted = sys.argv[1:] or ["arm64"]
    for abi in wanted:
        if abi not in ABIS:
            sys.exit(__doc__)
    sdk = Path(os.environ.get("ANDROID_HOME") or Path(os.environ["LOCALAPPDATA"]) / "Android" / "Sdk")
    tools = newest(sdk / "build-tools", "build tools")
    platform = newest(sdk / "platforms", "platform") / "android.jar"
    ndk = newest(sdk / "ndk", "NDK") / "toolchains" / "llvm" / "prebuilt" / "windows-x86_64" / "bin"
    java = Path(os.environ.get("JAVA_HOME") or r"C:\Program Files\Android\Android Studio\jbr")
    if not (java / "bin" / "javac.exe").exists():
        sys.exit(f"no Java in {java}: set JAVA_HOME")
    env = dict(os.environ, JAVA_HOME=str(java), PATH=f"{java / 'bin'};{os.environ['PATH']}")

    # ---- The game, as a library for each kind of machine.
    for abi in wanted:
        target, _ = ABIS[abi]
        clang = ndk / f"{target}{MIN_SDK}-clang.cmd"
        name = target.replace("-", "_")
        build = dict(env)
        build[f"CC_{name}"] = str(clang)
        build[f"AR_{name}"] = str(ndk / "llvm-ar.exe")
        build[f"CARGO_TARGET_{name.upper()}_LINKER"] = str(clang)
        run("cargo", "build", "--release", "--target", target, "-p", "viewer", "--lib",
            "--target-dir", ROOT / "target" / "android", cwd=ROOT, env=build)

    work = OUT / "work"
    shutil.rmtree(work, ignore_errors=True)
    (work / "classes").mkdir(parents=True)

    # ---- The activity.
    sources = sorted((ROOT / "android" / "java").rglob("*.java"))
    run(java / "bin" / "javac.exe", "-nowarn", "--release", "11", "-classpath", platform,
        "-d", work / "classes", *sources, env=env)
    classes = sorted((work / "classes").rglob("*.class"))
    run(tools / "d8.bat", "--release", "--lib", platform, "--min-api", MIN_SDK,
        "--output", work, *classes, env=env)

    # ---- The game's files, as the app packs them: what a Windows download has beside its
    # program, under game/.
    game = work / "assets" / "game"
    shutil.copytree(ROOT / "ui", game / "ui")
    (game / "scenes").mkdir()
    shutil.copy(ROOT / "scenes" / "outback.world", game / "scenes")
    shutil.copy(ROOT / "android" / "game.txt", game)
    (game / "assets" / "sounds").mkdir(parents=True)
    for sound in sorted((ROOT / "assets" / "sounds").glob("*.wav")):
        # A file whose name starts with `_` is there to be listened to, and one with a dot in
        # its name is a take: neither is played.
        if not sound.name.startswith("_") and sound.stem.count(".") == 0:
            shutil.copy(sound, game / "assets" / "sounds")

    # ---- The app: the manifest and the files, then the code and the libraries.
    bare = work / "bare.apk"
    run(tools / "aapt2.exe", "link", "-o", bare, "--manifest", ROOT / "android" / "AndroidManifest.xml",
        "-I", platform, "-A", work / "assets",
        "--min-sdk-version", MIN_SDK, "--target-sdk-version", TARGET_SDK,
        "--version-code", VERSION_CODE, "--version-name", VERSION_NAME, env=env)
    with zipfile.ZipFile(bare, "a") as apk:
        apk.write(work / "classes.dex", "classes.dex", compress_type=zipfile.ZIP_DEFLATED)
        for abi in wanted:
            target, folder = ABIS[abi]
            library = ROOT / "target" / "android" / target / "release" / "libsolstice_viewer.so"
            apk.write(library, f"lib/{folder}/libsolstice_viewer.so", compress_type=zipfile.ZIP_DEFLATED)
    aligned = work / "aligned.apk"
    run(tools / "zipalign.exe", "-f", "-P", "16", "4", bare, aligned, env=env)
    key = Path.home() / ".android" / "debug.keystore"
    if not key.exists():
        sys.exit(f"{key} is missing: Android Studio makes it the first time it builds an app")
    pack = OUT / f"{NAME}.apk"
    run(tools / "apksigner.bat", "sign", "--ks", key, "--ks-pass", "pass:android",
        "--ks-key-alias", "androiddebugkey", "--key-pass", "pass:android", "--out", pack, aligned, env=env)
    print(f"{pack}: {pack.stat().st_size / 1e6:.0f} MB, for {', '.join(wanted)}")


main()
