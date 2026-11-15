#!/usr/bin/env bash
# Plays the game by itself, to see that it works: a trial at the Crossroads, then one in the
# town with what the first kept. Nobody sees or hears it (`--trial`, docs/ui.md): its window is
# off the screen, it makes no sound, it reads no controller, and it keeps its own game in
# target/outback/trial, where its pictures and what it said are left to look at.
#
#   cargo build --release -p viewer --target-dir target/harness    (a copy of the game to try,
#   bash scripts/trial.sh                                           apart from one being played)
#
# At the Crossroads: out of the truck, gold found and dug, the nugget picked up (which slings
# the detector), put down and picked up again, carried to the tray; the detector taken in
# hand, put down, and picked up; a jump; back in the truck, and a drive with the nugget aboard.
#
# In the town: the truck put in the gold buyer's yard, the button held, and the lot sold.
set -u
cd "$(dirname "$0")/.."
out=target/outback/trial
game=target/harness/release/viewer.exe
rm -rf "$out"
mkdir -p "$out"

told() {
  grep "told at" "$1" | python -c "
import sys, json
for line in sys.stdin:
    at, told = line.split('told at ', 1)[1].split(': ', 1)
    t = json.loads(told)
    hud = t.get('hud') or {}
    near = t.get('near') or {}
    print(at.rjust(5), t.get('screen'), 'afoot' if t.get('afoot') else 'truck', '| holding', t.get('holding'), '| detector', t.get('detector'),
          '| use', t.get('use'), '| signal', None if t.get('signal') is None else round(t['signal'], 2), '| tray', t.get('tray'),
          '| km/h', hud.get('speed'), '| near', near.get('title'), near.get('inside'), '| money', t.get('money'), '| buyer', t.get('buyer'))
" | awk 'NR % 2 == 1'
}
said() {
  grep -E "screen:|menu:|gold:|driver:|trial:|WARN|ERROR|panicked" "$1" | grep -v "screen: drive" | cut -c12-19,27-200
}

echo "==== the Crossroads"
"$game" --world scenes/outback.world --ui ui --size 1280x720 --time 10:00 --weather fair --trial "$out" \
  --tap key:ArrowDown@5 --tap key:Enter@6 --tap key:Enter@7.5 \
  --tap key:KeyE@10-11 \
  --tap mouse:0,260@12.3 \
  --tap key:KeyW@13-14.5 \
  --tap key:KeyE@15.6 --tap shot:01_dug@16.8 \
  --tap key:KeyE@17.2 \
  --tap key:KeyG@18.6 --tap key:KeyE@19.6 \
  --tap mouse:-1571,0@20.4 \
  --tap key:KeyW@21-23.4 \
  --tap key:KeyE@24.2 \
  --tap key:KeyQ@26 \
  --tap key:KeyG@27 \
  --tap mouse:-785,-200@28 --tap shot:02_tray@29 \
  --tap key:KeyE@29.6 \
  --tap key:Space@31 --tap shot:03_jump@31.35 \
  --tap key:KeyE@33-34 \
  --tap shot:04_back_in@35.5 \
  --tap key:KeyW@36-41 --tap key:KeyS@41-44 \
  --tap shot:05_driven@44.5 \
  --tap key:Escape@45.5 \
  --tap quit@47 \
  2> "$out/hub.log"
echo "exit $?"
said "$out/hub.log"
told "$out/hub.log"
echo "---- what was kept"
cat "$out/profile.txt" 2>/dev/null

echo "==== the town"
"$game" --world scenes/outback.world --ui ui assets/town.solstice --size 1280x720 --time 10:00 --weather fair --trial "$out" \
  --tap key:ArrowDown@5 --tap key:Enter@6 --tap key:Enter@7.5 \
  --tap place:-14.75,65.4,6.81,-105.8@9 \
  --tap shot:06_at_the_buyers@11 \
  --tap key:KeyE@12-13 \
  --tap key:Enter@15 \
  --tap key:Escape@17 \
  --tap shot:07_sold@18 \
  --tap quit@19.5 \
  2> "$out/town.log"
echo "exit $?"
said "$out/town.log"
told "$out/town.log"
echo "---- what was kept"
cat "$out/profile.txt" 2>/dev/null
