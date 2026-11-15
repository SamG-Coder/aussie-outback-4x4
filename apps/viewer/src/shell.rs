//! The game round the driving: its title, the maps it is made of and how one is gone to from
//! another, what is paused, and what its pages are told to show (docs/ui.md).
//!
//! A game is several baked scenes. One is the hub: country with a road through it and a
//! trailhead for each of the others, a ring on the ground to drive into. Stopped in a ring,
//! the driver is offered that map, to roam or to lap against the clock; from a map, the pause
//! menu goes back to the hub.
//!
//! None of this draws anything. The pages in `ui/` do, the same pages on the desktop, where
//! they are laid over the window, as in a browser, where the scene is a canvas under them.
//! Each frame the shell is told what the pages said and what the devices are doing, says
//! whether the game is to be stepped, and hands back what the pages should show as JSON.
use glam::Vec3;
use serde_json::{json, Value};
use solstice_asset::GameFile;
use solstice_game::{Devices, Hud, InputMap};
use std::collections::BTreeMap;

/// What a menu is worked with when it is not the mouse: the arrows or the left stick or the
/// d-pad to move, Enter or the bottom button to choose, Escape or the right button to go
/// back. Escape and Start pause the driving.
const MENU_INPUT: &str = "\
menu_up key ArrowUp
menu_up key KeyW
menu_up gamepad up
menu_up gamepad left_y -1
menu_down key ArrowDown
menu_down key KeyS
menu_down gamepad down
menu_down gamepad left_y
menu_left key ArrowLeft
menu_left key KeyA
menu_left gamepad left
menu_left gamepad left_x -1
menu_right key ArrowRight
menu_right key KeyD
menu_right gamepad right
menu_right gamepad left_x
menu_accept key Enter
menu_accept key Space
menu_accept gamepad south
menu_back key Escape
menu_back key Backspace
menu_back gamepad east
pause key Escape
pause gamepad start
enter key Enter
enter key KeyF
enter key KeyE
enter gamepad south
swap key KeyQ
swap gamepad west
drop key KeyG
drop gamepad east
";

/// How fast the title's view goes round the truck, in pixels of mouse a second.
const TITLE_ORBIT: f32 = 26.0;
/// How long the button is held in a trailhead's ring to be offered its map, in seconds.
const HOLD: f32 = 0.45;

/// The times of day a map can be driven at: what each is called, and its hour.
pub const TIMES: [(&str, f32); 4] = [
    ("Dawn", 6.6),
    ("Day", 11.0),
    ("Dusk", 17.9),
    ("Night", 22.5),
];
/// The weathers: what each is called, the renderer's name for it, and how many seconds of it
/// there have been already, which is how wet the ground is.
pub const WEATHERS: [(&str, &str, f32); 4] = [
    ("Clear", "clear", 0.0),
    ("Fair", "fair", 0.0),
    ("Overcast", "overcast", 0.0),
    ("Storm", "storm", 900.0),
];
/// How often the pages are told what the instruments read, in seconds.
const TOLD_EVERY: f32 = 1.0 / 24.0;

/// One of the game's maps.
#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    pub name: String,
    /// The baked scene it is, as a path from where the game is run.
    pub scene: String,
    pub title: String,
    /// A line or two on what it is like.
    pub about: String,
    /// What kind of driving it is, and how long, in a few words each.
    pub kind: String,
    pub length: String,
}

/// The game's maps, and which of them is the hub.
///
/// ```text
/// hub crossroads
/// map crossroads assets/hub.solstice     The Crossroads
/// map wombat     assets/outback.solstice Wombat Creek Track
/// about  wombat A washed-out fire trail ...
/// kind   wombat Fire trail
/// length wombat 2.4 km
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct World {
    pub hub: String,
    pub maps: Vec<Map>,
}

impl World {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut world = Self::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let error = |message: &str| format!("line {}: {message}", number + 1);
            let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let rest = rest.trim();
            match word {
                "hub" => world.hub = rest.to_owned(),
                "map" => {
                    let mut words = rest.splitn(3, char::is_whitespace);
                    let (Some(name), Some(scene)) = (words.next(), words.next()) else {
                        return Err(error("expected `map NAME SCENE TITLE`"));
                    };
                    let title = words.next().unwrap_or(name).trim();
                    world.maps.push(Map {
                        name: name.to_owned(),
                        scene: scene.to_owned(),
                        title: title.to_owned(),
                        about: String::new(),
                        kind: String::new(),
                        length: String::new(),
                    });
                }
                "about" | "kind" | "length" => {
                    let (name, said) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                    let map = world
                        .maps
                        .iter_mut()
                        .find(|map| map.name == name)
                        .ok_or_else(|| error(&format!("there is no map `{name}` yet")))?;
                    let field = match word {
                        "about" => &mut map.about,
                        "kind" => &mut map.kind,
                        _ => &mut map.length,
                    };
                    said.trim().clone_into(field);
                }
                _ => return Err(error(&format!("`{word}` is not something a game has"))),
            }
        }
        if world.map(&world.hub).is_none() {
            return Err(format!("the hub, `{}`, is not one of the maps", world.hub));
        }
        Ok(world)
    }

    pub fn map(&self, name: &str) -> Option<&Map> {
        self.maps.iter().find(|map| map.name == name)
    }
}

/// A trailhead: a ring on the ground of one map, that another is gone to from.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    /// The map it leads to.
    pub map: String,
    pub at: Vec3,
    pub radius: f32,
}

/// The place that is the hub's shed, in a `.places` file, where a map's name would be.
pub const SHED: &str = "shed";
/// And what a town's gold buyer is called among its places: where what is in the truck's
/// tray is sold.
pub const BUYER: &str = "buyer";
/// What a kilogram of gold is bought for, in dollars. (A game's price: a real one would
/// make one good nugget the end of the game.)
pub const PRICE: f32 = 2400.0;

/// What this much gold is bought for.
fn price(kilos: f32) -> u64 {
    (kilos * PRICE).round().max(0.0) as u64
}
/// How long a night's sleep takes to go by on the screen, in seconds.
const NIGHT: f32 = 3.4;
/// How near the truck its driver stands, on foot, to get back into it.
pub const GET_IN: f32 = 4.2;
/// How fast the truck may be going for its driver to get out of it, in metres a second.
const STOPPED: f32 = 1.5;
/// How often where the truck is is kept, while it is driven: in seconds.
const KEPT_EVERY: f32 = 5.0;
/// When a night's sleep ends: half past six.
pub const MORNING: f32 = 6.5;

/// The trailheads a scene's gameplay files give: every `place MAP X Y Z RADIUS` line of every
/// file named `*.places`. A hub's shed is one of them, with [`SHED`] for its map.
pub fn places(files: &[GameFile]) -> Vec<Place> {
    let mut places = Vec::new();
    for file in files.iter().filter(|file| file.name.ends_with(".places")) {
        for line in file.text.lines() {
            let words: Vec<&str> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            if let ["place", map, x, y, z, radius] = words[..] {
                let numbers: Vec<f32> = [x, y, z, radius]
                    .iter()
                    .filter_map(|word| word.parse().ok())
                    .collect();
                if let [x, y, z, radius] = numbers[..] {
                    places.push(Place {
                        map: map.to_owned(),
                        at: Vec3::new(x, y, z),
                        radius,
                    });
                }
            }
        }
    }
    places
}

/// What a map is driven for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Anywhere, for as long as liked.
    Roam,
    /// Round the track against the clock.
    Laps,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Roam => "roam",
            Self::Laps => "laps",
        }
    }
}

