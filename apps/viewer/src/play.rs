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
    /// What the player's car's wheels throw up.
    spray: Spray,
    /// The plants a car flattens, and which are down already.
    scrub: Scrub,
    /// What is heard, once [`PlayMode::with_sound`] has opened an output for it, or, in a
    /// browser, once the page has said it will play what is asked of it.
    sound: Option<Soundscape>,
    /// Frames run, and whether to log where the player's car is every half second of them:
    /// set `SOLSTICE_TRACE` to try a track without watching it.
    frames: u32,
    trace: bool,
}

/// How bright the car's high beams are, in candela, as the renderer counts them.
const LAMPS: f32 = 560.0;

/// What a wheel throws up from the ground it is on: clods of mud, dust off dry clay, and
/// water where it fords a creek. One effect a wheel, moved with it, with how much of each it
/// throws and which way set from how the wheel is turning.
const WHEEL_SPRAY: &str = "\
param mud 0
param dust 0
param water 0
# Which way it is thrown, from the wheel: to the car's right, up, and behind it.
param vx 0
param vy 2.5
param vz 3

# Clods of mud off the tread: thrown up and back, falling where they land.
emitter clods
  budget 900
  spawn rate=320*mud
  shape sphere 0.14
  lifetime 0.7 1.4
  velocity vx vy vz cone=26 spread=0.5
  size 0.012 0.04
  gravity
  drag 0.1
  color 0.1 0.048 0.022 1
  collide depth field kill
  render sprite solid

# Dust off dry ground: it hangs, drifts with the wind, and thins.
emitter dust
  budget 420
  spawn rate=34*dust
  shape sphere 0.3
  lifetime 2.4 4
  velocity 0 0.5 1.2 cone=45 spread=0.5
  size 0.3 0.6
  drag 1.3
  wind 0.8
  curl 0.5 scale=1.6 change=0.4
  color 0.6 0.36 0.2 0.14
  color life 0=1 1 1 0 0.1=1 1 1 1 1=1 1 1 0
  size life 0=0.5 1=3.4
  render sprite sort soft=0.5 phase=0.3

# Water thrown out of a ford: sheets of drops to the side and up, brown with the silt.
emitter splash
  budget 2600
  spawn rate=1100*water
  shape sphere 0.25
  lifetime 0.7 1.4
  velocity vx vy vz cone=36 spread=0.55
  size 0.008 0.024
  gravity
  drag 0.2
  color 0.62 0.55 0.45 1
  collide depth field water kill
  render sprite solid phase=0.65

# And the spray that hangs over it.
emitter spume
  budget 320
  spawn rate=46*water
  shape sphere 0.4
  offset 0 0.3 0
  lifetime 1.2 2.2
  velocity 0 1.3 0.6 cone=60 spread=0.5
  size 0.2 0.45
  gravity 0.25
  drag 0.9
  wind 0.6
  color 0.82 0.8 0.74 0.07
  color life 0=1 1 1 0 0.2=1 1 1 1 1=1 1 1 0
  size life 0=0.6 1=2.8
  render sprite sort soft=0.5 phase=0.6
";

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
            engine = crate::sound::EngineNote {
                rpm: t.rpm,
                throttle: t.throttle,
                limiter: t.limiter,
                gain: 0.9,
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
        self.sounds.frame(|ask| {
            ask.engine(engine);
            for (name, (gain, pitch)) in TYRE_LOOPS.iter().zip(loops) {
                ask.looping(name, gain.min(1.0), pitch);
            }
            // The bush: birds and insects in the dry, the wind as it blows, the rain as it
            // falls.
            ask.looping("bush_day", 0.3 * (1.0 - 0.75 * rain), 1.0);
            ask.looping("bush_wind", 0.35 * wind, 1.0);
            ask.looping("rain_leaves", 0.55 * rain, 1.0);
            for (name, gain, pitch) in once {
                ask.once(name, gain, pitch);
            }
        });
    }
}

/// The spray off each wheel of the player's car.
#[derive(Default)]
struct Spray {
    effects: Option<[solstice_render::EffectId; 4]>,
    /// Whether starting the effects has been tried: once is enough to say it failed.
    tried: bool,
}

