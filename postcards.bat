@echo off
rem Takes a picture of each map, with the truck in it, for its loading screen and its
rem trailhead's card: ui\art\NAME.jpg. Run after bake.bat. Needs ffmpeg on the path.
setlocal
cd /d "%~dp0"
if not exist ui\art mkdir ui\art
call :card crossroads assets\hub.solstice 16:40 fair 230 howitt
call :card wombat assets\outback.solstice 17:10 fair 230 jamieson_troop
call :card granite assets\granite.solstice 9:20 clear 620 bogong
call :card blackwood assets\blackwood.solstice 15:30 overcast 210 ovens_tourer
call :card town assets\town.solstice 16:10 fair 250 licola
call :card arena assets\arena.solstice 16:40 fair 160 kestrel
exit /b 0

:card
rem NAME SCENE TIME WEATHER, how far round the truck the view is taken in pixels of mouse, and which truck.
if not exist %2 (
  echo %2 has not been baked: no picture of %1.
  exit /b 0
)
target\release\viewer.exe %2 --play --capture target\outback\card_%1.png --truck %6 --kit showroom --frames 160 --size 1920x1080 --time %3 --weather %4 --mouse %5@20 || exit /b 1
ffmpeg -y -loglevel error -i target\outback\card_%1.png -q:v 3 ui\art\%1.jpg || exit /b 1
echo ui\art\%1.jpg
exit /b 0
