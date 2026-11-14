@echo off
rem Drive the Wombat Creek track. Anything after the name is passed on to the viewer, for
rem example:  play --weather storm --weathered 900   (start in a downpour, ground already soaked)
cd /d "%~dp0"
if not exist assets\outback.solstice (
  echo The scene has not been baked yet: run bake.bat first.
  exit /b 1
)
target\release\viewer.exe assets\outback.solstice --play --size 1920x1080 --time 10:00 --weather fair %*
