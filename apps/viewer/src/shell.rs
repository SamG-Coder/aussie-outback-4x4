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

use crate::cinema::{Cinema, Next, Stage};
use solstice_game::par::RUN_UP;

/// What a menu is worked with when it is not the mouse: the arrows or the left stick or the
/// d-pad to move, Enter or the bottom button to choose, Escape or the right button to go
/// back, Q and E or the bumpers to turn a screen's tabs. Escape and Start pause the driving.
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
menu_prev key KeyQ
menu_prev gamepad l1
menu_next key KeyE
menu_next gamepad r1
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
/// What being put back on the track costs a timed run, each time, in seconds.
pub const PUT_BACK: f64 = 10.0;
/// The hour of an [`Order::Load`] that leaves the clock as it is: no hour at all.
pub const KEEP_HOUR: f32 = -1.0;
/// How often the pages are told what the instruments read, in seconds.
const TOLD_EVERY: f32 = 1.0 / 24.0;

/// How long the driver is told what a blow did to the truck, in seconds.
const BLOW_SAID: f32 = 6.0;

/// A piece of a truck as it is said: `bumper_front` is the front bumper, `lamp_head_R` the
/// right head lamp, `door_FL` the front left door.
fn piece_said(piece: &str) -> String {
    let mut words: Vec<&str> = piece.split('_').filter(|word| !word.is_empty()).collect();
    // Which side, or which corner, is said first.
    let side = match words.last().copied() {
        Some("L") => "left",
        Some("R") => "right",
        Some("FL") => "front left",
        Some("FR") => "front right",
        Some("RL") => "rear left",
        Some("RR") => "rear right",
        _ => "",
    };
    if !side.is_empty() {
        words.pop();
    }
    // `bumper_front` is said the other way about; `acc` only says that it was bought.
    words.retain(|word| *word != "acc");
    if let Some(end) = words
        .iter()
        .position(|word| matches!(*word, "front" | "rear"))
    {
        let end = words.remove(end);
        words.insert(0, end);
    }
    let mut said = side.to_owned();
    for word in words {
        if !said.is_empty() {
            said.push(' ');
        }
        said += word;
    }
    let mut letters = said.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => said,
    }
}

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
    /// How rich its ground is in gold: 1 is ordinary, and the harder a map is to get round
    /// the richer it is (`gold MAP RICH`; `gold::chances`).
    pub gold: f32,
}

/// One of the game's own trucks, that the player can drive in place of the truck its maps
/// have: its pack is `trucks/NAME.truck` beside the maps (`kit.rs`).
#[derive(Clone, Debug, PartialEq)]
pub struct Truck {
    pub name: String,
    /// Its grade, from 1, the cheapest and plainest; what it costs; and the year it is of.
    pub grade: u32,
    pub price: u32,
    pub year: u32,
    pub title: String,
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
    /// The trucks there are to choose from, besides the one the maps have.
    pub trucks: Vec<Truck>,
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
                        gold: 1.0,
                    });
                }
                "truck" => {
                    let mut words = rest.splitn(5, char::is_whitespace);
                    let (Some(name), Some(grade), Some(price), Some(year)) =
                        (words.next(), words.next(), words.next(), words.next())
                    else {
                        return Err(error("expected `truck NAME GRADE PRICE YEAR TITLE`"));
                    };
                    let number = |word: &str| {
                        word.parse::<u32>()
                            .map_err(|_| error(&format!("`{word}` is not a number")))
                    };
                    world.trucks.push(Truck {
                        name: name.to_owned(),
                        grade: number(grade)?,
                        price: number(price)?,
                        year: number(year)?,
                        title: words.next().unwrap_or(name).trim().to_owned(),
                    });
                }
                "gold" => {
                    let (name, rich) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                    let map = world
                        .maps
                        .iter_mut()
                        .find(|map| map.name == name)
                        .ok_or_else(|| error(&format!("there is no map `{name}` yet")))?;
                    map.gold = rich
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|rich| (0.05..=4.0).contains(rich))
                        .ok_or_else(|| error("expected `gold MAP RICH`, from 0.05 to 4"))?;
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
    /// Where a truck that comes in by it is put, and the way it faces, as a yaw in degrees:
    /// a way out at the edge of a map (`edge:MAP`) has one, a little way in along its road.
    pub arrive: Option<(Vec3, f32)>,
}

impl Place {
    /// The map it is the way out to, if it is one at the edge of the map (`edge:MAP`): a
    /// main road runs off the map there, and whoever drives down it is in that map.
    pub fn edge(&self) -> Option<&str> {
        self.map.strip_prefix("edge:")
    }

    /// Whether it is where a truck stands to be looked at (`stand:WHAT`), or where one is
    /// parked in the used-truck yard (`lot:N`): nothing in the game itself, and its fourth
    /// number is the way the truck faces.
    fn stand(&self) -> bool {
        self.map.starts_with("stand:") || self.lot().is_some()
    }

    /// Its place on the day's lot, if it is where a used truck is parked (`lot:N`).
    pub fn lot(&self) -> Option<usize> {
        self.map.strip_prefix("lot:")?.parse().ok()
    }

    /// Whether it is a ring a truck stops in: not a door, nor a way out at the map's edge.
    fn ring(&self) -> bool {
        self.door().is_none() && self.edge().is_none() && !self.stand()
    }

    /// What is behind it, if it is a door that is walked up to (`door:shop`, `door:used`,
    /// `door:bank`): the page of the yard that is opened there.
    pub fn door(&self) -> Option<&'static str> {
        match self.map.strip_prefix("door:")? {
            "shop" => Some("shop"),
            "used" => Some("used"),
            "bank" => Some("bank"),
            "buyer" => Some("buyer"),
            _ => None,
        }
    }
}

/// A used truck as it is parked in the yard: its place on the day's lot, its name, its
/// colour, what is asked for it and how far it has been.
#[derive(Clone, Debug, PartialEq)]
pub struct ForSale {
    pub place: usize,
    pub truck: String,
    pub paint: Option<u32>,
    pub price: u64,
    pub km: f32,
}

