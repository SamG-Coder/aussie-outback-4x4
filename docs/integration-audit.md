# How the systems fit together: an audit

15 November 2026. Written after the menu, the garage, the town's doors and the town's track
were built, to check that each system agrees with the ones it touches.

**How this was checked.** By reading the code along each path where two systems meet, by the
rule tests (84 in the game, 65 in the bake, all passing), by the design maps the bake draws,
by still pictures taken without a window, and by the pages in a browser preview. **Nothing in
this audit was checked by playing the game.** Each section ends with what to try in the game
to review it.

## Summary

| Section | Ties to | State |
|---|---|---|
| 1. Main menu and kept games | free drive, story, options | One fault fixed |
| 2. Story, bank and calendar | shed, town bank, calendar page | One fault fixed |
| 3. Trucks: garage, auto shop, used trucks | gold in the truck, showroom, saves | One fault fixed |
| 4. Gold and the detector | truck holds, chest, buyer, sound | No fault found |
| 5. Maps, gates and the town | saves, trailheads, weather, pause menu | Three faults fixed |
| 6. Timed runs | trucks' grades, money, calendar | One fault fixed |
| 7. Pages: look and wording | every screen | Two faults fixed, two styles remain |
| 8. Browser version | menu, trucks | Not working with the new menu: open |

## 1. Main menu and kept games

The main menu has no game behind it. A game is one of three slots; New Game and Load Game
choose one; Free Drive keeps nothing.

- **Fault, fixed: a free drive could be written into a story's slot.** "Quit to main menu"
  only changed the screen: the slot stayed open, so a free drive started afterwards (its gold,
  its day, where the truck was left) was kept into that story. Quitting to the menu now keeps
  the game, closes the slot and goes back to the Crossroads with no game loaded
  (`Saves::Close`, `Shell::close`).
- Options (volume, full screen) are the player's, not a game's: they are kept in
  `save/options.txt` and carried across loading a game.
- Deleting a slot works from the menu whichever slot was last played.
- Open: `save/slot.txt` is still written and nothing reads it.
- Open: the old title actions (`story`, `garage`, `truck_next`, `truck_prev`, `truck_fit`) and
  the yard's `home` page are no longer reachable from any page and can be removed.

Review: start a game in slot 1, quit to the main menu, do a free drive, sleep in the shed, quit,
load slot 1. The day and the truck's place should be as the story left them.

## 2. Story, bank and calendar

A story starts owing $100,000 on the block plus the first truck. $1,850 (or what is left)
comes out each Friday on waking. The bank is a door in the town; the calendar is in the shed.

