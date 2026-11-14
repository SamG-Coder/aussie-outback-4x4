@echo off
rem Play the game in Edge, from this folder. Build it for the browser first: bash scripts/build-game-web.sh
rem This window serves the game's files: close it when you have finished playing.
cd /d "%~dp0"
if not exist dist\play\solstice_viewer_bg.wasm (
  echo The browser build is missing: run  bash scripts/build-game-web.sh  first.
  exit /b 1
)
start "" msedge "http://localhost:8770/play/"
python scripts\serve.py --port 8770 --directory dist
