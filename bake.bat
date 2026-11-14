@echo off
rem Builds everything: the truck's model from the downloaded zip, the programs, and the scene.
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
cargo build --release -p bake -p viewer || exit /b 1
if not exist assets mkdir assets
target\release\bake.exe --scene scenes\outback.txt --output assets\outback.solstice
