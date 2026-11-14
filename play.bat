@echo off
rem Play the game: it starts at its title, in the hub, The Crossroads, with the tracks a drive
rem away. Anything after the name is passed on to the viewer, for example:
rem   play --weather storm --weathered 900      (start in a downpour, ground already soaked)
rem   play assets\outback.solstice              (start on Wombat Creek Track itself)
rem What the game says as it runs is kept in save\game.log, to look at if something goes wrong.
cd /d "%~dp0"
if not exist assets\hub.solstice (
  echo The scenes have not been baked yet: run bake.bat first.
  exit /b 1
)
if not exist save mkdir save
target\release\viewer.exe --world scenes\outback.world --ui ui --size 1920x1080 --time 10:00 --weather fair %* 2> save\game.log
