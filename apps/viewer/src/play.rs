//! Play mode in a viewer: a scene's game, driving the renderer's instances and camera.
//!
//! Built only with the `gameplay` feature, so the plain web viewer carries none of it.

use solstice_asset::{Collision, GameFile};
use solstice_game::animation::AnimationAssets;
pub use solstice_game::Message;
use solstice_game::{Devices, Play};
use solstice_render::{Camera, Renderer};

/// A scene's game, running in a viewer.
pub struct PlayMode {
    pub play: Play,
    /// What the input devices are doing, filled in from the window's events.
    pub devices: Devices,
    /// Whether it is dark: the player's car then drives with its lamps on.
    pub lamps: bool,
    /// The instances of the player's car that are its tail lamps, and its headlamps.
    tail_lamps: Vec<u32>,
    head_lamps: Vec<u32>,
    /// What the player's car's wheels throw up (`mud.rs`).
    spray: crate::mud::Fling,
    /// What the player's car wears, the instances that are it, and where its axles are:
    /// its front's and its rear's along it, half its track, and its wheels' radius.
    wear: Wear,
    /// The colour the truck's paint was made in, which a respray is told from.
    made: glam::Vec3,
    car: Vec<u32>,
    axles: [f32; 4],
    /// The plants a car flattens, and which are down already.
    scrub: Scrub,
    /// The player's truck's kit, if it is one built of pieces, and what is fitted to it
    /// (`kit.rs`).
    kit: Option<crate::kit::Fitted>,
    /// The trucks stood in a showroom to be looked at, and which of them is shown.
    display: Vec<crate::kit::Fitted>,
    displayed: usize,
    /// The trucks parked in the map's used-truck yard, each its place on the lot and the
    /// instances that draw it; and whether whichever of them is shown is pulled out of its
    /// row, to stand where the trucks that are looked at do.
    parked: Vec<(usize, Vec<u32>)>,
    pulled: bool,
    /// Which side of a truck in the showroom the camera is on: the more open one.
    show_side: Option<f32>,
    /// Where the showroom's view was last, and what it looked at; and when that was, in the
    /// seconds the showroom has been looked at.
    show_view: Option<(glam::Vec3, glam::Vec3)>,
    show_at: f32,
    /// Where the trucks that stand to be looked at are, and how they are turned, if they
    /// are not where the map's own truck is.
    stand: Option<(glam::Vec3, glam::Quat)>,
    /// Whether the player's truck is not drawn: behind the main menu no game is being
    /// played, and the bush is all that is looked at.
    unseen: bool,
    /// The scene's blanks, each to show a nugget with (`nugget.rs`), and
    /// the number of the nugget that was last made on it.
    nuggets: Vec<(crate::nugget::Slot, Option<u32>)>,
    /// The nuggets that are bodies in the world: each its nugget's number, and its body.
    things: Vec<(u32, solstice_game::Entity)>,
    /// How the detector is shown, how far its shaft is tipped to keep its coil on the
    /// ground, where the driver last was and how fast they are walking, and how long it
    /// has been swept from side to side.
    carried: crate::gold::Carried,
    tipped: f32,
    walked: (glam::Vec3, f32),
    swept: f32,
    /// What is heard, once [`PlayMode::with_sound`] has opened an output for it, or, in a
    /// browser, once the page has said it will play what is asked of it.
    sound: Option<Soundscape>,
    /// Frames run, and whether to log where the player's car is every half second of them:
    /// set `SOLSTICE_TRACE` to try a track without watching it.
    frames: u32,
    trace: bool,
    /// The player's truck's damage: what it has hit, piece by piece, and how that is drawn
    /// (`wreck.rs`).
    wrecks: crate::wreck::Wrecks,
}

/// How bright the car's high beams are, in candela, as the renderer counts them.
const LAMPS: f32 = 560.0;

/// The scrub a car drives through: bushes, ferns and whippy saplings, which go down under it
/// and stay down. The bake lists them (`outback.plants` among the scene's gameplay files), a
/// line each: where it stands, how far round it reaches, and the first of its two instances.
#[derive(Default)]
struct Scrub {
    plants: Vec<(glam::Vec3, f32, u32)>,
    down: Vec<bool>,
    /// Which plants stand in each square of ground, to find those by the car quickly.
    squares: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

/// How fast the detector's coil is swung, against how fast its sweep is counted on.
const SWEEPS: f32 = 1.9;

/// The size of those squares, in metres: more than a truck's length.
const SCRUB_SQUARE: f32 = 8.0;

impl Scrub {
    fn read(files: &[GameFile]) -> Self {
        let mut scrub = Self::default();
        let Some(file) = files.iter().find(|file| file.name == "outback.plants") else {
            return scrub;
        };
        for line in file.text.lines() {
            let numbers: Vec<f32> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .filter_map(|word| word.parse().ok())
                .collect();
            if let [x, y, z, reach, instance] = numbers[..] {
                let at = glam::Vec3::new(x, y, z);
                let key = (
                    (x / SCRUB_SQUARE).floor() as i32,
                    (z / SCRUB_SQUARE).floor() as i32,
                );
                scrub
                    .squares
                    .entry(key)
                    .or_default()
                    .push(scrub.plants.len() as u32);
                scrub.plants.push((at, reach, instance as u32));
            }
        }
        scrub.down = vec![false; scrub.plants.len()];
        scrub
    }

    /// Lays flat whatever of it the car's body has reached, the way the car is going. Returns
    /// how many plants went down, and how far round the biggest of them reached.
    fn flatten(&mut self, renderer: &mut Renderer, hud: &solstice_game::Hud) -> (u32, f32) {
        let mut laid = (0, 0.0_f32);
        if self.plants.is_empty() || hud.velocity.length() < 0.4 {
            return laid;
        }
        let middle = hud.wheels.iter().map(|wheel| wheel.at).sum::<glam::Vec3>() / 4.0;
        let forward = glam::Vec3::new(hud.forward.x, 0.0, hud.forward.z).normalize_or_zero();
        let right = forward.cross(glam::Vec3::Y);
        let going = glam::Vec3::new(hud.velocity.x, 0.0, hud.velocity.z).normalize_or(forward);
        let key = (
            (middle.x / SCRUB_SQUARE).floor() as i32,
            (middle.z / SCRUB_SQUARE).floor() as i32,
        );
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Some(near) = self.squares.get(&(key.0 + dx, key.1 + dz)) else {
                    continue;
                };
                for &index in near {
                    let index = index as usize;
                    if self.down[index] {
                        continue;
                    }
                    let (at, reach, instance) = self.plants[index];
                    let from = at - middle;
                    // Under the body: a truck is two metres wide and six long.
                    if from.dot(right).abs() > 1.05 + reach * 0.5
                        || from.dot(forward).abs() > 3.0 + reach * 0.5
                        || from.y.abs() > 2.5
                    {
                        continue;
                    }
                    self.down[index] = true;
                    laid = (laid.0 + 1, laid.1.max(reach));
                    // Laid over the way the car pushed it, each a little its own way, and
                    // crushed: about its foot, so that it stays rooted where it grew.
                    let twist = ((instance % 7) as f32 - 3.0) * 0.12;
                    let fall = glam::Quat::from_rotation_y(twist) * going;
                    let axis = glam::Vec3::Y.cross(fall).normalize_or(glam::Vec3::X);
                    let lay = glam::Mat4::from_translation(at)
                        * glam::Mat4::from_axis_angle(axis, 1.36)
                        * glam::Mat4::from_scale(glam::Vec3::new(1.0, 0.82, 1.0))
                        * glam::Mat4::from_translation(-at);
                    for id in instance..instance + 2 {
                        if let Some(stood) = renderer.instance_transform(id) {
                            renderer.set_instance_movable(id, true);
                            renderer.move_instance(id, lay * stood);
                        }
                    }
                }
            }
        }
        laid
    }
}

