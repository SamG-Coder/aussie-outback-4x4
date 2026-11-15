@echo off
rem The crash-test pad: a flat pad with a row of things to drive a truck into, each a test of
rem the game's damage (docs\damage.md). The truck is one of the game's own, built of pieces.
rem What each blow did is in the window's title for a few seconds after it, and in
rem save\crash.log. Stop in the green box behind the start line to mend the truck.
rem Anything after the name is passed on to the viewer, for example:
rem   crash --kit bullbar=tube              (the same truck with a bull bar on it)
rem   crash --truck kestrel                 (another of the game's trucks: assets\trucks)
cd /d "%~dp0"
if "%BLENDER%"=="" set BLENDER=D:\Blender\blender.exe
if not exist target\outback\crash.glb (
  if not exist target\outback mkdir target\outback
  "%BLENDER%" -b --python tools\blender\crash\build.py -- target\outback\crash.glb || exit /b 1
)
if not exist assets\damage.solstice (
  target\release\bake.exe --scene scenes\damage.txt --output assets\damage.solstice || exit /b 1
)
if not exist save mkdir save
target\release\viewer.exe assets\damage.solstice --play --size 1920x1080 --time 10:00 --weather fair %* 2> save\crash.log
