# Aussie Outback 4x4

A four-wheel-drive game set in the Australian bush, built on the
[Solstice](https://github.com/SamG-Coder/solstice) engine (Rust and WebGPU; its own README is
[SOLSTICE.md](SOLSTICE.md)).

The first map is **Wombat Creek**: a square kilometre of Victorian foothill forest with a 3.5 km
track looping through it, driven in a Ford F-150 Raptor R. Everything in the world is generated
by the bake from one line of the scene file (`scenes/outback.txt`: `outback 1000`): the land,
the creek, the track and its damage, the ground, and every plant.

## Run it

| | |
|---|---|
| `bake.bat` | Build the programs and the scene (about 2½ minutes for the scene) |
| `play.bat` | Drive, in fair weather |
| `play-wet.bat` | Drive after 15 minutes of storm: greasy clay, deep mud, full puddles |

Extra viewer options go after the script name, for example `play --time 17:30 --weather broken`.

**The truck's model is not in this repository.** It is the
[2024 Ford F-150 Raptor R](https://sketchfab.com/3d-models/2024-ford-f-150-raptor-r-91aedf1119724f3396dcfe87686545c1)
by Ddiaz Design, CC BY-NC-SA 4.0. Download its GLB zip, unzip it into
`target/outback/raptor_src/`, and `bake.bat` converts it with Blender
(`tools/blender/raptor/prepare.py`; set `BLENDER` if Blender is not at `D:\Blender\blender.exe`).

## Controls

| Action | Keyboard / mouse | Controller |
|---|---|---|
| Throttle / brake (reverse when stopped) | W / S, or ↑ / ↓ | R2 / L2 |
| Steer | A / D, or ← / → | Left stick |
| Handbrake | Space | Circle (B) |
| Transfer case: 2H → 4H → 4L | T | Square (X) |
| Diff locks: rear → front + rear → off | G | Cross (A) |
| Tyres: air down for mud and sand / pump up | Y | D-pad up |
| Put the truck back on the track | R | Triangle (Y) |
| Swing the camera round and over the truck | Mouse (click the window first) | |
| Glance round / over | J L / I K | Right stick |
| Camera view | C | Create (Back) |
| Look behind | B | R3 |
| Gears (the box is automatic by default) | Q / E | L1 / R1 |
| Leave the truck and fly the camera | V | |

The window's title bar is the dash: speed, gear, revs, 2H/4H/4L, which diffs are locked,
hill-descent control, tyre state, pitch and roll, and lap time.

PlayStation controllers (DualSense, DualShock 4) are read directly over USB or Bluetooth;
Xbox-style pads through the system's gamepad API.

## The 4x4 bits

- **Part-time 4WD with low range and lockers.** 2H drives the rear axle only. 4H locks the
  front and rear propshafts together. 4L adds a 2.64:1 reduction. Both differentials are open
  until locked, so a wheel in the air or in a bog takes the drive, which is what the lockers
  fix. Traction control brakes a wheel that runs away from the one across its axle.
- **Long-travel suspension**: 33 cm of wheel travel at the front, 36 cm at the rear, soft
  springs, firm dampers, almost no rear anti-roll bar so the axles can cross up.
- **Ground a tyre sinks into.** Clay, mud, bog, leaf litter, rock, creek bed and sand each have
  their own grip, drag, slip and sinkage. Wheels sink further the harder they are loaded and
  the more they spin, and the rut drags the truck back. A bog stops 2H; 4H gets through.
- **Rain changes it.** The rain the renderer draws wets the ground the tyres feel: clay loses
  40% of its grip, mud softens, puddles fill the ruts and bog holes.
- **Hill-descent control** in 4L with your feet off the pedals.
- **Airing down** trades hard-surface grip and rolling resistance for grip and flotation on
  soft ground.
- **Scrub gives way.** Tea-tree, bracken, grass trees and whippy saplings are flattened by the
  truck and stay down; trunks, boulders and fallen logs stop it.
- **Spray**: mud clods off spinning wheels, dust off dry clay, sheets of water in the fords.

## The track

Found by a path search across the generated hills between two fords, both sides' hilltops and
the wettest flat, then worn into the land and broken by water:

- twin wheel ruts with litter and grass between, tilted with the hillside, never graded flat;
- erosion rills that snake down the climbs from one wheel line to the other, baring stones;
- washouts where gullies cross and have cut through the track;
- runs of wheel-dug holes, left then right, that cross the axles;
- rock ledges on the steep pinches, drainage humps, bog holes in the wet flats;
- fallen logs across the track, and two creek fords through real waterholes, ramped in and out.

## The bush

Planted by how damp and how open each spot is:

| Where | What grows |
|---|---|
| Creek and gullies | Manna gum, silver wattle (in flower), soft tree fern |
| Slopes | Messmate stringybark, candlebark, gum saplings, prickly tea-tree, austral bracken |
| Dry ridges | Red ironbark, grass trees, kangaroo grass |
| Clearings | Tussock grass |
| Everywhere | Fallen timber, granite boulders, eucalypt leaf and bark litter |

## Where things are

| Path | What |
|---|---|
| `scenes/outback.txt`, `scenes/outback/` | The scene, the truck's actor and its `.vehicle` numbers |
| `crates/solstice-bake/src/outback/` | Land and track design, ground mesher, planting; `species/` and `grass/` are editable text |
| `crates/solstice-procgen/src/bush.rs` | Red clay, mud, gum litter and tussock ground textures |
| `crates/solstice-vehicle/` | 4WD drivetrain, surfaces, sinkage (`tests/raptor.rs` holds the truck to its figures) |
| `crates/solstice-game/src/ground.rs` | The map of what each square metre of ground is made of |
| `apps/viewer/src/play.rs`, `sony.rs` | Spray, flattened scrub, PlayStation controllers |
| `tools/blender/raptor/prepare.py` | Turns the downloaded Raptor GLB into the engine's form |

## Looking at a change without driving

```bash
SOLSTICE_TRACE=1 target/release/viewer.exe assets/outback.solstice --play --press autopilot@20 --capture target/outback/shots/lap.png --frames 21600
```

drives the loop by itself for six minutes and logs where the truck is every half second.

```bash
OUTBACK_VIEW=-155,-85,50 cargo test --release -p solstice-bake --lib outback::tests -- --ignored --nocapture
```

writes a map of the whole design to `target/outback/plan.ppm`, and a contour close-up of the
100 m round a point to `close.ppm`. `OUTBACK_NEAR=x,z` on a bake lists what stops a car there.

## Not done yet

- A dash drawn in the window rather than in its title bar.
- Sound.
- A lap driven clean end to end by the self-driver: the track still has places harsher than
  intended.
- More maps, more trucks, and a game to tie them together.