/// What is heard of a game being played: the truck's engine, its tyres on whatever each is on,
/// what it hits and drives through, and the bush round it in whatever weather there is.
///
/// Everything here is worked out from what the game already knows. Each wheel says what it is
/// on, how fast its tread passes over the ground and how hard it is pressed down
/// (`solstice_game::drive::HudWheel`), which is what drives the spray; the same numbers choose
/// each tyre's sound, and set how loud and how high it is.
struct Soundscape {
    sounds: crate::sound::Sounds,
    /// Whether each wheel was in a ford's water a frame ago: going in is a splash.
    in_water: [bool; 4],
    /// Seconds since the last splash, the last brush snapped, and the last thud, so that none
    /// is heard as a rattle of itself.
    since: [f32; 3],
    random: u32,
    /// The detector in the driver's hand, on foot: how strong its signal is, from 0 to 1.
    detector: Option<f32>,
    /// How far from one beep of the detector to the next it is, from 0 round to 1.
    beat: f32,
    /// Whether the main menu is up, with no truck behind it: the bush is all that is heard.
    menu: bool,
    /// How loud everything is, from 0 to 1: as the options say.
    volume: f32,
    /// How much the engine is running, from 0, switched off, to 1.
    running: f32,
    /// Sounds to play once, as the game asked for them.
    asked: Vec<&'static str>,
}

/// The loops a tyre can make, by name: rolling on each kind of ground, and spinning.
const TYRE_LOOPS: [&str; 8] = [
    "tyre_dirt",
    "tyre_gravel",
    "tyre_mud",
    "tyre_litter",
    "tyre_rock",
    "tyre_water",
    "tyre_spin_dirt",
    "tyre_spin_mud",
];

impl Soundscape {
    /// A number from 0 to 1, different each time: so that no two splashes are the same.
    fn chance(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random >> 8) as f32 / 16_777_216.0
    }

    /// Asks for what should be heard this frame. `flattened` is how many plants the car has
    /// just laid flat, and how far round the biggest of them reached.
    fn frame(
        &mut self,
        hud: Option<&solstice_game::Hud>,
        flattened: (u32, f32),
        renderer: &Renderer,
        seconds: f32,
    ) {
        for since in &mut self.since {
            *since += seconds;
        }
        // The weather: how hard it rains, from 0 to 1, and how hard it blows.
        let rain = (renderer.rain_state().rate / 12.0).clamp(0.0, 1.0).sqrt();
        let wind = ((renderer.weather().wind_speed - 3.0) / 13.0).clamp(0.0, 1.0);

        let mut loops = [(0.0_f32, 1.0_f32); 8];
        let mut engine = crate::sound::EngineNote::default();
        let mut once: Vec<(&str, f32, f32)> = Vec::new();
        if let Some(hud) = hud {
            let t = &hud.telemetry;
            // The driver switches it off as they get out, and it runs down; back in, it is
            // started, and comes up to its idle.
            let wanted = if hud.afoot { 0.0 } else { 1.0 };
            let rate = if hud.afoot { 2.2 } else { 1.6 };
            self.running += (wanted - self.running).clamp(-rate * seconds, rate * seconds);
            engine = crate::sound::EngineNote {
                rpm: t.rpm * (0.35 + 0.65 * self.running),
                throttle: t.throttle * self.running,
                limiter: t.limiter,
                gain: 0.9 * self.running.powf(1.5),
            };
            let speed = hud.velocity.length();
            for (index, wheel) in hud.wheels.iter().enumerate() {
                let wet = wheel.on_ground && wheel.surface.name() == "ford";
                if wet && !self.in_water[index] && speed > 1.5 && self.since[0] > 0.35 {
                    self.since[0] = 0.0;
                    let name = if speed > 5.0 {
                        "splash_big"
                    } else {
                        "splash_small"
                    };
                    let pitch = 0.9 + 0.2 * self.chance();
                    once.push((name, (0.45 + speed / 10.0).min(1.2), pitch));
                }
                self.in_water[index] = wet;
                if !wheel.on_ground {
                    continue;
                }
                // Which sound this tyre rolls with, and which it spins with.
                let (roll, spin) = match wheel.surface.name() {
                    "dirt" => (0, 6),
                    "gravel" | "sand" => (1, 6),
                    "mud" | "bog" => (2, 7),
                    "litter" | "grass" => (3, 6),
                    "ford" => (5, 7),
                    _ => (4, 6),
                };
                let pressed = wheel.load.clamp(0.0, 1.3) * 0.25;
                loops[roll].0 += (speed / 9.0).min(1.0).powf(0.7) * pressed;
                loops[roll].1 = 0.7 + 0.6 * (speed / 14.0).min(1.5);
                // Spinning, or locked and sliding: the tread passing over the ground.
                let scrub = (wheel.slip.abs() - 0.6).max(0.0);
                loops[spin].0 += (scrub / 5.0).min(1.0) * pressed;
                loops[spin].1 = 0.8 + 0.5 * (scrub / 8.0).min(1.0);
            }
            // A wheel dropping off a ledge, or the suspension reaching its stops: more than
            // twice its own weight on the truck, straight up.
            if t.g.z > 2.3 && self.since[2] > 0.4 {
                self.since[2] = 0.0;
                let pitch = 0.9 + 0.2 * self.chance();
                once.push((
                    "suspension_thud",
                    (0.4 + (t.g.z - 2.3) * 0.5).min(1.2),
                    pitch,
                ));
            }
            if flattened.0 > 0 && self.since[1] > 0.12 {
                self.since[1] = 0.0;
                let name = if flattened.1 > 0.5 && self.chance() < 0.5 {
                    "sapling_crack"
                } else {
                    "brush_snap"
                };
                let pitch = 0.85 + 0.3 * self.chance();
                once.push((name, (0.5 + 0.15 * flattened.0 as f32).min(1.0), pitch));
            }
        } else {
            self.in_water = [false; 4];
        }
        // The detector. With nothing in reach it hums, very quietly: it is on. Gold is
        // heard as beeps over that, and everything about them says how strong the signal
        // is: how often they come, how high they are, and how loud. At the edge of its
        // hearing, a low soft beep every second and a half; half way, two a second; a few
        // steps off, five a second and high; and over the gold, one unbroken high note. The
        // signal is stronger facing the gold than with a back to it (`Gold::heard`), so
        // turning on the spot until the beeps come fastest says which way to walk.
        let (detector, asked) = (self.detector, std::mem::take(&mut self.asked));
        let mut tone = (0.0_f32, 1.0_f32);
        if let Some(signal) = detector {
            tone = (0.05, 0.9);
            if signal >= crate::gold::OVER {
                tone = (1.0, 2.1);
            } else if signal > 0.0 {
                let near = signal / crate::gold::OVER;
                self.beat = (self.beat + seconds * (0.65 + 4.6 * near * near)).rem_euclid(1.0);
                if self.beat < 0.42 {
                    tone = (0.3 + 0.7 * near, 1.05 + 0.85 * near);
                } else {
                    tone.1 = 1.05 + 0.85 * near;
                }
            } else {
                // The first beep of a signal comes at once.
                self.beat = 0.0;
            }
        }
        let (menu, volume) = (self.menu, self.volume);
        self.sounds.frame(|ask| {
            ask.master(volume);
            ask.engine(engine);
            for (name, (gain, pitch)) in TYRE_LOOPS.iter().zip(loops) {
                ask.looping(name, gain.min(1.0), pitch);
            }
            // The bush: birds and insects in the dry, the wind as it blows, the rain as it
            // falls.
            // (Behind the main menu, where no truck is heard, it is all there is: louder.)
            let alone = if menu { 2.2 } else { 1.0 };
            ask.looping("bush_day", 0.3 * alone * (1.0 - 0.75 * rain), 1.0);
            ask.looping(
                "bush_wind",
                (0.35 * wind).max(if menu { 0.3 } else { 0.0 }),
                1.0,
            );
            ask.looping("rain_leaves", 0.55 * rain, 1.0);
            for (name, gain, pitch) in once {
                ask.once(name, gain, pitch);
            }
            // The detector's hum, while it is in hand: and none once it is not (a loop
            // goes on as it was last asked for, so it is asked for at nothing).
            ask.looping("detector_tone", tone.0, tone.1);
            for name in asked {
                ask.once(name, 0.9, 1.0);
            }
        });
    }
}