impl Spray {
    /// Moves each wheel's spray to the wheel, and sets what it throws: `wetness` is how wet
    /// the rain has left the ground.
    fn follow(&mut self, renderer: &mut Renderer, hud: &solstice_game::Hud, wetness: f32) {
        if self.effects.is_none() && !self.tried {
            self.tried = true;
            match solstice_render::Effect::parse(WHEEL_SPRAY) {
                Ok(effect) => {
                    let mut ids = Vec::new();
                    for _ in 0..4 {
                        ids.extend(renderer.particles().spawn(&effect, glam::Mat4::IDENTITY));
                    }
                    self.effects = ids.try_into().ok();
                    if self.effects.is_none() {
                        log::warn!("no room among the particles for the spray off the wheels");
                    }
                }
                Err(error) => log::error!("the spray off the wheels: {error}"),
            }
        }
        let Some(effects) = self.effects else {
            return;
        };
        let forward = hud.forward;
        let yaw = (-forward.x).atan2(-forward.z);
        let turned = glam::Quat::from_rotation_y(yaw);
        let speed = hud.velocity.length();
        let going = hud.telemetry.speed;
        for (index, (wheel, id)) in hud.wheels.iter().zip(effects).enumerate() {
            let particles = renderer.particles();
            particles.set_transform(
                id,
                glam::Mat4::from_rotation_translation(turned, wheel.at + glam::Vec3::Y * 0.1),
            );
            let (mut mud, mut dust, mut water) = (0.0_f32, 0.0_f32, 0.0_f32);
            // How fast the tread passes over the ground, and so how hard it throws.
            let scrub = wheel.slip.abs();
            let pressed = wheel.load.clamp(0.0, 1.5);
            if wheel.on_ground {
                match wheel.surface.name() {
                    "ford" => water = (speed * 0.22 + scrub * 0.35).min(2.2),
                    "mud" | "bog" => {
                        mud = ((scrub * 0.55 + speed * 0.1) * (0.5 + wheel.sink * 7.0)).min(1.6)
                    }
                    // Wet clay throws mud; dry, it smokes.
                    "dirt" | "sand" | "litter" | "gravel" => {
                        mud = (scrub * 0.4 + speed * 0.05).min(1.2) * wetness;
                        dust = (speed * 0.07 + scrub * 0.45).min(1.6) * (1.0 - wetness).powi(2);
                    }
                    "grass" => dust = (scrub * 0.25).min(0.6) * (1.0 - wetness).powi(2),
                    _ => {}
                }
            }
            // The left wheels throw to the left, the right to the right. A wheel throws
            // behind it the way its tread is going: forward, when it turns backwards.
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let back = if going + wheel.slip >= 0.0 { 1.0 } else { -1.0 };
            let (vx, vy, vz) = if water > 0.0 {
                (
                    side * (1.3 + 0.28 * speed).min(6.0),
                    (2.0 + 0.32 * speed).min(7.5),
                    back * (0.4 + 0.12 * speed),
                )
            } else {
                (
                    side * 0.5,
                    (1.4 + 0.3 * scrub + 0.06 * speed).min(6.0),
                    back * (1.4 + 0.45 * scrub + 0.1 * speed).min(8.0),
                )
            };
            for (name, value) in [
                ("mud", mud * pressed),
                ("dust", dust * pressed),
                ("water", water),
                ("vx", vx),
                ("vy", vy),
                ("vz", vz),
            ] {
                particles.set(id, name, value);
            }
        }
    }

    /// Stops every wheel throwing: the car is not being driven.
    fn rest(&mut self, renderer: &mut Renderer) {
        for id in self.effects.into_iter().flatten() {
            for name in ["mud", "dust", "water"] {
                renderer.particles().set(id, name, 0.0);
            }
        }
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
        Ok(Self {
            play,
            devices: Devices::new(),
            lamps: false,
            tail_lamps: Vec::new(),
            head_lamps: Vec::new(),
            spray: Spray::default(),
            scrub: Scrub::read(files),
            sound: None,
            frames: 0,
            trace: std::env::var_os("SOLSTICE_TRACE").is_some(),
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
        });
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
        renderer.set_lamp(lamp);
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
        for &instance in &self.tail_lamps {
            renderer.set_instance_glow(instance, -tail);
        }
        for &instance in &self.head_lamps {
            renderer.set_instance_glow(instance, head);
        }
        let mut flattened = (0, 0.0);
        match self.play.hud() {
            Some(hud) => {
                self.spray.follow(renderer, hud, wetness);
                flattened = self.scrub.flatten(renderer, hud);
            }
            None => self.spray.rest(renderer),
        }
        if let Some(sound) = &mut self.sound {
            sound.frame(self.play.hud(), flattened, renderer, seconds);
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
