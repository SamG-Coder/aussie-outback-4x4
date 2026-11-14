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
enter gamepad south
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

/// The trailheads a scene's gameplay files give: every `place MAP X Y Z RADIUS` line of every
/// file named `*.places`.
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
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Drive => "drive",
            Self::Pause => "pause",
            Self::Offer => "offer",
            Self::Loading => "loading",
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
}

impl Shell {
    /// A game of these maps, with the best laps there have been (`best MAP SECONDS` a line).
    pub fn new(world: World, kept: &str) -> Self {
        let mut bests = BTreeMap::new();
        for line in kept.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if let ["best", map, seconds] = words[..] {
                if let Ok(seconds) = seconds.parse::<f64>() {
                    bests.insert(map.to_owned(), seconds);
                }
            }
        }
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
        self.offered = None;
        self.inside = None;
        self.hold = 0.0;
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
        Some(
            self.bests
                .iter()
                .map(|(map, seconds)| format!("best {map} {seconds:.3}\n"))
                .collect(),
        )
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
            }
            (Screen::Pause, "resume") => self.screen = Screen::Drive,
            (Screen::Drive, "pause") => self.screen = Screen::Pause,
            // The ring's banner was clicked.
            (Screen::Drive, "enter") => {
                if let Some(index) = self.inside {
                    self.offered = Some(index);
                    self.screen = Screen::Offer;
                }
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
                // In a trailhead's ring, holding the button down is asking for its map.
                if self.inside.is_some() && menu.held("enter") {
                    self.hold += seconds;
                    if self.hold >= HOLD {
                        self.hold = 0.0;
                        self.offered = self.inside;
                        self.screen = Screen::Offer;
                    }
                } else {
                    self.hold = 0.0;
                }
            }
            Screen::Loading => {}
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
            Screen::Pause | Screen::Offer | Screen::Loading => Step::Held,
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
        self.inside = self
            .places
            .iter()
            .position(|place| flat_distance(place.at, at) < place.radius);
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
        let Some(place) = self
            .places
            .iter()
            .min_by(|a, b| flat_distance(a.at, at).total_cmp(&flat_distance(b.at, at)))
        else {
            return Value::Null;
        };
        let to = place.at - at;
        let ahead = hud.forward;
        let bearing = (ahead.x * to.z - ahead.z * to.x).atan2(ahead.x * to.x + ahead.z * to.z);
        let distance = flat_distance(place.at, at);
        json!({
            "title": self.world.map(&place.map).map_or(place.map.as_str(), |map| map.title.as_str()),
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
        assert_eq!(
            shell.take_orders(),
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
        // 301.5 is the best so far.
        shell.lapped(320.0);
        assert_eq!(shell.to_keep(), None);
        shell.lapped(288.25);
        assert_eq!(shell.to_keep().as_deref(), Some("best wombat 288.250\n"));
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