/// What the player's truck wears: the colour of its paint, how rusted it is, and the dust
/// and the mud it has gathered, each from 0 to 1; `wet` is how fresh its mud is. The
/// renderer draws it (`solstice_render::Coat`); the game gathers it as the truck is driven
/// (`PlayMode::gather`), and a kept game is to keep it with the truck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wear {
    pub paint: glam::Vec3,
    pub rust: f32,
    pub dust: f32,
    pub mud: f32,
    pub wet: f32,
    /// Whether the driving changes it: a picture of a truck in a given state holds it still.
    pub held: bool,
}

/// A colour as a page writes it, as light: a page's is as the eye sees it, and the renderer
/// wants light.
fn light(colour: u32) -> glam::Vec3 {
    let channel = |shift: u32| (((colour >> shift) & 0xff) as f32 / 255.0).powf(2.2);
    glam::Vec3::new(channel(16), channel(8), channel(0))
}

/// The colour the Raptor's paint was made in, which the renderer tells its paint from what
/// is painted on it by: measured from its texture. (To be said by each truck's own file.)
const PAINT_WAS: glam::Vec3 = glam::Vec3::new(0.164, 0.0004, 0.0004);

/// The colour of the country's ground, which a truck's mud and dust are: red clay.
const SOIL: glam::Vec3 = glam::Vec3::new(0.33, 0.15, 0.075);

impl Default for Wear {
    fn default() -> Self {
        Self {
            paint: PAINT_WAS,
            rust: 0.0,
            dust: 0.0,
            mud: 0.0,
            wet: 0.0,
            held: false,
        }
    }
}

impl Wear {
    /// A truck's state from words, for a picture: `paint=#1f4fa0,rust=0.4,dust=0.3,mud=0.7,wet=1`.
    /// The paint is as a page writes a colour. What is not said is clean, and it is held.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut wear = Self {
            held: true,
            ..Self::default()
        };
        for word in text.split(',').filter(|word| !word.trim().is_empty()) {
            let (name, value) = word
                .trim()
                .split_once('=')
                .ok_or_else(|| format!("`{word}` is not NAME=VALUE"))?;
            let number = || {
                value
                    .parse::<f32>()
                    .map_err(|_| format!("`{value}` is not a number"))
            };
            match name {
                "paint" => {
                    let hex = value.trim_start_matches('#');
                    let parsed = u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() == 6);
                    let colour =
                        parsed.ok_or_else(|| format!("`{value}` is not a colour like #1f4fa0"))?;
                    // As a page writes it, and so as the eye sees it: the renderer wants light.
                    let channel =
                        |shift: u32| (((colour >> shift) & 0xff) as f32 / 255.0).powf(2.2);
                    wear.paint = glam::Vec3::new(channel(16), channel(8), channel(0));
                }
                "rust" => wear.rust = number()?,
                "dust" => wear.dust = number()?,
                "mud" => wear.mud = number()?,
                "wet" => wear.wet = number()?,
                _ => {
                    return Err(format!(
                        "a truck has no `{name}`: paint, rust, dust, mud and wet"
                    ))
                }
            }
        }
        Ok(wear)
    }

    /// A frame's driving: what the wheels threw, between them, and how hard it rains.
    fn gather(&mut self, thrown: [f32; 3], rain: f32, seconds: f32) {
        if self.held {
            return;
        }
        let [mud, dust, water] = thrown;
        // A minute of dry road is a dusty truck; a bog hole is a muddy one in a few lengths.
        self.dust += dust * 0.004 * seconds * (1.0 - self.dust);
        self.mud += mud * 0.012 * seconds * (1.0 - self.mud);
        // Mud goes on wet, and dries in a few minutes.
        self.wet = (self.wet + mud * 0.3 * seconds).min(1.0);
        self.wet -= seconds / 240.0;
        // A ford washes what it reaches, and the rain washes all of it, slowly: dust first.
        self.dust -= (water * 0.05 + rain * 0.02) * seconds;
        self.mud -= (water * 0.02 + rain * 0.003) * seconds;
        if water > 0.0 || rain > 0.3 {
            self.wet = self.wet.max(0.6);
        }
        self.dust = self.dust.clamp(0.0, 1.0);
        self.mud = self.mud.clamp(0.0, 1.0);
        self.wet = self.wet.clamp(0.0, 1.0);
    }
}

