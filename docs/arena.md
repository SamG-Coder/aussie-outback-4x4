# Bulldust Bowl: the monster-truck showground

A map of the game (`arena` in `scenes/outback.world`, `assets/arena.solstice`): a showground
scraped out of red country, inside an earth bank, for throwing a big truck around. No road
leads to it. A free drive can start on it (`docs/ui.md`, "Free drive").

```text
                        north
    .-------------------------------------------.
    |   <-- way off     THE LOOP    <-- run-up  |    an earth bank all round,
    |                                           |    with a gate in the south
    |  hop  tabletop   double   step-up  -->    |
    |                                           |
    |  -->   BIG AIR (over wrecks)    wrecks    |
    |                                           |
    |    the wallow               mud run       |
    |                                           |
    '--------------------.  .-------------------'
                         gate  (the truck starts here, looking north)
```

## How it is made

| What | Where |
| --- | --- |
| The scene | `scenes/arena.txt`: one line, `arena`, and the truck and its driver |
| The design: land, jumps, pits, bank, country, planting, where the props stand, its map | `crates/solstice-bake/src/outback/arena/mod.rs` |
| The loop | `crates/solstice-bake/src/outback/arena/ring.rs` |
| The set dressing | `tools/blender/arena/build.py`, which writes `target/outback/arena.glb` |
| A truck driven round the loop and over each jump, with no window | `crates/solstice-bake/tests/arena.rs` |

```
blender -b --python tools/blender/arena/build.py -- target/outback/arena.glb
bake --scene scenes/arena.txt --output assets/arena.solstice     (about a minute, 50 MB)
viewer assets/arena.solstice --play --truck kestrel
```

`bake.bat` does the first two with everything else, and `postcards.bat` takes its loading
picture, `ui/art/arena.jpg`. The bake draws its map, `ui/maps/bulldust-bowl.png`. It is baked
beside `assets/shared.solstice`, which has the truck, the plants and four of its five ground
textures.

It is a design, as the mud proving ground is (`docs/mud.md`), not country the generator
found: `Design::spot(x, z)` says what the ground is at a place, and from it come the land
that is drawn (a quarter of a metre to the point inside the bank, a metre outside it, eight
on the skyline), the grid of heights that is driven on (half a metre), and the map of what a
tyre is on. So what is seen and what is driven on agree.

- **The floor** is hard red clay (`Surface::Dirt`), 248 m by 184 m, level, with loose dust
  in drifts. **The bank** is 3.4 m high, with a gate 16 m wide in its south side. It can be
  driven over: outside is open country, 528 m square, with a wall at its edge.
- **The jumps** (`JUMPS`) are earth: each is a line of heights along the way it is driven,
  rounded at the foot of each ramp and sharp at each lip, with sides that fall away at 1 in
  1.25. To change one, change its numbers. The hop is a metre high; the tabletop 2.4 m with
  a 10 m top; the double two humps of 2.2 m, 12 m crest to crest; the step-up a 2 m kicker
  on to a bench 2.8 m high; big air a 3 m ramp, a 14 m gap with six wrecks in it, and a
  25 m landing.
- **The wallow** is a bog (`Surface::Bog`) 54 m by 20 m in a bowl 0.6 m deep, with water
  standing in the middle of it. **The mud run** is 62 m of wet mud (`Surface::Mud`) with
  rollers 0.46 m high every 6.5 m. Both are the deforming mud of `docs/mud.md`: they rut.
- **The ring road** inside the bank is the map's track: what the game's own driver laps,
  and where a truck is put back.
- **Outside**: a red plain, a dry sandy creek down the east side, a dune along the west, a
  stony rise in the north-west, a station track south from the gate with a windmill and a
  tank beside it, mesas on the skyline, and scrub a long way apart: wattle, ironbark,
  tea-tree, grass trees, river gums along the creek, and tussock as spinifex. They are the
  bush's own stock (`flora::Stock::stand`), so they cost the map nothing.
- **The set dressing** is fourteen pieces in one model, stood by name (`Design::props`):
  two stands, eight lamp towers on the bank (each a light, for the night), the gate's arch,
  the announcer's box, containers, drums, stacks of tyres at each jump, half-buried tyres
  along the pits and the loop's run-up, the wrecks, the loop's board, the windmill and the
  tank. What of each stops a truck is boxes (`solid`), the same sizes as the script's.

## The loop

A steel deck 70 m long and 19 m high, bent up, over and back down.

- **Its shape** is a teardrop, not a circle (`ring::profile`): 18 m in radius where it
  leaves the ground, tightening to 7.5 m at the top. A circle that a truck 3.6 m high fits
  inside would press it into the deck seven times its weight at the bottom at the speed
  it needs for the top; this halves that.
- **It is skewed.** A loop comes back to where it started, so the way off has to lie beside
  the way on: the deck slides 15 m to the right on its way round (12 degrees), like a
  corkscrew. A truck drives on to it straight and comes off it straight, 11 m to the right
  of where it went on.
- **It is a trough, wide enough for the monster truck.** The flat of the deck is 6.5 m
  wide (a truck 3.9 m wide has 1.3 m either side of it), with a shoulder curving up 0.67 m
  on each side and a rail on top of that: 9.1 m between the rails. (`FLAT` and `SHIFT` in
  `ring.rs`: a wider deck needs more slide, which a test checks.) Going round a skewed loop a truck pitches and rolls at once and ends up pointing a
  few degrees off its line; a wheel that wanders on to a shoulder is pushed back by it,
  which steers the truck back to the middle. Nothing touches its body.
- **How it collides.** Nothing was added to the engine for it. The deck and its rails
  collide as the triangles they are drawn with (`Collide::Auto`), and a truck's wheels
  already look for the ground along the truck's own down, whichever way up it is
  (`Vehicle::substep`), with the truck's own gravity and springs. The frame is looked at,
  and stops a truck as boxes. Under the loop the ground is `Surface::Concrete`: that is
  what a tyre on the deck is on, and it keeps no rut.
- **What speed.** The board says 70 km/h. In the test a Kestrel gets round from a 45 m run
  (72 km/h on to the deck, 64 km/h over the top) and from 100 m (96 km/h), and a Howitt,
  with half its power, from 100 m (68 km/h on, 36 km/h over the top). What a monster truck
  stands on (`scenes/trucks/monster.vehicle`, as it was on 16 November 2026) gets round
  from 60 m and from 100 m, at 97 and 115 km/h on to the deck, and wanders at most 2 m from
  the middle of a flat that reaches 3.25 m. The run-up is 100 m.
  Foot down all the way round, wheel straight. Too slow, and it climbs part of the way and
  comes back down.
- **What does not fit, does not fit.** A truck longer or taller than the loop was sized
  for drags or does not go round. No truck's collision is changed for it.

## Checked, and not

`cargo test --release -p solstice-bake` drives it without a window: the loop, by two trucks
from three run-ups, wheel straight; a truck too slow for it; and each of the five jumps,
flown and landed on its wheels. The design's own tests check the land's seams, the jumps'
heights, the pits' surfaces and that nothing stands on the ring road.

Not checked: it has not been played. How the jumps feel, whether the landings are where a
player's speeds put them, the mud, the night under the lamps, and a monster truck on anything but
the loop are for whoever drives it. Some letters of the
gate's sign drop out when it is seen from far off.