/// What is on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The game's name and its first menu, the truck standing behind them.
    Title,
    Drive,
    Pause,
    /// Stopped in a trailhead's ring: its map is offered.
    Offer,
    /// A map is being read. Nothing else happens until [`Shell::enter`].
    Loading,
    /// Stopped in the shed's bay: to sleep, or to get out.
    Shed,
    /// Asleep: the night goes by in a moment.
    Sleep,
    /// Stopped at the gold buyer's: what is in the tray is weighed, to be sold.
    Buyer,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Drive => "drive",
            Self::Pause => "pause",
            Self::Offer => "offer",
            Self::Loading => "loading",
            Self::Shed => "shed",
            Self::Sleep => "sleep",
            Self::Buyer => "buyer",
        }
    }
}

/// How a frame of the game is to be run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// As played: the devices drive.
    Play,
    /// With nobody at the controls, the view going round the truck by this many pixels of
    /// mouse: what is behind the title.
    Unattended(f32),
    /// Not at all: everything stays where it is.
    Held,
}

/// What the shell asks of whatever runs it.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    /// Read this scene and play it, then call [`Shell::enter`]. With the hour and the weather
    /// (its name, and how many seconds of it there have been) to play it in, if they were
    /// chosen; otherwise as the game was started.
    Load {
        map: String,
        scene: String,
        sky: Option<(f32, &'static str, f32)>,
    },
    /// Close the game.
    Quit,
    /// Take hold of the mouse, to look round the truck with: the page was clicked where
    /// there is nothing to click, while driving.
    Look,
    /// A night has been slept through: it is the next morning, [`MORNING`].
    Sleep,
    /// Show a nugget with one of the map's nugget models: there, turned so, that big, and
    /// of that shape; or put the model away.
    Item(crate::gold::Shown),
    /// A nugget's body is to be put in the world, or taken out of it: nuggets are things,
    /// which fall, lie and are carried. So is the detector's, when it has been put down.
    Thing(crate::gold::Body),
    /// How the detector is to be shown: in the driver's hand, lying where it was put down,
    /// or not at all.
    Carry(crate::gold::Carried),
    /// Play one of the game's sounds, once.
    Sound(&'static str),
    /// The driver gets out of the truck, or, on foot beside it, back in.
    Swap,
    /// Put the truck where it was when this map was last left: there, facing that way (a
    /// yaw in degrees).
    Place { position: Vec3, yaw: f32 },
}

/// The game round the driving.
pub struct Shell {
    world: World,
    screen: Screen,
    /// The map being played, and what for.
    map: String,
    mode: Mode,
    places: Vec<Place>,
    /// The trailhead whose ring the truck is in, how long the button has been held there,
    /// and the trailhead being offered.
    inside: Option<usize>,
    hold: f32,
    offered: Option<usize>,
    /// The time of day and the weather chosen for the next track: which of [`TIMES`] and of
    /// [`WEATHERS`].
    time: usize,
    weather: usize,
    /// How far the title has taken the view round the truck, in pixels of mouse, and how far
    /// to take it back when the driving starts.
    orbited: f32,
    unwind: f32,
    /// The map and the mode there were before the map being read was asked for.
    left: Option<(String, Mode)>,
    menu: InputMap,
    /// What a menu was worked with since the pages were last told.
    nav: Vec<&'static str>,
    /// The best lap of each map, in seconds, and whether one has changed since they were
    /// last taken to be kept.
    bests: BTreeMap<String, f64>,
    unsaved: bool,
    orders: Vec<Order>,
    since_told: f32,
    /// Whether the pages have been told of the screen there is now.
    told_screen: Option<Screen>,
    /// Whether a gamepad was the last thing used, so the pages name its buttons, not keys.
    pad: bool,
    /// Which day of the game it is: a night's sleep in the shed ends one.
    day: u32,
    /// The gold: what is in the ground of the map being played, and every nugget dug up.
    gold: crate::gold::Gold,
    /// A number that is never the same twice, from how long each frame of the game took:
    /// what says where gold is, when a map is entered.
    chance: u64,
    /// What gold has been sold for, in dollars, and the last sale, while it is being
    /// looked at: how many nuggets, what they weighed, and what was paid.
    money: u64,
    sold: Option<(usize, f32, u64)>,
    /// Where the world last said the nuggets' bodies were.
    seen: Vec<(u32, Vec3, glam::Quat)>,
    /// How the detector was last said to be shown, if it has been said since the map was
    /// entered.
    carried: Option<crate::gold::Carried>,
    /// The map of the map being played: its picture, among the pages, and how far from the
    /// middle of the map the picture's edge is, in metres.
    chart: Option<(String, f32)>,
    /// Whether this is a trial, played by a script that cannot go looking for gold: there
    /// is then some three paces ahead of the driver wherever they get out.
    trial: bool,
    /// How many frames the driver has been out of the truck, for a trial.
    trial_out: u32,
    /// The way the driver faces on foot, where the truck is and how it is turned, and its
    /// tray, in its own space.
    walker_yaw: f32,
    origin: Vec3,
    turned: glam::Quat,
    tray: Option<(Vec3, Vec3)>,
    /// How long a night's sleep has been going by.
    slept: f32,
    /// Whether the driver is out of the truck, and where they are; where the truck is, and
    /// how fast it is going.
    afoot: bool,
    walker: Vec3,
    truck: Vec3,
    speed: f32,
    /// The button that is held to do things has done one, and has not been let go since.
    spent: bool,
    /// Where the truck was last on each map, and the way it faced, as a yaw in degrees: a
    /// map come back to is come back to there.
    poses: BTreeMap<String, (Vec3, f32)>,
    /// How long since that was last kept, and the map the game was last left on, to go
    /// back to when it is next started.
    kept_for: f32,
    resume: Option<String>,
}

impl Shell {
    /// A game of these maps, with what was kept of it the last time: the best laps there have
    /// been (`best MAP SECONDS` a line), the day it is, and what has been found.
    pub fn new(world: World, kept: &str) -> Self {
        let mut bests = BTreeMap::new();
        let (mut day, mut money) = (1, 0);
        let (mut poses, mut resume) = (BTreeMap::new(), None::<String>);
        for line in kept.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words[..] {
                ["best", map, seconds] => {
                    if let Ok(seconds) = seconds.parse::<f64>() {
                        bests.insert(map.to_owned(), seconds);
                    }
                }
                ["day", count] => day = count.parse().unwrap_or(1).max(1),
                ["money", dollars] => money = dollars.parse().unwrap_or(0),
                ["at", map, x, y, z, yaw] => {
                    if let (Ok(x), Ok(y), Ok(z), Ok(yaw)) =
                        (x.parse(), y.parse(), z.parse(), yaw.parse())
                    {
                        poses.insert(map.to_owned(), (Vec3::new(x, y, z), yaw));
                    }
                }
                ["map", name] => resume = Some(name.to_owned()),
                _ => {}
            }
        }
        let hub = world.hub.clone();
        Self {
            map: world.hub.clone(),
            world,
            screen: Screen::Title,
            mode: Mode::Roam,
            places: Vec::new(),
            inside: None,
            hold: 0.0,
            offered: None,
            time: 1,
            weather: 1,
            orbited: 0.0,
            unwind: 0.0,
            left: None,
            menu: InputMap::parse(MENU_INPUT).expect("the menu's actions read"),
            nav: Vec::new(),
            bests,
            unsaved: false,
            orders: Vec::new(),
            since_told: 0.0,
            told_screen: None,
            pad: false,
            day,
            gold: crate::gold::Gold::read(kept),
            chance: 0x9E37_79B9_7F4A_7C15,
            money,
            sold: None,
            seen: Vec::new(),
            carried: None,
            chart: None,
            trial: false,
            trial_out: 0,
            walker_yaw: 0.0,
            origin: Vec3::ZERO,
            turned: glam::Quat::IDENTITY,
            tray: None,
            slept: 0.0,
            afoot: false,
            walker: Vec3::ZERO,
            truck: Vec3::ZERO,
            speed: 0.0,
            spent: false,
            poses,
            kept_for: 0.0,
            // Only a track is gone back to: the game starts in the hub as it is.
            resume: resume.filter(|map| *map != hub),
        }
    }

    /// What is kept of the game, as it is now: for when it is closed with no warning.
    pub fn keep_now(&mut self) -> String {
        self.unsaved = true;
        self.to_keep().unwrap_or_default()
    }

    /// Where the driver and the truck are, for the gold.
    fn stands(&self) -> crate::gold::Stands<'_> {
        crate::gold::Stands {
            map: &self.map,
            walker: self.walker,
            yaw: self.walker_yaw,
            truck: self.origin,
            turned: self.turned,
            tray: self.tray,
        }
    }

    /// How strong the signal of the detector in the driver's hand is, from 0 to 1: nothing
    /// unless they are out of the truck with it, and the game is being played.
    pub fn detector(&self) -> Option<f32> {
        (self.afoot
            && self.screen == Screen::Drive
            && self.gold.detector == crate::gold::Detector::Hand)
            .then(|| self.gold.signal(&self.stands()))
    }

    /// Whoever runs the world says where the nuggets' bodies are, before [`Shell::after`]:
    /// each its nugget's number, where it is, and how it is turned.
    pub fn see(&mut self, seen: Vec<(u32, Vec3, glam::Quat)>) {
        self.seen = seen;
    }

    /// Says whether this is a trial: see [`Shell::after`].
    pub fn set_trial(&mut self, trial: bool) {
        self.trial = trial;
    }

    /// What gold has been sold for so far, in dollars.
    pub fn money(&self) -> u64 {
        self.money
    }

    /// Stops at a place the truck is in: the shed, the gold buyer's, or a trailhead, whose
    /// map is offered.
    fn stop_at(&mut self, index: usize) {
        let named = |name: &str| {
            self.places
                .get(index)
                .is_some_and(|place| place.map == name)
        };
        if named(SHED) {
            self.screen = Screen::Shed;
        } else if named(BUYER) {
            self.sold = None;
            self.screen = Screen::Buyer;
        } else {
            self.offered = Some(index);
            self.screen = Screen::Offer;
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The map that is, or is about to be, played.
    pub fn map(&self) -> &str {
        &self.map
    }

    /// A scene has been read and is being played: `files` are its gameplay files. The first
    /// one is behind the title; after that, each is driven straight away.
    pub fn enter(&mut self, files: &[GameFile]) {
        self.places = places(files);
        // Gold is put in the ground of the map now, where it was not before.
        self.gold.enter(
            files,
            self.chance ^ u64::from(self.day).wrapping_mul(0x2545_F491_4F6C_DD1D),
        );
        self.seen.clear();
        self.carried = None;
        // Its map, if it was baked with one: `chart FILE HALF` in its `.chart` file.
        self.chart = files
            .iter()
            .filter(|file| file.name.ends_with(".chart"))
            .flat_map(|file| file.text.lines())
            .find_map(
                |line| match line.split_whitespace().collect::<Vec<_>>()[..] {
                    ["chart", file, half] => Some((file.to_owned(), half.parse().ok()?)),
                    _ => None,
                },
            );
        self.offered = None;
        self.inside = None;
        self.hold = 0.0;
        self.afoot = false;
        // Roaming, a map is come back to where it was left.
        if self.mode == Mode::Roam {
            if let Some(&(position, yaw)) = self.poses.get(&self.map) {
                self.orders.push(Order::Place { position, yaw });
            }
        }
        self.unsaved = true;
        if self.screen == Screen::Loading {
            self.screen = Screen::Drive;
        }
    }

    /// What is to be kept from one run of the game to the next, if it has changed since
    /// this was last asked.
    pub fn to_keep(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.unsaved) {
            return None;
        }
        let mut kept: String = self
            .bests
            .iter()
            .map(|(map, seconds)| format!("best {map} {seconds:.3}\n"))
            .collect();
        kept += &format!("day {}\n", self.day);
        kept += &format!("money {}\n", self.money);
        self.gold.keep(&mut kept);
        kept += &format!("map {}\n", self.map);
        for (map, (at, yaw)) in &self.poses {
            kept += &format!("at {map} {:.2} {:.2} {:.2} {yaw:.1}\n", at.x, at.y, at.z);
        }
        Some(kept)
    }

    pub fn take_orders(&mut self) -> Vec<Order> {
        std::mem::take(&mut self.orders)
    }

    /// Says which map is being played, when it is not the hub the game starts in.
    pub fn start_in(&mut self, map: &str) {
        if self.world.map(map).is_some() {
            map.clone_into(&mut self.map);
        }
    }

    /// Says that the map is being driven already, and what for (`roam` or `laps`): a page in
    /// a browser reads each map afresh, and takes up the game where the last page left it.
    pub fn resume(&mut self, mode: &str) {
        self.mode = if mode == "laps" {
            Mode::Laps
        } else {
            Mode::Roam
        };
        self.screen = Screen::Drive;
    }

    /// What the map is being driven for, as `resume` is told it.
    pub fn mode_name(&self) -> &'static str {
        self.mode.name()
    }

    /// The map asked for could not be read: the one there was is still being played.
    pub fn stay(&mut self) {
        if let Some((map, mode)) = self.left.take() {
            self.map = map;
            self.mode = mode;
        }
        self.screen = Screen::Drive;
    }

    fn go(&mut self, map: &str, mode: Mode) {
        let Some(found) = self.world.map(map) else {
            return;
        };
        self.left = Some((self.map.clone(), self.mode));
        // A track is driven at the hour and in the weather chosen for it; the hub, as the
        // game was started.
        let sky = (found.name != self.world.hub).then(|| {
            let (_, name, soaked) = WEATHERS[self.weather];
            (TIMES[self.time].1, name, soaked)
        });
        self.orders.push(Order::Load {
            map: found.name.clone(),
            scene: found.scene.clone(),
            sky,
        });
        self.map = found.name.clone();
        self.mode = mode;
        self.screen = Screen::Loading;
    }

    /// Does what a page, or a menu worked from a pad, asked for.
    fn act(&mut self, what: &str) {
        match (self.screen, what) {
            (Screen::Title, "start") => {
                // The view went round the truck behind the title: it starts behind it.
                self.unwind = -std::mem::take(&mut self.orbited);
                self.screen = Screen::Drive;
                // A game left out on a track goes on from there.
                if let Some(map) = self
                    .resume
                    .take()
                    .filter(|map| self.world.map(map).is_some())
                {
                    self.go(&map, Mode::Roam);
                }
            }
            (Screen::Pause, "resume") => self.screen = Screen::Drive,
            (Screen::Drive, "pause") => self.screen = Screen::Pause,
            // The ring's banner was clicked.
            (Screen::Drive, "enter") => {
                if let Some(index) = self.inside {
                    self.stop_at(index);
                }
            }
            // In the shed: a night's sleep ends the day.
            (Screen::Shed, "sleep") => {
                self.day += 1;
                self.unsaved = true;
                self.slept = 0.0;
                self.orders.push(Order::Sleep);
                self.screen = Screen::Sleep;
            }
            (Screen::Shed | Screen::Buyer, "leave") => self.screen = Screen::Drive,
            // At the gold buyer's: what is in the tray is weighed, paid for, and gone.
            (Screen::Buyer, "sell") => {
                let (count, kilos) = self.gold.sell();
                if count > 0 {
                    let paid = price(kilos);
                    self.money += paid;
                    self.sold = Some((count, kilos, paid));
                    self.unsaved = true;
                    self.told_screen = None;
                    self.orders.push(Order::Sound("coins"));
                    log::info!("gold: sold {count} nuggets, {kilos:.2} kg, for ${paid}");
                    // Their bodies go now: nothing else looks while the game is stopped.
                    let stands = crate::gold::Stands {
                        map: &self.map,
                        walker: self.walker,
                        yaw: self.walker_yaw,
                        truck: self.origin,
                        turned: self.turned,
                        tray: self.tray,
                    };
                    for body in self.gold.sync(&stands) {
                        self.orders.push(Order::Thing(body));
                    }
                }
            }
            // Out of the truck, in the shed's bay or the buyer's yard.
            (Screen::Shed | Screen::Buyer, "walk") => {
                self.screen = Screen::Drive;
                self.spent = true;
                self.orders.push(Order::Swap);
            }
            (Screen::Pause, "title") => self.screen = Screen::Title,
            (Screen::Pause, "restart") => {
                let (map, mode) = (self.map.clone(), self.mode);
                self.go(&map, mode);
            }
            (Screen::Pause, "hub") if self.map != self.world.hub => {
                let hub = self.world.hub.clone();
                self.go(&hub, Mode::Roam);
            }
            (Screen::Offer, "roam" | "laps") => {
                if let Some(place) = self.offered.and_then(|index| self.places.get(index)) {
                    let map = place.map.clone();
                    let mode = if what == "laps" {
                        Mode::Laps
                    } else {
                        Mode::Roam
                    };
                    self.go(&map, mode);
                }
            }
            (Screen::Offer, "leave") => {
                self.offered = None;
                self.screen = Screen::Drive;
            }
            (Screen::Title | Screen::Pause, "quit") => self.orders.push(Order::Quit),
            _ => {}
        }
    }

    /// Takes in a line a page said: a key or the mouse, which are the game's devices, or
    /// something done in a menu.
    pub fn hear(&mut self, line: &str, devices: &mut Devices) {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words[..] {
            ["key", code, down] => {
                devices.key(code, down == "1");
                self.pad = false;
            }
            // The controls drawn on a screen that is touched: steering, and the pedals.
            ["touch", x, y] => {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    devices.touch_stick(x, y);
                    self.pad = false;
                }
            }
            // A finger dragged across a touched screen, on foot: looking round.
            ["look", x, y] => {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    devices.touch_look(x, y);
                }
            }
            // The screen was touched while a controller was the one in use: the controls
            // drawn on it are wanted back.
            ["touched"] => self.pad = false,
            // A controller, where the game cannot read one itself and whoever holds the
            // page reads it for it (a phone: `MainActivity.java`). One plugged in is taken
            // to be the one in use, and so is one that is pressed or pushed.
            ["pad", "here"] => self.pad = true,
            ["pad", "gone"] => {
                self.pad = false;
                devices.release_all();
            }
            ["pad", "button", name, value] => {
                if let Ok(value) = value.parse::<f32>() {
                    devices.gamepad_button(name, value);
                    self.pad |= value > 0.5;
                }
            }
            ["pad", "axis", name, value] => {
                if let Ok(value) = value.parse::<f32>() {
                    devices.gamepad_axis(name, value);
                    self.pad |= value.abs() > 0.5;
                }
            }
            ["mouse", x, y] => {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    devices.mouse_motion(x, y);
                }
            }
            ["do", what] => {
                log::info!("menu: {what}");
                self.act(what);
            }
            // What a map is to be driven in, chosen at its trailhead.
            ["set", "time", index] => {
                if let Some(index) = index.parse().ok().filter(|&i| i < TIMES.len()) {
                    self.time = index;
                }
            }
            ["set", "weather", index] => {
                if let Some(index) = index.parse().ok().filter(|&i| i < WEATHERS.len()) {
                    self.weather = index;
                }
            }
            // Something the page wants written down: a mistake in it, most likely.
            ["log", ..] => log::info!("page: {}", &line[3..].trim()),
            ["grab"] if self.screen == Screen::Drive => self.orders.push(Order::Look),
            // A page that has just loaded has been told nothing yet.
            ["hello"] => self.told_screen = None,
            // The page is no longer what is typed into: nothing is held any more.
            ["blur"] => devices.release_all(),
            _ => {}
        }
    }

    /// Before a frame: reads what the devices do to the menus, and says how the game is to
    /// be stepped. Unless it is to be played, the devices' presses are used up here.
    pub fn before(&mut self, devices: &mut Devices, seconds: f32) -> Step {
        let menu = self.menu.actions(devices, seconds);
        if pad_pressed(devices, &self.menu) {
            self.pad = true;
        }
        match self.screen {
            Screen::Drive => {
                if menu.pressed("pause") {
                    self.screen = Screen::Pause;
                }
                // What holding the button down does. On foot beside the truck, it gets back
                // in. In the truck: in a trailhead's ring or the shed's bay, it stops there;
                // anywhere else, roaming and at rest, it gets out.
                let held = menu.held("enter");
                if !held {
                    self.spent = false;
                }
                // On foot, a press of it does what there is to do with gold where the
                // driver stands: dig it, pick it up, put it in the tray or the chest, put
                // it down.
                if self.afoot && menu.pressed("enter") {
                    use crate::gold::Use;
                    let stands = crate::gold::Stands {
                        map: &self.map,
                        walker: self.walker,
                        yaw: self.walker_yaw,
                        truck: self.origin,
                        turned: self.turned,
                        tray: self.tray,
                    };
                    if let Some(done) = self.gold.act(&stands) {
                        log::info!("gold: {}", done.name());
                        self.unsaved = true;
                        self.told_screen = None;
                        self.orders.push(Order::Sound(match done {
                            Use::Dig => "dig",
                            Use::IntoChest => "coins",
                            _ => "nugget",
                        }));
                    }
                }
                // On foot, two more buttons: one slings the detector or takes it in hand
                // again, and one puts down whatever the hands hold.
                if self.afoot && menu.pressed("swap") && self.gold.swap() {
                    log::info!("gold: the detector is {:?}", self.gold.detector);
                    self.unsaved = true;
                    self.told_screen = None;
                }
                if self.afoot && menu.pressed("drop") {
                    let stands = crate::gold::Stands {
                        map: &self.map,
                        walker: self.walker,
                        yaw: self.walker_yaw,
                        truck: self.origin,
                        turned: self.turned,
                        tray: self.tray,
                    };
                    if let Some(done) = self.gold.put_down(&stands) {
                        log::info!("gold: put down, {}", done.name());
                        self.unsaved = true;
                        self.told_screen = None;
                        self.orders.push(Order::Sound("nugget"));
                    }
                }
                let does = if self.spent {
                    None
                } else if self.afoot {
                    (flat_distance(self.walker, self.truck) < GET_IN).then_some(None)
                } else if let Some(index) = self.inside {
                    Some(Some(index))
                } else {
                    (self.mode == Mode::Roam && self.speed < STOPPED).then_some(None)
                };
                match does.filter(|_| held) {
                    Some(does) => {
                        self.hold += seconds;
                        if self.hold >= HOLD {
                            self.hold = 0.0;
                            self.spent = true;
                            match does {
                                Some(index) => self.stop_at(index),
                                None => self.orders.push(Order::Swap),
                            }
                        }
                    }
                    None => self.hold = 0.0,
                }
            }
            Screen::Loading => {}
            Screen::Sleep => {
                self.slept += seconds;
                if self.slept >= NIGHT {
                    self.screen = Screen::Drive;
                }
            }
            _ => {
                for (action, name) in [
                    ("menu_up", "up"),
                    ("menu_down", "down"),
                    ("menu_left", "left"),
                    ("menu_right", "right"),
                    ("menu_accept", "accept"),
                    ("menu_back", "back"),
                ] {
                    if menu.pressed(action) {
                        self.nav.push(name);
                    }
                }
            }
        }
        let step = match self.screen {
            Screen::Drive => {
                let unwind = std::mem::take(&mut self.unwind);
                if unwind != 0.0 {
                    devices.mouse_motion(unwind, 0.0);
                }
                Step::Play
            }
            Screen::Title => {
                self.orbited += TITLE_ORBIT * seconds;
                Step::Unattended(TITLE_ORBIT * seconds)
            }
            Screen::Pause
            | Screen::Offer
            | Screen::Loading
            | Screen::Shed
            | Screen::Sleep
            | Screen::Buyer => Step::Held,
        };
        if step != Step::Play {
            devices.end_frame();
            devices.clear_movement();
        }
        step
    }

    /// After a frame: sees where the truck has got to, and keeps its best lap.
    pub fn after(&mut self, hud: Option<&Hud>) {
        let Some(hud) = hud else { return };
        if self.screen != Screen::Drive {
            return;
        }
        let at = truck(hud);
        let was_afoot = self.afoot;
        (self.afoot, self.walker, self.truck) = (hud.afoot, hud.walker, at);
        self.speed = hud.velocity.length();
        // Where the truck is, to come back to.
        if self.mode == Mode::Roam {
            let yaw = (-hud.forward.x).atan2(-hud.forward.z).to_degrees();
            self.poses.insert(self.map.clone(), (at, yaw));
        }
        // A place is stopped at in the truck: on foot, wherever the truck was left, there is
        // nothing to enter.
        self.inside = self
            .places
            .iter()
            .position(|place| flat_distance(place.at, at) < place.radius)
            .filter(|_| !self.afoot);
        // The gold. Nuggets are things in the world: where each has got to is taken from
        // where the world says its body is; a nugget in the hand of a driver who has got back
        // in is let go into the tray; the world is asked for the bodies there should be; and
        // every nugget in sight is shown where it is. What the truck carries it carries for
        // real: the nuggets lie on its tray's floor, and weigh on it.
        let boarded = was_afoot && !self.afoot;
        // (A few frames after they get out, once they are standing where they stand.)
        self.trial_out = if self.afoot { self.trial_out + 1 } else { 0 };
        if self.trial && self.trial_out == 6 {
            let ahead = Vec3::new(-hud.walker_yaw.sin(), 0.0, -hud.walker_yaw.cos());
            self.gold.bury(hud.walker + ahead * 3.0);
            log::info!("trial: gold is buried three paces ahead of the driver");
        }
        // A truck put somewhere else all at once, where it was left or back on its track,
        // takes what is aboard it along: no nugget is fast enough to follow.
        let jumped = hud.origin.distance(self.origin) > 4.0;
        (self.walker_yaw, self.origin, self.turned, self.tray) =
            (hud.walker_yaw, hud.origin, hud.rotation, hud.tray);
        let stands = crate::gold::Stands {
            map: &self.map,
            walker: if self.afoot { self.walker } else { at },
            yaw: self.walker_yaw,
            truck: self.origin,
            turned: self.turned,
            tray: self.tray,
        };
        if jumped {
            self.gold.reseat();
        } else {
            self.gold.see(&stands, &self.seen);
        }
        if boarded {
            self.gold.board(&stands);
            self.unsaved = true;
        }
        for body in self.gold.sync(&stands) {
            self.orders.push(Order::Thing(body));
        }
        for shown in self.gold.place(&stands, self.afoot) {
            self.orders.push(Order::Item(shown));
        }
        let carried = self.gold.carried(&stands, self.afoot);
        if self.carried != Some(carried) {
            self.carried = Some(carried);
            self.orders.push(Order::Carry(carried));
        }
        if self.mode == Mode::Laps {
            if let Some(best) = hud.lap.as_ref().and_then(|lap| lap.best) {
                self.lapped(best);
            }
        }
    }

    /// Keeps a lap of the map being played, if it is the best there has been.
    fn lapped(&mut self, seconds: f64) {
        let kept = self.bests.get(&self.map).copied();
        if kept.is_none_or(|kept| seconds < kept - 1.0e-4) {
            self.bests.insert(self.map.clone(), seconds);
            self.unsaved = true;
        }
    }

    /// What the pages should be told now, if anything: all of it whenever the screen changes
    /// or a menu is worked, and otherwise the instruments, a couple of dozen times a second
    /// while driving.
    pub fn tell(&mut self, hud: Option<&Hud>, seconds: f32) -> Option<String> {
        self.since_told += seconds;
        let changed = self.told_screen != Some(self.screen);
        let due = self.screen == Screen::Drive && self.since_told >= TOLD_EVERY;
        if !changed && !due && self.nav.is_empty() {
            return None;
        }
        self.since_told = 0.0;
        if changed {
            log::info!("screen: {}", self.screen.name());
            // Whenever the driving stops, for a menu or a map, where the truck is is kept.
            self.unsaved = true;
        }
        // No two frames take quite as long: that is where chance comes from.
        self.chance = (self.chance ^ u64::from(seconds.to_bits()))
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .rotate_left(23);
        self.kept_for += seconds;
        if self.screen == Screen::Drive && self.kept_for >= KEPT_EVERY {
            self.kept_for = 0.0;
            self.unsaved = true;
        }
        self.told_screen = Some(self.screen);
        let map = self.world.map(&self.map);
        let mut state = json!({
            "screen": self.screen.name(),
            "mode": self.mode.name(),
            "pad": self.pad,
            "nav": std::mem::take(&mut self.nav),
            "map": map.map(|map| self.described(map)),
            "hub": self.map == self.world.hub,
            "day": self.day,
            // The gold: how many nuggets are in the tray and in the chest and what they
            // weigh, what is in the hand, what the detector hears, and what the button
            // would do.
            "tray": self.gold.tray(),
            "chest": self.gold.in_chest(&self.map),
            "money": self.money,
            "hand": self.gold.in_hand().map(|nugget| nugget.kilos),
            // What the driver's hands hold, and where the detector is: in them (`hand`), on
            // their back (`slung`), or put down somewhere (`left`).
            "holding": match (self.gold.in_hand(), &self.gold.detector) {
                (Some(_), _) => "nugget",
                (None, crate::gold::Detector::Hand) => "detector",
                _ => "nothing",
            },
            "detector": match &self.gold.detector {
                crate::gold::Detector::Hand => "hand",
                crate::gold::Detector::Slung => "slung",
                crate::gold::Detector::Left(_) => "left",
            },
            "signal": self.detector(),
            "use": self.afoot.then(|| self.gold.can(&self.stands()).map(crate::gold::Use::name)).flatten(),
            // On foot, and whether beside the truck, where holding the button gets back in.
            "afoot": self.afoot,
            "by_truck": self.afoot && flat_distance(self.walker, self.truck) < GET_IN,
        });
        if let Some(place) = self.offered.and_then(|index| self.places.get(index)) {
            state["offer"] = match self.world.map(&place.map) {
                Some(map) => self.described(map),
                None => json!({ "name": place.map, "title": place.map, "built": false }),
            };
            state["times"] = TIMES.iter().map(|time| time.0).collect();
            state["weathers"] = WEATHERS.iter().map(|weather| weather.0).collect();
            state["time"] = json!(self.time);
            state["weather"] = json!(self.weather);
        }
        // The map: its picture and how much ground it covers, what is marked on it, and
        // where the truck is on it, or the driver on foot, and the way they face, in degrees
        // round from north.
        if let Some((file, half)) = &self.chart {
            state["chart"] = json!({ "file": file, "half": half });
            state["places"] = self
                .places
                .iter()
                .map(|place| {
                    let (title, kind) = match self.world.map(&place.map) {
                        Some(map) => (map.title.as_str(), "trailhead"),
                        None if place.map == SHED => ("The Shed", "shed"),
                        None if place.map == BUYER => ("Gold Buyer", "buyer"),
                        None => (place.map.as_str(), "trailhead"),
                    };
                    json!({ "title": title, "kind": kind, "x": place.at.x, "z": place.at.z })
                })
                .collect();
            if let Some(hud) = hud {
                let (at, heading) = if self.afoot {
                    (self.walker, -self.walker_yaw.to_degrees())
                } else {
                    (truck(hud), hud.forward.x.atan2(-hud.forward.z).to_degrees())
                };
                state["at"] = json!({ "x": at.x, "z": at.z, "heading": heading.round(), "afoot": self.afoot });
            }
        }
        // At the gold buyer's: what is in the tray, what it would fetch, and what was just
        // sold, if anything was.
        if self.screen == Screen::Buyer {
            let (count, kilos) = self.gold.tray();
            state["buyer"] = json!({
                "count": count,
                "kilos": kilos,
                "pays": price(kilos),
                "rate": PRICE,
                "sold": self.sold.map(|(count, kilos, paid)| json!({ "count": count, "kilos": kilos, "paid": paid })),
            });
        }
        if let Some(hud) = hud {
            state["hud"] = instruments(hud);
            if self.mode == Mode::Laps {
                if let Some(lap) = &hud.lap {
                    state["lap"] = json!({
                        "count": lap.count,
                        "time": lap.current,
                        "last": lap.last,
                        "best": lap.best,
                        "valid": lap.valid,
                        "last_valid": lap.last_valid,
                    });
                }
            }
            state["near"] = self.nearest(hud);
        }
        Some(state.to_string())
    }

    /// What the pages are told before there is a map to draw: that the one the game starts
    /// in is on its way. Their loading screen, as when a map is left for another.
    pub fn heading_out(&self) -> String {
        let map = self.world.map(&self.map);
        json!({
            "screen": Screen::Loading.name(),
            "mode": self.mode.name(),
            "pad": false,
            "nav": [],
            "map": map.map(|map| self.described(map)),
            "hub": self.map == self.world.hub,
        })
        .to_string()
    }

    /// A map as the pages are told of it.
    fn described(&self, map: &Map) -> Value {
        json!({
            "name": map.name,
            "title": map.title,
            "about": map.about,
            "kind": map.kind,
            "length": map.length,
            // A map with no length given has no track to time: a road, a town.
            "timed": !map.length.is_empty() && map.name != self.world.hub,
            "best": self.bests.get(&map.name),
            // A map with no scene yet is on the signs, and cannot be driven.
            "built": std::path::Path::new(&map.scene).exists() || cfg!(target_arch = "wasm32"),
        })
    }

    /// The nearest trailhead to the truck: what it leads to, how far off it is, which way
    /// from straight ahead (in degrees, to the right above zero), and whether the truck is in
    /// its ring.
    fn nearest(&self, hud: &Hud) -> Value {
        let at = truck(hud);
        // The place the truck is in; or the nearest trailhead, and the shed as well once
        // there is something in the truck to take back to it.
        let inside = self.inside.and_then(|index| self.places.get(index));
        let Some(place) = inside.or_else(|| {
            self.places
                .iter()
                .filter(|place| (place.map != SHED && place.map != BUYER) || self.gold.tray().0 > 0)
                .min_by(|a, b| flat_distance(a.at, at).total_cmp(&flat_distance(b.at, at)))
        }) else {
            return Value::Null;
        };
        let title = match self.world.map(&place.map) {
            Some(map) => map.title.as_str(),
            None if place.map == SHED => "The Shed",
            None if place.map == BUYER => "Gold Buyer",
            None => place.map.as_str(),
        };
        let to = place.at - at;
        let ahead = hud.forward;
        let bearing = (ahead.x * to.z - ahead.z * to.x).atan2(ahead.x * to.x + ahead.z * to.z);
        let distance = flat_distance(place.at, at);
        json!({
            "title": title,
            "distance": (distance - place.radius).max(0.0).round(),
            "bearing": bearing.to_degrees().round(),
            "inside": distance < place.radius,
            // How far through holding the button down the driver is, from 0 to 1.
            "hold": (self.hold / HOLD).min(1.0),
        })
    }
}