impl PlayMode {
    /// Starts the game in a scene's gameplay files, making every instance an actor draws with
    /// movable, and giving each animated actor its skeleton and clips. Returns every mistake
    /// found in the files, each with its file and line.
    pub fn start(
        renderer: &mut Renderer,
        files: &[GameFile],
        collision: Option<&Collision>,
        animation: &AnimationAssets<'_>,
    ) -> Result<Self, Vec<String>> {
        let mut play = Play::load(files, |id| renderer.instance_transform(id), collision)?;
        play.attach_animation(files, animation)?;
        for id in play.instances() {
            renderer.set_instance_movable(id, true);
        }
        // A truck that another has been put in the place of is not drawn (`shared.rs`).
        for id in crate::shared::hidden(files) {
            renderer.set_instance_hidden(id, true);
        }
        // Trucks stood to be looked at: the first is shown, and the rest are not.
        let display = crate::kit::displayed(files);
        for truck in display.iter().skip(1) {
            for id in truck.instances() {
                renderer.set_instance_hidden(id, true);
            }
        }
        // Trucks parked in a used-truck yard are each in a colour of their own.
        let (parked, pulled) = crate::shared::parked(files);
        let colours: Vec<(glam::Vec3, glam::Vec3, Vec<u32>)> = parked
            .iter()
            .filter_map(|(_, instances, paint, was)| {
                Some((light(paint.as_ref().copied()?), (*was)?, instances.clone()))
            })
            .collect();
        renderer.set_resprays(&colours);
        let parked: Vec<(usize, Vec<u32>)> = parked
            .into_iter()
            .map(|(place, instances, ..)| (place, instances))
            .collect();
        let kit = crate::kit::Fitted::find(files).map_err(|error| vec![error])?;
        let made = kit
            .as_ref()
            .and_then(|kit| kit.kit.paint)
            .unwrap_or(PAINT_WAS);
        Ok(Self {
            play,
            devices: Devices::new(),
            lamps: false,
            tail_lamps: Vec::new(),
            head_lamps: Vec::new(),
            spray: crate::mud::Fling::default(),
            wear: Wear {
                paint: made,
                ..Wear::default()
            },
            made,
            car: Vec::new(),
            // A truck's, until its wheels have said where they are.
            axles: [-2.0, 1.7, 0.94, 0.44],
            scrub: Scrub::read(files),
            kit,
            display,
            displayed: 0,
            parked,
            pulled,
            show_side: None,
            show_view: None,
            show_at: 0.0,
            stand: crate::shared::stand(files),
            unseen: false,
            nuggets: {
                let slots = crate::nugget::Slot::read(files);
                for slot in &slots {
                    renderer.set_instance_hidden(slot.instance, true);
                }
                slots.into_iter().map(|slot| (slot, None)).collect()
            },
            things: Vec::new(),
            // With nothing to say otherwise, the driver has it in their hands on foot.
            carried: crate::gold::Carried::Held,
            tipped: 0.0,
            walked: (glam::Vec3::ZERO, 0.0),
            swept: 0.0,
            sound: None,
            frames: 0,
            trace: std::env::var_os("SOLSTICE_TRACE").is_some(),
            wrecks: crate::wreck::Wrecks::new(files),
        })
    }

