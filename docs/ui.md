# The game round the driving, and its pages

A scene with a truck in it is something to drive. A game is more: a title, several maps and a
way from one to another, something to drive each for, a pause. This is how that is built, and
how what is seen of it is one set of web pages, on the desktop and in a browser.

## The parts

| Part | Where | What it does |
| --- | --- | --- |
| The world | `scenes/outback.world` | The game's maps: each one's baked scene, its name and what is said of it, and which is the hub. |
| The maps | `scenes/hub.txt`, `outback.txt`, `granite.txt`, `blackwood.txt` | Each a square kilometre built by one `outback` line, of its own kind of country. |
| The shell | `apps/viewer/src/shell.rs` | Which screen there is, what is paused, which map is being played and what for, and what the pages are told. Draws nothing, and is the same on every platform. |
| The pages | `ui/` | `index.html`, `ui.css`, `ui.js`: every screen. Shows what the shell says; says back what was done. |
| The overlay | `apps/viewer/src/overlay.rs` | On the desktop: a see-through web view over the game's window, showing the pages. |

`bake.bat` builds the programs, bakes the four maps and takes each one's picture
(`postcards.bat`). `play.bat` runs the game: `viewer --world scenes/outback.world --ui ui`.
What the game says as it runs is kept in `save/game.log`.

## Maps and their country

A world file names the maps:

```text
hub crossroads
map crossroads assets/hub.solstice     The Crossroads
map wombat     assets/outback.solstice Wombat Creek Track
about  wombat A washed-out fire trail ...
kind   wombat Fire trail
length wombat 3.5 km
```

Every map is a scene whose land, track and bush come from one line, `outback SIZE [KIND]`
(`crates/solstice-bake/src/outback/terrain.rs`, `Country`):

| Line | Map | What it is |
| --- | --- | --- |
| `outback 1000` | Wombat Creek Track | Foothill forest; a washed-out fire trail with fords, bog holes and rock steps. |
| `outback 1000 rock` | Granite Ridge | Higher, steeper and stonier; the track climbs harder and is mostly dry. |
| `outback 1000 mud` | Blackwood Gully | Lower and wetter; sixteen bog holes, deep ruts, more washouts. |
| `outback 1000 hub wombat granite blackwood town` | The Crossroads | The hub: a dirt main road two trucks wide, with a trailhead for each map named, in the order the road comes to them. |
| `outback 400 town crossroads` | Ironbark Flat | The town: a small map of gentler country. Its road is laid out, not found: one level street along the front of its buildings, the gold buyer's among them, and a lane back round behind, with a trailhead on it back to the Crossroads. |

A kind of country is a handful of numbers: where in the hills it is, how high they stand, how
much is bare rock, how wet it is, how steep a climb its track takes, how many bog holes it
may have and how deep its ruts are worn. Another map is another `Country`.

## Trailheads

A trailhead is a glowing ring on the hub's road in a clearing, and a line of the scene's
`outback.places` file, `place MAP X Y Z RADIUS`. The instruments point the way to the nearest.

In a ring, a banner says what it leads to. Holding the bottom button of a controller, Enter
or F for half a second, or clicking the banner, opens that map's card: its picture, what it
is like, and what to drive it in and for:

- **Class**: which of the five classes of truck the track is run in, each with its icon,
  and under them the class's bronze, silver and gold times and what each pays, or that it
  has been won. The times are worked out from the track when its map is entered, so a map
  never driven to shows none yet.