/// Whether any gamepad control bound to a menu action was pressed this frame.
fn pad_pressed(devices: &Devices, menu: &InputMap) -> bool {
    let pad_only = InputMap {
        bindings: menu
            .bindings
            .iter()
            .filter(|b| b.device == solstice_game::Device::Gamepad)
            .cloned()
            .collect(),
    };
    let actions = pad_only.actions(devices, 1.0);
    pad_only.bindings.iter().any(|b| actions.pressed(&b.action))
}

/// Where the truck is: the middle of its wheels.
fn truck(hud: &Hud) -> Vec3 {
    hud.wheels.iter().map(|wheel| wheel.at).sum::<Vec3>() / 4.0
}

/// How far apart two places are, on the map.
fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

/// The truck's instruments, as the pages are told of them.
fn instruments(hud: &Hud) -> Value {
    let t = &hud.telemetry;
    json!({
        "speed": (t.speed.abs() * 3.6).round(),
        "gear": match t.gear {
            -1 => "R".to_owned(),
            0 => "N".to_owned(),
            gear => gear.to_string(),
        },
        "rpm": t.rpm.round(),
        "limit": hud.limit.round(),
        "throttle": (hud.throttle * 100.0).round() / 100.0,
        "brake": (hud.brake * 100.0).round() / 100.0,
        "abs": t.abs_active,
        "traction": t.traction_active,
        "four_wheel_drive": hud.four_wheel_drive,
        "drive": t.drive.name(),
        "front_locked": t.front_locked,
        "rear_locked": t.rear_locked,
        "descent": t.descent_active,
        "aired_down": t.aired_down,
        "pitch": hud.pitch.round(),
        "roll": hud.roll.round(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: &str = "\
hub crossroads
map crossroads assets/hub.solstice The Crossroads
map wombat assets/outback.solstice Wombat Creek Track
about wombat A washed-out fire trail.
kind wombat Fire trail
length wombat 2.4 km
";

    fn shell() -> Shell {
        let mut shell = Shell::new(
            World::parse(WORLD).expect("the world reads"),
            "best wombat 301.5\n",
        );
        shell.enter(&[GameFile {
            name: "hub.places".to_owned(),
            text: "# the trailheads\nplace wombat 100 5 -40 9\n".to_owned(),
        }]);
        shell
    }

    #[test]
    fn gold_is_dug_on_foot_carried_to_the_truck_and_a_night_in_the_shed_ends_the_day() {
        let mut shell = Shell::new(World::parse(WORLD).expect("the world reads"), "");
        shell.map = "wombat".to_owned();
        let track: String = (0..30)
            .map(|i| format!("p {} 5 0 0 0 1 3 3\n", i * 10))
            .collect();
        shell.enter(&[
            GameFile {
                name: "outback.track".to_owned(),
                text: track,
            },
            GameFile {
                name: crate::nugget::FILE.to_owned(),
                text: "slot 700 0\nslot 701 3\n".to_owned(),
            },
        ]);
        shell.act("start");
        shell.take_orders();
        let mut devices = Devices::default();
        // On foot, far from any gold, the detector is quiet; walked over some, it is loud,
        // and the button digs.
        let truck = Vec3::new(-50.0, 5.0, 0.0);
        let mut hud = hud_at(truck, 0.0);
        (hud.afoot, hud.walker, hud.origin) = (true, Vec3::new(-80.0, 5.0, 0.0), truck);
        shell.after(Some(&hud));
        assert_eq!(shell.detector(), Some(0.0));
        let buried = shell.gold.clone();
        let listen = |at: Vec3| {
            buried.signal(&crate::gold::Stands {
                map: "wombat",
                walker: at,
                yaw: 0.0,
                truck,
                turned: glam::Quat::IDENTITY,
                tray: None,
            })
        };
        let over = (0..600)
            .map(|i| Vec3::new(i as f32 * 0.5, 5.0, 0.0))
            .flat_map(|at| [-1.0_f32, 1.0].map(|side| at + Vec3::Z * side))
            .chain((0..3000).map(|i| Vec3::new((i % 60) as f32 * 5.0, 5.0, (i / 60) as f32 - 25.0)))
            .max_by(|a, b| listen(*a).total_cmp(&listen(*b)))
            .expect("somewhere");
        hud.walker = over;
        shell.after(Some(&hud));
        assert!(
            shell.detector().is_some_and(|signal| signal > 0.9),
            "{:?}",
            shell.detector()
        );
        devices.key("KeyE", true);
        shell.before(&mut devices, 0.016);
        // (The game's own frame ends the devices' frame, after which a press is over.)
        devices.end_frame();
        devices.key("KeyE", false);
        shell.before(&mut devices, 0.016);
        assert!(shell.take_orders().contains(&Order::Sound("dig")));
        // The detector is in the driver's hands: the nugget comes out onto the ground, a body
        // the world is asked for, which says where it came to rest.
        shell.after(Some(&hud));
        let orders = shell.take_orders();
        let (dug, lay, turned) = orders
            .into_iter()
            .find_map(|order| match order {
                Order::Thing(crate::gold::Body::Put {
                    id,
                    at,
                    turned,
                    settle: true,
                    ..
                }) => Some((id, at, turned)),
                _ => None,
            })
            .expect("a body is asked for, to settle");
        shell.see(vec![(dug, Vec3::new(lay.x, 5.08, lay.z), turned)]);
        shell.after(Some(&hud));
        let told: Value =
            serde_json::from_str(&shell.tell(Some(&hud), 1.0).expect("told")).expect("JSON");
        assert_eq!(
            (&told["use"], &told["holding"], &told["detector"]),
            (&json!("pick"), &json!("detector"), &json!("hand"))
        );
        // The button picks it up: the detector is slung to do it, and is put out of sight.
        shell.take_orders();
        devices.end_frame();
        devices.key("KeyE", true);
        shell.before(&mut devices, 0.016);
        devices.end_frame();
        devices.key("KeyE", false);
        shell.before(&mut devices, 0.016);
        let told: Value =
            serde_json::from_str(&shell.tell(Some(&hud), 1.0).expect("told")).expect("JSON");
        assert!(
            told["hand"].as_f64().is_some_and(|kilos| kilos >= 0.029),
            "{told}"
        );
        assert_eq!(
            (&told["use"], &told["holding"], &told["detector"]),
            (&json!("drop"), &json!("nugget"), &json!("slung"))
        );
        // Shown in the hand by one of the map's models; and back in the truck, it is let go
        // into the tray: the world is asked for its body there, to fall and lie.
        shell.see(Vec::new());
        shell.after(Some(&hud));
        let orders = shell.take_orders();
        assert!(orders
            .iter()
            .any(|order| matches!(order, Order::Item(shown) if shown.at.is_some())));
        assert!(orders.contains(&Order::Carry(crate::gold::Carried::Away)));
        hud.afoot = false;
        hud.tray = Some((Vec3::new(1.55, 0.545, 1.7), Vec3::new(0.0, 0.905, 1.9)));
        shell.after(Some(&hud));
        let (count, kilos) = shell.gold.tray();
        assert!(count == 1 && kilos >= 0.03);
        let put = shell
            .take_orders()
            .into_iter()
            .find_map(|order| match order {
                Order::Thing(crate::gold::Body::Put {
                    id,
                    at,
                    turned,
                    settle: true,
                    ..
                }) => Some((id, at, turned)),
                _ => None,
            });
        let (id, at, turned) = put.expect("a body is asked for, to settle");
        // The world says where it came to rest, on the tray's floor: it is aboard.
        shell.see(vec![(id, Vec3::new(at.x, truck.y + 0.98, at.z), turned)]);
        shell.after(Some(&hud));
        assert_eq!(shell.gold.tray().0, 1);
        // It is kept with the game.
        assert!(shell.keep_now().contains(" aboard "));
        // The truck put back on its track all at once takes it along: its body is asked
        // for again, where the truck now is.
        shell.take_orders();
        let moved = Vec3::new(120.0, 5.0, 0.0);
        hud.origin = moved;
        shell.after(Some(&hud));
        let orders = shell.take_orders();
        assert!(orders.contains(&Order::Thing(crate::gold::Body::Take { id })));
        assert!(orders.iter().any(|order| matches!(order,
            Order::Thing(crate::gold::Body::Put { at, settle: false, .. }) if at.distance(moved) < 4.0)));
        // At the gold buyer's it is weighed and sold: the money is kept, and the tray is empty.
        shell.places.push(Place {
            map: BUYER.to_owned(),
            at: moved,
            radius: 8.0,
        });
        shell.stop_at(shell.places.len() - 1);
        assert_eq!(shell.screen(), Screen::Buyer);
        let told: Value =
            serde_json::from_str(&shell.tell(Some(&hud), 1.0).expect("told")).expect("JSON");
        assert_eq!(told["buyer"]["count"], 1);
        assert_eq!(told["buyer"]["pays"].as_u64(), Some(price(kilos)));
        shell.act("sell");
        assert_eq!(shell.money(), price(kilos));
        assert_eq!(shell.gold.tray().0, 0);
        let orders = shell.take_orders();
        assert!(orders.contains(&Order::Sound("coins")));
        assert!(orders.contains(&Order::Thing(crate::gold::Body::Take { id })));
        let told: Value =
            serde_json::from_str(&shell.tell(Some(&hud), 1.0).expect("told")).expect("JSON");
        assert_eq!(told["buyer"]["sold"]["paid"].as_u64(), Some(price(kilos)));
        assert_eq!(told["money"].as_u64(), Some(price(kilos)));
        assert!(shell
            .keep_now()
            .contains(&format!("money {}", price(kilos))));
        assert!(
            Shell::new(World::parse(WORLD).expect("reads"), &shell.keep_now()).money()
                == price(kilos)
        );
        shell.act("leave");
        assert_eq!(shell.screen(), Screen::Drive);
        // A night's sleep in the shed is the next day.
        shell.screen = Screen::Shed;
        shell.act("sleep");
        assert_eq!(shell.screen(), Screen::Sleep);
        assert!(shell.take_orders().contains(&Order::Sleep));
        for _ in 0..5 {
            shell.before(&mut devices, 1.0);
        }
        assert_eq!((shell.screen(), shell.day), (Screen::Drive, 2));
    }

    #[test]
    fn holding_the_button_gets_out_and_in_and_a_map_is_come_back_to_where_it_was_left() {
        let mut shell = shell();
        shell.act("start");
        let mut devices = Devices::default();
        // How many times the driver was asked to get out or in, among what else was asked.
        let swaps = |shell: &mut Shell| {
            shell
                .take_orders()
                .iter()
                .filter(|order| **order == Order::Swap)
                .count()
        };
        let hold = |shell: &mut Shell, devices: &mut Devices| {
            devices.key("KeyE", true);
            for _ in 0..40 {
                shell.before(devices, 0.016);
            }
        };
        // Rolling, the driver stays in. At rest, out of any ring, they get out.
        shell.after(Some(&hud_at(Vec3::new(20.0, 5.0, 30.0), 6.0)));
        hold(&mut shell, &mut devices);
        assert_eq!(swaps(&mut shell), 0);
        devices.key("KeyE", false);
        shell.before(&mut devices, 0.016);
        shell.after(Some(&hud_at(Vec3::new(20.0, 5.0, 30.0), 0.0)));
        hold(&mut shell, &mut devices);
        assert_eq!(swaps(&mut shell), 1);
        // Held on, it does nothing more until it is let go.
        hold(&mut shell, &mut devices);
        assert_eq!(swaps(&mut shell), 0);
        devices.key("KeyE", false);
        shell.before(&mut devices, 0.016);
        // On foot: far from the truck nothing; beside it, back in.
        let mut hud = hud_at(Vec3::new(20.0, 5.0, 30.0), 0.0);
        (hud.afoot, hud.walker) = (true, Vec3::new(40.0, 5.0, 30.0));
        shell.after(Some(&hud));
        hold(&mut shell, &mut devices);
        assert_eq!(swaps(&mut shell), 0);
        devices.key("KeyE", false);
        shell.before(&mut devices, 0.016);
        hud.walker = Vec3::new(22.0, 5.0, 30.0);
        shell.after(Some(&hud));
        hold(&mut shell, &mut devices);
        assert_eq!(swaps(&mut shell), 1);
        // Where the truck was is kept with the rest, and a game started from it puts the
        // truck back there when the map is entered.
        let kept = shell.keep_now();
        assert!(kept.contains("at crossroads 20.00 5.00 30.00"), "{kept}");
        let mut again = Shell::new(World::parse(WORLD).expect("the world reads"), &kept);
        again.enter(&[]);
        assert!(again.take_orders().iter().any(|order| matches!(
            order,
            Order::Place { position, .. } if (*position - Vec3::new(20.0, 5.0, 30.0)).length() < 0.01
        )));
    }

    /// A truck's instruments with it at a place, at a speed.
    fn hud_at(at: Vec3, speed: f32) -> Hud {
        let mut hud = Hud::default();
        for wheel in &mut hud.wheels {
            wheel.at = at;
        }
        hud.forward = Vec3::X;
        hud.velocity = Vec3::X * speed;
        hud
    }

    #[test]
    fn a_world_is_its_maps_and_its_hub() {
        let world = World::parse(WORLD).expect("the world reads");
        assert_eq!(world.hub, "crossroads");
        assert_eq!(world.maps.len(), 2);
        let wombat = world.map("wombat").expect("it is there");
        assert_eq!(wombat.title, "Wombat Creek Track");
        assert_eq!(wombat.scene, "assets/outback.solstice");
        assert_eq!(wombat.about, "A washed-out fire trail.");
        assert_eq!(
            (wombat.kind.as_str(), wombat.length.as_str()),
            ("Fire trail", "2.4 km")
        );
        assert!(World::parse("hub nowhere\n").is_err());
        assert!(World::parse("hub a\nmap a x A\nabout b text\n").is_err());
    }

    #[test]
    fn the_title_gives_way_to_driving_and_driving_can_be_paused() {
        let mut shell = shell();
        let mut devices = Devices::new();
        assert_eq!(shell.screen(), Screen::Title);
        assert!(matches!(
            shell.before(&mut devices, 0.016),
            Step::Unattended(_)
        ));
        shell.hear("do start", &mut devices);
        assert_eq!(shell.before(&mut devices, 0.016), Step::Play);
        devices.key("Escape", true);
        assert_eq!(shell.before(&mut devices, 0.016), Step::Held);
        assert_eq!(shell.screen(), Screen::Pause);
        shell.hear("do resume", &mut devices);
        assert_eq!(shell.screen(), Screen::Drive);
    }

    #[test]
    fn stopping_in_a_ring_offers_its_map_and_taking_it_loads_it() {
        let mut shell = shell();
        let mut devices = Devices::new();
        shell.hear("do start", &mut devices);
        // Holding the button outside the ring: nothing.
        shell.after(Some(&hud_at(Vec3::new(130.0, 5.0, -40.0), 0.0)));
        devices.gamepad_button("south", 1.0);
        for _ in 0..60 {
            shell.before(&mut devices, 0.016);
        }
        assert_eq!(shell.screen(), Screen::Drive);
        // In the ring, not holding it: nothing, at any speed.
        devices.gamepad_button("south", 0.0);
        shell.after(Some(&hud_at(Vec3::new(101.0, 5.0, -41.0), 8.0)));
        shell.before(&mut devices, 0.016);
        assert_eq!(shell.screen(), Screen::Drive);
        // Held a moment: not yet. Held for half a second: the map is offered, and the game
        // is held.
        devices.gamepad_button("south", 1.0);
        for _ in 0..10 {
            shell.before(&mut devices, 0.016);
        }
        assert_eq!(shell.screen(), Screen::Drive);
        for _ in 0..25 {
            shell.before(&mut devices, 0.016);
        }
        assert_eq!(shell.screen(), Screen::Offer);
        devices.gamepad_button("south", 0.0);
        assert_eq!(shell.before(&mut devices, 0.016), Step::Held);
        let told: Value =
            serde_json::from_str(&shell.tell(None, 0.016).expect("a new screen is told of"))
                .expect("it is JSON");
        assert_eq!(told["screen"], "offer");
        assert_eq!(told["offer"]["title"], "Wombat Creek Track");
        assert_eq!(told["offer"]["best"], 301.5);
        // Turned down, the driving goes on; clicking the ring's banner offers it again.
        shell.hear("do leave", &mut devices);
        assert_eq!(shell.screen(), Screen::Drive);
        shell.hear("do enter", &mut devices);
        assert_eq!(shell.screen(), Screen::Offer);
        // Taken, for laps, at dusk in a storm.
        shell.hear("set time 2", &mut devices);
        shell.hear("set weather 3", &mut devices);
        shell.hear("do laps", &mut devices);
        assert_eq!(shell.screen(), Screen::Loading);
        // (Among whatever else was asked for on the way.)
        let loads: Vec<Order> = shell
            .take_orders()
            .into_iter()
            .filter(|order| matches!(order, Order::Load { .. }))
            .collect();
        assert_eq!(
            loads,
            vec![Order::Load {
                map: "wombat".to_owned(),
                scene: "assets/outback.solstice".to_owned(),
                sky: Some((17.9, "storm", 900.0)),
            }]
        );
        shell.enter(&[]);
        assert_eq!(shell.screen(), Screen::Drive);
        assert_eq!(shell.map(), "wombat");
        // And from there the pause menu goes back to the hub.
        shell.hear("do pause", &mut devices);
        shell.hear("do hub", &mut devices);
        assert!(
            matches!(&shell.take_orders()[..], [Order::Load { map, sky: None, .. }] if map == "crossroads")
        );
    }

    #[test]
    fn a_better_lap_is_kept_and_a_worse_one_is_not() {
        let mut shell = shell();
        shell.map = "wombat".to_owned();
        // A map entered is kept; after that, nothing is until something changes.
        assert!(shell.to_keep().is_some());
        // 301.5 is the best so far.
        shell.lapped(320.0);
        assert_eq!(shell.to_keep(), None);
        shell.lapped(288.25);
        // With the laps, the day it is, what has been found, and the map it is.
        assert_eq!(
            shell.to_keep().as_deref(),
            Some("best wombat 288.250\nday 1\nmoney 0\ndetector hand\nmap wombat\n")
        );
        assert_eq!(shell.to_keep(), None);
    }

    #[test]
    fn a_menu_worked_from_the_keys_is_told_to_the_pages_once() {
        let mut shell = shell();
        let mut devices = Devices::new();
        let _ = shell.tell(None, 0.016);
        devices.key("ArrowDown", true);
        shell.before(&mut devices, 0.016);
        let told: Value =
            serde_json::from_str(&shell.tell(None, 0.016).expect("a move is told of"))
                .expect("it is JSON");
        assert_eq!(told["nav"], json!(["down"]));
        // Held, it is not pressed again; and with nothing new there is nothing to tell.
        shell.before(&mut devices, 0.016);
        assert_eq!(shell.tell(None, 0.016), None);
    }
}