    /// Opens the sound output and reads the sounds in `folder`, so that the game is heard
    /// from the next frame on. A viewer that only takes pictures does not ask for this.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_sound(&mut self, folder: &std::path::Path) {
        self.sound = crate::sound::Sounds::new(folder).map(|sounds| Soundscape {
            sounds,
            in_water: [false; 4],
            since: [1.0; 3],
            random: 0x9E37_79B9,
            detector: None,
            beat: 0.0,
            menu: false,
            volume: 1.0,
            running: 1.0,
            asked: Vec::new(),
        });
    }

    /// Shows a nugget with one of the scene's blanks, where the game says it is; or puts the
    /// blank away. A blank that is to show a nugget it was not showing before has that
    /// nugget made on it, from the nugget's number: its points written over the blank's,
    /// and its colours given to the blank's materials (`nugget::Lump`).
    pub fn show_item(&mut self, renderer: &mut Renderer, shown: crate::gold::Shown) {
        let Some((slot, made)) = self.nuggets.get_mut(shown.slot) else {
            return;
        };
        let Some((at, turned, size)) = shown.at else {
            renderer.set_instance_hidden(slot.instance, true);
            return;
        };
        if *made != Some(shown.shape) {
            let lump = crate::nugget::Lump::of(shown.shape);
            for &(page, offset, first, count) in &slot.writes {
                if let Some(bytes) = lump.bytes(first, count) {
                    renderer.rewrite_page(page, offset, bytes);
                }
            }
            for (index, material) in lump.materials.into_iter().enumerate() {
                renderer.set_material(slot.material + index as u32, material);
            }
            *made = Some(shown.shape);
        }
        renderer.set_instance_movable(slot.instance, true);
        renderer.set_instance_hidden(slot.instance, false);
        renderer.move_instance(
            slot.instance,
            glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(size), turned, at),
        );
    }

    /// Says how the detector is to be shown from now on.
    pub fn set_carried(&mut self, carried: crate::gold::Carried) {
        self.carried = carried;
    }

    /// Puts the detector where it is to be seen this frame. Its model's origin is its grip,
    /// and its coil is ahead of that and below (`tools/blender/detector/build.py`).
    ///
    /// Held, the grip is in the driver's right hand and the shaft is tipped about it until
    /// the coil is just over whatever ground is under it, uphill or down; walking, the coil
    /// is swept from side to side. Lying, it is where its body is, flat along its shaft.
    fn place_carried(&mut self, seconds: f32) {
        use crate::gold::Carried;
        use glam::{Quat, Vec3};
        // The coil's middle from the grip, in the model: to the left a little, down, ahead.
        const COIL: Vec3 = Vec3::new(-0.07, -0.91, -1.25);
        // The grip, from the driver's feet, and how high over the ground the coil is held.
        const GRIP: Vec3 = Vec3::new(0.27, 0.98, -0.30);
        const CLEAR: f32 = 0.07;
        let reach = COIL.y.hypot(COIL.z);
        let rest = (-COIL.y).atan2(-COIL.z);
        let pose = match self.carried {
            Carried::Away => None,
            Carried::Lying(at, turned) => {
                // Its body is a box along the shaft, about the shaft's middle.
                let flat = turned * Quat::from_rotation_x(rest);
                Some((at - flat * (COIL * 0.5), flat))
            }
            Carried::Held => self
                .play
                .hud()
                .filter(|hud| hud.afoot)
                .map(|hud| (hud.walker, hud.walker_yaw))
                .map(|(walker, yaw)| {
                    let facing = Quat::from_rotation_y(yaw);
                    // How fast the driver is walking: the coil is swept as they go.
                    let pace = (walker - self.walked.0).length() / seconds.max(1.0e-3);
                    self.walked = (
                        walker,
                        self.walked.1 + (pace.min(5.0) - self.walked.1) * (seconds * 6.0).min(1.0),
                    );
                    self.swept += seconds * (1.2 + self.walked.1 * 0.9);
                    // Standing, it is still swept, gently: a detector hears only what
                    // its coil passes over.
                    let sweep =
                        0.24 * (self.swept * SWEEPS).sin() * (self.walked.1 / 1.5).clamp(0.4, 1.0);
                    let swing = facing * Quat::from_rotation_y(sweep);
                    let grip = walker + facing * GRIP;
                    // The ground under where the coil is, if it is near the driver's own.
                    let under = grip + swing * Vec3::new(COIL.x, 0.0, COIL.z);
                    let ground = self
                        .play
                        .surface_below(Vec3::new(under.x, walker.y + 0.9, under.z))
                        .filter(|ground| (ground - walker.y).abs() < 0.9)
                        .unwrap_or(walker.y);
                    let drop = grip.y - (ground + CLEAR);
                    let wanted = (drop / reach).clamp(0.12, 0.98).asin();
                    self.tipped += (wanted - self.tipped) * (seconds * 10.0).min(1.0);
                    (grip, swing * Quat::from_rotation_x(-(self.tipped - rest)))
                }),
        };
        self.play.carry(pose);
    }

    /// Puts a nugget's body in the world, or takes it out: a nugget is a thing, which falls,
    /// lies where it comes to rest, and rides in what it lies in.
    pub fn thing(&mut self, body: crate::gold::Body) {
        use crate::gold::Body;
        match body {
            Body::Put {
                id,
                at,
                turned,
                size,
                kilos,
                settle,
            } => {
                // Let go, it lies on whatever is under it: found now, so that it is put
                // down on it and not dropped from a height.
                let at = match settle.then(|| self.play.surface_below(at)).flatten() {
                    Some(top) if top > at.y - 4.0 => {
                        glam::Vec3::new(at.x, top + size.y * 0.5 + 0.03, at.z)
                    }
                    _ => at,
                };
                match self.things.iter().find(|(thing, _)| *thing == id) {
                    Some(&(_, entity)) => self.play.move_thing(entity, at, turned),
                    None => {
                        let entity = self.play.add_thing(at, turned, size, kilos);
                        self.things.push((id, entity));
                    }
                }
            }
            Body::Take { id } => {
                if let Some(index) = self.things.iter().position(|(thing, _)| *thing == id) {
                    let (_, entity) = self.things.swap_remove(index);
                    self.play.remove_thing(entity);
                }
            }
        }
    }

    /// Where the nuggets' bodies are now: each its nugget's number, where it is and how it
    /// is turned. One that has got under the land, as a small thing thrown hard at a seam
    /// in it can, is put back on top of it: gold is not lost through the floor.
    pub fn things(&mut self) -> Vec<(u32, glam::Vec3, glam::Quat)> {
        let mut seen = Vec::with_capacity(self.things.len());
        for &(id, entity) in &self.things {
            let Some((mut at, turned)) = self.play.thing(entity) else {
                continue;
            };
            let land = self
                .play
                .land_below(glam::Vec3::new(at.x, at.y + 60.0, at.z));
            if let Some(land) = land.filter(|land| at.is_finite() && at.y < land - 0.6) {
                at.y = land + 0.3;
                self.play.move_thing(entity, at, turned);
            }
            seen.push((id, at, turned));
        }
        seen
    }

    /// Says how loud everything is to be, from 0 to 1.
    pub fn set_volume(&mut self, share: f32) {
        if let Some(sound) = &mut self.sound {
            sound.volume = share;
        }
    }

    /// Says how strong the signal of the detector in the driver's hand is, from 0 to 1, or
    /// that it is not in their hand.
    pub fn set_detector(&mut self, signal: Option<f32>) {
        if let Some(sound) = &mut self.sound {
            sound.detector = signal;
        }
    }

    /// Plays one of the game's sounds, once.
    pub fn sound_once(&mut self, name: &'static str) {
        if let Some(sound) = &mut self.sound {
            sound.asked.push(name);
        }
    }

    /// Stops everything that is heard, until the next frame asks for it again: for when
    /// the game is not being looked at, as on a phone when another app is in front.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn hush(&mut self) {
        if let Some(sound) = &mut self.sound {
            sound.sounds.frame(|_| {});
        }
    }

    /// In a browser: from now on what should be heard is kept for the page to take, each
    /// frame, with [`PlayMode::sound_asks`], and play.
    #[cfg(target_arch = "wasm32")]
    pub fn with_web_sound(&mut self) {
        self.sound = Some(Soundscape {
            sounds: crate::sound::Sounds::new(),
            in_water: [false; 4],
            since: [1.0; 3],
            random: 0x9E37_79B9,
            detector: None,
            beat: 0.0,
            menu: false,
            volume: 1.0,
            running: 1.0,
            asked: Vec::new(),
        });
    }

    /// In a browser: what the page is to play, as JSON (`sound_web::Sounds::take`), or
    /// nothing if the game has no sound.
    #[cfg(target_arch = "wasm32")]
    pub fn sound_asks(&mut self) -> String {
        self.sound
            .as_mut()
            .map(|sound| sound.sounds.take())
            .unwrap_or_default()
    }

    /// What the player's truck wears, and has gathered.
    pub fn wear(&self) -> Wear {
        self.wear
    }

    /// Fits the player's truck: `stock`, `showroom`, or choices to put over what it is sold
    /// with, as `bullbar=plate,tyres=mud`. A mistake, or a truck that is not built of
    /// pieces, is an error.
    pub fn fit(&mut self, build: &str) -> Result<(), String> {
        let kit = self
            .kit
            .as_mut()
            .ok_or("the truck here is not built of pieces")?;
        kit.fit(build)
    }

    /// Opens a door or a tailgate of the player's truck, so far of all the way: by its
    /// piece's name, or `all` of them.
    pub fn swing(&mut self, piece: &str, share: f32) -> Result<(), String> {
        let kit = self
            .kit
            .as_mut()
            .ok_or("the truck here is not built of pieces")?;
        kit.swing(piece, share)
    }

    /// The player's truck's kit, and what is fitted to it, if it is built of pieces.
    pub fn kit(&self) -> Option<&crate::kit::Fitted> {
        self.kit.as_ref()
    }

    /// Where the camera is to be for a truck in the showroom, and the way it looks, as a yaw
    /// and a pitch: before the truck and to one side of it, as a truck for sale is looked
    /// at, swaying a little as time goes by. It is to whichever side the ground falls away
    /// on, and it is kept above the ground: a truck stands on a hillside as often as not.
    ///
    /// With a part of the truck to look at (`focus`, a group of its kit's), it goes in close
    /// to where that part is: its nose, its roof, a wheel, its back. It moves there, and
    /// back, over a moment.
    pub fn showroom_camera(
        &mut self,
        origin: glam::Vec3,
        turned: glam::Quat,
        seconds: f32,
        focus: Option<&str>,
    ) -> (glam::Vec3, f32, f32) {
        use glam::Vec3;
        let flat = |way: Vec3| Vec3::new(way.x, 0.0, way.z).normalize_or(Vec3::NEG_Z);
        let (forward, right) = (flat(turned * Vec3::NEG_Z), flat(turned * Vec3::X));
        let land = |at: Vec3| {
            let found = self.play.land_below(Vec3::new(at.x, origin.y + 60.0, at.z));
            found.unwrap_or(origin.y)
        };
        let stand = |side: f32, angle: f32| {
            origin + (forward * angle.cos() + right * side * angle.sin()) * 7.4
        };
        let side = *self.show_side.get_or_insert_with(|| {
            if land(stand(1.0, 0.62)) <= land(stand(-1.0, 0.62)) {
                1.0
            } else {
                -1.0
            }
        });
        let out = right * side;
        let (mut eye, at) = match focus {
            // Its nose: a bar, and the lamps on it.
            Some("bullbar" | "spots" | "lightbar") => (
                origin + forward * 5.3 + out * 1.9 + Vec3::Y * 1.15,
                origin + forward * 1.9 + Vec3::Y * 0.95,
            ),
            Some("snorkel") => (
                origin + forward * 3.5 + out * 3.1 + Vec3::Y * 1.75,
                origin + forward * 0.9 + Vec3::Y * 1.35,
            ),
            // Its roof.
            Some("rack") => (
                origin + forward * 3.8 + out * 3.3 + Vec3::Y * 3.1,
                origin - forward * 0.3 + Vec3::Y * 1.75,
            ),
            // A front wheel, and what is by it: low, from the side.
            Some("arches" | "steps" | "suspension" | "tyres" | "rims") => (
                origin + forward * 2.9 + out * 4.0 + Vec3::Y * 0.75,
                origin + forward * 1.2 + out * 0.9 + Vec3::Y * 0.5,
            ),
            // Its back.
            Some("tub" | "tray" | "spare") => (
                origin - forward * 5.2 + out * 2.8 + Vec3::Y * 2.0,
                origin - forward * 1.5 + Vec3::Y * 1.15,
            ),
            // All of it: a little to the left of the middle, what it is being down the right.
            _ => {
                let eye = stand(side, 0.62 + 0.28 * (seconds * 0.22).sin());
                let eye = Vec3::new(eye.x, origin.y + 1.5, eye.z);
                let middle = origin + Vec3::Y * 0.95;
                let aside = (middle - eye).cross(Vec3::Y).normalize_or_zero();
                (eye, middle + aside * 0.7)
            }
        };
        let least = if focus.is_some() { 0.45 } else { 1.25 };
        eye.y = eye.y.max(land(eye) + least);
        // It goes from where it was to where it is to be, not all at once.
        let passed = (seconds - self.show_at).clamp(0.0, 0.1);
        self.show_at = seconds;
        let nearer = 1.0 - (-passed * 5.0).exp();
        let (eye, at) = match self.show_view {
            Some((was_eye, was_at)) => (was_eye.lerp(eye, nearer), was_at.lerp(at, nearer)),
            None => (eye, at),
        };
        self.show_view = Some((eye, at));
        let look = at - eye;
        (
            eye,
            (-look.x).atan2(-look.z),
            look.y.atan2(look.x.hypot(look.z)),
        )
    }

    /// Where what is looked at in a showroom stands, and how it is turned: the trucks stood
    /// there, or the map's own truck.
    pub fn stand(&self) -> Option<(glam::Vec3, glam::Quat)> {
        self.stand.or_else(|| self.play.car_pose())
    }

    /// Behind the main menu: the player's truck is not drawn, and is not heard; or, when a
    /// game is gone into, it is again, as it is fitted.
    pub fn set_unseen(&mut self, renderer: &mut Renderer, unseen: bool) {
        if self.unseen == unseen {
            return;
        }
        self.unseen = unseen;
        for id in self.play.car_instances() {
            renderer.set_instance_hidden(id, unseen);
        }
        if !unseen {
            let fitted = self.kit.as_ref().map(|kit| kit.hidden());
            for (id, hidden) in fitted.into_iter().flatten() {
                renderer.set_instance_hidden(id, hidden);
            }
        }
    }

    /// Behind the main menu: a view down the track from where the truck would stand, from
    /// a little above somebody's head, looking slowly from one side of it to the other.
    pub fn preview_camera(
        &mut self,
        origin: glam::Vec3,
        turned: glam::Quat,
        seconds: f32,
    ) -> (glam::Vec3, f32, f32) {
        use glam::Vec3;
        let ahead = turned * Vec3::NEG_Z;
        let land = self
            .play
            .land_below(Vec3::new(origin.x, origin.y + 60.0, origin.z));
        let eye = Vec3::new(origin.x, land.unwrap_or(origin.y) + 2.6, origin.z);
        let yaw = (-ahead.x).atan2(-ahead.z) + 0.6 * (seconds * 0.045).sin();
        (eye, yaw, -0.05)
    }

    /// Shows another of the trucks that stand to be looked at, by its place among them, in
    /// its colour: as a page writes one, or with none the colour it was made in. The one
    /// that was shown is hidden. Nothing is read: they are all there.
    pub fn display(&mut self, renderer: &mut Renderer, index: usize, colour: Option<u32>) {
        if index >= self.display.len() {
            return;
        }
        if let Some(was) = self
            .display
            .get(self.displayed)
            .filter(|_| index != self.displayed)
        {
            for id in was.instances() {
                renderer.set_instance_hidden(id, true);
            }
        }
        // In a used-truck yard, the one that is shown has been pulled out of its row.
        if self.pulled {
            for (place, instances) in &self.parked {
                for &id in instances {
                    renderer.set_instance_hidden(id, *place == index);
                }
            }
        }
        self.displayed = index;
        let truck = &mut self.display[index];
        truck.redraw();
        // It is what is painted, and its own paint is what a respray is told from.
        self.car = truck.instances();
        self.made = truck.kit.paint.unwrap_or(PAINT_WAS);
        self.wear = Wear {
            paint: self.made,
            ..Wear::default()
        };
        self.paint(colour);
    }

    /// A truck parked in the used-truck yard has been sold: it is gone from its row.
    pub fn unpark(&mut self, renderer: &mut Renderer, place: usize) {
        for (_, instances) in self.parked.iter().filter(|(parked, _)| *parked == place) {
            for &id in instances {
                renderer.set_instance_hidden(id, true);
            }
        }
        self.parked.retain(|(parked, _)| *parked != place);
    }

    /// Fits the truck that is shown, where it stands: `stock`, `showroom`, or choices over
    /// what it is sold with. Every piece of it is there already, if it was stood there with
    /// all of them.
    pub fn display_fit(&mut self, build: &str) -> Result<(), String> {
        let truck = self.display.get_mut(self.displayed);
        truck.ok_or("no truck stands here to be fitted")?.fit(build)
    }

    /// What is on the player's truck: dust, mud and rust, each from 0 to 1.
    pub fn dirt(&self) -> [f32; 3] {
        [self.wear.dust, self.wear.mud, self.wear.rust]
    }

    /// Puts dust, mud and rust on the player's truck: what it had on it when it was last
    /// driven.
    pub fn set_dirt(&mut self, [dust, mud, rust]: [f32; 3]) {
        (self.wear.dust, self.wear.mud, self.wear.rust) = (dust, mud, rust);
    }

    /// Resprays the player's truck a colour, as a page writes one; or, with none, puts it
    /// back in the colour it was made in.
    pub fn paint(&mut self, colour: Option<u32>) {
        self.wear.paint = match colour {
            Some(colour) => light(colour),
            None => self.made,
        };
    }

    /// Gives the player's truck its paint and its dirt: from a kept game, or for a picture.
    pub fn set_wear(&mut self, wear: Wear) {
        self.wear = wear;
    }

    /// The truck gathers what its wheels throw, and the renderer is told what it wears
    /// and where it is.
    fn coat(&mut self, renderer: &mut Renderer, seconds: f32) {
        use glam::{Mat4, Vec3};
        if self.car.is_empty() {
            self.car = self.play.car_instances();
        }
        let Some((at, turned)) = self.play.car_pose() else {
            renderer.set_coat(None);
            return;
        };
        let car = Mat4::from_rotation_translation(turned, at);
        // Where its tyres touch says where its axles are, how wide it is, and how far under
        // it the ground is: when all four are down.
        let mut ground = 0.0;
        if let Some(hud) = self
            .play
            .hud()
            .filter(|hud| hud.wheels.iter().all(|wheel| wheel.on_ground))
        {
            let to_car = car.inverse();
            let touch: Vec<Vec3> = hud
                .wheels
                .iter()
                .map(|wheel| to_car.transform_point3(wheel.at))
                .collect();
            let ease = (seconds * 4.0).min(1.0);
            let now = [
                (touch[0].z + touch[1].z) * 0.5,
                (touch[2].z + touch[3].z) * 0.5,
                touch.iter().map(|at| at.x.abs()).sum::<f32>() * 0.25,
                self.axles[3],
            ];
            for (kept, now) in self.axles.iter_mut().zip(now) {
                *kept += (now - *kept) * ease;
            }
            ground = touch.iter().map(|at| at.y).sum::<f32>() * 0.25;
        }
        let rain = (renderer.rain_state().rate / 12.0).clamp(0.0, 1.0);
        self.wear.gather(self.spray.thrown, rain, seconds);
        let wear = self.wear;
        renderer.set_coat(Some((
            solstice_render::Coat {
                // Its own space is measured from the ground under it.
                car: car * Mat4::from_translation(Vec3::Y * ground),
                paint: wear.paint,
                was: self.made,
                rust: wear.rust,
                dust: wear.dust,
                mud: wear.mud,
                wet: wear.wet,
                soil: SOIL,
                seed: 1851,
                axles: self.axles,
            },
            &self.car,
        )));
    }

    /// Says which instances of the player's car are its tail lamps and its headlamps, to be
    /// lit as it is driven.
    pub fn set_lamps(&mut self, tail: Vec<u32>, head: Vec<u32>) {
        self.tail_lamps = tail;
        self.head_lamps = head;
    }

    /// The sound, taken from a game that is ending to be given to the one that follows it, so
    /// that the output is opened and the sounds are read once.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn take_sound(&mut self) -> Option<Box<dyn std::any::Any>> {
        self.sound
            .take()
            .map(|sound| Box::new(sound) as Box<dyn std::any::Any>)
    }

    /// Gives this game the sound another had, from [`PlayMode::take_sound`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn give_sound(&mut self, sound: Box<dyn std::any::Any>) {
        if let Ok(mut sound) = sound.downcast::<Soundscape>() {
            sound.in_water = [false; 4];
            self.sound = Some(*sound);
        }
    }

    /// Runs a frame with nobody at the controls, the view going round the player's car by
    /// `orbit` pixels of mouse: what a title is shown over. The devices are left as they are.
    pub fn frame_unattended(
        &mut self,
        renderer: &mut Renderer,
        camera: &mut Camera,
        seconds: f32,
        orbit: f32,
    ) -> Vec<Message> {
        let held = std::mem::take(&mut self.devices);
        self.devices.mouse_motion(orbit, 0.0);
        let messages = self.frame(renderer, camera, seconds);
        self.devices = held;
        messages
    }

    /// A frame in which the game is not run: nothing moves, and the truck is not heard, but
    /// the bush still is.
    pub fn frame_held(&mut self, renderer: &Renderer, seconds: f32) {
        if let Some(sound) = &mut self.sound {
            sound.frame(None, (0, 0.0), renderer, seconds);
        }
    }

    /// Runs a frame `seconds` long: moves the instances that moved, poses the characters that
    /// changed, and puts the camera where the player's eye is, if there is a player. Returns
    /// what scripts logged and every error.
    pub fn frame(
        &mut self,
        renderer: &mut Renderer,
        camera: &mut Camera,
        seconds: f32,
    ) -> Vec<Message> {
        self.play.set_viewer(camera.position);
        self.place_carried(seconds);
        // The rain the renderer draws is the rain the tyres feel.
        let wetness = renderer.rain_state().wetness;
        self.play.set_wetness(wetness);
        let view = self
            .play
            .frame(f64::from(seconds), &mut self.devices, |id, world| {
                renderer.move_instance(id, world);
            });
        for (pose, bones) in self.play.take_poses() {
            renderer.set_pose(pose, &bones);
        }
        // By dark the car's high beams are on: one beam from between its headlamps, pointed
        // a little down the road, narrow, and fading as it goes.
        let lamp = self.play.hud().filter(|_| self.lamps).map(|hud| {
            let middle = hud.wheels.iter().map(|w| w.at).sum::<glam::Vec3>() / 4.0;
            let ahead = hud.forward.normalize_or_zero();
            solstice_render::Lamp {
                position: middle + glam::Vec3::Y * 1.05 + ahead * 2.5,
                direction: (ahead - glam::Vec3::Y * 0.045).normalize_or_zero(),
                half_angle: 0.6,
                range: 150.0,
                intensity: glam::Vec3::new(1.0, 0.95, 0.84) * LAMPS,
            }
        });
        // (As brightly as the head lamps that are not broken light it: `wreck.rs`.)
        renderer.set_lamp(self.beam(lamp));
        // Its lamps glow as they are lit. By dark everything is seen with eyes opened up, a
        // lamp's glow with it, so each is turned down to look as it should: the tail lamps a
        // dull red, and bright under braking; by day, lit only under braking.
        let braking = self.play.hud().is_some_and(|hud| hud.brake > 0.05);
        let (tail, head) = match (self.lamps, braking) {
            (true, true) => (0.2, 0.05),
            (true, false) => (0.05, 0.05),
            (false, true) => (1.0, 0.4),
            (false, false) => (0.0, 0.4),
        };
        // Below zero is red: the model's tail lamps are white behind what should be red glass.
        // (A lamp that is broken is out.)
        let lit = |instance: u32, glow: f32| {
            if self.wrecks.is_dark(instance) {
                0.0
            } else {
                glow
            }
        };
        for &instance in &self.tail_lamps {
            renderer.set_instance_glow(instance, lit(instance, -tail));
        }
        for &instance in &self.head_lamps {
            renderer.set_instance_glow(instance, lit(instance, head));
        }
        let mut flattened = (0, 0.0);
        // What the wheels throw; and the lumps of it that land lie where they fell, in the
        // map of the ruts, which is drawn round the truck (`mud.rs`).
        let mut lumps = Vec::new();
        let mut looked_at = camera.position;
        match self.play.hud() {
            Some(hud) => {
                lumps = self.spray.follow(renderer, hud, wetness, seconds);
                flattened = self.scrub.flatten(renderer, hud);
                if !hud.afoot {
                    looked_at = hud.origin;
                }
            }
            None => self.spray.rest(renderer),
        }
        if !lumps.is_empty() {
            self.play.ruts(|ruts| {
                for (at, across) in lumps {
                    ruts.splat(at, across, 1.0);
                }
            });
        }
        crate::mud::draw(renderer, &self.play, looked_at);
        self.coat(renderer, seconds);
        if let Some(truck) = self.display.get_mut(self.displayed) {
            truck.show(renderer);
        }
        if let Some(kit) = &mut self.kit {
            if !self.unseen {
                kit.show(renderer);
            }
            kit.advance(seconds);
            for (instance, by) in kit.swings() {
                self.play.move_within(instance, by);
            }
        }
        // What the truck hit this frame, and what that has done to it.
        self.wrecks
            .frame(&mut self.play, self.kit.as_ref(), renderer, seconds);
        if let Some(sound) = &mut self.sound {
            // A truck that is not there is not heard either.
            let hud = self.play.hud().filter(|_| !self.unseen);
            sound.menu = self.unseen;
            sound.frame(hud, flattened, renderer, seconds);
        }
        self.frames += 1;
        if let (true, Some(hud)) = (self.trace && self.frames % 30 == 0, self.play.hud()) {
            let at = hud.wheels.iter().map(|w| w.at).sum::<glam::Vec3>() / 4.0;
            let on: Vec<String> = hud
                .wheels
                .iter()
                .map(|w| {
                    if w.on_ground {
                        format!("{}:{:.2}", &w.surface.name()[..3], w.sink)
                    } else {
                        "air".to_owned()
                    }
                })
                .collect();
            // How far over it is: the height of its left wheels above its right.
            let lean =
                (hud.wheels[0].at.y + hud.wheels[2].at.y - hud.wheels[1].at.y - hud.wheels[3].at.y)
                    * 0.5;
            log::info!(
                "trace {:5.1}s at {:7.1} {:6.1} {:7.1}  {}  lean {:5.2}  [{}]",
                self.frames as f32 / 60.0,
                at.x,
                at.y,
                at.z,
                hud.line(),
                lean,
                on.join(" ")
            );
        }
        if let Some(view) = view {
            camera.position = view.position;
            camera.yaw = view.yaw;
            camera.pitch = view.pitch;
        }
        self.play.take_messages()
    }
}

