#!/usr/bin/env bash
# Builds the game's site into dist/: its home page, and under play/ the game for a browser
# (the viewer that plays, the game's pages, and its maps' list). Nothing of the plain viewer,
# the editor or the bake that scripts/build-web.sh also builds. Bake the maps first (bake.bat).
#
#   bash scripts/build-game-web.sh            to play here: the maps are linked into dist/
#   bash scripts/build-game-web.sh --hosted   to put on the web: the maps are in R2 (site/)
#
# To play here: play-web.bat. To put it on the web: see docs/hosting.md.
set -euo pipefail
cd "$(dirname "$0")/.."
hosted=""
[ "${1:-}" = "--hosted" ] && hosted=1

# The wasm-bindgen tool must match the wasm-bindgen library exactly: see build-web.sh.
version=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p')
tools=target/tools
bindgen="$tools/bin/wasm-bindgen"
if ! "$bindgen" --version 2>/dev/null | grep -q "$version"; then
  echo "installing wasm-bindgen-cli $version..."
  cargo install wasm-bindgen-cli --version "$version" --root "$tools" --locked
fi

rm -rf dist
mkdir -p dist/play
RUSTFLAGS="-C target-feature=+simd128" \
  cargo build --release --target wasm32-unknown-unknown -p viewer --lib --target-dir target/web-play
"$bindgen" --target web --no-typescript --out-dir dist/play/play \
  target/web-play/wasm32-unknown-unknown/release/solstice_viewer.wasm

# The game: the viewer's page, told in the page itself which game it is, so that its address
# needs nothing after it; and, on the web, where its maps are.
cp apps/viewer/web/loader.js apps/viewer/web/auto-quality.js apps/viewer/web/game.js apps/viewer/web/sound.js scenes/outback.world dist/play/
cp -r ui dist/play/ui
said='<title>Aussie Outback 4x4</title>\n<meta name="solstice-world" content="outback.world">'
[ -n "$hosted" ] && said="$said"'\n<meta name="solstice-scenes" content="/scenes/">'
sed "s|<title>Solstice</title>|$said|" apps/viewer/web/index.html > dist/play/game.html
grep -q 'solstice-world' dist/play/game.html || { echo "the viewer's page has no <title>Solstice</title> to replace"; exit 1; }
# And the page it is played in, which stays while that one is loaded again for each map.
cp apps/viewer/web/shell.html dist/play/index.html

# The sounds, squeezed for the web: 33 MB of them as they are made come to about 6. The engine's
# loops are kept exactly as they are (FLAC), as a steady note shows the least flaw where a loop
# comes round; the rest are Opus, whose flaws the bush and the tyres hide. sounds.json says
# which file each is. Needs ffmpeg.
mkdir -p dist/play/sounds
list=""
for sound in assets/sounds/*.wav; do
  [ -e "$sound" ] || continue
  name=$(basename "$sound" .wav)
  # A file whose name starts with `_` is there to be listened to, and one with a dot is a take.
  case "$name" in _*|*.*) continue ;; esac
  case "$name" in
    engine_*) file="$name.flac"; ffmpeg -y -loglevel error -i "$sound" -c:a flac -compression_level 8 "dist/play/sounds/$file" ;;
    *) file="$name.ogg"; ffmpeg -y -loglevel error -i "$sound" -c:a libopus -b:a 72k -application audio "dist/play/sounds/$file" ;;
  esac
  list="$list\"$name\": \"$file\", "
done
printf '{ %s }\n' "${list%, }" > dist/play/sounds/sounds.json

# The home page, with how big the download is, if it has been packed (scripts/package-release.sh).
cp site/index.html site/_headers dist/
pack=target/release-pack/AussieOutback4x4-windows.zip
if [ -e "$pack" ]; then
  size=$(python -c "import os, sys; print(f'{os.path.getsize(sys.argv[1]) / 1e6:.0f} MB')" "$pack")
  sed -i "s|<span data-size>About 40 MB</span>|<span data-size>$size</span>|g" dist/index.html
fi

if [ -z "$hosted" ]; then
  # Hard links keep the maps from taking disk space twice.
  mkdir -p dist/play/assets
  for scene in assets/hub.solstice assets/outback.solstice assets/granite.solstice assets/blackwood.solstice; do
    [ -e "$scene" ] || continue
    ln -f "$scene" "dist/play/assets/$(basename "$scene")" 2>/dev/null || cp "$scene" dist/play/assets/
    # And the list of its pieces, if it has been cut (tools/hosting/chunks.py).
    [ -e "$scene.chunks" ] && cp "$scene.chunks" dist/play/assets/
  done
  echo "built dist/. To play: play-web.bat"
else
  echo "built dist/ for the web. To put it there: cd site && wrangler pages deploy"
fi
