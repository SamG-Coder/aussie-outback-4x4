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
| `outback 1000 hub wombat granite blackwood` | The Crossroads | The hub: a dirt main road two trucks wide, with a trailhead for each map named, in the order the road comes to them. |

A kind of country is a handful of numbers: where in the hills it is, how high they stand, how
much is bare rock, how wet it is, how steep a climb its track takes, how many bog holes it
may have and how deep its ruts are worn. Another map is another `Country`.

## Trailheads

A trailhead is a glowing ring on the hub's road in a clearing, and a line of the scene's
`outback.places` file, `place MAP X Y Z RADIUS`. The instruments point the way to the nearest.

In a ring, a banner says what it leads to. Holding the bottom button of a controller, Enter
or F for half a second, or clicking the banner, opens that map's card: its picture, what it
is like, and what to drive it in and for:

- **Time**: dawn, day, dusk or night.
- **Weather**: clear, fair, overcast, or a storm, which starts with the ground already soaked.
- **Free roam**, with no clock, or **Timed laps**, against the best lap kept for that map.

Either reads the map's scene into the same window, behind a loading screen with the map's
picture. From a map, the pause menu goes back to the hub, which is always as the game was
started. A map whose scene is not baked yet is still on the signs, and its card says so.

The best lap of each map is kept in `save/profile.txt`.

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

`title`, `drive`, `pause`, `offer` (a trailhead's card) and `loading`. Behind the title the
game runs with nobody at the controls and the view goes slowly round the truck; paused,
offered a map or loading, the game is not stepped at all.

Menus are worked with the mouse, with the arrows and Enter and Escape, or with a controller's
d-pad or left stick and its bottom and right buttons. Escape, or Start, pauses.

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
| `do WHAT` | Chosen in a menu: `start`, `pause`, `resume`, `restart`, `hub`, `title`, `quit`, `enter`, `roam`, `laps`, `leave`. |
| `set time N` / `set weather N` | Which time of day, and which weather, the next track is driven in. |
| `log TEXT` | Something for the game's log: a mistake in the page. |
| `grab` | On the desktop: pressed where there is nothing, while driving. Take hold of the mouse, to look round with. |
| `touch X Y` | The controls drawn on a screen that is touched: steering, and the pedals, each from -1 to 1 (right, and braking, above zero). |
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

then `http://localhost:8765/index.html?screen=drive&mode=laps&map=1`. `screen` is any screen's
name, or `controls`; `pad` shows a controller's buttons; `inside` and `unbuilt` show a ring's
banner and a map not yet made; `still` stops the fades, for a picture.

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
`time`, `weather` and `soaked` if they were chosen. Best laps are kept in the browser's own
storage. The sound is the browser's to mix (docs/sound.md), and starts with the first key or
click, which is when a browser lets a page be heard.

## Still to do

- In a browser: going from map to map without loading the page again.
- Going back to the hub puts the truck at the camp, not at the trailhead it left from.
- Restarting a map reads its scene again, which takes as long as going to it did.
- Settings (sound levels, units, the controls themselves) have no screen.
- The high beams cast no shadows, and do not light dust or rain.
- Holding the controller's bottom button in a ring also works the diff locks, which it is
  the button for.