/// The player's truck's damage (`damage.rs`, `wreck.rs`): what a kept game keeps of it, what
/// a repair shop would do to it, and what a picture is told to show.
impl PlayMode {
    /// The truck's damage: each piece that is hurt, and how hard it has been hit.
    pub fn damage(&self) -> &crate::damage::Damage {
        self.wrecks.damage()
    }

    /// The parts table the damage is reckoned with: how strong each kind of piece is.
    pub fn parts(&self) -> &crate::damage::Table {
        &self.wrecks.table
    }

    /// Puts damage on the truck, from its text (`Damage::to_text`): what it had on it when
    /// it was last driven.
    pub fn set_damage(&mut self, text: &str) -> Result<(), String> {
        self.wrecks.set_damage(crate::damage::Damage::parse(text)?);
        Ok(())
    }

    /// Damage as words say it, for a picture: `bumper_front=dented,door_FL=loose`.
    pub fn stage_damage(&mut self, words: &str) {
        self.wrecks.stage(words);
    }

    /// Hits the truck, as if the game had measured the blow: for a picture, or a trial.
    pub fn strike(&mut self, hit: crate::damage::Hit) {
        self.wrecks.strike(hit);
    }

    /// Mends the whole truck, or one piece of it. Whether that piece was hurt.
    pub fn repair(&mut self) {
        self.wrecks.repair();
    }