- **Time**: dawn, day, dusk or night. Night adds half to what a timed run pays.
- **Weather**: clear, fair, overcast, or a storm, which starts with the ground already soaked
  and adds half to what a timed run pays. The one picked to begin with is the day's own
  (the calendar's).
- **Free roam**, with no clock; **Practice**, against the clock lap after lap with nothing at
  stake; or a **Timed run**, the one lap that counts. A map with no `length` in the world
  file has no track to time, a town or the road, and is only driven to.

Timed runs, their classes, medals, purses, the other drivers and the watching of their runs
are set out in docs/campaign.md (Timed runs). The rules are in `apps/viewer/src/timed.rs`
and `crates/solstice-game/src/par.rs`; the watching is `apps/viewer/src/cinema.rs`.

Either reads the map's scene into the same window, behind a loading screen with the map's
picture. A map whose scene is not baked yet is still on the signs, and its card says so.

**Ways out.** Every map has a way home. A track has a gate beside where a truck starts on
it (`outback SIZE KIND +crossroads` in its scene): a ring, as a trailhead is. Going through
a trailhead arrives there; coming back to the Crossroads arrives at the trailhead that was
left.

The town is not a trailhead. A main road runs from the Crossroads' road, beside the shed,
to the edge of the map (`>town`, the last of the hub's maps), and whoever drives down it is
in the town's map, with nothing asked: on the town's own road, which comes in from the edge
of its map (`outback 700 town >crossroads`) and ends at the town's street. Driving back
down that road to the edge is driving back onto the Crossroads' road, facing the shed. Each
is a `place edge:MAP X Y Z RADIUS X Y Z YAW` of its map's places: where a truck is as good
as gone, and where one that comes in by it is put. A truck that has just come in is not
sent back until it has been clear of the way out.

Every road is a main road, the Crossroads' as the town's: smooth, level across, with no hump
across it, no ledge of rock in it and no log across it. The Crossroads' road still fords the
creek, twice: it goes round a valley.

The road to the town is a main road, to be driven fast: two trucks wide, its bends opened
out, level across, smooth, and with no creek to cross, no log across it, no hump to turn
the rain off and no ledge of rock. No road has a log across it; a track does.

**The game's own code is kept once.** Who drives and what the keys and a controller do (the
driver's actor file, its script, the input actions) and a truck's numbers are in the shared
file, `scenes/shared.txt`. A map has copies from when it was baked; the shared file's take
their place as the two are joined (`shared::share_code`). So what a controller does is
changed by baking the shared file alone, and is the same on every map.

**Gold** is in the ground of every map: two dozen places a map, each within thirty metres
of its road or track, and none where anything is built or stopped at. Nothing of it is in
any file. A map's ground is scattered when the map is first come into, from a number of the
game's own, and is kept (`scatter MAP DAY SEED DUG` in the kept game): what has been dug
stays dug, and the rest is where it was. Once any of it has been dug and more than three
days have gone by (`RESCATTER_DAYS`), it is scattered again the next time the map is come
into. Ground nobody has dug in is left as it is. The detector hears gold from sixty metres,
and says so when a map is dug out.

What is dug up is drawn from a table (`gold::FINDS`), by weight:

| Find | Weighs | In a thousand | Worth, at $2,400 a kilo |
|---|---|---|---|
| A speck | 10 to 30 g | 400 | $24 to $72 |
| A small nugget | 30 to 80 g | 300 | $72 to $190 |
| A fair nugget | 80 to 200 g | 170 | $190 to $480 |
| A good nugget | 200 to 500 g | 85 | $480 to $1,200 |
| A big nugget | 0.5 to 1.5 kg | 30 | $1,200 to $3,600 |
| A great nugget | 1.5 to 4 kg | 6 | $3,600 to $9,600 |
| The find of a lifetime | 4 to 10 kg | 1 | $9,600 to $24,000 |

Those are the odds in ordinary ground, where a find is worth $269 on average. How rich a
map's ground is is the world file's (`gold MAP RICH`): the richer, the more of the big ones
(`gold::chances`). As it stands: the town 0.35 ($115 a find), the Crossroads 0.5 ($147),
Wombat Creek 1 ($269), Blackwood Gully 1.2 ($323), Granite Ridge 1.4 ($380). The bank takes
$1,850 a week, so seven finds on Wombat Creek are a week's repayment.

**Running** is held on the keys (Shift). On a controller the left stick is pressed once to
run and again to walk, and standing still is walking again.

**Where to.** The way pointed out at the top of the screen is to the nearest trailhead,
unless somewhere was chosen on the map: a place in its list, or anywhere on it that was
clicked (`set waypoint X Z`). Chosen again it is let go; come to, it is let go.

**Free roam is free.** A truck is put back on the track when the driver asks. It is not put
back for being a long way down the hill from it.

**A timed run is driven through its gates.** A track is in sections of about four hundred
metres, a gate of two posts at the start of each and four at the line. They are crossed in
turn. Being put back on the track puts the truck at the start of the section it is in, and
adds ten seconds (`PUT_BACK`); it no longer costs the run its medal. The section and what
has been added so far are with the lap's time.

**Where roads join.** A made road's cuttings and fillings run out a long way into the land
(`ROAD_BATTER`), and by one the ground is all the nearby road's, each stretch by how much
of the place is its own (`Trail::bench`), not the nearest stretch's: so where a road leaves
the track, or bends back beside itself, the one runs into the other with no step. The road
that leaves is as high as the track for as long as it runs beside it, and the track's own
banks are a road's for sixty metres either side of the junction. `OUTBACK_VIEW=x,z,half` on
the plan's test says how steep the steepest step of ground in a view is.

A timed track is driven at the hour and in the weather picked for it. The Crossroads and
the town are at whatever hour the clock has got to, in the day's own weather as the
calendar forecast it, and a night's sleep brings the next day's.

One of the game's trucks runs a track in the class of its own grade: the card says which,
and there is nothing to pick. What is fitted to it changes how it drives (`kit::tuned`):
its tyres' tread on firm ground and on soft, its suspension's springs, dampers and travel,
how high it stands, and what an engine overdue for its oil still pulls.

The best lap of each map is kept in `save/profile.txt`, with what has been won in each class
of it (`run MAP CLASS MEDAL SECONDS`), each map's gold times as last worked out (`par MAP ...`),
the class last chosen (`class N`) and the game's own seed for its weather (`seed N`).

## Time

A game's clock runs as clocks do, a minute in a minute, from the hour it starts at: ten in the
morning in the hub, and whatever was chosen at a trailhead on a track. Driven on into the
evening it gets dark, and the lamps come on.

## Night

The renderer keeps night as dark as it is beside the day, which is too dark to drive in. A
game is shown it as eyes used to the dark see it: dim and blue, by the moon or the stars
(`night_eyes` in `apps/viewer/src/main.rs`). When the sun is well down the truck's high beams
come on: one spot beam from between its headlamps, narrow, fading with distance, without
shadows (`Lamp` in `solstice-render`, lit in `lighting/deferred.wgsl`). Its tail lamps glow a
dull red and brighten under braking; by day they light only under braking. The model has one
white lamp material front and rear, so `tools/blender/raptor/prepare.py` gives the rear lamps
a material of their own, and the game tells the renderer to glow them red.

## The screens

**The main menu** (`title`) has no game behind it: the Crossroads with no truck, looked
slowly round (`Shell::preview`, `PlayMode::preview_camera`), and the bush to hear. Its menu
is New game, Free drive, Load game, Options and Quit.

- **New game** (`newgame`) and **Load game** (`loadgame`) are three slots, a tile each: the
  truck that is driven in it, the day, what is in hand and what is owed. A new game in a
  slot that has one asks twice; so does deleting one. A slot is a file,
  `save/profile.txt` for the first and `save/profileN.txt` for the others (`slot_file` in
  `desktop.rs`); what is done with them is
  `Order::Saves` (`Use`, `New`, `Delete`, and `Close` when the main menu is gone back to,
  which keeps the game and stops playing it).
- **Free drive** is any of the game's trucks, chosen in the showroom by its grade, with
  nothing owed and nothing kept.
- **Options** (`controls`) is in tabs: Keyboard, Controller, Sound (how loud) and Display
  (a window, or the whole screen). They are the player's, not a game's, and are kept in
  `save/options.txt`.

**The showroom** (`yard`) is one page with a truck standing behind it: the first truck of a
story (`start`, and then its colour, `colour`), a free drive's (`free`), the garage
(`garage`), the auto shop (`shop`), the used trucks (`used`) and the bank (`bank`). Each
page is come to from its own place in the game and has one way out.

It is looked at where it is done. The first truck, a free drive's and the garage are at the
Crossroads, by the shed (the map `showroom`). What is done in the town is in the town
(`showroom_town`, the town's own scene): the truck stands in the yard before Motor Spares,
the hotel or the bank, facing the road, with the building behind it. Where is a
`place stand:WHAT X Y Z YAW` of the town's places (`Town::stands`), which
`shared::show_trucks` stands the trucks at and `PlayMode::stand` looks at.

Everything there is to choose is in the strip along the bottom, the one in hand in the
middle, and pressing on it chooses it: trucks, a first truck's colours, and in the auto
shop its sections (each group of parts, the respray, the oil), which open into what is in
them. A part or a colour in hand is put on the truck to be seen before it is bought
(`try:GROUP:CHOICE`, `tint:RRGGBB`), and taken off again when the section is left
(`untry`). The bank's counter is the same strip: what to borrow, and what to pay off.
Down the side is only what the truck is.

**Driving**: `drive`, `pause` (Resume, Map, Options, Quit to main menu; Restart on a run
against the clock; Back to the Crossroads only on a map with no gate home), `offer` (a
trailhead's or a gate's card), `shed`, `sleep`, `buyer` (the gold buyer's), `loading`, and
for a track driven against the clock `lineup`, `watch` and `result`. While a run is watched
the game drives the truck and the camera is the shot's (`Shell::view`); paused, offered a
map, looking at a result or loading, the game is not stepped at all.

The pages have two more of their own, opened over the game's: `map`, from the pause menu,
and `calendar`, from the shed (four weeks of days, each one's weather as far as it is
forecast, and what the bank takes on the Fridays).

Two layouts, and no others. A menu, somewhere there are a few things to do (the main menu,
the pause menu, the shed, the gold buyer's), is a rail down the left. A sheet, somewhere
there is something to read or to choose among (a trailhead's card, the kept games, the
options, the calendar, the map), is a card in the middle.

Everything is worked with the mouse, with the arrows and Enter and Escape, or with a
controller's d-pad or left stick and its bottom and right buttons, and nothing needs the
mouse. Escape, or Start, pauses.

**Where things are done.** What a game is made of is out in it, not in a menu. The garage
is at the shed. In the town the auto shop is behind the door of Motor Spares, the used
trucks are asked about at the hotel's, and the bank is the bank: each a `place door:WHAT`
of the town's places, walked up to on foot, where the button goes in; coming out, the
driver is on foot at the door. Gold is sold from the tray in the buyer's yard.

## The day: the shed, and gold

The game has a day to it. It starts at the shed at the Crossroads' camp, goes out by a
trailhead to a track, and comes back.

- **Gold in the ground** (`apps/viewer/src/gold.rs`). When a map is entered, gold is put in
  the ground of it: fourteen places beside its track, a little way off into the bush, and not
  the places it was in the last time. Nothing shows where. Nothing is baked: a map's file
  holds no gold.
- **The detector.** Out of the truck the driver has a gold detector
  (`scenes/outback/detector.actor`, its model made in Blender by
  `tools/blender/detector/build.py`, its origin its grip). In their hands, its signal is
  stronger the nearer it is to gold (`Gold::signal`): the pages show it as a bar, and it
  is heard, which is how gold is found. It sounds as a gold detector does. It does not
  beep: it hums, low and steady, its threshold (`tools/audio/detector.py` makes the hum),
  and gold is heard as the hum changing. Each time the coil passes the middle of its sweep
  the hum dips a little, rises in pitch and swells, and falls back, and by how much says
  how near the gold is (`Soundscape::frame` in `apps/viewer/src/play.rs`). It hears a long
  way, 45 m: out there it is a flicker in the hum; nearer, a clear rise with every pass,
  higher, louder and longer; and over the gold the hum stays high and loud, and wails as
  the coil goes over. So the driver walks whichever way makes it rise. The coil is swept
  faster the faster they walk, and gently while they stand. Standing over gold, the button digs, and the
  nugget comes out of the ground onto it.
  - It is held as it is used (`place_carried` in `apps/viewer/src/play.rs`): its grip in
    the driver's right hand, its shaft tipped about the grip until the coil is just over
    whatever ground is under it, uphill or down, and swept from side to side as they walk.
  - It is a thing, as a nugget is (`Detector` in `gold.rs`). Hands hold one thing. **Q**
    (a controller's square) slings it on the driver's back, leaving their hands empty, and
    takes it in hand again; picking a nugget up slings it without being asked. **G** (the
    circle) puts down whatever is held: the detector then lies where it was put, on the
    ground or in the tray, a body like any other, kept with the game, until the button picks
    it up again.
- **A nugget is made when it is dug up**, by the game, and not before. No map and no file
  has a nugget in it: there is no model of one and no picture of its skin. It has a number
  of its own, and everything about it is worked out from that number when it is found, so
  that it is that nugget every time it is seen (`apps/viewer/src/nugget.rs`):
  - **What it weighs**: an ounce or two, often; half a kilogram, as often; now and then a
    great deal more. It is drawn bigger than gold is, to be seen, but small where it is
    light, and no two of a weight are of a size (`Nugget::size`).
  - **What it is made of** (`nugget::Kind`), as they come out of Victorian ground: gold and
    nothing else, most often, water-worn, with clay in its pits and creases; or gold with
    quartz still in it; or gold in dark ironstone; or, seldom, a piece of the reef itself,
    quartz with gold through it in veins. How much of it is rock, how wide the veins are
    and how much clay it holds are its own.
  - **Its colours** (`Lump::colours`): its gold anywhere from deep yellow to pale, or red
    with copper; its clay from red to brown to nearly black; its quartz white, smoky or
    stained with rust, its ironstone black to red-brown.
  - **What shape it is** (`gold::Form`, `Lump::of`). Its skin is some five thousand
    triangles, and each point of it is put where the nugget's own noise says: lobes, knobs
    with creases between them, and pits. Then the whole is flattened and drawn out, from a
    button to a finger of gold and from a leaf to all but a ball, and some are far along
    one way or another: long slugs, thin flakes, teardrops thicker at one end, peanuts
    pinched at the middle, beans, hooks and horseshoes bent round, saddles, and twisted
    things. It falls and lies as a box of about its own length, thickness and width.

  **What a scene has for them.** A scene's geometry is fixed when it is opened, so a map
  that is to be played is given forty blank lumps as it is read (`nugget::join_loaded`,
  `join_streamed`): balls built there and then by the builder a bake uses, with nothing
  coarser made from them (`BuildOptions::flat`), so that all of each is in pages that are
  always loaded, and with bounds as wide as any nugget. A gameplay file the game adds,
  `nugget.slots`, says which instance and materials each is and where in which pages its
  points are. When a nugget comes in sight it is made on a blank that shows nothing: its
  points are written over the blank's (`Renderer::rewrite_page`) and its three colours
  given to the blank's materials (`Renderer::set_material`), its gold, its clay and its
  rock. Those have no texture: each point holds how much of what is seen there is clay
  and how much rock, and the three are blended by those shares (`Material::BLENDS`), with
  an edge where two kinds meet. If there are ever more nuggets in sight than blanks, the
  nearest are shown.
- **A nugget is a thing in the world.** Nothing counts nuggets or keeps them in places of
  their own. One is in the driver's hand, or it is loose: a body that falls, rolls, lies
  where it comes to rest and is pushed by what meets it (`Play::add_thing` in
  `crates/solstice-game/src/play.rs`; a box, with rapier's continuous collision so that it
  does not pass through a floor). One button, the one that uses things, does what there is
  to do where the driver stands: dig, pick up the nearest nugget in reach, or let go of the
  one in the hand onto whatever is in front of them. Beside the truck's tray that is the
  tray; beside the chest, the chest; anywhere else, the ground. It is put down on what is
  under it, not dropped from a height. Got back into the truck with one in the hand, it goes
  in the tray.
- **The tray is the truck's own.** The truck's body is not one box but its cab and what is
  under and round its tray (`tray WIDTH DEPTH LENGTH at X Y Z` in a `.vehicle` file, measured
  from the model; `crates/solstice-physics/src/car.rs`), so what is put in the tray lies on
  its floor between its sides and its tailgate, rides there, slides when the truck brakes,
  and can be thrown out by a hard enough landing. A nugget thrown out lies where it fell,
  and the detector hears it as it hears gold in the ground.
- **Its weight is real.** Nothing is added to what the truck weighs: the nuggets lie on it,
  and what they weigh presses on its springs through where they touch it. A third of a
  tonne in the tray sets it lower (`crates/solstice-game/tests/things.rs`).
- **The chest stands open**, and is hollow to things as the tray is: a floor and four sides
  (`shed.rs`; `chest X Y Z YAW` in `outback.items`). What is let go over it lies in it, to be
  seen and taken out again.
- **Where each is, is kept** (`Lies` in `gold.rs`): loose on a map, where it lay and turned
  as it lay; or aboard the truck, where it lay in the truck's own space, so that it goes
  where the truck goes, from map to map. Each frame the world says where the bodies are
  (`Shell::see`), a nugget lying in the tray is aboard and one anywhere else is loose, and
  the world is asked for the bodies there should be (`Order::Thing`). A truck put somewhere
  all at once, where it was left or back on its track, takes what is aboard it along.
- **Selling.** Ironbark Flat has a gold buyer's (`place buyer X Y Z RADIUS` in the town's
  `.places`; `crates/solstice-bake/src/outback/town.rs`, its buildings made in Blender by
  `tools/blender/town/build.py`). Stopped in the yard before it, holding the button opens
  his card: what is lying in the tray is weighed, and **Sell the lot** pays for it by the
  kilogram (`PRICE` in `shell.rs`) and takes it. What bounced out on the way is not there to
  sell. The money is kept with the game; nothing is bought with it yet.
- **The shed** (`crates/solstice-bake/src/outback/shed.rs`; its model is made in Blender by
  `tools/blender/shed/build.py`). Its bay is a place like a trailhead's ring, `place shed X Y Z
  RADIUS` in the hub's `.places`. Stopped in it, holding the button opens the shed's card:
  **Sleep till morning** ends the day (the clock goes to half past six the next morning and
  the day's number goes up), and **Get out** puts the driver on foot there, to carry the gold
  from the tray to the chest by the room's wall.
- **What is kept** (`save/profile.txt`, or the browser's own store): the best laps, the day,
  the money, every nugget with its weight, its shape and where it is (`nugget 3 2.41 9120331
  aboard 0.210 0.985 1.730 0.0000 0.3827 0.0000 0.9239`, `nugget 4 0.62 771 loose wombat
  -151.200 70.180 -122.750 0.0000 0.7071 0.0000 0.7071`, `nugget 5 1.10 42 hand`; a game kept
  before nuggets were things said `tray N`, `chest` or `ground MAP X Y Z YAW`, and still
  reads), the map the game was left on,
  and where the truck was last on every map and the way it faced (`map wombat`, `at wombat
  -158.09 70.73 -160.11 177.1`). It is kept whenever the driving stops for a menu or a map,
  every five seconds while it goes on, whenever gold is moved, and when the window is closed.
  Roaming, a map come back to is come back to where the truck was left on it; a game started
  again goes on from the track it was left on. Timed laps always start at the line.

While there is gold in the tray, the marker at the top of the driving screen points the way
to the shed and to the gold buyer's as well as to the trailheads.

### On foot

In free roam, with the truck at rest, holding **E** (or a controller's bottom button, the
one that enters a trailhead's ring) gets the driver out: they are an eye on legs
(`scenes/outback/driver.actor`), walking with the keys that drive and looking with the mouse
or the right stick, and the truck stands on its handbrake with its engine switched off: it
is heard to run down as they get out, and to start as they get back in. Beside the truck,
holding the same button gets back in. In a ring or the shed's bay that button stops there
instead, so the shed's card has **Get out** on it; on foot no place is entered, wherever the
truck was left.

They step up anything 60 cm high or less (`MAX_STEP` in `solstice-physics`) and jump about
a metre with **Space** or a controller's triangle: a walk's `y` is the speed to leave the
ground with (`Walk` in `solstice-world`), which the driver's script sets as the button is
pressed.

A tap of the same button uses what is near. The door of the shed's room is an actor of its
own (`scenes/outback/shed_door.actor`, its model made with the shed's): used, it swings open
into the room, and used again, shut, and it stops somebody while it is shut.

The engine plays one actor at a time, and here there are two that can be played, the truck
and its driver (`walker` in `crates/solstice-game/src/play.rs`). The one not being played is
marked `Idle`: a script hears nothing, and a car is asked for nothing but its handbrake. What
the keys do is in `scenes/outback/game.input`.

While there is something in the truck, the marker at the top of the driving screen points
the way to the shed as well as to the trailheads.

## The map

Every map has a map: a picture drawn from its design when it is baked
(`crates/solstice-bake/src/outback/chart.rs`), `ui/maps/NAME.png`, named for the country.
It is drawn as a map is, not as the ground looks from above: paper tinted by forest and
grass, contour lines every five metres with a little shading from the north-west, water in
blue, the road cased and filled or a track as a broken red line, buildings in black, a ring
at each trailhead, a hundred-metre bar and an arrow to the north. The scene's `outback.chart`
file says which picture it is and how much ground it covers (`chart FILE HALF`).

The pause menu opens it whole, with the truck's arrow where the truck is and a label at each
place. While driving, and on foot, it is in the corner at the lower left, small: north up,
about 260 m across, the arrow in its middle turning as the truck does.

## Trying it without playing it

The game can play itself, to see that a thing works, where nobody sees or hears it:

```bash
cargo build --release -p viewer --target-dir target/harness   # a copy apart from one being played
bash scripts/trial.sh                                         # the Crossroads, then the town
```

`--trial FOLDER` is what makes a run a trial (`apps/viewer/src/desktop.rs`). Its window is off
the screen and is never the one typed into; it makes no sound and reads no controller; what
it keeps (`profile.txt`) and the pictures it takes go in FOLDER, so a real game's are not
touched; and once a second it says in its log what the pages were told (`told at SECONDS:
{...}`), beside what the game says anyway (`screen:`, `menu:`, `gold:`, `driver:`). The first
time the driver gets out, gold is buried three paces ahead of them, since a script cannot go
looking for it.

What it does is said with `--tap WHAT@SECONDS`, or `WHAT@FROM-TO` to hold something:

| What | Does |
| --- | --- |
| `key:KeyE`, `key:Enter` | A key, by its `KeyboardEvent.code`. |
| `south`, `west`, ... | A controller's button, by its name. |
| `mouse:DX,DY` | The mouse moved that many pixels: right and down above zero. |
| `place:X,Y,Z,YAW` | The truck put there, facing that way in degrees. |
| `shot:NAME` | A picture of the next frame, `FOLDER/NAME.png`: the game, without its pages. |
| `quit` | The game closed, as its window's cross does, keeping what it has. |

A trial sees what is drawn and what the game says of itself. It does not hear the sounds,
see the pages over the picture, or feel how the truck drives.

## What passes between the shell and the pages

Text, both ways, so that the same pages work wherever the shell runs.

**To the pages**: JSON, handed to `ui.receive(state)`. All of it whenever the screen changes
or a menu is worked; while driving, about 24 times a second.

```json
{ "screen": "drive", "mode": "laps", "hub": false, "pad": false, "nav": [],
  "map":  { "name": "wombat", "title": "Wombat Creek Track", "about": "...", "kind": "Fire trail",
            "length": "3.5 km", "best": 312.48, "built": true },
  "offer": { ...a map, when its card is open... },
  "times": ["Dawn", "Day", "Dusk", "Night"], "time": 1,
  "weathers": ["Clear", "Fair", "Overcast", "Storm"], "weather": 1,
  "hud":  { "speed": 37, "gear": "2", "rpm": 3100, "limit": 6800, "abs": false, "traction": true,
            "four_wheel_drive": true, "drive": "4L", "front_locked": false, "rear_locked": true,
            "descent": false, "aired_down": true, "pitch": 12, "roll": -4 },
  "lap":  { "count": 1, "time": 83.4, "last": 318.07, "best": 312.48, "valid": true, "last_valid": true },
  "near": { "title": "Wombat Creek Track", "distance": 240, "bearing": 35, "inside": false, "hold": 0 } }
```

With these, always: `day`, the day of the game it is; `money`, what gold has been sold for,
in dollars; `tray` and `chest`, how many nuggets are lying in the truck's tray and in the
chest and what they weigh in kilograms; `afoot`, whether the
driver is out of the truck, and `by_truck`, whether they are near enough to it to get back in;
and on foot, `signal`, what the detector hears from 0 to 1, `hand`, the weight of the nugget
in the hand, and `use`, what the button would do: `dig`, `pick`, `tray`, `chest`, `untray`,
`unchest`, `drop`, or `take`, picking the detector up from where it was put down; `holding`,
what the driver's hands hold, `detector`, `nugget` or `nothing`, and `detector`, where the
detector is, in `hand`, `slung` or `left` somewhere. With a map that has one: `chart`, its
picture and half its width in metres (`file`, `half`); `places`, what is marked on it, each a
`title`, a `kind` and where (`x`, `z`); and `at`, where the truck or the driver on foot is
(`x`, `z`, `heading` in degrees round from north, `afoot`). `screen` is also `shed`, the shed's card, `sleep`, the night going by,
and `buyer`, the gold buyer's card, which has `buyer` with it: `count` and `kilos`, what is in
the tray; `pays`, what it would fetch, and `rate`, the price of a kilogram; and `sold`, the
sale just made, if one was (`count`, `kilos`, `paid`). A map has `timed`, whether it has a
track to time: one that has not is offered without laps.

For a track driven against the clock there is more. `class` is the class chosen, from 0.
With a trailhead's card: `classes`, each class of the map (`name`, `medals` and `purses`,
bronze's first, or null where the times are not known yet, `held`, the best medal won,
`best`, and `rivals`, the three other drivers: `name`, `truck`, `paint`, `medal`, `time`);
`bonus`, what each time of day and each weather adds to a purse in hundredths; and `today`,
which weather is the day's own. On the track: `run`, the class being driven as one of
`classes` is told, with `night`, `rain`, `bonus`, `hour` and `weather`; while the clock runs,
`pace`: `gaps`, how far behind each medal's time the lap is here (ahead, below zero), `on`,
the best medal it is still inside, and `ghosts`, where each of the others would be by now
(`medal`, `x`, `z`, `done`). Watching: `watch` (`name`, `truck`, `paint`, `medal`, `class`,
`time`, `clock`, `shot`, `shots`). After a timed run: `result` (`time`, `valid`, `medal`,
`held`, `best`, `record`, `medals`, `firsts`, each a `medal` and its `purse`, `repeat`,
`night`, `rain`, `paid`, and `grams`, the gold put in the tray). Paused and in the shed:
`calendar` (`day`, `weekday`, `date`, `month`, `months`, `sky`, `days`, each a `day`, `date`,
`month`, `when` (`past`, `today`, `forecast`, `outlook` or `later`), `sky` and `due`, and
`next`, what is due next and in how many days).

`nav` is how a menu was moved from the keys or a controller since the pages were last told:
`up`, `down`, `left`, `right`, `accept`, `back`. `pad` says a controller was used last, so the
pages name its buttons. `near` is the nearest trailhead: how far, which way from straight
ahead in degrees, whether the truck is in its ring, and how far through holding the button.

**From the pages**: a line at a time.

| Line | Means |
| --- | --- |
| `hello` | The page has loaded: tell it everything. |
| `key CODE 1` / `key CODE 0` | A key went down or up, by its `KeyboardEvent.code`. |
| `mouse X Y` | The mouse moved, while the page had hold of it. |
| `blur` | The page is no longer typed into: nothing is held. |
| `do WHAT` | Chosen in a menu: `start`, `pause`, `resume`, `restart`, `hub`, `title`, `quit`, `enter`, `roam`, `practice`, `timed`, `leave`, in the shed `sleep` and `walk`, at the gold buyer's `sell` and `walk`, at the line `go`, `watch0` to `watch2` (bronze's driver to gold's) and `roam`, while watching `skip`, and after a run `again`, `roam` and `hub`. |
| `set time N` / `set weather N` / `set class N` | Which time of day, which weather and which class the next track is driven in. |
| `log TEXT` | Something for the game's log: a mistake in the page. |
| `grab` | On the desktop: pressed where there is nothing, while driving. Take hold of the mouse, to look round with. |
| `touch X Y` | The controls drawn on a screen that is touched: steering, and the pedals, each from -1 to 1 (right, and braking, above zero). |
| `look X Y` | On foot, a finger dragged across a touched screen by this much: looking round. |
| `touched` | The screen was touched while a controller was in use: draw the controls on it again. |
| `pad here` / `pad gone` | A controller was plugged in, or the last one taken out. |
| `pad button NAME V` / `pad axis NAME V` | A controller, where the game cannot read one itself and the page's host reads it for it (a phone, docs/android.md). |

Who has the keys and the mouse is not the same in the two places.

In a browser the page is over the canvas, and once clicked it is what they go to: so it
passes the keys and the mouse on, with `key` and `mouse`, and takes hold of the mouse itself
(pointer lock) when it is clicked where there is nothing, while driving.

On the desktop the web view is a window over the game's, and if it were let it would take a
click, and with the click the keyboard: the game's window would then no longer be the one in
use, and would hear no keys. So the view is made deaf (`deafen` in `overlay.rs`: the window is
disabled, which the system passes over when it looks for what a click is on). It is shown and
is given nothing. Every key and click is the game's window's, as with no pages at all, and the
game says to the page what the mouse did: `ui.pointer("move" | "down" | "up", x, y)`. The page
finds the button under it, takes it in hand as the mouse passes, and chooses it when the
mouse is pressed and let go on it. Pressed on nothing while driving, it says `grab`, and the
game takes hold of the mouse; Escape lets it go. A controller is read by the game itself.

On a screen that is touched and has no mouse, a phone or a tablet, in a browser or as the
Android app, the page draws the truck's controls on itself (`body.touch`, `#touch`): a pad
that steers, the pedals, and buttons for the rest. While a controller is the one in use
(`pad` in what the page is told) they are not drawn, and the screen is as on a desk. A screen
that is short, a phone on its side, has everything set a little larger for its height and
closer together (`@media (max-height: 560px)` in `ui.css`).

## Before there is a map: a game that was downloaded

The game somebody downloads is the program, the pages and the sounds, and no maps
(docs/hosting.md). Beside the program is a `game.txt`, which says what the program does when it
is opened with nothing asked of it (`apps/viewer/src/game.rs`): what it is run with, the sites
its maps are fetched from, and the file its log goes to. So opening the program is starting
the game; there is nothing else to open.

With maps to fetch, the window opens before there is a scene to draw in it (`Lobby` in
`main.rs`). The pages are shown their loading screen for the map the game starts in, and
`ui.stream` says along the top what is being downloaded and how much of it has come, as it
does in a browser. When the maps are here the first is read and the game begins in the same
window. If the map it starts in could not be had, `ui.stuck(why)` says so on that screen, which
waits to be closed; with that map here and others missing, the game is played with what there
is, and their signs say they cannot be driven yet.

## Working on the pages

They need no game. Opened in a browser on their own they show made-up readings:

```bash
python -m http.server 8765 --directory ui
```

then `http://localhost:8765/index.html?screen=drive&mode=timed&map=1`. `screen` is any screen's
name, or `controls`, `map` or `calendar`; `mode` is `roam`, `practice` or `timed`; `class=N`
chooses a class, from 0; `foul` is a night in a storm; with `screen=result`, `repeat` and
`void` show a run that won nothing new and one with no time; `unknown` shows a trailhead
whose times are not known yet; `pad` shows a controller's buttons; `inside` and `unbuilt`
show a ring's banner and a map not yet made; `still` stops the fades, for a picture.

On the desktop the pages are read from `ui/` as they are asked for, so a page changed on disk
is the changed page the next time the game is started.

## Trying the menus with no hand on them

The viewer can press things itself, a number of seconds after its window opens:

```bash
viewer --world scenes/outback.world --ui ui --tap gamepad:south@8 --tap key:Escape@12
viewer --world scenes/outback.world --ui ui --ui-run "document.querySelector('[data-do=start]').click()@8"
```

`--tap DEVICE:CONTROL@SECONDS` presses a key or a controller's button for a moment;
`--ui-run SCRIPT@SECONDS` runs a script in the pages. The shell logs each screen it shows and
each thing chosen, so a run's log says what happened.

## In a browser

```bash
bash scripts/build-game-web.sh     # the viewer that plays, the pages and the maps, into dist/
play-web.bat                       # serves dist/ and opens Edge
```

which is `http://localhost:8770/?world=outback.world` in Edge or Chrome. The viewer's own page
(`apps/viewer/web/index.html`) draws the scene; with `?world=` it loads
`apps/viewer/web/game.js`, which lays the game's pages over it in a frame and passes between
them and the shell, which is in the viewer (`ui_start`, `ui_say`, `ui_told`, `ui_orders`,
`ui_keep` in `apps/viewer/src/web.rs`). The pages are the same files as on the desktop.

While a map is being downloaded, and while more of it comes as it is driven, a line along the
top of the pages says so: what is coming, and how much of the map is here (`ui.stream` in
`ui/ui.js`, told by `game.js`). What has been read is kept by the browser and not fetched
again (docs/hosting.md).

A browser reads one scene a page. So going to a map loads the page again with that map's
scene. The page that is loaded again is `game.html`, inside another that stays, `index.html`
(`apps/viewer/web/shell.html`), which keeps what has to last from map to map: the sound, which
a browser lets start only after a key or a click, and would otherwise fall silent at every
map until something was pressed again. The inner page's address says where the game has got
to: `scene`, `map`, `mode`, `drive`, and
`time`, `weather` and `soaked` if they were chosen. The mode carries the class, the time of
day and the weather with it after dots (`timed.4.3.3`), so that the page that follows knows
what a run pays. Best laps and what has been won are kept in the browser's own
storage. The sound is the browser's to mix (docs/sound.md), and starts with the first key or
click, which is when a browser lets a page be heard.

In a browser the game is a free drive in the map's own truck, and its main menu shows only
that: there are no kept games to begin or go on with, and none of the game's own trucks,
whose pieces a browser cannot yet be sent a part at a time (`Shell::set_web`).

## Still to do

- In a browser: the game's own trucks, and with them a story; and going from map to map
  without loading the page again.
- Restarting a map in free roam reads its scene again, which takes as long as going to it
  did. (Against the clock it goes back to the line.)
- The others in a timed run are times, dots on the map and a cinematic of the map's own
  truck in their colour: their own trucks on the track wait for a truck being a file of
  its own (docs/campaign.md).
- The game's own driver does not get round Wombat Creek: it is put back again and again
  about a hundred metres in (-164, 55, -55). A watched run cuts away after twelve seconds
  of a shot, so it shows as a truck that stops.
- Settings (sound levels, units, the controls themselves) have no screen.
- The high beams cast no shadows, and do not light dust or rain.
- Holding the controller's bottom button in a ring also works the diff locks, which it is
  the button for.
