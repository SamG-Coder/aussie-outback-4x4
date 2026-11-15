@echo off
rem Builds everything: the truck's model from the downloaded zip, the programs, and the scenes:
rem Wombat Creek Track, and the hub it is driven to from.
rem Needs Rust (rustup) and Blender. The truck's model is not in the repository: download the
rem 2024 Ford F-150 Raptor R from Sketchfab (see README.md) and unzip it into
rem target\outback\raptor_src, or into ..\assets\raptor_src beside this folder.
setlocal
cd /d "%~dp0"
if "%BLENDER%"=="" set BLENDER=D:\Blender\blender.exe
set GLB=target\outback\raptor_src\source\2024_ford_f-150_raptor_r.glb
if not exist "%GLB%" set GLB=..\assets\raptor_src\source\2024_ford_f-150_raptor_r.glb
if not exist target\outback\raptor.glb (
  if not exist "%GLB%" (
    echo The truck's model is missing: unzip 2024-ford-f-150-raptor-r.zip into target\outback\raptor_src first.
    exit /b 1
  )
  if not exist target\outback mkdir target\outback
  "%BLENDER%" -b --python tools\blender\raptor\prepare.py -- "%GLB%" target\outback\raptor.glb || exit /b 1
)
rem The shed at the Crossroads is made from nothing by its script, in Blender. Delete
rem target\outback\shed.glb to have it made again after the script has changed.
if not exist target\outback mkdir target\outback
if not exist target\outback\shed.glb (
  "%BLENDER%" -b --python tools\blender\shed\build.py -- target\outback\shed.glb || exit /b 1
)
rem So is the detector the driver carries. (Nuggets are made by the game, as they are found.)
if not exist target\outback\detector.glb (
  "%BLENDER%" -b --python tools\blender\detector\build.py -- target\outback\detector.glb || exit /b 1
)
rem And the town's buildings, which are built of the shed's pieces.
if not exist target\outback\town.glb (
  "%BLENDER%" -b --python tools\blender\town\build.py -- target\outback\town.glb || exit /b 1
)
cargo build --release -p bake -p viewer || exit /b 1
if not exist assets mkdir assets
rem What every map shares is in a file of its own beside the maps, which the game reads with
rem whichever map is played (scenes\shared.txt): the truck and what its driver carries, and
rem what every square of bush is made of. It is baked first: a map baked beside it leaves out
rem of its own file what this one has.
target\release\bake.exe --scene scenes\shared.txt --output assets\shared.solstice || exit /b 1
target\release\bake.exe --scene scenes\outback.txt --output assets\outback.solstice || exit /b 1
target\release\bake.exe --scene scenes\hub.txt --output assets\hub.solstice || exit /b 1
target\release\bake.exe --scene scenes\granite.txt --output assets\granite.solstice || exit /b 1
target\release\bake.exe --scene scenes\blackwood.txt --output assets\blackwood.solstice || exit /b 1
target\release\bake.exe --scene scenes\town.txt --output assets\town.solstice || exit /b 1
call postcards.bat