- **Fault, fixed: the calendar had its own idea of what was due.** It showed a fixed "The
  block $1,500, The truck $350" every Friday, in a free drive too and after the debt was paid.
  It now shows what the bank will really take (`Yard::friday`'s amount) and what is still
  owed, and nothing where there is no debt.
- Buying a truck or a part with too little in hand goes on the debt up to the bank's limit:
  the shop, the used trucks and the bank all use the one `Yard`.
- Open: an overdue oil change has no effect on the truck yet.

Review: in a story open the calendar in the shed: Fridays show $1,850 and "Still owed". Pay
some off at the bank, sleep to Friday, and check the debt drops by the repayment.

## 3. Trucks: garage, auto shop, used trucks

The garage is at the shed. The auto shop (Motor Spares), the used trucks (the Hotel) and the
bank are doors in the town, walked up to on foot. Each opens one page in the showroom.

- **Fault, fixed: gold aboard one truck turned up inside another.** What is in a truck's tray
  or behind its doors is kept in that truck's own space. Taking another truck out, or selling
  the one driven, left the nuggets where they were, inside the new truck's body. Both are now
  refused while anything is aboard: "There is gold aboard this one. Put it in the chest first."
- A first truck's colour and the shop's respray are the one colour row.
- Dirt, kilometres, parts and paint are kept per truck and follow it through the showroom.
- Open: leaving a door puts the driver back in the truck where it was parked, not at the door.
- Open: in a free drive the doors do nothing and say nothing.
- Open: lift kits and tyres do not change how a truck drives; the `.vehicle` figures are
  first estimates.

Review: put a nugget in the tray, park in the shed, open the Garage and try to take another
truck out. Then walk to each of the three doors in town.

## 4. Gold and the detector

Gold is in the ground at random each time a map is entered; it is found on foot, dug, carried,
put in the tray, a hold or the chest, and sold by weight at the buyer's.

- The detector, the on-screen readout and the sound all read the one signal (`Gold::heard`):
  beeps quicken with nearness and with facing the gold.
- A timed run's purse is weighed out as a nugget into the tray, so it is sold like any other.
- The buyer pays into the same money the shop and the bank use.
- No fault found on these paths.

Review: find gold by the beeps alone, carry it to town in the tray, sell it, and check the
"In hand" figure at the buyer's, in the shed and in the shop agree.

## 5. Maps, gates and the town

The Crossroads is the hub. The town is reached from a gate beside the shed, by its own
670 m track with a ford; its gate is the way back.

- **Fault, fixed: the town was offered as a timed track.** Giving the town a length made the
  trailhead card offer Practice and Timed run on a track that is not a loop. The town is now
  only driven to: its card says "Gate" and "Drive there", with no class, hour, weather or lap.
- **Fault, fixed: the weather did not follow the calendar.** The hub was always in the
  weather the game was started with, and the clock was put back to the starting hour on every
  return. The hub and the town are now in the day's forecast weather, the clock runs on, and a
  night's sleep brings the next day's weather. A timed track is still driven at the hour and
  weather picked for it.
- **Fault, fixed: "Back to the Crossroads" showed in the town**, which has its gate. It now
  shows only on a map with no trailhead home.
- Going through a trailhead arrives at the other map's gate; coming back arrives at the
  trailhead that was left.
- Open: Wombat Creek, Granite Ridge and Blackwood Gully have no gate home, so they keep the
  pause menu's "Back to the Crossroads".
- Open: picking an hour for a timed track moves the day's clock to it.

Review: drive from the shed's gate to the town and back; sleep, and check the weather matches
the calendar's square for the new day.

## 6. Timed runs

Each track has five classes with bronze, silver and gold times, three other drivers to watch,
and purses paid in gold; first wins pay in full, and night and rain pay more.

- **Fault, fixed: the class was not tied to the truck.** Any class could be picked whatever
  was driven, so a grade 5 truck could take the grade 1 purses. One of the game's trucks now
  runs in its own grade's class: the card says "Your truck's class" and there is nothing to
  pick.
- Open: the medal times are untuned, and no full timed lap has been driven in the game.

Review: at a trailhead in a grade 1 truck the card should read "Class 1"; buy a grade 2 truck
and it should read "Class 2".

## 7. Pages: look and wording

- **Fault, fixed: the same money had two names**: "In the bank" at the shed and the buyer's,
  "In hand" in the garage and shops. It is "In hand" everywhere; the bank is where money is owed.
- **Fault, fixed: pages promised a save that does not happen in a free drive** ("The game is
  saved", "The game is kept as you play"). They now say so only in a story.
- The controls list in Options now names going in a door.
- Open: there are two page layouts. The main menu and the pause menu use the rail down the
  left; the shed, the buyer, the trailhead card, the line-up, the result, the calendar and the
  map use the centred card. They share type and colour, not the frame.
- Open: `docs/ui.md` and `docs/campaign.md` describe the menus as they were before this work.

Review: every page reached with the controller only; nothing should need the mouse.

## 8. Browser version

- Open: the browser version has no slots, so the new main menu cannot start or load a story
  there, and it still drives only the map's own truck. It compiles; it has not been run.

## Files by section, for review

| Section | Files |
|---|---|
| Engine: piece packs, glass, holds | `crates/solstice-asset`, `-import`, `-game`, `-physics`, `-render`, `-vehicle` |
| Trucks: generator, kits, packs | `tools/blender/truck`, `tools/blender/showroom`, `scenes/trucks`, `apps/bake`, `ui/trucks` |
| Maps: town track, gate, doors | `crates/solstice-bake`, `tools/blender/town`, `scenes/hub.txt`, `scenes/town.txt`, `scenes/outback.world`, `postcards.bat` |
| Game: menu, saves, yard, gold, timed runs | `apps/viewer/src`, `scripts/trial.sh` |
| Pages | `ui/index.html`, `ui/ui.css`, `ui/ui.js` |
| Notes | `docs` |