/// How near a parked truck's middle somebody on foot is to be looking at it, in metres.
const BY_A_TRUCK: f32 = 4.4;

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
/// The times of day a sleep can be till, and the least a sleep is, in hours: one is offered
/// only if it is that far off.
pub const SLEEPS: [(&str, f32); 5] = [
    ("Morning", MORNING),
    ("Midday", 12.0),
    ("Afternoon", 15.0),
    ("Evening", 18.5),
    ("Night", 22.0),
];
const SLEEP_LEAST: f32 = 6.0;

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
            if let ["place", map, numbers @ ..] = &words[..] {
                let numbers: Vec<f32> = numbers
                    .iter()
                    .filter_map(|word| word.parse().ok())
                    .collect();
                // `X Y Z RADIUS`, and for a way out `X Y Z YAW` after: where it is come in by.
                let arrive = match numbers[..] {
                    [.., x, y, z, yaw] if numbers.len() == 8 => Some((Vec3::new(x, y, z), yaw)),
                    _ => None,
                };
                if let [x, y, z, radius, ..] = numbers[..] {
                    places.push(Place {
                        map: (*map).to_owned(),
                        at: Vec3::new(x, y, z),
                        radius,
                        arrive,
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
    /// Round the track against the clock, lap after lap, with nothing at stake.
    Practice,
    /// One lap from the line, which counts: for a medal, and its purse.
    Timed,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Roam => "roam",
            Self::Practice => "practice",
            Self::Timed => "timed",
        }
    }

    /// From its name. A page written before there were two kinds of timed driving says
    /// `laps`, which was practice.
    fn named(name: &str) -> Self {
        match name {
            "timed" => Self::Timed,
            "practice" | "laps" => Self::Practice,
            _ => Self::Roam,
        }
    }

    /// Whether the track is driven against the clock.
    fn timed(self) -> bool {
        self != Self::Roam
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
    /// On a track, before a timed run or practice: the class, the times to beat and who
    /// set them. The view goes round the truck at the line.
    Lineup,
    /// Another driver's run is being watched: the game drives, and any button skips it.
    Watch,
    /// A timed run is over: its time, its medal, and what it paid.
    Result,
    /// Money and trucks (`yard.rs`): the start of a story, the trucks owned, the bank, the
    /// auto shop and the used yard, a page of it at a time.
    Yard,
}

impl Screen {
    /// From its name, as [`Screen::name`] gives it.
    fn named(name: &str) -> Option<Self> {
        const ALL: [Screen; 12] = [
            Screen::Title,
            Screen::Drive,
            Screen::Pause,
            Screen::Offer,
            Screen::Loading,
            Screen::Shed,
            Screen::Sleep,
            Screen::Buyer,
            Screen::Lineup,
            Screen::Watch,
            Screen::Result,
            Screen::Yard,
        ];
        ALL.into_iter().find(|screen| screen.name() == name)
    }

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
            Self::Lineup => "lineup",
            Self::Watch => "watch",
            Self::Result => "result",
            Self::Yard => "yard",
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
    /// The options have been changed: they are to be kept, and the window made to suit.
    Options,
    /// Take hold of the mouse, to look round the truck with: the page was clicked where
    /// there is nothing to click, while driving.
    Look,
    /// A sleep has been slept: it is this hour of the day, so many days on (none, or one).
    Sleep {
        hours: f32,
        days: u32,
    },
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
    /// Swing a door of the truck open, or shut: the piece of the truck that it is.
    Door {
        piece: String,
        open: bool,
    },
    /// Respray the truck a colour, as a page writes one; or, with none, the colour it was
    /// made in.
    Paint(Option<u32>),
    /// A truck parked in the used-truck yard has been sold: it is gone from its place.
    Unpark(usize),
    /// Put on the truck the dust, mud and rust it had on it when it was last driven.
    Dirt([f32; 3]),
    /// What the truck has hit is on it: its damage, as text (`Damage::to_text`), or
    /// nothing, for a truck that is sound or has been mended.
    Damage(String),
    /// Whether what the truck hits hurts it: a free drive may be driven with damage off.
    Harm(bool),
    /// In the showroom: show another of the trucks that stand there, by its place among
    /// them, in its colour; or fit the one that is shown, where it stands.
    Show {
        index: usize,
        paint: Option<u32>,
    },
    Fit(String),
    /// Another kept game is to be played, a new one begun, or one deleted.
    Saves(Saves),
    /// The driver gets out of the truck, or, on foot beside it, back in.
    Swap,
    /// The driver is on foot, standing here: they have come out of a door.
    Afoot(Vec3),
    /// Put the truck where it was when this map was last left: there, facing that way (a
    /// yaw in degrees).
    Place {
        position: Vec3,
        yaw: f32,
    },
    /// For a timed run, or the watching of somebody else's: put the truck at the line, have
    /// the game drive it, paint it (`cinema::stage` does each).
    Stage(Stage),
}

/// The map that is the showroom, if the game has one: a studio a truck is stood in to be
/// looked at, while the pages of the yard are shown over it.
const SHOWROOM: &str = "showroom";
/// And the town's: the same town, with the truck stood in its yard before the building that
/// what is dealt with is in (the auto shop, the used trucks, the bank).
const SHOWROOM_TOWN: &str = "showroom_town";

/// How far from an open door of the truck the driver has walked off, for it to be shut
/// behind them: in metres.
const WALKED_OFF: f32 = 2.6;

/// What is to be done with the kept games: each is known by its number, from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saves {
    /// Play this one: what is being played is kept first.
    Use(u32),
    /// Begin a new one in this place, and play it: what was kept there is gone.
    New(u32),
    /// Delete this one. If it is the one being played, another is played, or a new one.
    Delete(u32),
    /// Keep the one being played, and stop playing it: the main menu has been gone back to,
    /// where no game is being played.
    Close,
}

/// What a bill is for: what is in the auto shop's cart, a used truck by its place on the
/// lot, or a new one by its name.
#[derive(Clone, Debug, PartialEq)]
enum Deal {
    Cart,
    Used(usize),
    New(String),
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
    /// Whose buttons the pages draw for a controller: the family of the one last seen.
    pad_kind: PadKind,
    /// The game's own actions, as the map being played binds them (its `.input` file), for
    /// the pages to draw the right key or button beside what it does.
    keys: Option<InputMap>,
    /// What the pages were last told is in hand, and whether they have the bindings.
    told_device: Option<(bool, PadKind)>,
    told_binds: bool,
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
    /// The truck's holds, the places in its cab that gold can be put, and its doors, each
    /// open or shut: if it is a truck with a kit (`kit.rs`).
    holds: Vec<crate::gold::Hold>,
    doors: Vec<crate::gold::Door>,
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
    /// Timed runs (docs/campaign.md). The track of the map being played as every class
    /// drives it, if it has one to time; the class chosen, from 0; what has been won; and
    /// each map's gold times as they were last worked out, for its trailhead's card.
    course: Option<crate::timed::Course>,
    class: usize,
    /// The truck the player drives, if it is one of the game's own and not the one the maps
    /// have; whether it is built up, as it is in its pictures, or bare, as it is sold; and
    /// what the scene that is being played was read with.
    drives: Option<String>,
    built: bool,
    /// Whether a free drive's truck is stood on the monster kit: any of the game's trucks,
    /// put together as a monster truck as its scene is read (`kit::assemble_monster`).
    monster: bool,
    worn: Option<(String, String)>,
    /// Money and trucks, in a story; whether it is a story that is being played, or a free
    /// drive; the page of the yard that is shown, the screen it was opened from, and what
    /// was last said there; and the kit of the truck the scene was read with, by its name.
    yard: crate::yard::Yard,
    story: bool,
    yard_page: &'static str,
    yard_from: Screen,
    yard_said: String,
    kit: Option<(String, crate::kit::Kit)>,
    /// The truck that stands in the showroom: which, how it is fitted, and its colour if it
    /// is not the one it was made in. And the map the showroom was come to from.
    show: Option<(String, String, Option<u32>)>,
    show_back: Option<String>,
    /// The trucks that stand in the showroom, as it was read: each its name and how it is
    /// fitted. And which of them is shown.
    stood: Vec<(String, String)>,
    show_index: usize,
    /// How long the showroom has been looked at, in seconds: the view sways as it goes by.
    show_for: f32,
    /// What is on the truck in a free drive, which no truck that is owned keeps.
    free_dirt: [f32; 3],
    /// The kept games there are, each its number and what is said of it; and which of them
    /// is being played.
    saves: Vec<(u32, Option<serde_json::Value>)>,
    /// The options: how loud the game is, from 0 to 10, whether it fills the screen, and
    /// how fast a controller's right stick looks round, from 1 to 10.
    volume: u8,
    fullscreen: bool,
    look: u8,
    /// The hour of the day it is, as whoever runs the clock last said.
    hour: f32,
    /// What the sleep that is being slept is till: its name.
    slept_till: &'static str,
    save: u32,
    /// The grade of trucks a free drive is being chosen from, from 1; and whether, when the
    /// map that is being read has been, the game is at its title and not being driven.
    free_grade: u32,
    /// The map a free drive starts on, if one has been chosen: else the hub.
    free_map: Option<String>,
    /// How a free drive's truck is built: of each group of its kit that is not as it is
    /// sold, the choice (or `none`). And its colour, if it is not the one it was made in.
    /// They are chosen on the page `tune`, with the truck stood there with every piece.
    free_build: Vec<(String, String)>,
    free_paint: Option<u32>,
    /// Whether the hour and the weather chosen for a free drive are yet to be the sky of
    /// the map it starts on; and whether a weather was chosen for it, which is then the
    /// weather wherever it goes.
    free_sky: bool,
    free_weather: bool,
    /// Whether what a free drive's truck hits hurts it. (A story's always does.)
    free_damage: bool,
    /// The colour a first truck is taken in, if one has been chosen: else as it was made.
    start_paint: Option<u32>,
    /// Whether the game is in a browser, which reads one scene a page: what is being done
    /// is carried from each page to the next ([`Shell::carry`]).
    web: bool,
    /// What has been chosen in the auto shop and not yet paid for: parts, each its group
    /// and which of it; a colour; and an oil change. And what a bill is being shown for,
    /// to be accepted or turned down.
    cart: Vec<(String, String)>,
    cart_paint: Option<u32>,
    cart_oil: bool,
    /// Whether mending the truck is in the cart; and what its last blow did, to say to the
    /// driver, with how long it has been said for.
    cart_repair: bool,
    blow: (String, f32),
    quote: Option<Deal>,
    /// Which part of the truck in the showroom is being looked at, if one is: the view
    /// goes to it (a group of the kit's, as `bullbar`).
    focus: Option<String>,
    /// Where the driver has said they are going, on the map: what it is called, and where
    /// it is. The way to it is what is pointed out, until it is come to or another is chosen.
    waypoint: Option<(String, Vec3)>,
    /// The map that was driven out of by its edge, while the next is read: it is come into
    /// by the way that leads back there. And whether the truck has been clear of every way
    /// out since it came into this map: only then does one take it.
    came_from: Option<String>,
    clear_of_edge: bool,
    /// Where the driver stood when they went in at a door: they come out there, on foot.
    door_from: Option<(String, Vec3)>,
    /// The first truck that has been chosen, while its colour is: the second step.
    start_truck: Option<String>,
    to_title: bool,
    records: crate::timed::Records,
    pars: BTreeMap<String, [f32; 5]>,
    /// Whether the run being driven has been seen at the line with no lap behind it; how
    /// the last one ended, while that is being looked at; and the run being watched.
    armed: bool,
    result: Option<crate::timed::Outcome>,
    cinema: Option<Cinema>,
    /// What the game's weather is drawn from: its own, from when it was first started.
    seed: Option<u64>,
}

impl Shell {
    /// A game of these maps, with what was kept of it the last time: the best laps there have
    /// been (`best MAP SECONDS` a line), the day it is, and what has been found.
    pub fn new(world: World, kept: &str) -> Self {
        let mut bests = BTreeMap::new();
        let (mut day, mut money) = (1, 0);
        let (mut drives, mut built) = (None::<String>, false);
        let (mut poses, mut resume) = (BTreeMap::new(), None::<String>);
        let (mut class, mut seed, mut pars) = (0, None, BTreeMap::new());
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
                ["truck", name] => drives = Some(name.to_owned()),
                ["truck", name, "built"] => (drives, built) = (Some(name.to_owned()), true),
                ["at", map, x, y, z, yaw] => {
                    if let (Ok(x), Ok(y), Ok(z), Ok(yaw)) =
                        (x.parse(), y.parse(), z.parse(), yaw.parse())
                    {
                        poses.insert(map.to_owned(), (Vec3::new(x, y, z), yaw));
                    }
                }
                ["map", name] => resume = Some(name.to_owned()),
                // The class last chosen, from 1; the weather's seed; a map's gold times.
                ["class", number] => {
                    class = number
                        .parse::<usize>()
                        .map_or(0, |class| class.clamp(1, 5) - 1);
                }
                ["seed", number] => seed = number.parse().ok(),
                ["par", map, a, b, c, d, e] => {
                    let golds: Vec<f32> = [a, b, c, d, e]
                        .iter()
                        .filter_map(|gold| gold.parse().ok())
                        .collect();
                    if let Ok(golds) = <[f32; 5]>::try_from(golds) {
                        pars.insert(map.to_owned(), golds);
                    }
                }
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
            pad_kind: PadKind::Xbox,
            keys: None,
            told_device: None,
            told_binds: false,
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
            holds: Vec::new(),
            doors: Vec::new(),
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
            course: None,
            class,
            drives,
            built,
            monster: kept.lines().any(|line| line.trim() == "monster"),
            worn: None,
            yard: crate::yard::Yard::read(kept),
            // A story that has begun is what is played, until a free drive is asked for.
            story: crate::yard::Yard::read(kept).begun(),
            yard_page: "garage",
            yard_from: Screen::Title,
            yard_said: String::new(),
            kit: None,
            show: None,
            show_back: None,
            stood: Vec::new(),
            show_index: 0,
            show_for: 0.0,
            free_dirt: [0.0; 3],
            saves: Vec::new(),
            volume: 10,
            fullscreen: false,
            look: 6,
            hour: 12.0,
            slept_till: SLEEPS[0].0,
            save: 0,
            free_grade: 1,
            free_map: None,
            free_build: Vec::new(),
            free_paint: None,
            free_sky: false,
            free_weather: false,
            free_damage: true,
            start_paint: None,
            start_truck: None,
            web: false,
            cart: Vec::new(),
            cart_paint: None,
            cart_oil: false,
            cart_repair: false,
            blow: (String::new(), f32::MAX),
            quote: None,
            focus: None,
            waypoint: None,
            came_from: None,
            clear_of_edge: false,
            door_from: None,
            to_title: false,
            records: crate::timed::Records::read(kept),
            pars,
            armed: false,
            result: None,
            cinema: None,
            seed,
        }
    }

    /// What the game's weather is drawn from.
    fn seed(&self) -> u64 {
        self.seed.unwrap_or(crate::calendar::NO_SEED)
    }

    /// Whether the track is being driven at night, and in the rain: a run then pays more.
    fn conditions(&self) -> (bool, bool) {
        (
            TIMES[self.time].0 == "Night",
            WEATHERS[self.weather].1 == "storm",
        )
    }

    /// Whether a map has a track to time: one with no length given is a road, or a town.
    fn timed(&self, map: &Map) -> bool {
        !map.length.is_empty() && map.name != self.world.hub
    }

    /// Where whoever runs the game is to put the camera, in place of where the game would:
    /// there, looking that way (a yaw and a pitch in radians). While a run is watched.
    pub fn view(&self) -> Option<(Vec3, f32, f32)> {
        (self.screen == Screen::Watch)
            .then(|| self.cinema.as_ref()?.view())
            .flatten()
    }

    /// In the showroom: where the truck that is looked at stands, how it is turned, and how
    /// long it has been looked at. Whoever runs the game puts the camera before it and to
    /// one side, above the ground (`PlayMode::showroom_camera`).
    pub fn showroom(&self) -> Option<(Vec3, glam::Quat, f32)> {
        (self.screen == Screen::Yard && self.in_showroom()).then_some((
            self.origin,
            self.turned,
            self.show_for,
        ))
    }

    /// On the main menu, where no game is being played yet: where the map's truck would
    /// stand, and how long the menu has been up, in seconds. Whoever runs the game does not
    /// draw the truck, and looks slowly round the bush from there (`PlayMode::preview_camera`).
    pub fn preview(&self) -> Option<(Vec3, f32)> {
        (self.screen == Screen::Title).then_some((self.origin, self.orbited / TITLE_ORBIT))
    }

    /// A timed run, or a lap of practice, begins: the truck is put at the line.
    fn start_run(&mut self) {
        self.result = None;
        self.armed = false;
        // The view went round the truck at the line: it starts behind it.
        self.unwind = -std::mem::take(&mut self.orbited);
        self.orders.push(Order::Stage(Stage::Line(RUN_UP)));
        self.screen = Screen::Drive;
    }

    /// One of the class's other drivers is watched round the track.
    fn watch(&mut self, medal: crate::timed::Medal) {
        let Some(course) = &self.course else {
            return;
        };
        let gold = course.golds()[self.class];
        let rival = crate::timed::rivals(&self.map, self.class)[medal as usize - 1].clone();
        log::info!("timed: watching {}, in a {}", rival.name, rival.truck);
        let cinema = Cinema::plan(course, rival, self.class, gold);
        self.orders
            .push(Order::Stage(Stage::Paint(Some(cinema.rival.paint))));
        self.orders
            .push(Order::Stage(Stage::Drive(Some(cinema.pace()))));
        self.orders.push(Order::Stage(cinema.puts()));
        self.cinema = Some(cinema);
        self.screen = Screen::Watch;
    }

    /// The watching is over, or was skipped: the truck is the player's again, at the line.
    fn end_watch(&mut self) {
        self.cinema = None;
        self.orders.push(Order::Stage(Stage::Drive(None)));
        self.orders.push(Order::Stage(Stage::Paint(None)));
        self.orders.push(Order::Stage(Stage::Line(RUN_UP)));
        self.screen = Screen::Lineup;
    }

    /// Follows the truck through the shot being watched.
    fn watching(&mut self, hud: &Hud) {
        let along = hud.lap.as_ref().map_or(0.0, |lap| lap.along);
        let next = match (&self.course, &mut self.cinema) {
            (Some(course), Some(cinema)) => cinema.follow(course, truck(hud), along),
            _ => Next::Over,
        };
        match next {
            Next::Roll => {}
            Next::Cut(stage) => self.orders.push(Order::Stage(stage)),
            Next::Over => self.end_watch(),
        }
    }

    /// A timed run is over, in this many seconds: its medal, and what it pays. Each time the
    /// truck was put back on the track is so many seconds on its time ([`PUT_BACK`]).
    fn finish(&mut self, seconds: f64, valid: bool, resets: u32) {
        let driven = seconds;
        let seconds = seconds + PUT_BACK * f64::from(resets);
        use crate::timed::{grams, payout, Medal, Outcome};
        let Some(course) = &self.course else {
            return;
        };
        let gold = course.golds()[self.class];
        let (night, rain) = self.conditions();
        let won = valid.then(|| Medal::won(seconds, gold)).flatten();
        let before = if valid {
            self.records.take(&self.map, self.class, seconds, won)
        } else {
            self.records.of(&self.map, self.class)
        };
        let paid = payout(gold, self.class, before.medal, won, night, rain);
        // The purse is weighed out in gold and put in the tray, a nugget like any other; a
        // truck with no tray is paid in money.
        let mut weighed = None;
        if paid.dollars > 0 {
            match self.tray {
                Some(tray) => {
                    let grams = grams(paid.dollars, PRICE);
                    self.gold.award(grams as f32 / 1000.0, tray);
                    weighed = Some(grams);
                }
                None => self.money += paid.dollars,
            }
            self.orders.push(Order::Sound("coins"));
        }
        log::info!(
            "timed: class {} round {} in {}{}: {}, ${} paid",
            self.class + 1,
            self.map,
            solstice_game::race::format_time(seconds),
            if resets == 0 {
                String::new()
            } else {
                format!(" (driven in {driven:.1}, put back {resets} times)")
            },
            won.map_or("no medal", Medal::name),
            paid.dollars
        );
        if valid {
            self.lapped(seconds);
        }
        self.result = Some(Outcome {
            class: self.class,
            seconds,
            resets,
            penalty: PUT_BACK * f64::from(resets),
            valid,
            won,
            before,
            paid,
            grams: weighed,
            gold,
            night,
            rain,
        });
        self.armed = false;
        self.unsaved = true;
        self.screen = Screen::Result;
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
            holds: &self.holds,
            doors: &self.doors,
        }
    }

    /// How strong the signal of the detector in the driver's hand is, from 0 to 1: nothing
    /// unless they are out of the truck with it, and the game is being played.
    pub fn detector(&self) -> Option<f32> {
        (self.afoot
            && self.screen == Screen::Drive
            && self.gold.detector == crate::gold::Detector::Hand)
            .then(|| self.gold.heard(&self.stands(), self.walker_yaw).0)
    }

    /// The share of its engine's pull the truck that is driven still has: less, in a story,
    /// if its oil is overdue.
    pub fn power(&self) -> f32 {
        match self.yard.driven().filter(|_| self.story) {
            Some(owned) => owned.power(),
            None => 1.0,
        }
    }

    /// The day's own weather, as the calendar has it: the renderer's name for it, and how
    /// many seconds of it there have been already. What it is at the hub and in a town.
    pub fn today_sky(&self) -> (&'static str, f32) {
        let today = crate::calendar::weather(self.seed(), self.day) as usize;
        let (_, name, soaked) = WEATHERS[today.min(WEATHERS.len() - 1)];
        (name, soaked)
    }

    /// The class the truck that is driven runs in: its grade, a class a grade. None for a
    /// truck that is not one of the game's own, which may be run in any.
    fn class_driven(&self) -> Option<usize> {
        let (name, _) = self.would_wear()?;
        let truck = self.world.trucks.iter().find(|truck| truck.name == name)?;
        let classes = crate::timed::CLASSES.len();
        Some((truck.grade.max(1) as usize - 1).min(classes - 1))
    }

    /// Whether the map has a trailhead that leads back to the hub: a way home to drive.
    fn way_home(&self) -> bool {
        let hub = self.world.hub.as_str();
        self.map == hub
            || self
                .places
                .iter()
                .any(|place| place.map == hub || place.edge() == Some(hub))
    }

    /// The door the driver stands before, on foot, in a story: what is behind it.
    fn at_door(&self) -> Option<&'static str> {
        if !(self.afoot && self.screen == Screen::Drive) {
            return None;
        }
        // The shed is gone into where a truck is parked in it, or a pace or two off.
        let shed = |place: &&Place| {
            place.map == SHED && flat_distance(place.at, self.walker) < place.radius + 2.0
        };
        if self.places.iter().any(|place| shed(&place)) {
            return Some("shed");
        }
        let near = |place: &&Place| flat_distance(place.at, self.walker) < place.radius;
        let door = self.places.iter().filter(near).find_map(Place::door)?;
        // (The gold buyer buys from anybody. The rest is a story's: its trucks, its debt.)
        (self.story || door == "buyer").then_some(door)
    }

    /// The used truck the driver stands by, on foot, in the yard it is parked in: its place
    /// on today's lot, if it is still for sale. In a story: nobody else has money.
    fn at_truck(&self) -> Option<usize> {
        if !(self.afoot && self.story && self.screen == Screen::Drive) {
            return None;
        }
        let near = |place: &&Place| flat_distance(place.at, self.walker) < BY_A_TRUCK;
        let by = |a: &&Place, b: &&Place| {
            flat_distance(a.at, self.walker).total_cmp(&flat_distance(b.at, self.walker))
        };
        let spot = self.places.iter().filter(near).min_by(by)?.lot()?;
        let lot = self.yard.lot(self.seed(), self.day, &self.world.trucks);
        lot.get(spot).filter(|(_, here)| *here).map(|_| spot)
    }

    /// The day's used trucks that are still for sale, to be parked in a map that has a yard
    /// for them: whoever reads a map asks. And whether the one that is looked over is
    /// pulled out of its row, as it is on the page of used trucks.
    pub fn parked(&self) -> (Vec<ForSale>, bool) {
        let lot = self.yard.lot(self.seed(), self.day, &self.world.trucks);
        let known = |name: &str| self.world.trucks.iter().any(|truck| truck.name == name);
        let parked = lot
            .into_iter()
            .enumerate()
            .filter(|(_, (listing, here))| *here && known(&listing.truck))
            .map(|(place, (listing, _))| ForSale {
                place,
                truck: listing.truck,
                paint: Some(listing.paint),
                price: listing.price,
                km: listing.km,
            })
            .collect();
        (parked, self.in_showroom() && self.yard_page == "used")
    }

    /// How far off the gold that detector hears is, in metres, if it hears any.
    fn gold_away(&self) -> Option<f32> {
        self.detector()?;
        self.gold.heard(&self.stands(), self.walker_yaw).1
    }

    /// Whoever runs the world says where the nuggets' bodies are, before [`Shell::after`]:
    /// each its nugget's number, where it is, and how it is turned.
    pub fn see(&mut self, seen: Vec<(u32, Vec3, glam::Quat)>) {
        self.seen = seen;
    }

    /// Says that the game is in a browser: see `web`.
    pub fn set_web(&mut self) {
        self.web = true;
    }

    /// Whether what the truck hits hurts it: always, in a story; in a free drive, unless
    /// damage was switched off.
    fn harm(&self) -> bool {
        self.story || self.free_damage
    }

    /// Which of the kept games is being played: none, at the main menu and in a free drive.
    pub fn playing(&self) -> u32 {
        self.save
    }

    /// What is being done, to be taken up by another shell ([`Shell::take_up`]): a browser
    /// reads a page for each map, and the game that is in this one is gone with it. So as
    /// another map is asked for, what a shell knows that is kept nowhere is written down:
    /// the map that is being gone to and what for, what is on the screen, whether it is a
    /// story and which kept game, the page of the yard that is shown and the truck that
    /// stands there, the door that was gone in at, the edge that was driven out by. A line
    /// each; and after a line `kept`, the game as it is kept ([`Shell::keep_now`]).
    pub fn carry(&mut self) -> String {
        let flag = |on: bool| u8::from(on);
        let mut out = format!(
            "map {}\nscreen {}\nmode {}\nflags {} {} {}\nsave {}\n",
            self.map,
            self.screen.name(),
            self.mode_name(),
            flag(self.story),
            flag(self.to_title),
            flag(self.built),
            self.save
        );
        out += &format!(
            "yard {} {} {} {}\n",
            self.yard_page,
            self.yard_from.name(),
            self.show_index,
            self.free_grade
        );
        let colour =
            |paint: Option<u32>| paint.map_or("-".to_owned(), |paint| format!("{paint:06x}"));
        if let Some((truck, build, paint)) = &self.show {
            out += &format!("show {truck} {build} {}\n", colour(*paint));
        }
        if let Some(back) = &self.show_back {
            out += &format!("back {back}\n");
        }
        if let Some((map, at)) = &self.door_from {
            out += &format!("door {map} {} {} {}\n", at.x, at.y, at.z);
        }
        if let Some(from) = &self.came_from {
            out += &format!("came {from}\n");
        }
        if let Some(map) = &self.free_map {
            out += &format!("free_map {map}\n");
        }
        if !self.free_build.is_empty() {
            // (All of it: what a monster truck does not wear is its own again when it is
            // not one.)
            let each = self.free_build.iter();
            let words: Vec<String> = each.map(|(g, c)| format!("{g}={c}")).collect();
            out += &format!("free_build {}\n", words.join(","));
        }
        if let Some(paint) = self.free_paint {
            out += &format!("free_paint {paint:06x}\n");
        }
        if self.free_weather {
            out += "free_weather\n";
        }
        if !self.free_damage {
            out += "free_unhurt\n";
        }
        if let Some(truck) = &self.start_truck {
            out += &format!("start {truck}\n");
        }
        if self.start_paint.is_some() {
            out += &format!("start_paint {}\n", colour(self.start_paint));
        }
        if let Some(truck) = &self.drives {
            out += &format!("drives {truck}\n");
        }
        if self.monster {
            out += "monster\n";
        }
        let [dust, mud, rust] = self.free_dirt;
        out += &format!(
            "chance {}\noptions {} {}\ndirt {dust} {mud} {rust}\nkept\n",
            self.chance, self.volume, self.look
        );
        out += &self.keep_now();
        out
    }

    /// The game as another shell kept it when it wrote down what it was doing: what follows
    /// the line `kept` of what [`Shell::carry`] wrote, if anything was written.
    pub fn carried_kept(carried: &str) -> Option<&str> {
        if carried.starts_with("kept\n") {
            return Some(&carried[5..]);
        }
        carried
            .find("\nkept\n")
            .map(|at| &carried[at + "\nkept\n".len()..])
    }

    /// Takes up what another shell was doing, from what it wrote down ([`Shell::carry`]).
    /// This one was made with the game as it is kept; the map that is being read is the one
    /// that shell was going to, and when it has been read ([`Shell::enter`]) the game goes
    /// on as it would have there.
    pub fn take_up(&mut self, carried: &str) {
        let number = |word: &str| word.parse::<f32>().ok();
        let colour = |word: &str| u32::from_str_radix(word, 16).ok();
        for line in carried.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words[..] {
                ["kept"] => break,
                ["map", map] => self.start_in(map),
                ["screen", screen] => {
                    self.screen = Screen::named(screen).unwrap_or(Screen::Loading);
                }
                ["mode", mode] => {
                    let screen = self.screen;
                    self.resume(mode);
                    self.screen = screen;
                }
                ["flags", story, to_title, built] => {
                    (self.story, self.to_title, self.built) =
                        (story == "1", to_title == "1", built == "1");
                }
                ["save", save] => self.save = save.parse().unwrap_or(0),
                ["yard", page, from, index, grade] => {
                    const PAGES: [&str; 10] = [
                        "start", "colour", "used", "garage", "free", "dealer", "shop", "bank",
                        "tune", "where",
                    ];
                    if let Some(page) = PAGES.into_iter().find(|known| *known == page) {
                        self.yard_page = page;
                    }
                    self.yard_from = Screen::named(from).unwrap_or(Screen::Title);
                    self.show_index = index.parse().unwrap_or(0);
                    self.free_grade = grade.parse().unwrap_or(1);
                }
                ["show", truck, build, paint] => {
                    self.show = Some((truck.to_owned(), build.to_owned(), colour(paint)));
                }
                ["back", map] => self.show_back = Some(map.to_owned()),
                ["door", map, x, y, z] => {
                    if let (Some(x), Some(y), Some(z)) = (number(x), number(y), number(z)) {
                        self.door_from = Some((map.to_owned(), Vec3::new(x, y, z)));
                    }
                }
                ["came", map] => self.came_from = Some(map.to_owned()),
                ["free_map", map] => self.free_map = Some(map.to_owned()),
                ["free_build", build] => {
                    let each = build.split(',').filter_map(|word| word.split_once('='));
                    self.free_build = each.map(|(g, c)| (g.to_owned(), c.to_owned())).collect();
                }
                ["free_paint", paint] => self.free_paint = colour(paint),
                ["free_weather"] => self.free_weather = true,
                ["free_unhurt"] => self.free_damage = false,
                ["start", truck] => self.start_truck = Some(truck.to_owned()),
                ["start_paint", paint] => self.start_paint = colour(paint),
                ["drives", truck] => self.drives = Some(truck.to_owned()),
                ["monster"] => self.monster = true,
                ["chance", chance] => self.chance = chance.parse().unwrap_or(self.chance),
                ["options", volume, look] => {
                    self.volume = volume.parse().unwrap_or(self.volume).min(10);
                    self.look = look.parse().unwrap_or(self.look).clamp(1, 10);
                }
                ["dirt", dust, mud, rust] => {
                    if let (Some(dust), Some(mud), Some(rust)) =
                        (number(dust), number(mud), number(rust))
                    {
                        self.free_dirt = [dust, mud, rust];
                    }
                }
                _ => {}
            }
        }
        self.told_screen = None;
    }

    /// The trucks whose packs the scene that is to be read is read with, by their names:
    /// those that stand in a showroom, or the one that is driven; and those parked in the
    /// used-truck yard, if the map is the one the yard is in. For whoever has to fetch them
    /// before the scene can be read.
    pub fn packs(&self) -> Vec<String> {
        let wanted: Vec<(String, String)> = if self.in_showroom() {
            let page = self.page_trucks().into_iter();
            page.map(|(truck, build, _)| (truck, build)).collect()
        } else {
            self.would_wear().into_iter().collect()
        };
        // (A monster truck stands on the monster kit, which is a pack like a truck's.)
        let monster = |(_, build): &(String, String)| build.starts_with(crate::kit::MONSTER_BUILD);
        let kit = wanted
            .iter()
            .any(monster)
            .then(|| crate::kit::MONSTER.to_owned());
        let mut names: Vec<String> = wanted
            .into_iter()
            .map(|(truck, _)| truck)
            .chain(kit)
            .collect();
        // (The yard is in the town, which is the map its showroom is too.)
        let scene = |map: &str| self.world.map(map).map(|map| map.scene.clone());
        if scene(SHOWROOM_TOWN).is_some() && scene(SHOWROOM_TOWN) == scene(&self.map) {
            names.extend(self.parked().0.into_iter().map(|sale| sale.truck));
        }
        names.sort();
        names.dedup();
        names
    }

    /// The truck that is driven, by its name, if it is one of a story's.
    pub fn driven(&self) -> Option<String> {
        let owned = self.yard.driven().filter(|_| self.story);
        owned.map(|owned| owned.truck.clone())
    }

    /// Says what kit the truck that is driven has, where the scene it was last seen in is
    /// gone: a browser's page, which has the truck's pack before it reads its scene. What
    /// the truck is fitted with is said with it ([`Shell::wearing`]).
    pub fn know_kit(&mut self, truck: &str, kit: crate::kit::Kit) {
        if self.yard.driven().is_some_and(|owned| owned.truck == truck) {
            self.kit = Some((truck.to_owned(), kit));
        }
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
            // The weather there is today is the one picked, until another is.
            self.weather = crate::calendar::weather(self.seed(), self.day) as usize;
            // And the class is the truck's own.
            if let Some(class) = self.class_driven() {
                self.class = class;
            }
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
        // (Where the driver was going was somewhere on the map that has been left.)
        self.waypoint = None;
        // The map's gold: scattered now, if it has not been come into before or it is time
        // it was scattered again; else as it was left. (A showroom has none.)
        let ground = (!self.in_showroom()).then(|| self.map.clone());
        let rich = self.world.map(&self.map).map_or(1.0, |map| map.gold);
        self.gold
            .enter(files, ground.as_deref(), self.day, self.chance, rich);
        self.seen.clear();
        self.carried = None;
        // The truck's holds and doors, if it is one built of pieces with a cab to put
        // things in. Its doors are shut.
        let fitted = crate::kit::Fitted::find(files).ok().flatten();
        (self.holds, self.doors) = fitted
            .as_ref()
            .map_or_else(Default::default, |fitted| fitted.kit.holds());
        // Its kit is kept, by the name of the truck that the scene was read with: and in a
        // story the truck that is driven is given what it was sold with, and its colour.
        self.kit = match (&self.worn, fitted) {
            (Some((name, _)), Some(fitted)) => Some((name.clone(), fitted.kit)),
            _ => None,
        };
        // (Its own kit: not that of another truck that is being looked at.)
        let driven = self.yard.driven().map(|owned| owned.truck.clone());
        if self.story || self.in_showroom() {
            if let Some((_, kit)) = self
                .kit
                .as_ref()
                .filter(|(of, _)| Some(of) == driven.as_ref())
            {
                self.yard.stocked(kit);
            }
        }
        // What was on the truck when it was last driven is on it still.
        let dirt = match self.yard.driven().filter(|_| self.story) {
            Some(owned) => owned.dirt,
            None => self.free_dirt,
        };
        if !self.in_showroom() {
            self.orders.push(Order::Dirt(dirt));
            // And what it has hit: a truck of one's own stays hurt until it is mended.
            let damage = self.yard.driven().filter(|_| self.story);
            self.orders.push(Order::Damage(
                damage.map_or(String::new(), |owned| owned.damage.clone()),
            ));
            self.orders.push(Order::Harm(self.harm()));
        }
        if self.in_showroom() {
            // The trucks that stand there, and the one of them that is shown: with the kit
            // of the one that is driven, if it is among them, for the auto shop.
            let stood = crate::kit::displayed(files);
            let driven = self.yard.driven().map(|owned| owned.truck.clone());
            let at = match self.yard_page {
                "garage" => self.yard.driving,
                _ => 0,
            };
            self.kit = match (driven, stood.into_iter().nth(at)) {
                (Some(name), Some(fitted))
                    if self.stood.get(at).is_some_and(|(of, _)| *of == name) =>
                {
                    Some((name, fitted.kit))
                }
                _ => None,
            };
            if let Some((_, kit)) = &self.kit {
                self.yard.stocked(kit);
            }
            // A free drive's truck, being built: its kit is the one that is chosen from, and
            // it is fitted and painted as it has been built so far.
            if matches!(self.yard_page, "tune" | "where") {
                let stood = crate::kit::displayed(files).into_iter().next();
                self.kit = self.drives.clone().zip(stood.map(|fitted| fitted.kit));
                if self.kit.is_some() {
                    self.orders.push(Order::Fit(self.free_worn()));
                }
            }
            // In the auto shop it stands with every piece of it: it is fitted as it is.
            if self.yard_page == "shop" {
                if let (Some(owned), Some((_, kit))) = (self.yard.driven(), &self.kit) {
                    self.orders.push(Order::Fit(owned.build_text(Some(kit))));
                }
            }
            self.orders.push(Order::Show {
                index: self.show_index,
                paint: self.show.as_ref().and_then(|show| show.2),
            });
        } else if self.story {
            self.orders.push(Order::Paint(
                self.yard.driven().and_then(|owned| owned.paint),
            ));
        } else if self.free_paint.is_some() {
            // A free drive's truck is in the colour it was built in.
            self.orders.push(Order::Paint(self.free_paint));
        }
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
        // A track to time is worked out now, for every class: its medals are from the track
        // as it is. The gold times are kept, for its trailhead's card.
        let timed = self.world.map(&self.map).is_some_and(|map| self.timed(map));
        self.course = timed.then(|| crate::timed::Course::read(files)).flatten();
        (self.result, self.cinema, self.armed) = (None, None, false);
        if let Some(course) = &self.course {
            let golds = course.golds();
            log::info!(
                "timed: {} is {:.0} m round; gold, class by class: {}",
                self.map,
                course.track.length(),
                golds
                    .map(|gold| solstice_game::race::format_time(f64::from(gold)))
                    .join(" ")
            );
            for (class, profile) in course.profiles.iter().enumerate() {
                log::debug!(
                    "timed: class {}: {:.0}% of the lap under 3 m/s, {:.0}% under 6, {:.0}% under 10",
                    class + 1,
                    profile.share_below(3.0) * 100.0,
                    profile.share_below(6.0) * 100.0,
                    profile.share_below(10.0) * 100.0
                );
            }
            self.pars.insert(self.map.clone(), golds);
        }
        // A map come into by its edge is come into there, on its road, facing in. Otherwise,
        // roaming, it is come back to where it was left.
        self.clear_of_edge = false;
        let from = self.came_from.take();
        let by_edge = self
            .places
            .iter()
            .find(|place| place.edge().is_some() && place.edge() == from.as_deref())
            .and_then(|place| place.arrive);
        if let Some((position, yaw)) = by_edge {
            self.orders.push(Order::Place { position, yaw });
        } else if self.mode == Mode::Roam {
            if let Some(&(position, yaw)) = self.poses.get(&self.map) {
                self.orders.push(Order::Place { position, yaw });
            }
        }
        // And a driver who went in at a door of it comes out at the door, on foot.
        let out = !self.in_showroom();
        if let Some((map, at)) = self.door_from.take_if(|_| out) {
            if map == self.map {
                self.orders.push(Order::Afoot(at));
            }
        }
        self.unsaved = true;
        if self.screen == Screen::Loading {
            self.screen = if std::mem::take(&mut self.to_title) {
                Screen::Title
            } else {
                Screen::Drive
            };
        }
        // Before a timed run, or practice: the line-up.
        if self.screen == Screen::Drive && self.mode.timed() && self.course.is_some() {
            self.orbited = 0.0;
            self.screen = Screen::Lineup;
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
        self.yard.keep(&mut kept);
        if let Some(truck) = &self.drives {
            kept += &format!("truck {truck}{}\n", if self.built { " built" } else { "" });
        }
        if self.monster {
            kept += "monster\n";
        }
        self.gold.keep(&mut kept);
        // (The showroom is not a place a game is left in: the map it was come to from is.)
        let map = match (&self.show_back, self.in_showroom()) {
            (Some(back), true) => back,
            (None, true) => &self.world.hub,
            _ => &self.map,
        };
        kept += &format!("map {map}\n");
        for (map, (at, yaw)) in &self.poses {
            kept += &format!("at {map} {:.2} {:.2} {:.2} {yaw:.1}\n", at.x, at.y, at.z);
        }
        // Timed runs: what has been won, each map's gold times, and the class last chosen.
        self.records.keep(&mut kept);
        for (map, golds) in &self.pars {
            let golds = golds.map(|gold| format!("{gold:.1}")).join(" ");
            kept += &format!("par {map} {golds}\n");
        }
        if self.class != 0 {
            kept += &format!("class {}\n", self.class + 1);
        }
        if let Some(seed) = self.seed {
            kept += &format!("seed {seed}\n");
        }
        Some(kept)
    }

    /// The truck the next scene is to be read with, and how it is fitted: one of the game's
    /// own, by its name, and `showroom` or `stock`; or none, for the truck the maps have.
    /// Whoever reads the scene asks, and what is said is what the scene is then taken to
    /// have been read with.
    pub fn wearing(&mut self) -> Option<(String, String)> {
        self.worn = self.would_wear();
        self.worn.clone()
    }

    /// The truck a scene read now would be read with, and how it is fitted. In a story it
    /// is the one of the trucks owned that is driven, with what is fitted to it; in a free
    /// drive, the one chosen at the title, bare or built up.
    fn would_wear(&self) -> Option<(String, String)> {
        // (One that was kept and is no longer among the game's trucks is not driven.)
        let known = |name: &str| self.world.trucks.iter().any(|truck| truck.name == name);
        // In the showroom nothing is driven: the trucks that stand there are looked at.
        if self.in_showroom() {
            return None;
        }
        if self.story {
            if let Some(owned) = self.yard.driven().filter(|owned| known(&owned.truck)) {
                let kit = self.kit.as_ref().filter(|(of, _)| *of == owned.truck);
                return Some((
                    owned.truck.clone(),
                    owned.build_text(kit.map(|(_, kit)| kit)),
                ));
            }
        }
        // A free drive is in the truck chosen at the title: the first of the game's trucks,
        // if none has been.
        let fit = self.free_fit(&self.free_worn());
        let free = self.drives.clone().filter(|name| known(name));
        let free = free.or_else(|| self.world.trucks.first().map(|truck| truck.name.clone()));
        free.map(|truck| (truck, fit))
    }

    /// How a free drive's truck is fitted, said so that whoever puts it together knows
    /// whether it is to be a monster truck: `monster:` before it, if it is.
    fn free_fit(&self, fit: &str) -> String {
        let monster = if self.monster {
            crate::kit::MONSTER_BUILD
        } else {
            ""
        };
        format!("{monster}{fit}")
    }

    /// How a free drive's truck was built on the page `tune`, as a build is written:
    /// `bullbar=tube,rack=none`. A monster truck stands on the monster kit's own springs,
    /// tyres and rims, so what was chosen of those for the truck is left out.
    fn free_text(&self) -> String {
        let under = |group: &str| self.monster && matches!(group, "suspension" | "tyres" | "rims");
        let each = self.free_build.iter().filter(|(group, _)| !under(group));
        let words: Vec<String> = each
            .map(|(group, choice)| format!("{group}={choice}"))
            .collect();
        words.join(",")
    }

    /// What a free drive's truck wears: as it was built, if it was; else as it is in its
    /// pictures, or as it is sold.
    fn free_worn(&self) -> String {
        match self.free_text() {
            built if !built.is_empty() => built,
            _ if self.built => "showroom".to_owned(),
            _ => "stock".to_owned(),
        }
    }

    /// What a free drive's truck has fitted of a group of its kit, as it is built: what was
    /// chosen, or else what it is sold with. Nothing, for a group with nothing on it.
    fn free_fitted<'a>(&'a self, kit: &'a crate::kit::Kit, group: &str) -> Option<&'a str> {
        let chosen = self.free_build.iter().find(|(named, _)| named == group);
        match chosen {
            Some((_, choice)) => Some(choice.as_str()).filter(|choice| *choice != "none"),
            None => kit.stock.fitted(group),
        }
    }

    /// Opens a page of the yard, from the screen that is shown.
    fn open_yard(&mut self, page: &'static str) {
        self.open_yard_at(page, 0);
    }

    /// As [`Self::open_yard`], at one of the page's trucks: its place among them.
    fn open_yard_at(&mut self, page: &'static str, index: usize) {
        if self.screen != Screen::Yard {
            self.yard_from = self.screen;
        }
        self.yard_page = page;
        self.yard_said.clear();
        self.screen = Screen::Yard;
        self.told_screen = None;
        self.show_card(index);
    }

    /// The trucks of the page of the yard that is shown, each its name, how it is fitted
    /// and its colour: its own row of trucks, or, on a page with none, the one driven. In
    /// the auto shop the one driven has every piece of its kit with it (`all`), to be tried.
    fn page_trucks(&self) -> Vec<(String, String, Option<u32>)> {
        let trucks = &self.world.trucks;
        let owned = |owned: &crate::yard::Owned, all: bool| {
            let kit = self.kit.as_ref().filter(|(of, _)| *of == owned.truck);
            let build = if all {
                "all".to_owned()
            } else {
                owned.build_text(kit.map(|(_, kit)| kit))
            };
            (owned.truck.clone(), build, owned.paint)
        };
        match self.yard_page {
            // (Its colour is chosen with the same trucks standing there: nothing is read.)
            "start" | "colour" => {
                let offered = crate::yard::starters(trucks).into_iter();
                offered
                    .map(|truck| (truck.name.clone(), "stock".to_owned(), self.start_paint))
                    .collect()
            }
            "used" => {
                let lot = self.yard.lot(self.seed(), self.day, trucks).into_iter();
                lot.map(|(listing, _)| (listing.truck, "stock".to_owned(), Some(listing.paint)))
                    .collect()
            }
            "garage" => self
                .yard
                .trucks
                .iter()
                .map(|truck| owned(truck, false))
                .collect(),
            "free" | "dealer" => {
                // (A free drive's are looked at as they would be driven: as monster trucks,
                // if that is what is being chosen.)
                let fit = match self.yard_page {
                    "free" => self.free_fit("stock"),
                    _ => "stock".to_owned(),
                };
                let grade = trucks.iter().filter(|truck| truck.grade == self.free_grade);
                grade
                    .map(|truck| (truck.name.clone(), fit.clone(), None))
                    .collect()
            }
            "shop" => self
                .yard
                .driven()
                .map(|truck| owned(truck, true))
                .into_iter()
                .collect(),
            // A free drive's truck being built, and where and when it is to be driven being
            // chosen: it stands there by itself, with every piece of it.
            "tune" | "where" => {
                let truck = self.drives.clone().into_iter();
                truck
                    .map(|truck| (truck, self.free_fit("all"), self.free_paint))
                    .collect()
            }
            _ => self
                .yard
                .driven()
                .map(|truck| owned(truck, false))
                .into_iter()
                .collect(),
        }
    }

    /// The trucks the showroom is to be read with, when it is: whoever reads it asks.
    pub fn showing(&mut self) -> Vec<(String, String)> {
        if !self.in_showroom() {
            self.stood.clear();
            return Vec::new();
        }
        let page = self.page_trucks().into_iter();
        self.stood = page.map(|(truck, build, _)| (truck, build)).collect();
        self.stood.clone()
    }

    /// Whether the map is a showroom: the one by the shed, or the town's.
    fn in_showroom(&self) -> bool {
        self.map == SHOWROOM || self.map == SHOWROOM_TOWN
    }

    /// The showroom the page of the yard that is shown is looked at in: the town's, for what
    /// is done in the town, if the game has one; else the one by the shed.
    fn showroom_for(&self) -> &'static str {
        let town = matches!(self.yard_page, "shop" | "used" | "dealer" | "bank");
        if town && self.world.map(SHOWROOM_TOWN).is_some() {
            SHOWROOM_TOWN
        } else {
            SHOWROOM
        }
    }

    /// Where in the town's showroom the trucks stand: before the building of the page that
    /// is shown (`place stand:WHAT` of the town's places). None by the shed, where they
    /// stand where the map's own truck does.
    pub fn stand(&self) -> Option<&'static str> {
        // (New trucks stand where the used ones do.)
        let page = if self.yard_page == "dealer" {
            "used"
        } else {
            self.yard_page
        };
        (self.map == SHOWROOM_TOWN).then_some(page)
    }

    /// Which part of the truck in the showroom the view is to go to, if any.
    pub fn focus(&self) -> Option<&str> {
        self.focus.as_deref().filter(|_| self.in_showroom())
    }

    /// The truck that is driven, as it would be with what is in the cart fitted to it.
    fn cart_build(&self) -> Option<String> {
        let owned = self.yard.driven()?;
        let kit = self.kit.as_ref().filter(|(of, _)| *of == owned.truck);
        let mut tried = owned.clone();
        for (group, choice) in &self.cart {
            tried.fitted.set(group, Some(choice));
        }
        Some(tried.build_text(kit.map(|(_, kit)| kit)))
    }

    /// What a bill is for, a line each, and what each costs.
    fn deal_lines(&self, deal: &Deal) -> Vec<(serde_json::Value, u64)> {
        let trucks = &self.world.trucks;
        match deal {
            Deal::Cart => {
                let grade = self
                    .yard
                    .driven()
                    .and_then(|owned| trucks.iter().find(|truck| truck.name == owned.truck))
                    .map_or(1, |truck| truck.grade);
                let mut lines: Vec<_> = self
                    .cart
                    .iter()
                    .map(|(group, choice)| {
                        (
                            json!({ "group": group, "choice": choice }),
                            crate::yard::part_price(group, choice, grade),
                        )
                    })
                    .collect();
                if let Some(colour) = self.cart_paint {
                    lines.push((
                        json!({ "paint": format!("{colour:06x}") }),
                        crate::yard::RESPRAY,
                    ));
                }
                if self.cart_oil {
                    lines.push((json!({ "oil": true }), crate::yard::OIL_CHANGE));
                }
                let (hurt, cost) = self.repairs();
                if self.cart_repair && hurt > 0 {
                    lines.push((json!({ "repair": hurt }), cost));
                }
                lines
            }
            Deal::Used(place) => {
                let lot = self.yard.lot(self.seed(), self.day, trucks);
                let listed = lot.get(*place).filter(|(_, here)| *here);
                let line = listed.and_then(|(listing, _)| {
                    let truck = trucks.iter().find(|truck| truck.name == listing.truck)?;
                    Some((json!({ "truck": truck.title, "used": true }), listing.price))
                });
                line.into_iter().collect()
            }
            Deal::New(name) => {
                let truck = trucks.iter().find(|truck| truck.name == *name);
                let line =
                    truck.map(|truck| (json!({ "truck": truck.title }), u64::from(truck.price)));
                line.into_iter().collect()
            }
        }
    }

    /// The cart, as the pages are told of it.
    fn cart_told(&self) -> serde_json::Value {
        let lines = self.deal_lines(&Deal::Cart);
        json!({
            "count": lines.len(),
            "total": lines.iter().map(|(_, price)| price).sum::<u64>(),
            "paint": self.cart_paint.map(|colour| format!("{colour:06x}")),
            "oil": self.cart_oil,
            "repair": self.cart_repair,
        })
    }

    /// What mending the truck that is driven would be: how many of its pieces are hurt,
    /// and what it costs, each piece by how far gone it is (`yard::REPAIRS`).
    fn repairs(&self) -> (usize, u64) {
        let Some(owned) = self.yard.driven() else {
            return (0, 0);
        };
        let Ok(damage) = crate::damage::Damage::parse(&owned.damage) else {
            return (0, 0);
        };
        let table = crate::damage::Table::built_in();
        let hurt = damage.list(&table);
        let cost = hurt
            .iter()
            .map(|(_, stage)| match stage {
                crate::damage::Stage::Sound => 0,
                crate::damage::Stage::Scuffed => crate::yard::REPAIRS[0],
                crate::damage::Stage::Dented => crate::yard::REPAIRS[1],
                crate::damage::Stage::Loose => crate::yard::REPAIRS[2],
                crate::damage::Stage::Off => crate::yard::REPAIRS[3],
            })
            .sum();
        (hurt.len(), cost)
    }

    /// The bill that is being shown, if one is: what it is for, what it comes to, what of
    /// that is paid from hand and what goes on the debt, and what is owed and comes out
    /// each Friday now and after. Or why the bank will not have it.
    fn quote_told(&self) -> serde_json::Value {
        let Some(deal) = &self.quote else {
            return serde_json::Value::Null;
        };
        let lines = self.deal_lines(deal);
        let total: u64 = lines.iter().map(|(_, price)| price).sum();
        let quote = self.yard.quote(self.money, total);
        json!({
            "what": match deal { Deal::Cart => "cart", Deal::Used(_) => "used", Deal::New(_) => "new" },
            "items": lines.iter().map(|(what, price)| {
                let mut line = what.clone();
                line["price"] = json!(price);
                line
            }).collect::<Vec<_>>(),
            "total": total,
            "money": self.money,
            "debt": self.yard.debt,
            "repayment": self.yard.repayment(),
            "interest_year": (self.yard.rate() * 52.0 * 1000.0).round() / 10.0,
            "limit": self.yard.limit(),
            "ok": quote.is_ok(),
            "cash": quote.map_or(0, |quote| quote.cash),
            "financed": quote.map_or(0, |quote| quote.financed),
            "debt_after": quote.map_or(self.yard.debt, |quote| quote.debt),
            "repayment_after": quote.map_or(self.yard.repayment(), |quote| quote.repayment),
        })
    }

    /// Shows a truck in the showroom: one of those on the page of the yard that is shown,
    /// by its place among them. The showroom is gone to, if the game has one and is not in
    /// it, and is read with every truck of the page; after that another of them is shown at
    /// once, with nothing read.
    fn show_card(&mut self, index: usize) {
        let page = self.page_trucks();
        if page.is_empty() {
            return;
        }
        let room = self.showroom_for();
        let Some(scene) = self.world.map(room).map(|map| map.scene.clone()) else {
            return;
        };
        let index = index.min(page.len() - 1);
        self.show_index = index;
        self.show = Some(page[index].clone());
        let stand: Vec<(String, String)> = page
            .iter()
            .map(|(truck, build, _)| (truck.clone(), build.clone()))
            .collect();
        // The same trucks stand there already: the one asked for is shown.
        let fitted = |stood: &(String, String), wanted: &(String, String)| {
            // (One that stands with every piece of it can be fitted any way where it
            // stands; but a monster truck is another truck than the one it is made of.)
            let monster = |build: &str| build.starts_with(crate::kit::MONSTER_BUILD);
            let every = stood
                .1
                .strip_prefix(crate::kit::MONSTER_BUILD)
                .unwrap_or(&stood.1)
                == "all";
            stood.0 == wanted.0
                && (stood.1 == wanted.1 || (every && monster(&stood.1) == monster(&wanted.1)))
        };
        let same = self.map == room
            && self.stood.len() == stand.len()
            && self
                .stood
                .iter()
                .zip(&stand)
                .all(|(stood, wanted)| fitted(stood, wanted));
        if same {
            self.orders.push(Order::Show {
                index,
                paint: page[index].2,
            });
            return;
        }
        if !self.in_showroom() {
            self.show_back = Some(self.map.clone());
        }
        self.map = room.to_owned();
        // (At whatever hour it has got to, and fair: a truck is looked at in the dry.)
        self.orders.push(Order::Load {
            map: room.to_owned(),
            scene,
            sky: Some((KEEP_HOUR, "fair", 0.0)),
        });
    }

    /// The maps a free drive can start on: every map of the game that is driven on, the hub
    /// first. (A showroom is a map too, and is not one of them.)
    fn free_maps(&self) -> Vec<&Map> {
        let driven = |map: &&Map| map.name != SHOWROOM && map.name != SHOWROOM_TOWN;
        let mut maps: Vec<&Map> = self.world.maps.iter().filter(driven).collect();
        maps.sort_by_key(|map| map.name != self.world.hub);
        maps
    }

    /// The map a free drive starts on: the one that was chosen, or the hub.
    fn free_start(&self) -> String {
        let chosen = self.free_map.as_ref();
        chosen
            .filter(|name| self.free_maps().iter().any(|map| map.name == **name))
            .cloned()
            .unwrap_or_else(|| self.world.hub.clone())
    }

    /// Leaves the showroom for the map it was come to from, in the truck that is driven.
    fn leave_showroom(&mut self) {
        self.show = None;
        let back = self
            .show_back
            .take()
            .unwrap_or_else(|| self.world.hub.clone());
        self.go(&back, Mode::Roam);
    }

    /// Does what was asked for on a page of the yard: `page:NAME`, `begin:TRUCK`,
    /// `borrow:DOLLARS`, `repay:DOLLARS` (or `repay:all`), `buy:PLACE`, `drive:INDEX`,
    /// `sell:INDEX`, `fit:GROUP:CHOICE` (or `fit:GROUP:none`), `paint:RRGGBB`, `oil`, `leave`.
    fn yard_act(&mut self, what: &str) {
        let words: Vec<&str> = what.split(':').collect();
        let number = |word: &str| word.parse::<usize>().ok();
        let seed = self.seed();
        let trucks = self.world.trucks.clone();
        let grade = |name: &str| {
            trucks
                .iter()
                .find(|t| t.name == name)
                .map_or(1, |t| t.grade)
        };
        // Leaving a page is leaving what was chosen on it and not paid for.
        if what == "leave" && self.yard_page != "colour" {
            self.cart.clear();
            (self.cart_paint, self.cart_oil, self.quote, self.focus) = (None, false, None, None);
            self.cart_repair = false;
        }
        let mut said = String::new();
        let dollars = |n: u64| {
            format!(
                "${}",
                n.to_string()
                    .as_bytes()
                    .rchunks(3)
                    .rev()
                    .map(|c| std::str::from_utf8(c).unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        match words[..] {
            // A free drive: the grade of trucks to choose from, and the one chosen. It is
            // driven from the Crossroads, with nothing owed and nothing kept.
            ["grade", grade] => {
                if let Some(grade) = number(grade).filter(|grade| (1..=5).contains(grade)) {
                    self.free_grade = grade as u32;
                    self.show_card(0);
                }
            }
            // A free drive's truck as a monster truck, or as it is.
            ["monster", on @ ("on" | "off")] if matches!(self.yard_page, "free" | "tune") => {
                self.monster = on == "on";
                self.show_card(self.show_index);
            }
            // The map a free drive starts on: any of the game's that is driven on.
            ["where", map] if matches!(self.yard_page, "free" | "where") => {
                if self.free_maps().iter().any(|known| known.name == map) {
                    self.free_map = Some(map.to_owned());
                }
            }
            // The hour it starts at, and the weather it is driven in: which of each.
            ["time", index] if self.yard_page == "where" => {
                if let Some(index) = number(index).filter(|index| *index < TIMES.len()) {
                    self.time = index;
                }
            }
            ["damage", on @ ("on" | "off")] if self.yard_page == "where" => {
                self.free_damage = on == "on";
            }
            ["weather", index] if self.yard_page == "where" => {
                if let Some(index) = number(index).filter(|index| *index < WEATHERS.len()) {
                    self.weather = index;
                }
            }
            // A free drive's truck is chosen: it is built next, where it stands by itself
            // with every piece of its kit. (Another truck than the last is as it is sold.)
            ["free", name] => {
                if trucks.iter().any(|truck| truck.name == name) {
                    if self.drives.as_deref() != Some(name) {
                        self.free_build.clear();
                        self.free_paint = None;
                    }
                    self.drives = Some(name.to_owned());
                    (self.built, self.story) = (false, false);
                    self.yard_page = "tune";
                    self.show_card(0);
                }
            }
            // It is driven: from the map that was chosen (the hub, if none was), at the
            // hour and in the weather that were.
            ["drive"] if self.yard_page == "where" && self.drives.is_some() => {
                self.story = false;
                (self.free_sky, self.free_weather) = (true, true);
                self.unwind = -std::mem::take(&mut self.orbited);
                self.show_back = Some(self.free_start());
                self.leave_showroom();
            }
            // Built on the page `tune`: a part is fitted (or, `none`, taken off), tried
            // where the truck stands, and a colour chosen. Nothing is paid for.
            ["fit", group, choice] if self.yard_page == "tune" => {
                self.free_build.retain(|(named, _)| named != group);
                let stock = self
                    .kit
                    .as_ref()
                    .and_then(|(_, kit)| kit.stock.fitted(group));
                if stock != Some(choice).filter(|choice| *choice != "none") {
                    self.free_build.push((group.to_owned(), choice.to_owned()));
                }
                self.orders.push(Order::Fit(self.free_worn()));
            }
            ["try", group, choice] if self.yard_page == "tune" => {
                let kept = self.free_build.clone();
                self.free_build.retain(|(named, _)| named != group);
                self.free_build.push((group.to_owned(), choice.to_owned()));
                self.orders.push(Order::Fit(self.free_worn()));
                self.free_build = kept;
            }
            ["untry"] if matches!(self.yard_page, "tune" | "where") => {
                if self.yard_page == "tune" {
                    self.orders.push(Order::Fit(self.free_worn()));
                    self.orders.push(Order::Paint(self.free_paint));
                }
            }
            ["paint", colour] if self.yard_page == "tune" => {
                if let Ok(colour) = u32::from_str_radix(colour, 16) {
                    self.free_paint =
                        Some(colour).filter(|colour| self.free_paint != Some(*colour));
                    self.orders.push(Order::Paint(self.free_paint));
                }
            }
            // (Back from where and when is back to its build, and from that to which truck.)
            ["leave"] if self.yard_page == "where" => {
                self.yard_page = "tune";
                self.show_card(0);
            }
            ["leave"] if self.yard_page == "tune" => {
                self.yard_page = "free";
                self.focus = None;
                self.show_card(0);
            }
            ["page", page] => {
                for known in [
                    "start", "garage", "bank", "shop", "used", "dealer", "free", "tune", "where",
                ] {
                    if page == known {
                        self.yard_page = known;
                        self.show_card(0);
                    }
                }
            }
            // The truck in hand on the page: it is the one that stands in the showroom.
            ["show", index] => {
                if let Some(index) = number(index) {
                    self.show_card(index);
                }
            }
            // A first truck is chosen in two steps: which, and then in what colour.
            ["choose", name] if self.yard_page == "start" => {
                let offered = crate::yard::starters(&trucks);
                if let Some(index) = offered.iter().position(|truck| truck.name == name) {
                    self.start_truck = Some(name.to_owned());
                    self.yard_page = "colour";
                    self.show_card(index);
                }
            }
            ["take", colour] if self.yard_page == "colour" => {
                self.start_paint = u32::from_str_radix(colour, 16).ok().or(self.start_paint);
                if let Some(name) = self.start_truck.take() {
                    self.yard_page = "start";
                    return self.yard_act(&format!("begin:{name}"));
                }
            }
            // (Back from its colour is back to which.)
            ["leave"] if self.yard_page == "colour" => {
                let index = self.show_index;
                self.start_truck = None;
                self.start_paint = None;
                self.yard_page = "start";
                self.show_card(index);
            }
            ["begin", name] => {
                let offered = crate::yard::starters(&trucks);
                if let Some(truck) = offered.into_iter().find(|truck| truck.name == name) {
                    self.yard.begin(truck);
                    // In the colour that was chosen for it.
                    if let (Some(paint), Some(owned)) =
                        (self.start_paint.take(), self.yard.trucks.last_mut())
                    {
                        owned.paint = Some(paint);
                    }
                    self.story = true;
                    if self.seed.is_none() {
                        self.seed = Some(self.chance | 1);
                    }
                    self.unwind = -std::mem::take(&mut self.orbited);
                    self.show = None;
                    self.show_back = None;
                    let hub = self.world.hub.clone();
                    self.go(&hub, Mode::Roam);
                }
            }
            ["borrow", amount] => {
                if let Some(amount) = number(amount) {
                    said = match self.yard.borrow(&mut self.money, amount as u64) {
                        Ok(()) => format!("Borrowed {}.", dollars(amount as u64)),
                        Err(no) => no.said().to_owned(),
                    };
                }
            }
            ["repay", amount] => {
                let amount = number(amount).map_or(u64::MAX, |n| n as u64);
                let paid = self.yard.repay(&mut self.money, amount);
                said = if paid > 0 {
                    format!("Paid {} off what you owe.", dollars(paid))
                } else {
                    "There is nothing to pay it with.".to_owned()
                };
            }
            // The view goes to a part of the truck, or back to all of it.
            ["focus", part] => {
                self.focus = (part != "off").then(|| part.to_owned());
                return;
            }
            // A truck is asked for, used or new: the bill for it is shown.
            ["buy", place] => {
                self.quote = number(place).map(Deal::Used);
            }
            ["new", name] => {
                if trucks.iter().any(|truck| truck.name == name) {
                    self.quote = Some(Deal::New(name.to_owned()));
                }
            }
            // The cart is taken to the counter: the bill for it is shown.
            ["checkout"] => {
                if !self.deal_lines(&Deal::Cart).is_empty() {
                    self.quote = Some(Deal::Cart);
                }
            }
            ["empty"] => {
                self.cart.clear();
                (self.cart_paint, self.cart_oil) = (None, false);
                self.cart_repair = false;
                return self.yard_act("untry");
            }
            // The bill is turned down: nothing is bought, and what was chosen is still chosen.
            ["reject"] => self.quote = None,
            // The bill is accepted: it is paid, from hand and then on the debt.
            ["confirm"] => {
                let Some(deal) = self.quote.take() else {
                    return;
                };
                let total: u64 = self.deal_lines(&deal).iter().map(|(_, price)| price).sum();
                said = match (self.yard.quote(self.money, total), deal) {
                    (Err(no), _) => no.said().to_owned(),
                    (Ok(_), Deal::Used(place)) => {
                        match self
                            .yard
                            .buy(&mut self.money, seed, self.day, &trucks, place)
                        {
                            Ok(_) => {
                                // It is gone from the yard's row.
                                self.orders.push(Order::Unpark(place));
                                "It is yours. It is in the shed: take it out from the garage."
                                    .to_owned()
                            }
                            Err(no) => no.said().to_owned(),
                        }
                    }
                    (Ok(_), Deal::New(name)) => {
                        let truck = trucks.iter().find(|truck| truck.name == name);
                        match truck.map(|truck| self.yard.buy_new(&mut self.money, truck)) {
                            Some(Ok(_)) => {
                                "It is yours, new. It is in the shed: take it out from the garage."
                                    .to_owned()
                            }
                            Some(Err(no)) => no.said().to_owned(),
                            None => String::new(),
                        }
                    }
                    (Ok(_), Deal::Cart) => {
                        let driven = self.yard.driven().map(|owned| owned.truck.clone());
                        let kit = self
                            .kit
                            .clone()
                            .filter(|(of, _)| Some(of) == driven.as_ref());
                        if let Some((name, kit)) = kit {
                            for (group, choice) in std::mem::take(&mut self.cart) {
                                let _ = self.yard.fit(
                                    &mut self.money,
                                    &kit,
                                    grade(&name),
                                    &group,
                                    Some(&choice),
                                );
                            }
                        }
                        if let Some(colour) = self.cart_paint.take() {
                            if self.yard.respray(&mut self.money, colour).is_ok() {
                                if let Some(show) = &mut self.show {
                                    show.2 = Some(colour);
                                }
                            }
                        }
                        if std::mem::take(&mut self.cart_oil) {
                            let _ = self.yard.oil(&mut self.money);
                        }
                        if std::mem::take(&mut self.cart_repair) {
                            let (_, cost) = self.repairs();
                            if self.yard.repair(&mut self.money, cost).is_ok() {
                                self.orders.push(Order::Damage(String::new()));
                            }
                        }
                        self.yard_act("untry");
                        format!("Paid: {}.", dollars(total))
                    }
                };
            }
            // (What is aboard a truck is in it for real. Another is not taken out, and the
            // one driven is not sold, from under it: it is put in the chest first.)
            ["drive", index] if self.gold.aboard() && number(index) != Some(self.yard.driving) => {
                said = "There is gold aboard this one. Put it in the chest first.".to_owned();
            }
            ["sell", index] if self.gold.aboard() && number(index) == Some(self.yard.driving) => {
                said = "There is gold aboard it. Put it in the chest first.".to_owned();
            }
            ["drive", index] => {
                if let Some(index) = number(index) {
                    if self.yard.drive(index).is_ok() {
                        said = "That is the one you take out.".to_owned();
                    }
                }
            }
            ["sell", index] => {
                if let Some(index) = number(index) {
                    said = match self.yard.sell(&mut self.money, index, &trucks) {
                        Ok(fetched) => {
                            self.show_card(0);
                            format!("Sold, for {}.", dollars(fetched))
                        }
                        Err(no) => no.said().to_owned(),
                    };
                }
            }
            ["fit", group, choice] => {
                let driven = self.yard.driven().map(|owned| owned.truck.clone());
                let kit = self
                    .kit
                    .clone()
                    .filter(|(of, _)| Some(of) == driven.as_ref());
                let owns = self.yard.driven().is_some_and(|owned| {
                    choice == "none"
                        || owned.parts.contains(&format!("{group}.{choice}"))
                        || kit
                            .as_ref()
                            .is_some_and(|(_, kit)| kit.stock.fitted(group) == Some(choice))
                });
                if !owns && kit.is_some() {
                    // Not the truck's yet: it goes in the cart, or, chosen again, out of it.
                    let had = self.cart.iter().any(|(g, c)| g == group && c == choice);
                    self.cart.retain(|(g, _)| g != group);
                    if !had {
                        self.cart.push((group.to_owned(), choice.to_owned()));
                    }
                    said = if had {
                        "Out of the cart."
                    } else {
                        "In the cart."
                    }
                    .to_owned();
                    if let Some(build) = self.cart_build() {
                        self.orders.push(Order::Fit(build));
                    }
                } else if let (Some((name, kit)), Some(_)) = (kit, driven) {
                    // (What is put on, or taken off, takes the place of what was in the
                    // cart for there.)
                    self.cart.retain(|(g, _)| g != group);
                    let choice = Some(choice).filter(|choice| *choice != "none");
                    said = match self
                        .yard
                        .fit(&mut self.money, &kit, grade(&name), group, choice)
                    {
                        Ok(0) => "Done.".to_owned(),
                        Ok(paid) => format!("Bought and fitted, for {}.", dollars(paid)),
                        Err(no) => no.said().to_owned(),
                    };
                    // It is fitted where it stands: every piece of it is there.
                    if let Some(build) = self.cart_build() {
                        self.orders.push(Order::Fit(build));
                    }
                }
            }
            // A part tried where the truck stands, or a colour: it is seen on it, and
            // nothing is bought. And put back as it was, when the trying is over.
            ["try", group, choice] => {
                if let Some(owned) = self.yard.driven() {
                    let kit = self.kit.as_ref().filter(|(of, _)| *of == owned.truck);
                    let mut tried = owned.clone();
                    // (With what is in the cart on it as well.)
                    for (carted, which) in &self.cart {
                        tried.fitted.set(carted, Some(which));
                    }
                    let choice = Some(choice).filter(|choice| *choice != "none");
                    tried.fitted.set(group, choice);
                    self.orders
                        .push(Order::Fit(tried.build_text(kit.map(|(_, kit)| kit))));
                }
            }
            ["tint", colour] => {
                if let Ok(colour) = u32::from_str_radix(colour, 16) {
                    self.orders.push(Order::Paint(Some(colour)));
                }
            }
            ["untry"] => {
                if let Some(build) = self.cart_build() {
                    self.orders.push(Order::Fit(build));
                }
                if let Some(owned) = self.yard.driven() {
                    self.orders
                        .push(Order::Paint(self.cart_paint.or(owned.paint)));
                }
            }
            // A first truck's colour: it is shown in it, and taken in it. Nothing is paid.
            ["paint", colour] if self.yard_page == "colour" => {
                if let Ok(colour) = u32::from_str_radix(colour, 16) {
                    self.start_paint = Some(colour);
                    if let Some(show) = &mut self.show {
                        show.2 = Some(colour);
                    }
                    self.orders.push(Order::Show {
                        index: self.show_index,
                        paint: Some(colour),
                    });
                }
            }
            // A respray, and an oil change: in the cart, or, chosen again, out of it.
            ["paint", colour] => {
                if let Ok(colour) = u32::from_str_radix(colour, 16) {
                    let had = self.cart_paint == Some(colour);
                    self.cart_paint = (!had).then_some(colour);
                    said = if had {
                        "Out of the cart."
                    } else {
                        "In the cart."
                    }
                    .to_owned();
                    let worn = self.yard.driven().and_then(|owned| owned.paint);
                    self.orders.push(Order::Paint(self.cart_paint.or(worn)));
                }
            }
            ["oil"] => {
                self.cart_oil = !self.cart_oil;
                said = if self.cart_oil {
                    "In the cart."
                } else {
                    "Out of the cart."
                }
                .to_owned();
            }
            ["repair"] => {
                let (hurt, _) = self.repairs();
                self.cart_repair = hurt > 0 && !self.cart_repair;
                said = if hurt == 0 {
                    "There is nothing on it to mend."
                } else if self.cart_repair {
                    "In the cart."
                } else {
                    "Out of the cart."
                }
                .to_owned();
            }
            ["leave"] => {
                if self.in_showroom() {
                    // Out of the showroom: back to the title, if that is where it was come
                    // to from; else in the truck that is driven.
                    self.to_title = self.yard_from == Screen::Title;
                    self.story = !self.to_title && (self.story || self.yard.begun());
                    self.leave_showroom();
                } else {
                    self.screen = self.yard_from;
                    // Another truck, or the same one fitted otherwise: the map is read again.
                    if self.story
                        && self.yard_from != Screen::Title
                        && self.worn != self.would_wear()
                    {
                        let map = self.map.clone();
                        self.go(&map, Mode::Roam);
                    }
                }
            }
            _ => {}
        }
        self.yard_said = said;
        self.unsaved = true;
        self.told_screen = None;
    }

    /// What a page of the yard is told: what is owed and in hand, and, for the page that is
    /// shown, its trucks or its parts.
    fn yard_told(&self) -> serde_json::Value {
        let trucks = &self.world.trucks;
        let of = |name: &str| trucks.iter().find(|truck| truck.name == name);
        let card = |truck: &Truck| json!({ "name": truck.name, "title": truck.title, "year": truck.year, "grade": truck.grade, "price": truck.price });
        let colour = |paint: Option<u32>| paint.map(|paint| format!("#{paint:06x}"));
        let owned: Vec<_> = self
            .yard
            .trucks
            .iter()
            .enumerate()
            .filter_map(|(index, owned)| {
                let mut told = card(of(&owned.truck)?);
                told["index"] = json!(index);
                told["km"] = json!(owned.km.round());
                told["paint"] = json!(colour(owned.paint));
                told["driving"] = json!(index == self.yard.driving);
                told["fetches"] = json!(self.yard.fetches(index, trucks));
                told["oil"] = json!(owned.oil_overdue().round());
                told["roll"] = json!(owned.roll);
                told["fitted"] = json!(owned.fitted.to_text());
                Some(told)
            })
            .collect();
        let used: Vec<_> = self
            .yard
            .lot(self.seed(), self.day, trucks)
            .into_iter()
            .enumerate()
            .filter_map(|(place, (listing, here))| {
                let mut told = card(of(&listing.truck)?);
                told["place"] = json!(place);
                told["km"] = json!(listing.km);
                told["roll"] = json!(listing.roll);
                told["paint"] = json!(colour(Some(listing.paint)));
                told["price"] = json!(listing.price);
                told["bargain"] = json!(listing.bargain);
                told["here"] = json!(here);
                Some(told)
            })
            .collect();
        // The auto shop: every group of the driven truck's kit, each choice with what it
        // costs, whether it is owned, and whether it is on the truck.
        let driven = self.yard.driven();
        let kit = self
            .kit
            .as_ref()
            .filter(|(name, _)| driven.is_some_and(|owned| owned.truck == *name));
        let shop = match (driven, kit) {
            (Some(owned), Some((name, kit))) => {
                let grade = of(name).map_or(1, |truck| truck.grade);
                let groups: Vec<_> = kit.groups().into_iter().map(|(group, choices)| {
                    let choices: Vec<_> = choices.into_iter().map(|choice| {
                        let have = owned.parts.contains(&format!("{group}.{choice}")) || kit.stock.fitted(group) == Some(choice);
                        let carted = self.cart.iter().any(|(g, c)| g == group && c == choice);
                        json!({ "choice": choice, "price": crate::yard::part_price(group, choice, grade), "owned": have,
                            "fitted": owned.fitted.fitted(group) == Some(choice), "cart": carted })
                    }).collect();
                    json!({ "group": group, "choices": choices, "bare": owned.fitted.fitted(group).is_none() })
                }).collect();
                json!({ "groups": groups })
            }
            _ => json!(null),
        };
        let mut told = json!({
            "page": self.yard_page,
            "from": self.yard_from.name(),
            // Whether the truck stands in the showroom behind the page, to be seen.
            "showroom": self.in_showroom(),
            "said": self.yard_said,
            "money": self.money,
            "debt": self.yard.debt,
            "credit": self.yard.credit,
            "limit": self.yard.limit(),
            "repayment": self.yard.repayment(),
            // The interest on what is owed: a year's, in hundredths, and this Friday's.
            "interest_year": (self.yard.rate() * 52.0 * 1000.0).round() / 10.0,
            "interest_week": self.yard.interest(),
            "cart": self.cart_told(),
            "quote": self.quote_told(),
            "dealer": trucks.iter().filter(|truck| truck.grade == self.free_grade).map(card).collect::<Vec<_>>(),
            "block": crate::yard::BLOCK,
            "begun": self.yard.begun(),
            "start": crate::yard::starters(trucks).into_iter().map(card).collect::<Vec<_>>(),
            "free": trucks.iter().filter(|truck| truck.grade == self.free_grade).map(card).collect::<Vec<_>>(),
            "grade": self.free_grade,
            // The maps a free drive can start on, and the one it will.
            "maps": self.free_maps().iter().map(|map| json!({ "name": map.name, "title": map.title, "kind": map.kind, "about": map.about })).collect::<Vec<_>>(),
            "start_map": self.free_start(),
            "trucks": owned,
            "used": used,
            "shop": shop,
            "colours": crate::yard::COLOURS.iter().map(|(name, colour)| json!({ "name": name, "hex": format!("{colour:06x}") })).collect::<Vec<_>>(),
            "respray": crate::yard::RESPRAY,
            "start_paint": self.start_paint.map(|colour| format!("{colour:06x}")),
            "chosen": self.start_truck.as_deref().and_then(of).map(card),
            "oil_change": crate::yard::OIL_CHANGE,
            "repairs": json!({ "hurt": self.repairs().0, "cost": self.repairs().1 }),
        });
        // A free drive, as it is being set up: told with the rest, and written apart.
        let free = json!({
            "monster": self.monster,
            "free_truck": self.drives.as_deref().and_then(of).map(card),
            "free_paint": self.free_paint.map(|paint| format!("{paint:06x}")),
            // A free drive's truck being built: each group of its kit, and its choices,
            // with the one that is on it. (Told as the auto shop's is: nothing has a price.)
            "tune": match (&self.kit, self.yard_page) {
                (Some((_, kit)), "tune") => json!({ "groups": kit.groups().into_iter().map(|(group, choices)| {
                    let fitted = self.free_fitted(kit, group);
                    let choices: Vec<_> = choices.into_iter().map(|choice| json!({ "choice": choice, "fitted": fitted == Some(choice) })).collect();
                    json!({ "group": group, "choices": choices, "bare": fitted.is_none() })
                }).collect::<Vec<_>>() }),
                _ => json!(null),
            },
            "damage": self.free_damage,
            "times": TIMES.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "weathers": WEATHERS.iter().map(|(name, ..)| *name).collect::<Vec<_>>(),
            "time": self.time,
            "weather": self.weather,
        });
        if let (Some(told), Some(free)) = (told.as_object_mut(), free.as_object()) {
            told.extend(free.clone());
        }
        told
    }

    /// Drives this truck, fitted so, whatever was kept: for a trial, or a picture.
    pub fn wear(&mut self, truck: Option<&str>, built: bool) {
        (self.drives, self.built) = (truck.map(str::to_owned), built);
    }

    /// Whoever keeps the games says which places there are for them, each with what is kept
    /// in it, if anything is; and which is being played: none, at the title, until one is
    /// begun or gone back to.
    pub fn set_saves(&mut self, saves: &[(u32, Option<String>)], playing: u32) {
        self.saves = saves
            .iter()
            .map(|(number, kept)| (*number, kept.as_ref().map(|kept| self.summary(kept))))
            .collect();
        self.save = playing;
        self.told_screen = None;
    }

    /// Stops playing the game there is: what is left is the main menu, with no game behind
    /// it, at the Crossroads. Whoever runs the game has kept it first.
    pub fn close(&mut self) {
        let (trial, saves) = (self.trial, std::mem::take(&mut self.saves));
        let options = (self.volume, self.fullscreen, self.look, self.hour);
        let (world, web) = (self.world.clone(), self.web);
        *self = Shell::new(world, "");
        (self.trial, self.saves, self.save) = (trial, saves, 0);
        self.web = web;
        (self.volume, self.fullscreen, self.look, self.hour) = options;
        self.to_title = true;
        let hub = self.world.hub.clone();
        self.go(&hub, Mode::Roam);
    }

    /// How loud the game is to be, from 0 to 1; and whether it is to fill the screen.
    pub fn volume(&self) -> f32 {
        f32::from(self.volume) / 10.0
    }

    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// How many times as fast as it was made to a controller's right stick looks round:
    /// from a little over half, to nearly two and a half times.
    pub fn look_gain(&self) -> f32 {
        0.4 + 0.2 * f32::from(self.look)
    }

    /// Whoever runs the clock says what hour of the day it is.
    pub fn set_hour(&mut self, hours: f32) {
        self.hour = hours.rem_euclid(24.0);
    }

    /// The sleeps there are to choose from now: each time of day that is six hours off or
    /// more. Its place among them all, its name, its hour, whether it is tomorrow, and how
    /// many hours off it is.
    fn sleeps(&self) -> Vec<(usize, &'static str, f32, bool, f32)> {
        let all = SLEEPS.iter().enumerate();
        all.filter_map(|(index, (name, hour))| {
            let off = (hour - self.hour).rem_euclid(24.0);
            (off >= SLEEP_LEAST).then_some((index, *name, *hour, *hour <= self.hour, off))
        })
        .collect()
    }

    /// The options, as they are kept: a line each.
    pub fn options_text(&self) -> String {
        format!(
            "volume {}\nfullscreen {}\nlook {}\n",
            self.volume,
            u8::from(self.fullscreen),
            self.look
        )
    }

    /// The options as they were kept: what is not said is left as it is.
    pub fn set_options(&mut self, kept: &str) {
        for line in kept.lines() {
            match line.split_whitespace().collect::<Vec<_>>()[..] {
                ["volume", share] => {
                    self.volume = share
                        .parse::<u8>()
                        .map_or(self.volume, |share| share.min(10));
                }
                ["fullscreen", on] => self.fullscreen = on == "1",
                ["look", speed] => {
                    self.look = speed
                        .parse::<u8>()
                        .map_or(self.look, |speed| speed.clamp(1, 10));
                }
                _ => {}
            }
        }
        self.told_screen = None;
    }

    /// What is said of a kept game on its tile: its day, its truck, and its money.
    fn summary(&self, kept: &str) -> serde_json::Value {
        let yard = crate::yard::Yard::read(kept);
        let word = |key: &str| {
            let line = kept
                .lines()
                .find(|line| line.split_whitespace().next() == Some(key));
            line.and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_owned()
        };
        let day = word("day").parse::<u32>().unwrap_or(1);
        let money = word("money").parse::<u64>().unwrap_or(0);
        let truck = yard.driven().and_then(|owned| {
            let found = self
                .world
                .trucks
                .iter()
                .find(|truck| truck.name == owned.truck);
            found.map(|truck| (truck.name.clone(), truck.title.clone()))
        });
        match truck {
            Some((name, title)) => {
                json!({ "day": day, "truck": name, "title": title, "money": money,
                "debt": yard.debt, "trucks": yard.trucks.len() })
            }
            None => json!({ "day": day, "money": money }),
        }
    }

    /// A kept game is played from here on: this is what was kept of it. One whose story has
    /// begun goes on from where it was left; a new one begins with the choice of a truck.
    pub fn load(&mut self, kept: &str) {
        let (trial, saves, save) = (self.trial, std::mem::take(&mut self.saves), self.save);
        let options = (self.volume, self.fullscreen, self.look, self.hour);
        let (world, web) = (self.world.clone(), self.web);
        *self = Shell::new(world, kept);
        (self.trial, self.saves, self.save) = (trial, saves, save);
        self.web = web;
        (self.volume, self.fullscreen, self.look, self.hour) = options;
        if self.yard.begun() {
            self.story = true;
            self.act("start");
        } else {
            self.open_yard("start");
        }
        self.told_screen = None;
    }

    /// Whoever runs the game says what is on the truck now: it is kept with the truck that
    /// is driven, to be on it wherever it is driven next.
    pub fn see_dirt(&mut self, dirt: [f32; 3]) {
        if self.in_showroom() {
            return;
        }
        let driving = self.yard.driving;
        match self.yard.trucks.get_mut(driving).filter(|_| self.story) {
            Some(owned) => owned.dirt = dirt,
            None => self.free_dirt = dirt,
        }
    }

    /// Whoever runs the game says what blows have just done to the truck: its damage now,
    /// as text, which is kept with the truck that is driven, and each piece whose stage a
    /// blow changed, which the driver is told of for a few seconds.
    pub fn see_damage(&mut self, damage: String, changes: &[crate::damage::Change]) {
        if self.in_showroom() {
            return;
        }
        let driving = self.yard.driving;
        if let Some(owned) = self.yard.trucks.get_mut(driving).filter(|_| self.story) {
            owned.damage = damage;
        }
        // The worst of it first, and no more than fits a line.
        let mut worst: Vec<&crate::damage::Change> = changes
            .iter()
            .filter(|change| change.to > change.from && change.to >= crate::damage::Stage::Dented)
            .collect();
        worst.sort_by(|a, b| b.to.cmp(&a.to));
        if worst.is_empty() {
            return;
        }
        let mut words: Vec<String> = worst
            .iter()
            .take(3)
            .map(|change| format!("{} {}", piece_said(&change.piece), change.to.name()))
            .collect();
        if worst.len() > 3 {
            words.push(format!("and {} more", worst.len() - 3));
        }
        self.blow = (words.join("  \u{b7}  "), 0.0);
        self.since_told = f32::MAX;
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
        // With it, after dots: the class, the time of day and the weather it is driven in.
        let mut words = mode.split('.');
        self.mode = Mode::named(words.next().unwrap_or(""));
        let mut number = |below: usize| {
            words
                .next()
                .and_then(|word| word.parse::<usize>().ok())
                .filter(|index| *index < below)
        };
        if let Some(class) = number(crate::timed::CLASSES.len()) {
            self.class = class;
        }
        if let Some(time) = number(TIMES.len()) {
            self.time = time;
        }
        if let Some(weather) = number(WEATHERS.len()) {
            self.weather = weather;
        }
        self.screen = Screen::Drive;
    }

    /// What the map is being driven for, as `resume` is told it.
    pub fn mode_name(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.mode.name(),
            self.class,
            self.time,
            self.weather
        )
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
        // A track is driven at the hour and in the weather chosen for it. Anywhere else
        // (the hub, a town) it is whatever hour it has got to, and the weather is the day's
        // own, as the calendar has it: what was forecast is what falls. The showroom is fair.
        let timed = found.name != self.world.hub && !found.length.is_empty();
        let room = found.name == SHOWROOM || found.name == SHOWROOM_TOWN;
        // A free drive starts at the hour and in the weather that were chosen for it,
        // wherever it starts; and goes on in that weather wherever it goes.
        let starts = !room && std::mem::take(&mut self.free_sky);
        let sky = Some(if timed || starts {
            let (_, name, soaked) = WEATHERS[self.weather];
            (TIMES[self.time].1, name, soaked)
        } else if room {
            (KEEP_HOUR, "fair", 0.0)
        } else if self.free_weather && !self.story {
            let (_, name, soaked) = WEATHERS[self.weather];
            (KEEP_HOUR, name, soaked)
        } else {
            let (name, soaked) = self.today_sky();
            (KEEP_HOUR, name, soaked)
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
        // The options, from wherever they are opened: `option:volume:N`, `option:fullscreen:1`.
        if let Some(asked) = what.strip_prefix("option:") {
            match asked.split(':').collect::<Vec<_>>()[..] {
                ["volume", share] => match share.parse::<u8>() {
                    Ok(share) => self.volume = share.min(10),
                    Err(_) => return,
                },
                ["fullscreen", on] => self.fullscreen = on == "1",
                ["look", speed] => match speed.parse::<u8>() {
                    Ok(speed) => self.look = speed.clamp(1, 10),
                    Err(_) => return,
                },
                _ => return,
            }
            self.orders.push(Order::Options);
            self.told_screen = None;
            return;
        }
        if self.screen == Screen::Yard {
            return self.yard_act(what);
        }
        // The kept games, from the title: `save_new`, `save_use:NUMBER`, `save_delete:NUMBER`.
        if let (Screen::Title, Some(asked)) = (self.screen, what.strip_prefix("save_")) {
            let number = |word: &str| word.parse::<u32>().ok();
            let order = match asked.split(':').collect::<Vec<_>>()[..] {
                ["new", slot] => number(slot).map(Saves::New),
                ["use", slot] => number(slot).map(Saves::Use),
                ["delete", slot] => number(slot).map(Saves::Delete),
                _ => None,
            };
            self.orders.extend(order.map(Order::Saves));
            return;
        }
        match (self.screen, what) {
            // A free drive is chosen in the showroom: any truck there is. (A trial, which
            // plays by itself, takes the first of them and drives.)
            (Screen::Title, "free_page") => {
                if self.trial {
                    self.act("free");
                } else {
                    self.story = false;
                    self.open_yard("free");
                }
            }
            // In the shed, in a story: the garage, where the trucks that are owned are.
            (Screen::Shed, "garage") if self.story => self.open_yard("garage"),
            (Screen::Title, "start" | "free") => {
                // A free drive is in the truck chosen at the title, with nothing owed.
                if what == "free" {
                    self.story = false;
                }
                // The view went round the truck behind the title: it starts behind it.
                self.unwind = -std::mem::take(&mut self.orbited);
                self.screen = Screen::Drive;
                // Another truck than the one that is behind the title: the map is read
                // again, with it.
                if self.worn != self.would_wear() && self.resume.is_none() {
                    self.resume = Some(self.map.clone());
                }
                // A game started for the first time is given its own weather.
                if self.seed.is_none() {
                    self.seed = Some(self.chance | 1);
                    self.unsaved = true;
                }
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
            // In the shed: a sleep, till whichever time of day was chosen (`sleep:N`), or
            // the first there is (`sleep`). Past midnight, it is the next day.
            (Screen::Shed, sleep) if sleep == "sleep" || sleep.starts_with("sleep:") => {
                let sleeps = self.sleeps();
                let asked = sleep
                    .strip_prefix("sleep:")
                    .and_then(|index| index.parse::<usize>().ok());
                let chosen = match asked {
                    Some(index) => sleeps.iter().find(|sleep| sleep.0 == index),
                    None => sleeps.first(),
                };
                let Some(&(_, name, hours, tomorrow, _)) = chosen else {
                    return;
                };
                if tomorrow {
                    self.day += 1;
                    // A Friday: the week's repayment comes out.
                    if self.story && crate::calendar::pay_day(self.day) {
                        let (paid, short) = self.yard.friday(&mut self.money);
                        log::info!("yard: Friday, ${paid} paid and ${short} short");
                    }
                }
                self.hour = hours;
                self.slept_till = name;
                self.unsaved = true;
                self.slept = 0.0;
                self.orders.push(Order::Sleep {
                    hours,
                    days: u32::from(tomorrow),
                });
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
                        holds: &self.holds,
                        doors: &self.doors,
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
            // Back to the main menu: the game is kept and closed, so that nothing done from
            // the menu after it (a free drive, say) is written into it. (A trial keeps none.)
            (Screen::Pause, "title") => {
                if self.trial {
                    self.screen = Screen::Title;
                } else {
                    self.orders.push(Order::Saves(Saves::Close));
                }
            }
            // A timed run starts again from the line, without the map being read again.
            (Screen::Pause, "restart") if self.mode.timed() && self.course.is_some() => {
                self.result = None;
                self.orbited = 0.0;
                self.orders.push(Order::Stage(Stage::Line(RUN_UP)));
                self.screen = Screen::Lineup;
            }
            (Screen::Pause, "restart") => {
                let (map, mode) = (self.map.clone(), self.mode);
                // A free drive starts its map again: where the map starts and not where
                // the truck was left, at the hour that was chosen for it, and mended.
                if !self.story && mode == Mode::Roam {
                    self.poses.remove(&map);
                    self.free_sky = self.free_weather;
                }
                self.go(&map, mode);
            }
            // A free drive's damage, on or off, as it is driven.
            (Screen::Pause, "damage") if !self.story => {
                self.free_damage = !self.free_damage;
                self.orders.push(Order::Harm(self.harm()));
                self.told_screen = None;
            }
            // At the line: away, or one of the others watched first; and after a run, again.
            (Screen::Lineup, "go") | (Screen::Result, "again") => self.start_run(),
            (Screen::Lineup, what) if crate::cinema::chosen(what).is_some() => {
                if let Some(medal) = crate::cinema::chosen(what) {
                    self.watch(medal);
                }
            }
            (Screen::Watch, "skip") => self.end_watch(),
            // The clock is left for the bush: the truck stays where it is.
            (Screen::Lineup | Screen::Result, "roam") => {
                self.mode = Mode::Roam;
                self.result = None;
                self.unwind = -std::mem::take(&mut self.orbited);
                self.screen = Screen::Drive;
            }
            (Screen::Lineup | Screen::Result, "hub") => {
                let hub = self.world.hub.clone();
                self.go(&hub, Mode::Roam);
            }
            (Screen::Pause, "hub") if self.map != self.world.hub => {
                let hub = self.world.hub.clone();
                self.go(&hub, Mode::Roam);
            }
            (Screen::Offer, "roam" | "practice" | "timed" | "laps") => {
                if let Some(place) = self.offered.and_then(|index| self.places.get(index)) {
                    let map = place.map.clone();
                    // The way out of one map is the way into the next. A track gone to by
                    // its trailhead is come into at its gate, not wherever a truck was last
                    // left on it; and the Crossroads is come back to where it was left, which
                    // is the trailhead that was driven out of.
                    if map != self.world.hub {
                        self.poses.remove(&map);
                    }
                    self.go(&map, Mode::named(what));
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
        // A trial is played by what it is told to press, and by nothing else: not by a
        // controller that is plugged in, nor a key, which its pages would pass on.
        if self.trial
            && matches!(
                words.first(),
                Some(&("key" | "pad" | "touch" | "look" | "mouse"))
            )
        {
            return;
        }
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
            // What that controller says it is, for its buttons to be drawn as they are:
            // a browser's `Gamepad.id`, or a maker's number and a name.
            ["pad", "kind", ..] => self.pad_kind = PadKind::of(line, None),
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
            // Where the driver is going, chosen on the map: the place nearest where was
            // clicked, if one is near, or just there. Chosen again, or `off`, it is nowhere.
            ["set", "waypoint", "off"] => {
                self.waypoint = None;
                self.told_screen = None;
            }
            ["set", "waypoint", x, z] => {
                if let (Ok(x), Ok(z)) = (x.parse::<f32>(), z.parse::<f32>()) {
                    let there = Vec3::new(x, 0.0, z);
                    let near = self
                        .places
                        .iter()
                        .filter(|place| !place.stand())
                        .filter(|place| flat_distance(place.at, there) < 30.0)
                        .min_by(|a, b| {
                            flat_distance(a.at, there).total_cmp(&flat_distance(b.at, there))
                        });
                    let chosen = match near {
                        Some(place) => (self.called(place).to_owned(), place.at),
                        None => ("Marker".to_owned(), there),
                    };
                    let again = self
                        .waypoint
                        .as_ref()
                        .is_some_and(|(_, at)| flat_distance(*at, chosen.1) < 6.0);
                    self.waypoint = (!again).then_some(chosen);
                    self.told_screen = None;
                }
            }
            ["set", "weather", index] => {
                if let Some(index) = index.parse().ok().filter(|&i| i < WEATHERS.len()) {
                    self.weather = index;
                }
            }
            // The class a track is to be run in.
            // (One of the game's trucks runs in its own class: there is nothing to choose.)
            ["set", "class", _] if self.class_driven().is_some() => {}
            ["set", "class", index] => {
                let classes = crate::timed::CLASSES.len();
                if let Some(index) = index.parse().ok().filter(|&i| i < classes) {
                    self.class = index;
                    self.unsaved = true;
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
        // Whichever was pressed last is the one in hand, and the pages draw its buttons: a
        // controller's button, trigger or stick, or a key or a click of the mouse. A stick
        // that only rests off its middle, and a mouse that is only moved, change nothing.
        use solstice_game::Device;
        if devices.pressed_on(Device::Gamepad) {
            self.pad = true;
        } else if devices.pressed_on(Device::Key) || devices.pressed_on(Device::Mouse) {
            self.pad = false;
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
                // Before a door, it goes in: the auto shop, the used trucks, the bank.
                let door = self.at_door().filter(|_| menu.pressed("enter"));
                if let Some(page) = door {
                    log::info!("door: {page}");
                    match page {
                        // The shed and the gold buyer's are where they are: nothing is read.
                        "shed" => self.screen = Screen::Shed,
                        "buyer" => {
                            self.sold = None;
                            self.screen = Screen::Buyer;
                        }
                        _ => {
                            self.door_from = Some((self.map.clone(), self.walker));
                            self.open_yard(page);
                        }
                    }
                } else if let Some(spot) = self.at_truck().filter(|_| menu.pressed("enter")) {
                    // By a truck in the used-truck yard, it is looked over: the page of
                    // used trucks, at that one.
                    log::info!("yard: looking over the truck in place {spot}");
                    self.door_from = Some((self.map.clone(), self.walker));
                    self.open_yard_at("used", spot);
                } else if self.afoot && menu.pressed("enter") {
                    use crate::gold::Use;
                    let stands = crate::gold::Stands {
                        map: &self.map,
                        walker: self.walker,
                        yaw: self.walker_yaw,
                        truck: self.origin,
                        turned: self.turned,
                        tray: self.tray,
                        holds: &self.holds,
                        doors: &self.doors,
                    };
                    let door = stands.doorway().map(|(_, door)| door);
                    if let Some(done) = self.gold.act(&stands) {
                        log::info!("gold: {}", done.name());
                        self.unsaved = true;
                        self.told_screen = None;
                        match (done, door) {
                            // A door of the truck's, opened or shut.
                            (Use::OpenDoor | Use::ShutDoor, Some(door)) => {
                                let door = &mut self.doors[door];
                                door.open = done == Use::OpenDoor;
                                self.orders.push(Order::Door {
                                    piece: door.piece.clone(),
                                    open: door.open,
                                });
                            }
                            _ => self.orders.push(Order::Sound(match done {
                                Use::Dig => "dig",
                                Use::IntoChest => "coins",
                                _ => "nugget",
                            })),
                        }
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
                        holds: &self.holds,
                        doors: &self.doors,
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
            // Somebody else's run: any button skips it.
            Screen::Watch => {
                if ["menu_accept", "menu_back", "pause"]
                    .iter()
                    .any(|action| menu.pressed(action))
                {
                    self.end_watch();
                } else if let Some(cinema) = &mut self.cinema {
                    cinema.tick(seconds);
                }
            }
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
                    // A screen with tabs: the one before, and the next.
                    ("menu_prev", "prev"),
                    ("menu_next", "next"),
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
            Screen::Title | Screen::Lineup => {
                self.orbited += TITLE_ORBIT * seconds;
                Step::Unattended(TITLE_ORBIT * seconds)
            }
            // In the showroom the view goes round the truck that stands there.
            Screen::Yard if self.in_showroom() => {
                self.show_for += seconds;
                Step::Unattended(0.0)
            }
            // The game drives, and the camera is put where the shot is from (`Shell::view`).
            Screen::Watch => Step::Unattended(0.0),
            Screen::Result
            | Screen::Pause
            | Screen::Offer
            | Screen::Loading
            | Screen::Shed
            | Screen::Sleep
            | Screen::Yard
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
        if self.screen == Screen::Watch {
            self.watching(hud);
            return;
        }
        if self.screen != Screen::Drive {
            return;
        }
        let at = truck(hud);
        let was_afoot = self.afoot;
        (self.afoot, self.walker, self.truck) = (hud.afoot, hud.walker, at);
        self.speed = hud.velocity.length();
        // Where the truck is, to come back to.
        if self.mode == Mode::Roam && !self.in_showroom() {
            let yaw = (-hud.forward.x).atan2(-hud.forward.z).to_degrees();
            self.poses.insert(self.map.clone(), (at, yaw));
        }
        // A place is stopped at in the truck: on foot, wherever the truck was left, there is
        // nothing to enter.
        // (A door is walked up to, not parked in.)
        self.inside = self
            .places
            .iter()
            .position(|place| place.ring() && flat_distance(place.at, at) < place.radius)
            .filter(|_| !self.afoot);
        // Where the driver was going is come to: there is nowhere they are going now.
        if self
            .waypoint
            .as_ref()
            .is_some_and(|(_, there)| flat_distance(*there, at) < 12.0)
        {
            self.waypoint = None;
        }
        // A way out at the edge of the map: a truck driven down it is in the next map, with
        // nothing asked. (One that has just come in by it is not sent straight back: not
        // until it has been clear of it.)
        let out = self
            .places
            .iter()
            .find(|place| place.edge().is_some() && flat_distance(place.at, at) < place.radius)
            .and_then(|place| place.edge())
            .map(str::to_owned);
        match out {
            Some(map) if self.clear_of_edge && !self.afoot && self.mode == Mode::Roam => {
                log::info!("edge: out of {} to {map}", self.map);
                self.came_from = Some(self.map.clone());
                self.go(&map, Mode::Roam);
                return;
            }
            Some(_) => {}
            None => self.clear_of_edge = true,
        }
        // The gold. Nuggets are things in the world: where each has got to is taken from
        // where the world says its body is; a nugget in the hand of a driver who has got back
        // in is let go into the tray; the world is asked for the bodies there should be; and
        // every nugget in sight is shown where it is. What the truck carries it carries for
        // real: the nuggets lie on its tray's floor, and weigh on it.
        let boarded = was_afoot && !self.afoot;
        // Nobody drives off with a door open.
        if boarded {
            for door in self.doors.iter_mut().filter(|door| door.open) {
                door.open = false;
                self.orders.push(Order::Door {
                    piece: door.piece.clone(),
                    open: false,
                });
            }
        }
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
        let was_at = self.origin;
        (self.walker_yaw, self.origin, self.turned, self.tray) =
            (hud.walker_yaw, hud.origin, hud.rotation, hud.tray);
        // The truck has gone as far as it has moved, driven: not when it is put somewhere.
        let moved = flat_distance(self.origin, was_at);
        if self.story && !self.afoot && moved > 0.0 && moved < 30.0 {
            self.yard.went(moved / 1000.0);
        }
        // A door left open is pushed shut once the driver has walked off from it.
        if self.afoot {
            let from = self.turned.inverse() * (self.walker - self.origin);
            for (index, door) in self.doors.iter_mut().enumerate() {
                let ways = self.holds.iter().flat_map(|hold| &hold.ways);
                let near = ways
                    .filter(|(of, _)| *of == index)
                    .any(|(_, at)| flat_distance(*at, from) < WALKED_OFF);
                if door.open && !near {
                    door.open = false;
                    self.orders.push(Order::Door {
                        piece: door.piece.clone(),
                        open: false,
                    });
                }
            }
        }
        let stands = crate::gold::Stands {
            map: &self.map,
            walker: if self.afoot { self.walker } else { at },
            yaw: self.walker_yaw,
            truck: self.origin,
            turned: self.turned,
            tray: self.tray,
            holds: &self.holds,
            doors: &self.doors,
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
        if let Some(lap) = &hud.lap {
            match self.mode {
                Mode::Roam => {}
                Mode::Practice => {
                    if let Some(best) = lap.best {
                        self.lapped(best);
                    }
                }
                // The run that counts is one lap: from a lap not yet begun, seen at the
                // line, to the first one finished.
                Mode::Timed if lap.count == 0 => self.armed = true,
                Mode::Timed => {
                    if let (true, None, Some(seconds)) = (self.armed, &self.result, lap.last) {
                        self.finish(seconds, lap.last_valid, lap.last_resets);
                    }
                }
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
        self.blow.1 = (self.blow.1 + seconds).min(BLOW_SAID * 2.0);
        let changed = self.told_screen != Some(self.screen);
        let due =
            matches!(self.screen, Screen::Drive | Screen::Watch) && self.since_told >= TOLD_EVERY;
        // The pages are told at once when another device is taken in hand, so that the
        // buttons they draw are its buttons; and of the bindings, once they are known.
        let device = (self.pad, self.pad_kind);
        let swapped = self.told_device != Some(device);
        let unbound = !self.told_binds;
        if !changed && !due && self.nav.is_empty() && !swapped && !unbound {
            return None;
        }
        self.told_device = Some(device);
        self.told_binds = true;
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
            "pad_kind": self.pad_kind.name(),
            "nav": std::mem::take(&mut self.nav),
            "map": map.map(|map| self.described(map)),
            "hub": self.map == self.world.hub,
            // Whether there is a way back to the hub to drive: else the pause menu has one.
            "way_home": self.way_home(),
            "web": self.web,
            "day": self.day,
            // The kept games: each its number and what is said of it, and which is played.
            "saves": self.saves.iter().map(|(number, game)| json!({ "slot": number, "game": game, "empty": game.is_none(), "playing": *number == self.save })).collect::<Vec<_>>(),
            "options": json!({ "volume": self.volume, "fullscreen": self.fullscreen, "look": self.look }),
            // What a sleep can be till, in the shed; and what the one being slept is till.
            "sleeps": self.sleeps().into_iter().map(|(index, name, hour, tomorrow, off)| json!({
                "index": index, "name": name, "tomorrow": tomorrow, "off": off.round(),
                "hour": format!("{}:{:02}", hour.floor(), ((hour % 1.0) * 60.0).round()),
            })).collect::<Vec<_>>(),
            "slept_till": self.slept_till,
            // The story: whether there is one, and what is owed in it.
            "story": json!({ "on": self.story, "begun": self.yard.begun(), "debt": self.yard.debt,
                "truck": self.yard.driven().and_then(|owned| self.world.trucks.iter().find(|truck| truck.name == owned.truck)).map(|truck| truck.title.clone()) }),
            "yard": (self.screen == Screen::Yard).then(|| self.yard_told()),
            // The truck that is chosen: one of the game's own, or the one the maps have.
            "truck": match self.drives.as_ref().and_then(|name| self.world.trucks.iter().find(|truck| truck.name == *name)).or(self.world.trucks.first()) {
                Some(truck) => json!({ "name": truck.name, "title": truck.title, "year": truck.year, "price": truck.price,
                    "grade": truck.grade, "built": self.built, "own": true }),
                None => json!({ "name": "raptor", "title": "Raptor R", "own": false }),
            },
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
            // How far off what it hears is, to the nearest five metres.
            "gold_away": self.gold_away().map(|away| (away / 5.0).round() * 5.0),
            // Whether there is any gold in this map's ground to look for.
            "gold_here": self.gold.any_buried(),
            "use": match self.at_door() {
                Some(page) => Some(match page {
                    "shop" => "door_shop",
                    "used" => "door_used",
                    "shed" => "door_shed",
                    "buyer" => "door_buyer",
                    _ => "door_bank",
                }),
                None if self.at_truck().is_some() => Some("truck_look"),
                None => self.afoot.then(|| self.gold.can(&self.stands()).map(crate::gold::Use::name)).flatten(),
            },
            // Whether what it hits hurts it.
            "harm": self.harm(),
            // What the truck's last blow did to it, for a few seconds after.
            "blow": (self.blow.1 < BLOW_SAID).then(|| self.blow.0.clone()),
            // The used truck the driver stands by, in the yard: what its board says, and
            // what a look at it would tell.
            "tag": self.at_truck().and_then(|spot| {
                let lot = self.yard.lot(self.seed(), self.day, &self.world.trucks);
                let (listing, _) = lot.get(spot)?;
                let truck = self.world.trucks.iter().find(|truck| truck.name == listing.truck)?;
                Some(json!({
                    "title": truck.title,
                    "year": truck.year,
                    "km": listing.km,
                    "roll": listing.roll,
                    "price": listing.price,
                    "bargain": listing.bargain,
                }))
            }),
            // On foot, and whether beside the truck, where holding the button gets back in.
            "afoot": self.afoot,
            // For a trial, which reads what the pages are told: where the driver stands, where
            // the truck is, which of its doors are open, and how much gold is in each hold.
            "trial": self.trial.then(|| json!({
                "walker": [self.walker.x, self.walker.y, self.walker.z],
                "truck": [self.origin.x, self.origin.y, self.origin.z],
                "open": self.doors.iter().filter(|door| door.open).map(|door| door.piece.as_str()).collect::<Vec<_>>(),
                "holds": self.gold.in_holds(&self.stands()),
            })),
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
            // A track to time: every class of it, with its medal times if the map has been
            // driven to since they were last worked out, and what has been won. What each
            // time of day and each weather adds to a purse, in hundredths; and which
            // weather is today's.
            if let Some(map) = self.world.map(&place.map).filter(|map| self.timed(map)) {
                state["classes"] = (0..crate::timed::CLASSES.len())
                    .map(|class| {
                        crate::timed::told(
                            &map.name,
                            class,
                            self.pars.get(&map.name).map(|golds| golds[class]),
                            self.records.of(&map.name, class),
                        )
                    })
                    .collect();
            }
            let percent = |bonus: f32| (bonus * 100.0).round() as u32;
            state["bonus"] = json!({
                "time": TIMES.map(|time| if time.0 == "Night" { percent(crate::timed::NIGHT_BONUS) } else { 0 }),
                "weather": WEATHERS.map(|weather| if weather.1 == "storm" { percent(crate::timed::RAIN_BONUS) } else { 0 }),
            });
            state["today"] = json!(crate::calendar::weather(self.seed(), self.day) as usize);
        }
        state["class"] = json!(self.class);
        state["class_fixed"] = json!(self.class_driven().is_some());
        // A track being driven against the clock: the class, its times, who holds them and
        // what has been won; and what the hour and the weather add to a purse.
        if let (true, Some(golds)) = (
            self.mode.timed() && self.course.is_some(),
            self.pars.get(&self.map),
        ) {
            let record = self.records.of(&self.map, self.class);
            let mut run =
                crate::timed::told(&self.map, self.class, Some(golds[self.class]), record);
            let (night, rain) = self.conditions();
            run["night"] = json!(night);
            run["rain"] = json!(rain);
            run["bonus"] = json!(((f32::from(u8::from(night)) * crate::timed::NIGHT_BONUS
                + f32::from(u8::from(rain)) * crate::timed::RAIN_BONUS)
                * 100.0)
                .round());
            run["hour"] = json!(TIMES[self.time].0);
            run["weather"] = json!(WEATHERS[self.weather].0);
            state["run"] = run;
        }
        if let Some(cinema) = self
            .cinema
            .as_ref()
            .filter(|_| self.screen == Screen::Watch)
        {
            state["watch"] = cinema.told();
        }
        if let Some(result) = self
            .result
            .as_ref()
            .filter(|_| self.screen == Screen::Result)
        {
            state["result"] = result.told();
        }
        // The calendar is a page of the pause menu, and of the shed's card.
        if matches!(self.screen, Screen::Pause | Screen::Shed) {
            // What falls due on it is the bank's repayment: none, where nothing is owed.
            let (weekly, owed) = match self.story {
                true => (self.yard.repayment(), self.yard.debt),
                false => (0, 0),
            };
            state["calendar"] = crate::calendar::told(self.seed(), self.day, weekly, owed);
        }
        // The map: its picture and how much ground it covers, what is marked on it, and
        // where the truck is on it, or the driver on foot, and the way they face, in degrees
        // round from north.
        if let Some((file, half)) = &self.chart {
            state["chart"] = json!({ "file": file, "half": half });
            state["waypoint"] = match &self.waypoint {
                Some((title, at)) => json!({ "title": title, "x": at.x, "z": at.z }),
                None => Value::Null,
            };
            state["places"] = self
                .places
                .iter()
                .filter(|place| !place.stand())
                .map(|place| {
                    let named = place.edge().unwrap_or(&place.map);
                    let (title, kind) = match self.world.map(named) {
                        Some(map) if place.edge().is_some() => (map.title.as_str(), "edge"),
                        Some(map) => (map.title.as_str(), "trailhead"),
                        None if place.map == SHED => ("The Shed", "shed"),
                        None if place.map == BUYER => ("Gold Buyer", "buyer"),
                        None => match place.door() {
                            Some("shop") => ("Auto shop", "door"),
                            Some("used") => ("Used trucks", "door"),
                            Some(_) => ("Bank", "door"),
                            None => (place.map.as_str(), "trailhead"),
                        },
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
            if self.mode.timed() {
                if let Some(lap) = &hud.lap {
                    state["lap"] = json!({
                        "count": lap.count,
                        "time": lap.current,
                        "last": lap.last,
                        "best": lap.best,
                        "valid": lap.valid,
                        "last_valid": lap.last_valid,
                        // Which section of the lap it is in, of how many; and how often the
                        // truck has been put back on it, and what that has cost.
                        "section": lap.sector + 1,
                        "sections": lap.best_splits.len(),
                        "resets": lap.resets,
                        "penalty": PUT_BACK * f64::from(lap.resets),
                    });
                    // How the lap stands against each medal, and where the others are.
                    if let (Some(course), Some(seconds), Screen::Drive) =
                        (&self.course, lap.current, self.screen)
                    {
                        state["pace"] = course.pace(self.class, seconds, lap.along);
                    }
                }
            }
            state["near"] = self.nearest(hud);
        }
        // Which key and which button does what: with every new screen, and whenever they
        // change. The pages keep what they were last told.
        if changed || unbound {
            state["binds"] = self.binds();
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
            "pad": self.pad,
            "pad_kind": self.pad_kind.name(),
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

    /// What a place is called: the map it leads to, or what is there.
    fn called<'a>(&'a self, place: &'a Place) -> &'a str {
        match self.world.map(place.edge().unwrap_or(&place.map)) {
            Some(map) => map.title.as_str(),
            None if place.map == SHED => "The Shed",
            None if place.map == BUYER => "Gold Buyer",
            None => match place.door() {
                Some("shop") => "Auto shop",
                Some("used") => "Used trucks",
                Some(_) => "Bank",
                None => place.map.as_str(),
            },
        }
    }

    /// The nearest trailhead to the truck: what it leads to, how far off it is, which way
    /// from straight ahead (in degrees, to the right above zero), and whether the truck is in
    /// its ring.
    fn nearest(&self, hud: &Hud) -> Value {
        let at = truck(hud);
        // The place the truck is in; or the nearest trailhead, and the shed as well once
        // there is something in the truck to take back to it.
        let inside = self.inside.and_then(|index| self.places.get(index));
        // Somewhere chosen on the map is where the way is pointed to, unless the truck is
        // in a ring, which says what it leads to.
        if let (None, Some((title, there))) = (inside, &self.waypoint) {
            let to = *there - at;
            let ahead = hud.forward;
            let bearing = (ahead.x * to.z - ahead.z * to.x).atan2(ahead.x * to.x + ahead.z * to.z);
            return json!({
                "title": title,
                "distance": flat_distance(*there, at).round(),
                "bearing": bearing.to_degrees().round(),
                "inside": false,
                "hold": 0.0,
            });
        }
        let Some(place) = inside.or_else(|| {
            self.places
                .iter()
                .filter(|place| place.door().is_none() && !place.stand())
                .filter(|place| (place.map != SHED && place.map != BUYER) || self.gold.tray().0 > 0)
                .min_by(|a, b| flat_distance(a.at, at).total_cmp(&flat_distance(b.at, at)))
        }) else {
            return Value::Null;
        };
        let title = self.called(place);
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

/// Whose controller it is, which decides how the pages draw its buttons: the same button is
/// A on one and a cross on the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKind {
    /// An Xbox controller, and any that is not known to be something else.
    Xbox,
    /// A PlayStation controller: a DualSense or a DualShock.
    PlayStation,
}

impl PadKind {
    /// What the pages are told it is.
    pub fn name(self) -> &'static str {
        match self {
            Self::Xbox => "xbox",
            Self::PlayStation => "playstation",
        }
    }

    /// From what a controller says of itself: its name (a browser's `Gamepad.id`, which has
    /// its maker's number in it, or the system's name for it) and its maker's number, where
    /// that is known apart.
    pub fn of(name: &str, vendor: Option<u16>) -> Self {
        const SONY: u16 = 0x054C;
        let name = name.to_lowercase();
        let sony = vendor == Some(SONY)
            || [
                "054c",
                "dualsense",
                "dualshock",
                "playstation",
                "sony",
                "ps3",
                "ps4",
                "ps5",
            ]
            .iter()
            .any(|word| name.contains(word))
            // What Windows calls one, with nothing before it: an Xbox pad is an "Xbox
            // Wireless Controller".
            || name.trim() == "wireless controller";
        if sony {
            Self::PlayStation
        } else {
            Self::Xbox
        }
    }
}

impl Shell {
    /// Whose controller is plugged in, from whoever reads it: its buttons are the ones the
    /// pages draw.
    pub fn pad_kind(&mut self, kind: PadKind) {
        self.pad_kind = kind;
    }

    /// The game's actions as the map being played binds them, from its `.input` file. Told
    /// again and again, it is only taken in when it has changed.
    pub fn bound(&mut self, keys: &InputMap) {
        if self.keys.as_ref() != Some(keys) {
            self.keys = Some(keys.clone());
            self.told_binds = false;
        }
    }

    /// What does what, for the pages: each action of the game's and of the menus', and the
    /// controls bound to it on the keyboard (`key`), the mouse (`mouse`) and a controller
    /// (`pad`), in the order they are bound. One that works the action backwards, a key
    /// that steers left, has a `-` before it.
    fn binds(&self) -> Value {
        use solstice_game::Device;
        let mut actions: BTreeMap<&str, BTreeMap<&str, Vec<String>>> = BTreeMap::new();
        let game = self.keys.iter().flat_map(|keys| &keys.bindings);
        for bound in game.chain(&self.menu.bindings) {
            let device = match bound.device {
                Device::Key => "key",
                Device::Mouse => "mouse",
                Device::Gamepad => "pad",
                Device::Touch => continue,
            };
            let sign = if bound.scale < 0.0 { "-" } else { "" };
            let control = format!("{sign}{}", bound.control);
            let controls = actions
                .entry(&bound.action)
                .or_default()
                .entry(device)
                .or_default();
            if !controls.contains(&control) {
                controls.push(control);
            }
        }
        json!(actions)
    }
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
    fn a_controller_is_known_by_what_it_says_it_is() {
        use PadKind::{PlayStation, Xbox};
        // A browser's names for them, the system's, and a maker's number alone.
        for (name, vendor, kind) in [
            (
                "DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)",
                None,
                PlayStation,
            ),
            ("054c-09cc-Wireless Controller", None, PlayStation),
            ("Wireless Controller", None, PlayStation),
            ("PS4 Controller", None, PlayStation),
            ("HID-compliant game controller", Some(0x054C), PlayStation),
            ("Xbox Wireless Controller", None, Xbox),
            ("Xbox 360 Controller (XInput STANDARD GAMEPAD)", None, Xbox),
            ("Xbox Wireless Controller", Some(0x045E), Xbox),
            // One that is nobody's in particular is drawn as an Xbox's.
            ("Generic USB Joystick", Some(0x0079), Xbox),
            ("", None, Xbox),
        ] {
            assert_eq!(PadKind::of(name, vendor), kind, "{name:?} {vendor:?}");
        }
    }

    #[test]
    fn the_pages_are_told_whose_buttons_to_draw_the_moment_another_device_is_in_hand() {
        let mut shell = shell();
        let mut devices = Devices::new();
        let told = |shell: &mut Shell| -> Option<Value> {
            let told = shell.tell(None, 0.001)?;
            Some(serde_json::from_str(&told).expect("it is JSON"))
        };
        // At first: the keyboard, and what is bound to what.
        let first = told(&mut shell).expect("the first screen is told of");
        assert_eq!(
            (&first["pad"], &first["pad_kind"]),
            (&json!(false), &json!("xbox"))
        );
        assert_eq!(
            first["binds"]["menu_accept"]["key"],
            json!(["Enter", "Space"])
        );
        assert_eq!(first["binds"]["menu_accept"]["pad"], json!(["south"]));
        assert!(told(&mut shell).is_none(), "nothing has changed");
        // A stick resting off its middle is not a hand on the controller.
        devices.gamepad_axis("left_x", 0.3);
        shell.before(&mut devices, 0.016);
        devices.end_frame();
        assert!(told(&mut shell).is_none());
        // A trigger pulled is, though no menu is worked with it: told at once, on a menu.
        devices.gamepad_button("r2", 1.0);
        shell.before(&mut devices, 0.016);
        devices.end_frame();
        let pad = told(&mut shell).expect("a controller in hand is told of");
        assert_eq!(
            (&pad["pad"], &pad["pad_kind"]),
            (&json!(true), &json!("xbox"))
        );
        assert!(pad.get("binds").is_none(), "the pages have them already");
        // It turns out to be a PlayStation's.
        shell.hear(
            "pad kind DualSense Wireless Controller (Vendor: 054c Product: 0ce6)",
            &mut devices,
        );
        assert_eq!(told(&mut shell).expect("told")["pad_kind"], "playstation");
        // The trigger held, a key is pressed: the keyboard, and the family is not forgotten.
        shell.hear("key KeyW 1", &mut devices);
        shell.before(&mut devices, 0.016);
        devices.end_frame();
        let keys = told(&mut shell).expect("the keyboard in hand is told of");
        assert_eq!(
            (&keys["pad"], &keys["pad_kind"]),
            (&json!(false), &json!("playstation"))
        );
        // And a click of the mouse after the controller is the keyboard and mouse too.
        shell.pad = true;
        devices.mouse_button("left", true);
        shell.before(&mut devices, 0.016);
        assert!(!shell.pad);
    }

    #[test]
    fn the_pages_are_told_what_the_map_binds_and_again_when_it_changes() {
        let mut shell = shell();
        let told = |shell: &mut Shell| -> Option<Value> {
            let told = shell.tell(None, 0.001)?;
            Some(serde_json::from_str(&told).expect("it is JSON"))
        };
        told(&mut shell);
        let keys = InputMap::parse(
            "steer key KeyD\nsteer key KeyA -1\nsteer gamepad left_x\nsteer touch stick_x\n\
             orbit mouse x -0.003\nthrottle key KeyW\nthrottle key ArrowUp\nthrottle gamepad r2\n",
        )
        .unwrap();
        shell.bound(&keys);
        let binds = told(&mut shell).expect("new bindings are told of")["binds"].clone();
        // Each device's controls in the order they are bound; backwards ones marked; a
        // touched screen's left out; the menus' own beside the game's.
        assert_eq!(
            binds["steer"],
            json!({ "key": ["KeyD", "-KeyA"], "pad": ["left_x"] })
        );
        assert_eq!(binds["orbit"], json!({ "mouse": ["-x"] }));
        assert_eq!(
            binds["throttle"],
            json!({ "key": ["KeyW", "ArrowUp"], "pad": ["r2"] })
        );
        assert_eq!(
            binds["pause"],
            json!({ "key": ["Escape"], "pad": ["start"] })
        );
        // The same again is not news.
        shell.bound(&keys);
        assert!(told(&mut shell).is_none());
        // Another screen has them with it, whatever the page has kept.
        shell.hear("hello", &mut Devices::new());
        assert!(told(&mut shell).expect("told")["binds"]["steer"].is_object());
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
                holds: &[],
                doors: &[],
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
            arrive: None,
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
        // (From midday, the first sleep there is is till the morning, which is tomorrow's.)
        assert!(shell
            .take_orders()
            .iter()
            .any(|order| matches!(order, Order::Sleep { days: 1, .. })));
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
    fn driving_off_the_edge_of_a_map_by_its_road_is_driving_into_the_next() {
        let mut shell = shell();
        let mut devices = Devices::new();
        shell.hear("do start", &mut devices);
        // The hub has a way out to the track at its edge: a truck that comes in by it is
        // put thirty metres in, facing back.
        shell.places.push(Place {
            map: "edge:wombat".to_owned(),
            at: Vec3::new(-488.0, 5.0, 100.0),
            radius: 8.0,
            arrive: Some((Vec3::new(-460.0, 5.0, 100.0), 90.0)),
        });
        // A truck that is in it when the map is come into is not sent on: not until it has
        // been clear of it.
        shell.after(Some(&hud_at(Vec3::new(-487.0, 5.0, 100.0), 6.0)));
        assert_eq!(shell.screen(), Screen::Drive);
        shell.after(Some(&hud_at(Vec3::new(-440.0, 5.0, 100.0), 6.0)));
        assert_eq!(shell.screen(), Screen::Drive);
        // Driven down the road to it, the next map is read, with nothing asked.
        shell.after(Some(&hud_at(Vec3::new(-486.0, 5.0, 101.0), 12.0)));
        assert_eq!(shell.screen(), Screen::Loading);
        let loads: Vec<Order> = shell
            .take_orders()
            .into_iter()
            .filter(|order| matches!(order, Order::Load { .. }))
            .collect();
        assert!(matches!(&loads[..], [Order::Load { map, .. }] if map == "wombat"));
        // And that map is come into by its own way out to here: on its road, facing in,
        // wherever a truck was last left in it.
        let files = [GameFile {
            name: "outback.places".to_owned(),
            text: "place edge:crossroads 300 7 -340 8 270 7.5 -330 -120\n".to_owned(),
        }];
        shell.enter(&files);
        let placed = shell
            .take_orders()
            .into_iter()
            .find_map(|order| match order {
                Order::Place { position, yaw } => Some((position, yaw)),
                _ => None,
            });
        assert_eq!(placed, Some((Vec3::new(270.0, 7.5, -330.0), -120.0)));
        // There it is in nothing that is stopped in, and the way home is that road.
        shell.after(Some(&hud_at(Vec3::new(300.0, 7.0, -340.0), 0.0)));
        assert_eq!(shell.screen(), Screen::Drive);
        assert!(shell.inside.is_none() && shell.way_home());
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
        // (Entering a map also puts back on the truck what was on it: that is not a map read.)
        let loads: Vec<Order> = shell
            .take_orders()
            .into_iter()
            .filter(|order| matches!(order, Order::Load { .. }))
            .collect();
        // The hub is come back to at whatever hour it has got to, in the day's own weather.
        assert!(matches!(
            &loads[..],
            [Order::Load { map, sky: Some((hour, _, _)), .. }] if map == "crossroads" && *hour == KEEP_HOUR
        ));
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

    /// A game taken from the Crossroads to Wombat Creek, which has a track to time, in a
    /// class (from 0), for `what`: `timed` or `practice`. At night in a storm, if asked.
    fn at_the_line(kept: &str, class: usize, what: &str, foul: bool) -> (Shell, Devices) {
        let mut shell = Shell::new(World::parse(WORLD).expect("the world reads"), kept);
        shell.enter(&[GameFile {
            name: "hub.places".to_owned(),
            text: "place wombat 100 5 -40 9\n".to_owned(),
        }]);
        let mut devices = Devices::new();
        shell.hear("do start", &mut devices);
        shell.stop_at(0);
        shell.hear(&format!("set class {class}"), &mut devices);
        if foul {
            shell.hear("set time 3", &mut devices);
            shell.hear("set weather 3", &mut devices);
        } else {
            shell.hear("set weather 1", &mut devices);
        }
        shell.hear(&format!("do {what}"), &mut devices);
        assert_eq!(shell.screen(), Screen::Loading);
        shell.take_orders();
        shell.enter(&crate::timed::tests::square());
        (shell, devices)
    }

    fn told(shell: &mut Shell, hud: Option<&Hud>) -> Value {
        serde_json::from_str(&shell.tell(hud, 1.0).expect("told")).expect("JSON")
    }

    /// The truck's instruments on the track, with a lap: so many finished, the last of them
    /// in so many seconds.
    fn lapping(shell: &Shell, count: u32, last: Option<f64>, valid: bool) -> Hud {
        let track = &shell.course.as_ref().expect("a course").track;
        let mut hud = hud_at(Vec3::new(0.0, 5.0, -40.0), 9.0);
        hud.tray = Some((Vec3::new(1.55, 0.545, 1.7), Vec3::new(0.0, 0.905, 1.9)));
        let mut lap = solstice_game::race::Lap::new(track);
        (lap.count, lap.last, lap.last_valid) = (count, last, valid);
        (lap.current, lap.along) = (Some(20.0), 200.0);
        hud.lap = Some(lap);
        hud
    }

    fn staged(orders: &[Order]) -> Vec<Stage> {
        orders
            .iter()
            .filter_map(|order| match order {
                Order::Stage(stage) => Some(*stage),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_timed_run_is_lined_up_driven_once_and_paid_for_in_gold() {
        use crate::timed::Medal;
        let (mut shell, mut devices) = at_the_line("", 1, "timed", false);
        // On the map, before the run: the line-up, with the view going round the truck.
        assert_eq!(shell.screen(), Screen::Lineup);
        assert!(matches!(
            shell.before(&mut devices, 0.016),
            Step::Unattended(_)
        ));
        let gold = shell.course.as_ref().expect("a course").golds()[1];
        let said = told(&mut shell, None);
        assert_eq!(
            (&said["screen"], &said["mode"]),
            (&json!("lineup"), &json!("timed"))
        );
        assert_eq!(said["run"]["class"], 1);
        assert_eq!(said["run"]["name"], "Old tourers");
        let near = |said: &Value| {
            said.as_f64()
                .is_some_and(|time| (time - f64::from(gold)).abs() < 1.0e-3)
        };
        assert!(near(&said["run"]["medals"][2]), "{said}");
        assert_eq!(said["run"]["rivals"].as_array().map(Vec::len), Some(3));
        assert!(near(&said["run"]["rivals"][2]["time"]), "{said}");
        assert_eq!(
            (&said["run"]["held"], &said["run"]["bonus"]),
            (&Value::Null, &json!(0.0))
        );
        // Away: the truck is put behind the line, and it is driven.
        shell.hear("do go", &mut devices);
        assert_eq!(staged(&shell.take_orders()), vec![Stage::Line(RUN_UP)]);
        assert_eq!(shell.before(&mut devices, 0.016), Step::Play);
        // On the lap: how it stands against each medal, and where the others are.
        let hud = lapping(&shell, 0, None, true);
        shell.after(Some(&hud));
        let said = told(&mut shell, Some(&hud));
        assert_eq!(said["pace"]["gaps"].as_array().map(Vec::len), Some(3));
        assert_eq!(said["pace"]["ghosts"].as_array().map(Vec::len), Some(3));
        assert_eq!(shell.screen(), Screen::Drive);
        // Over the line inside the gold time: gold, and every purse of the class, for the
        // first time. It is weighed out into the tray.
        let seconds = f64::from(gold) - 1.5;
        let hud = lapping(&shell, 1, Some(seconds), true);
        shell.after(Some(&hud));
        assert_eq!(shell.screen(), Screen::Result);
        assert_eq!(shell.before(&mut devices, 0.016), Step::Held);
        let purses: u64 = Medal::ALL.iter().map(|medal| medal.purse(gold, 1)).sum();
        let said = told(&mut shell, Some(&hud));
        let result = &said["result"];
        assert_eq!(
            (&result["medal"], &result["held"]),
            (&json!("gold"), &Value::Null)
        );
        assert_eq!(result["firsts"].as_array().map(Vec::len), Some(3));
        assert_eq!(result["paid"].as_u64(), Some(purses));
        assert_eq!(
            (&result["night"], &result["rain"]),
            (&Value::Null, &Value::Null)
        );
        let grams = crate::timed::grams(purses, PRICE);
        assert_eq!(result["grams"].as_u64(), Some(u64::from(grams)));
        let (count, kilos) = shell.gold.tray();
        assert_eq!(count, 1);
        assert!((kilos - grams as f32 / 1000.0).abs() < 1.0e-4);
        assert_eq!(shell.money(), 0, "it is gold until it is sold");
        assert!(shell.take_orders().contains(&Order::Sound("coins")));
        // What was won is kept, with the times the track was given.
        let kept = shell.keep_now();
        assert!(
            kept.contains(&format!("run wombat 2 3 {seconds:.3}\n")),
            "{kept}"
        );
        assert!(
            kept.contains("par wombat ") && kept.contains("class 2\n"),
            "{kept}"
        );
        // Again, as fast: the lap that was is not this run's; the next is, and pays a tenth
        // of gold's purse, since nothing new was won.
        shell.hear("do again", &mut devices);
        assert_eq!(staged(&shell.take_orders()), vec![Stage::Line(RUN_UP)]);
        shell.after(Some(&hud));
        assert_eq!(shell.screen(), Screen::Drive);
        let _ = shell.tell(Some(&hud), 1.0);
        shell.after(Some(&lapping(&shell, 0, None, true)));
        shell.after(Some(&lapping(&shell, 1, Some(seconds + 0.5), true)));
        assert_eq!(shell.screen(), Screen::Result);
        let said = told(&mut shell, Some(&hud));
        let result = &said["result"];
        assert_eq!(
            (&result["medal"], &result["held"]),
            (&json!("gold"), &json!("gold"))
        );
        assert_eq!(result["firsts"].as_array().map(Vec::len), Some(0));
        let tenth = Medal::Gold.purse(gold, 1) / 10;
        assert!(result["paid"]
            .as_u64()
            .is_some_and(|paid| paid.abs_diff(tenth) <= 5));
        assert_eq!(result["record"], false);
        assert_eq!(shell.gold.tray().0, 2);
        // A run on which the truck was put back on the track has a time and nothing else.
        shell.hear("do again", &mut devices);
        let _ = shell.tell(Some(&hud), 1.0);
        shell.after(Some(&lapping(&shell, 0, None, true)));
        shell.after(Some(&lapping(&shell, 1, Some(seconds - 30.0), false)));
        let said = told(&mut shell, Some(&hud));
        assert_eq!(
            (&said["result"]["valid"], &said["result"]["medal"]),
            (&json!(false), &Value::Null)
        );
        assert_eq!(said["result"]["paid"], 0);
        assert_eq!(shell.gold.tray().0, 2);
        assert!(shell
            .keep_now()
            .contains(&format!("run wombat 2 3 {seconds:.3}\n")));
        // From the result the clock can be left for the bush, on the same map.
        shell.hear("do roam", &mut devices);
        assert_eq!((shell.screen(), shell.mode), (Screen::Drive, Mode::Roam));
        // A game started from what was kept has what was won.
        let again = Shell::new(World::parse(WORLD).expect("reads"), &shell.keep_now());
        assert_eq!(again.records.of("wombat", 1).medal, Some(Medal::Gold));
        assert_eq!(again.class, 1);
    }

    #[test]
    fn a_run_at_night_in_a_storm_pays_double_and_a_slower_one_wins_less() {
        use crate::timed::Medal;
        let (mut shell, mut devices) = at_the_line("", 0, "timed", true);
        let gold = shell.course.as_ref().expect("a course").golds()[0];
        let said = told(&mut shell, None);
        assert_eq!(
            (&said["run"]["night"], &said["run"]["rain"]),
            (&json!(true), &json!(true))
        );
        assert_eq!(said["run"]["bonus"], 100.0);
        shell.hear("do go", &mut devices);
        shell.after(Some(&lapping(&shell, 0, None, true)));
        // Inside silver's time and outside gold's: bronze's purse and silver's, doubled.
        let seconds = f64::from(Medal::Silver.time(gold)) - 0.2;
        let hud = lapping(&shell, 1, Some(seconds), true);
        shell.after(Some(&hud));
        let said = told(&mut shell, Some(&hud));
        let base = Medal::Bronze.purse(gold, 0) + Medal::Silver.purse(gold, 0);
        let result = &said["result"];
        assert_eq!(result["medal"], "silver");
        assert_eq!(result["night"].as_u64(), Some(base / 2));
        assert_eq!(result["rain"].as_u64(), Some(base / 2));
        assert_eq!(result["paid"].as_u64(), Some(base * 2));
        // Then gold in the dry by day: only gold's purse is left to win.
        let kept = shell.keep_now();
        let (mut shell, mut devices) = at_the_line(&kept, 0, "timed", false);
        shell.hear("do go", &mut devices);
        shell.after(Some(&lapping(&shell, 0, None, true)));
        let hud = lapping(&shell, 1, Some(f64::from(gold) - 3.0), true);
        shell.after(Some(&hud));
        let said = told(&mut shell, Some(&hud));
        assert_eq!(said["result"]["held"], "silver");
        assert_eq!(
            said["result"]["paid"].as_u64(),
            Some(Medal::Gold.purse(gold, 0))
        );
    }

    #[test]
    fn practice_is_timed_lap_after_lap_with_nothing_at_stake() {
        let (mut shell, mut devices) = at_the_line("", 2, "practice", false);
        assert_eq!(shell.screen(), Screen::Lineup);
        shell.hear("do go", &mut devices);
        shell.take_orders();
        shell.after(Some(&lapping(&shell, 0, None, true)));
        // A lap finished inside the gold time: no result, no medal, nothing paid.
        let mut hud = lapping(&shell, 1, Some(50.0), true);
        hud.lap.as_mut().expect("a lap").best = Some(50.0);
        shell.after(Some(&hud));
        assert_eq!(shell.screen(), Screen::Drive);
        assert_eq!(shell.gold.tray().0, 0);
        assert_eq!(
            shell.records.of("wombat", 2),
            crate::timed::Record::default()
        );
        // Only the best lap is kept.
        let kept = shell.keep_now();
        assert!(
            kept.contains("best wombat 50.000\n") && !kept.contains("run wombat"),
            "{kept}"
        );
        let said = told(&mut shell, Some(&hud));
        assert_eq!(
            (&said["mode"], &said["lap"]["count"]),
            (&json!("practice"), &json!(1))
        );
        // Restarting from the pause menu goes back to the line without reading the map.
        shell.hear("do pause", &mut devices);
        assert!(told(&mut shell, Some(&hud))["calendar"]["days"].is_array());
        shell.hear("do restart", &mut devices);
        assert_eq!(shell.screen(), Screen::Lineup);
        assert_eq!(staged(&shell.take_orders()), vec![Stage::Line(RUN_UP)]);
    }

    #[test]
    fn another_drivers_run_is_watched_and_can_be_skipped() {
        let (mut shell, mut devices) = at_the_line("", 3, "timed", false);
        shell.hear("do watch2", &mut devices);
        assert_eq!(shell.screen(), Screen::Watch);
        // The truck is painted as theirs, the game drives it, and it is put at the start.
        let stages = staged(&shell.take_orders());
        assert!(matches!(
            stages[..],
            [Stage::Paint(Some(_)), Stage::Drive(Some(_)), Stage::Line(_)]
        ));
        // The game runs with nobody at the controls, and the camera is the shot's once the
        // truck has been seen.
        assert_eq!(shell.before(&mut devices, 0.016), Step::Unattended(0.0));
        assert_eq!(shell.view(), None);
        let hud = lapping(&shell, 0, None, true);
        shell.after(Some(&hud));
        let (eye, _, _) = shell.view().expect("a shot");
        assert!(eye.distance(Vec3::new(0.0, 5.0, -40.0)) < 60.0);
        let said = told(&mut shell, Some(&hud));
        assert_eq!(
            (&said["screen"], &said["watch"]["medal"]),
            (&json!("watch"), &json!("gold"))
        );
        assert_eq!(said["watch"]["shot"], 1);
        // Left to run with the truck going nowhere, every shot is cut and it ends at the line.
        let mut cuts = 0;
        for _ in 0..8000 {
            shell.before(&mut devices, 0.05);
            shell.after(Some(&hud));
            cuts += staged(&shell.take_orders())
                .iter()
                .filter(|stage| matches!(stage, Stage::Line(back) if *back != RUN_UP))
                .count();
            if shell.screen() != Screen::Watch {
                break;
            }
        }
        assert!(cuts >= 1, "{cuts}");
        assert_eq!((shell.screen(), shell.view()), (Screen::Lineup, None));
        // Watched again, and skipped with a button: the truck is the player's again.
        shell.hear("do watch0", &mut devices);
        shell.take_orders();
        shell.before(&mut devices, 0.016);
        devices.end_frame();
        devices.key("Enter", true);
        shell.before(&mut devices, 0.016);
        assert_eq!(shell.screen(), Screen::Lineup);
        assert_eq!(
            staged(&shell.take_orders()),
            vec![Stage::Drive(None), Stage::Paint(None), Stage::Line(RUN_UP)]
        );
    }

    #[test]
    fn a_trailhead_offers_every_class_and_todays_weather() {
        let mut shell = shell();
        let mut devices = Devices::new();
        shell.hear("do start", &mut devices);
        // A game started for the first time is given its own weather, which is kept.
        let seed = shell.seed.expect("a seed");
        assert!(shell.keep_now().contains(&format!("seed {seed}\n")));
        shell.stop_at(0);
        let said = told(&mut shell, None);
        let classes = said["classes"].as_array().expect("classes");
        assert_eq!(classes.len(), 5);
        // Its times are not known until the map has been driven to.
        assert_eq!(
            (&classes[0]["name"], &classes[0]["medals"]),
            (&json!("Paddock bashers"), &Value::Null)
        );
        assert_eq!(classes[4]["rivals"].as_array().map(Vec::len), Some(3));
        let today = crate::calendar::weather(seed, 1) as u64;
        assert_eq!(
            (said["today"].as_u64(), said["weather"].as_u64()),
            (Some(today), Some(today))
        );
        assert_eq!(
            said["bonus"],
            json!({ "time": [0, 0, 0, 50], "weather": [0, 0, 0, 50] })
        );
        // Once it has been, they are on its card.
        let (mut shell, mut devices) = at_the_line("", 0, "roam", false);
        assert_eq!(shell.screen(), Screen::Drive);
        let kept = shell.keep_now();
        let mut shell_again = Shell::new(World::parse(WORLD).expect("reads"), &kept);
        shell_again.enter(&[GameFile {
            name: "hub.places".to_owned(),
            text: "place wombat 100 5 -40 9\n".to_owned(),
        }]);
        shell_again.hear("do start", &mut devices);
        shell_again.stop_at(0);
        let said = told(&mut shell_again, None);
        let medals = said["classes"][0]["medals"].as_array().expect("times");
        assert!(medals[0].as_f64() > medals[1].as_f64() && medals[1].as_f64() > medals[2].as_f64());
        assert!(
            said["classes"][0]["purses"][2].as_u64() > said["classes"][0]["purses"][0].as_u64()
        );
        // In a browser a map is another page: what it is driven for and in goes with it.
        shell.class = 3;
        (shell.time, shell.weather, shell.mode) = (3, 3, Mode::Timed);
        assert_eq!(shell.mode_name(), "timed.3.3.3");
        let mut next = Shell::new(World::parse(WORLD).expect("reads"), "");
        next.start_in("wombat");
        next.resume("timed.3.3.3");
        next.enter(&crate::timed::tests::square());
        assert_eq!(
            (next.screen(), next.class, next.conditions()),
            (Screen::Lineup, 3, (true, true))
        );
        next.resume("laps");
        assert_eq!(next.mode, Mode::Practice);
    }

    #[test]
    fn a_hurt_piece_is_said_as_a_driver_would_say_it() {
        assert_eq!(piece_said("bumper_front"), "Front bumper");
        assert_eq!(piece_said("lamp_head_R"), "Right lamp head");
        assert_eq!(piece_said("door_FL"), "Front left door");
        assert_eq!(piece_said("acc_bullbar_tube"), "Bullbar tube");
        assert_eq!(piece_said("grille"), "Grille");
    }

    const STORY: &str = "hub crossroads
map crossroads assets/hub.solstice The Crossroads
map town assets/town.solstice Ironbark Flat
map showroom assets/hub.solstice The Showroom
kind showroom Showroom
map showroom_town assets/town.solstice Ironbark Flat
kind showroom_town Township
truck bendoc 1 6000 1981 Bendoc
truck dargo 1 6500 1984 Dargo
truck omeo 1 8000 1988 Omeo
truck mitta 2 16000 1995 Mitta
";

    #[test]
    fn a_game_is_carried_from_one_page_of_a_browser_to_the_next() {
        let world = || World::parse(STORY).expect("the world reads");
        let taken_up = |carried: &str| {
            let kept = Shell::carried_kept(carried).expect("the game is carried with it");
            let mut next = Shell::new(world(), kept);
            next.take_up(carried);
            next
        };
        // A new game is begun in the first place: its first truck is chosen in the showroom,
        // which is another page.
        let mut shell = Shell::new(world(), "");
        shell.set_web();
        shell.set_saves(&[(1, Some(String::new()))], 1);
        shell.load("");
        let asked = shell.take_orders();
        assert!(asked
            .iter()
            .any(|order| matches!(order, Order::Load { map, .. } if map == SHOWROOM)));
        let mut next = taken_up(&shell.carry());
        assert_eq!(
            (
                next.screen,
                next.yard_page,
                next.map.as_str(),
                next.playing()
            ),
            (Screen::Yard, "start", SHOWROOM, 1)
        );
        // That page is read with the trucks that are offered, and no others.
        let mut offered: Vec<String> = crate::yard::starters(&world().trucks)
            .into_iter()
            .map(|truck| truck.name.clone())
            .collect();
        offered.sort();
        assert_eq!(next.packs(), offered);
        // One is taken: the story has begun, and the hub is read with that truck.
        next.enter(&[]);
        next.yard_act(&format!("begin:{}", offered[0]));
        let mut last = taken_up(&next.carry());
        assert!(last.story);
        assert_eq!(last.map, "crossroads");
        assert_eq!(last.packs(), vec![offered[0].clone()]);
        assert_eq!(
            last.wearing().map(|(truck, _)| truck),
            Some(offered[0].clone())
        );
        last.enter(&[]);
        assert_eq!(last.screen, Screen::Drive);
        // In the town the day's used trucks are parked, and their packs are wanted too.
        last.go("town", Mode::Roam);
        let town = taken_up(&last.carry());
        assert!(town.packs().len() > 1);
        // A free drive's truck as a monster truck: its build says so, the monster kit's pack
        // is wanted with its own, and the next page knows.
        let mut free = Shell::new(world(), "");
        free.open_yard("free");
        free.yard_act("monster:on");
        assert!(free
            .page_trucks()
            .iter()
            .all(|(_, build, _)| build == "monster:stock"));
        assert!(free.packs().contains(&crate::kit::MONSTER.to_owned()));
        free.enter(&[]);
        // The truck is chosen: it is built next, standing by itself with every piece of it
        // and of the monster kit; and then where and when it is driven are chosen.
        free.yard_act("free:dargo");
        assert_eq!(free.yard_page, "tune");
        assert_eq!(
            free.page_trucks(),
            [("dargo".to_owned(), "monster:all".to_owned(), None)]
        );
        let mut building = taken_up(&free.carry());
        assert_eq!(building.yard_page, "tune");
        assert_eq!(building.packs(), ["dargo", "monster"]);
        // A part is fitted and a colour chosen, and its own tyres too: but a monster truck
        // stands on the kit's tyres, and those are not said of it.
        building.yard_act("fit:bullbar:tube");
        building.yard_act("fit:tyres:mud");
        building.yard_act("paint:b01818");
        assert_eq!(building.free_worn(), "bullbar=tube");
        building.yard_act("page:where");
        assert_eq!(building.yard_page, "where");
        // At dusk, in a storm.
        building.yard_act("time:2");
        building.yard_act("weather:3");
        building.take_orders();
        building.yard_act("drive");
        let asked = building.take_orders();
        let sky = asked.iter().find_map(|order| match order {
            Order::Load { map, sky, .. } if map == "crossroads" => *sky,
            _ => None,
        });
        assert_eq!(sky, Some((TIMES[2].1, "storm", WEATHERS[3].2)));
        let mut driven = taken_up(&building.carry());
        assert_eq!(
            driven.wearing(),
            Some(("dargo".to_owned(), "monster:bullbar=tube".to_owned()))
        );
        assert_eq!(driven.packs(), ["dargo", "monster"]);
        assert_eq!(driven.free_paint, Some(0xb01818));
        // As it is, and not a monster truck, it has the tyres that were chosen for it.
        driven.monster = false;
        assert_eq!(
            driven.would_wear(),
            Some(("dargo".to_owned(), "bullbar=tube,tyres=mud".to_owned()))
        );
        // It goes on in the weather that was chosen, at whatever hour it has got to.
        driven.enter(&[]);
        driven.take_orders();
        driven.go("town", Mode::Roam);
        let on = driven.take_orders();
        let sky = on.iter().find_map(|order| match order {
            Order::Load { sky, .. } => *sky,
            _ => None,
        });
        assert_eq!(sky, Some((KEEP_HOUR, "storm", WEATHERS[3].2)));
        // Paused, its damage is switched off, which whoever runs the game is told; and its
        // map is started again, where the map starts and at the hour that was chosen.
        driven.enter(&[]);
        driven.take_orders();
        driven.poses.insert("town".to_owned(), (Vec3::ONE, 0.0));
        driven.screen = Screen::Pause;
        driven.act("damage");
        let told = driven.take_orders();
        assert!(told.iter().any(|order| matches!(order, Order::Harm(false))));
        driven.act("restart");
        let again = driven.take_orders();
        assert!(again.iter().any(|order| matches!(
            order,
            Order::Load { map, sky: Some((hour, ..)), .. } if map == "town" && *hour == TIMES[2].1
        )));
        assert!(!driven.poses.contains_key("town"));
        // It is still off on the map that is read next, a page later.
        let mut after = taken_up(&driven.carry());
        after.enter(&[]);
        let told = after.take_orders();
        assert!(told.iter().any(|order| matches!(order, Order::Harm(false))));
        // A free drive that is to start on another map than the hub: the map chosen is
        // known a page later, while the truck is still being chosen, and is the map that
        // is read when one is.
        let mut free = Shell::new(world(), "");
        free.open_yard("free");
        assert_eq!(free.yard_told()["start_map"], "crossroads");
        let offered: Vec<String> = free
            .free_maps()
            .iter()
            .map(|map| map.name.clone())
            .collect();
        assert_eq!(offered, ["crossroads", "town"]);
        free.yard_act("where:showroom");
        free.yard_act("where:town");
        let mut choosing = taken_up(&free.carry());
        assert_eq!(choosing.yard_told()["start_map"], "town");
        choosing.enter(&[]);
        choosing.take_orders();
        choosing.yard_act("free:dargo");
        choosing.yard_act("page:where");
        choosing.yard_act("drive");
        assert!(choosing
            .take_orders()
            .iter()
            .any(|order| matches!(order, Order::Load { map, .. } if map == "town")));
        let there = taken_up(&choosing.carry());
        assert_eq!((there.map.as_str(), there.story), ("town", false));
        // A door gone in at is come out of, on foot, a page later.
        let mut inside = taken_up(&last.carry());
        inside.door_from = Some(("town".to_owned(), Vec3::new(1.0, 2.0, 3.0)));
        let out = taken_up(&inside.carry());
        assert_eq!(
            out.door_from,
            Some(("town".to_owned(), Vec3::new(1.0, 2.0, 3.0)))
        );
    }
}
