#!/usr/bin/env bash
# Puts the site's home page into dist/, with what it says of the downloads filled in:
#
#   bash scripts/build-home.sh
#
# For each download that has been packed (the Windows zip, scripts/package-release.py; the
# Android app, scripts/package-android.py) the page is told how big it is, and its link is
# made to name the build: a copy of an older one, kept somewhere between the site and whoever
# asks, is then not what is asked for. scripts/build-game-web.sh runs this as its last step;
# run by itself it changes the home page and leaves the browser version in dist/ as it was.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
cp site/index.html site/_headers dist/
# Its pictures, which are of the game and are kept beside it (site/art).
rm -rf dist/art
cp -r site/art dist/art

# NAME-ON-THE-SITE  FILE  WHAT-MARKS-ITS-SIZE-IN-THE-PAGE
fill() {
  local name=$1 pack=$2 mark=$3
  [ -e "$pack" ] || return 0
  local size build
  size=$(python -c "import os, sys; print(f'{os.path.getsize(sys.argv[1]) / 1e6:.0f} MB')" "$pack")
  build=$(python -c "import hashlib, sys; print(hashlib.sha256(open(sys.argv[1], 'rb').read()).hexdigest()[:10])" "$pack")
  sed -i "s|<span $mark>[^<]*</span>|<span $mark>$size</span>|g" dist/index.html
  sed -i "s|download/$name\"|download/$name?v=$build\"|g" dist/index.html
  echo "home page: $name is $size, build $build"
}
fill AussieOutback4x4-windows.zip target/release-pack/AussieOutback4x4-windows.zip data-size
fill AussieOutback4x4.apk target/android-pack/AussieOutback4x4.apk data-apk-size