    pub fn mend(&mut self, piece: &str) -> bool {
        self.wrecks.mend(piece)
    }

    /// What blows have done to the truck since this was last asked: each piece whose
    /// stage changed, for whoever tells the player.
    pub fn take_damage_changes(&mut self) -> Vec<crate::damage::Change> {
        self.wrecks.take_changes()
    }

    /// What the truck's last blow did to it, to show beside its instruments for a few
    /// seconds after: nothing once that long has passed, or if it did nothing.
    pub fn damage_line(&self) -> String {
        self.wrecks.told()
    }

    /// Measures the truck's pieces from the scene it is in, if its own files do not say
    /// where they are: a truck baked into its map, and not put together from its pack.
    pub fn measure_truck(&mut self, scene: &solstice_asset::LoadedScene) {
        let stood = self.play.car_pose();
        if let (Some(kit), Some((at, turned))) = (&mut self.kit, stood) {
            if !kit.is_measured() {
                kit.measure(scene, glam::Mat4::from_rotation_translation(turned, at));
            }
        }
    }

    /// The beam of the truck's head lamps, as many of them as still light.
    fn beam(&self, lamp: Option<solstice_render::Lamp>) -> Option<solstice_render::Lamp> {
        let share = self.wrecks.beam();
        lamp.filter(|_| share > 0.0)
            .map(|lamp| solstice_render::Lamp {
                intensity: lamp.intensity * share,
                ..lamp
            })
    }
}
