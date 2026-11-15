//! Mud: the ruts the trucks leave drawn, and what a truck's wheels throw (`docs/mud.md`).
//!
//! * **The ruts.** The game's physics keeps the map of what tyres have pressed into the
//!   ground (`solstice_vehicle::rut`), which every wheel stands in. [`draw`] hands the
//!   renderer the squares of it round the truck, as they change, and the renderer draws
//!   them (`solstice_render::ruts`).
//! * **The fling.** [`Fling`] is what comes off each wheel of the player's truck: mud off
//!   the tread, thrown the way the tread is going where it leaves the ground; water out of
//!   a creek; dust off dry ground. How much follows how fast the tread passes, how deep
//!   the tyre is in, what the ground is and how wet, and how much tread the tyre has. The
//!   lumps that land are laid on the ground where they fall, in the same map as the ruts,
//!   and what goes up the truck's flanks is on the truck (`play.rs`, `Wear`).

use glam::{Mat4, Quat, Vec2, Vec3};
use solstice_game::drive::HudWheel;
use solstice_game::rut::{self, Ruts};
use solstice_render::{EffectId, Renderer, RutSource};

// The map's squares are copied to the renderer as they are kept: both must agree on what a
// square is.
const _: () = assert!(
    rut::TILE == solstice_render::ruts::TILE as usize
        && rut::LEVELS == solstice_render::ruts::LEVELS as usize
);

/// The map of ruts, as the renderer asks for it.
struct Kept<'a>(&'a Ruts);

impl RutSource for Kept<'_> {
    fn cell(&self) -> f32 {
        rut::CELL
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn version(&self, tile: (i32, i32)) -> u32 {
        self.0.version(tile)
    }

    fn levels(&self, tile: (i32, i32), layer: usize) -> Option<Vec<Vec<u8>>> {
        let mut levels = vec![self.0.layer(tile, layer)?.to_vec()];
        levels.extend(self.0.smaller(tile, layer)?);
        Some(levels)
    }
}

/// Brings the ruts the renderer draws up to date with the ones the game keeps, round
/// `centre`: the truck, or wherever is looked at. Nothing is done on ground nobody has
/// driven on.
pub fn draw(renderer: &mut Renderer, play: &solstice_game::Play, centre: Vec3) {
    play.ruts(|ruts| {
        // Nobody else keeps a list of what changed: each square's own count says.
        ruts.take_changed();
        renderer.sync_ruts(Vec2::new(centre.x, centre.z), &Kept(ruts));
    });
}

/// What comes off one wheel, as the particles are told it (`WHEEL_FLING`'s parameters).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thrown {
    /// How much mud, dust and water, each from 0 to about 2.
    pub mud: f32,
    pub dust: f32,
    pub water: f32,
    /// The way the tread throws what leaves it, in the world: metres a second. And the
    /// way it throws its lumps, which leave it sooner.
    pub tread: Vec3,
    pub lumps: Vec3,
    /// Where on the tyre it leaves from, in the world.
    pub from: Vec3,
}

/// How far round the tyre from the bottom of it mud lets go of the tread, in radians: it
/// is carried up behind the contact, and thrown along the tread's way there.
const LEAVES_AT: f32 = 0.45;
/// And where the lumps let go, which are the heaviest of it and the first to: lower, so
/// they are thrown flatter and land sooner.
const LUMPS_LEAVE_AT: f32 = 0.27;
/// How fast a tread must pass, in metres a second, to throw any of what it carries, and to
/// throw all of it.
const THROWS_FROM: f32 = 1.2;
const THROWS_ALL: f32 = 9.0;
/// How many lumps a second land and are laid on the ground, for each unit of mud thrown,
/// and the most in one frame.
const LUMPS_A_SECOND: f32 = 9.0;
const LUMPS_A_FRAME: usize = 6;

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What one wheel throws.
///
/// * `forward` is the way the truck points and `going` how fast it goes that way;
///   `speed` is how fast it is going in all.
/// * `wetness` is how wet the rain has left the ground, from 0 to 1.
pub fn thrown(wheel: &HudWheel, forward: Vec3, going: f32, speed: f32, wetness: f32) -> Thrown {
    use solstice_game::Surface;
    // How fast the tread passes, and which way: backwards under a wheel in reverse.
    let tread = going + wheel.slip;
    let passing = tread.abs();
    let scrub = wheel.slip.abs();
    let pressed = wheel.load.clamp(0.0, 1.5);
    let soil = wheel.surface.soil(wetness);
    // A tyre with deep lugs carries more of what it is in, and throws it further.
    let lugs = ((wheel.tread - 0.8) / 0.4).clamp(0.0, 1.0);
    let holds = 0.55 + 0.9 * lugs;
    let flung = smoothstep(THROWS_FROM, THROWS_ALL, passing);

    let (mut mud, mut dust, mut water) = (0.0_f32, 0.0_f32, 0.0_f32);
    // The mud still on the tread from where it has been comes off whatever it is on now,
    // and in the air.
    let mut on_tread = 0.6 * wheel.carried.1;
    if wheel.on_ground {
        let sunk = (wheel.sink.max(0.0) / 0.06).min(1.5);
        match wheel.surface {
            Surface::Ford => water = (speed * 0.22 + scrub * 0.35).min(2.2),
            surface => {
                // Mud is what is wet, churned and deep: all three in a bog, none on a dry
                // track.
                on_tread = on_tread.max(soil.churn * soil.wet * (0.25 + sunk) * pressed);
                // A bog has water standing in it, which goes up with the mud.
                if soil.wet >= 1.0 {
                    water = 0.4 * flung * sunk;
                }
                let dusty = match surface {
                    Surface::Dirt => 1.0,
                    Surface::Sand => 1.1,
                    Surface::Gravel => 0.8,
                    Surface::Litter => 0.5,
                    Surface::Grass => 0.35,
                    _ => 0.0,
                };
                let dry = (1.0 - wetness).powi(2) * (1.0 - soil.wet).max(0.0);
                dust = (speed * 0.07 + scrub * 0.45).min(1.6) * dusty * dry * pressed;
            }
        }
    }
    if passing > THROWS_FROM * 0.5 {
        mud = (on_tread * holds * (0.12 + 1.15 * flung)).min(2.2);
    }

    // The tread's own speed where the mud leaves it, behind the contact and a little up
    // the tyre, on top of the truck's: along the tyre's edge there.
    let back = if tread >= 0.0 { 1.0 } else { -1.0 };
    let (sin, cos) = LEAVES_AT.sin_cos();
    let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let velocity = flat * (going - tread * cos) + Vec3::Y * (passing * sin);
    let from =
        wheel.at + Vec3::Y * (wheel.radius * (1.0 - cos)) - flat * (back * wheel.radius * sin);
    let (sin, cos) = LUMPS_LEAVE_AT.sin_cos();
    Thrown {
        mud,
        dust,
        water,
        tread: velocity,
        lumps: flat * (going - tread * cos) + Vec3::Y * (passing * sin),
        from,
    }
}

/// What the wheels of the player's truck throw.
pub struct Fling {
    effects: Option<[EffectId; 4]>,
    /// Whether starting the effects has been tried: once is enough to say it failed.
    tried: bool,
    /// What the four wheels threw between them this frame: mud, dust and water. The
    /// truck wears it (`Wear::gather`).
    pub thrown: [f32; 3],
    /// For where each lump lands.
    random: u32,
    /// Each wheel's lumps not yet landed: the part of one that a frame's throwing came to.
    owed: [f32; 4],
}

impl Default for Fling {
    fn default() -> Self {
        Self {
            effects: None,
            tried: false,
            thrown: [0.0; 3],
            random: 0x2f6b_1d3c,
            owed: [0.0; 4],
        }
    }
}

impl Fling {
    /// A number from 0 to 1, different each time.
    fn chance(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random >> 8) as f32 / 16_777_216.0
    }

    /// Moves each wheel's fling to its wheel, and sets what it throws. Returns the lumps of
    /// mud that land this frame, each where it lands and how far across it is, in metres,
    /// to be laid on the ground ([`Ruts::splat`]).
    pub fn follow(
        &mut self,
        renderer: &mut Renderer,
        hud: &solstice_game::Hud,
        wetness: f32,
        seconds: f32,
    ) -> Vec<(Vec2, f32)> {
        self.thrown = [0.0; 3];
        if self.effects.is_none() && !self.tried {
            self.tried = true;
            match solstice_render::Effect::parse(WHEEL_FLING) {
                Ok(effect) => {
                    let mut ids = Vec::new();
                    for _ in 0..4 {
                        ids.extend(renderer.particles().spawn(&effect, Mat4::IDENTITY));
                    }
                    self.effects = ids.try_into().ok();
                    if self.effects.is_none() {
                        log::warn!("no room among the particles for what the wheels throw");
                    }
                }
                Err(error) => log::error!("what the wheels throw: {error}"),
            }
        }
        let forward = hud.forward;
        let yaw = (-forward.x).atan2(-forward.z);
        let turned = Quat::from_rotation_y(yaw);
        let speed = hud.velocity.length();
        let going = hud.telemetry.speed;
        let mut lumps = Vec::new();
        for (index, wheel) in hud.wheels.iter().enumerate() {
            let thrown = thrown(wheel, forward, going, speed, wetness);
            for (sum, one) in self
                .thrown
                .iter_mut()
                .zip([thrown.mud, thrown.dust, thrown.water])
            {
                *sum += one;
            }
            // The left wheels throw a little to the left, the right to the right.
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let passing = (going + wheel.slip).abs();

            // The lumps heavy enough to land as lumps: each thrown as the tread throws,
            // some faster and higher than others, and laid where it comes down.
            self.owed[index] += thrown.mud * LUMPS_A_SECOND * seconds.min(0.1);
            while self.owed[index] >= 1.0 && lumps.len() < LUMPS_A_FRAME {
                self.owed[index] -= 1.0;
                if wheel.surface == solstice_game::Surface::Ford {
                    continue;
                }
                let hard = 0.5 + 0.6 * self.chance();
                let lift = 0.6 + 0.9 * self.chance();
                let across = turned * Vec3::X * (side * 0.3 + (self.chance() - 0.5) * 2.4);
                let velocity = Vec3::new(thrown.lumps.x, 0.0, thrown.lumps.z) * hard
                    + across
                    + Vec3::Y * thrown.lumps.y * hard * lift;
                // As long in the air as it takes to come back down to the ground under
                // the wheel.
                let height = (thrown.from.y - wheel.at.y).max(0.0);
                let flight =
                    (velocity.y + (velocity.y * velocity.y + 19.62 * height).sqrt()) / 9.81;
                let lands = thrown.from + velocity * flight;
                // The bigger the lump, the rarer.
                let size = 0.035 + 0.075 * self.chance().powi(3);
                lumps.push((Vec2::new(lands.x, lands.z), size));
            }
            self.owed[index] = self.owed[index].min(2.0);

            let Some(effects) = self.effects else {
                continue;
            };
            let id = effects[index];
            let particles = renderer.particles();
            particles.set_transform(id, Mat4::from_rotation_translation(turned, thrown.from));
            // In the effect's own space: x to the truck's right, y up, z behind it.
            let own = turned.inverse() * thrown.tread;
            let lumps = turned.inverse() * thrown.lumps;
            // Water goes out to the side in sheets, and up.
            let back = if going + wheel.slip >= 0.0 { 1.0 } else { -1.0 };
            for (name, value) in [
                ("mud", thrown.mud),
                ("dust", thrown.dust),
                ("water", thrown.water),
                ("tx", own.x + side * (0.25 + 0.04 * passing)),
                ("ty", own.y),
                ("tz", own.z),
                ("cx", lumps.x + side * (0.25 + 0.04 * passing)),
                ("cy", lumps.y),
                ("cz", lumps.z),
                ("wx", side * (1.3 + 0.28 * speed).min(6.0)),
                ("wy", (2.0 + 0.32 * speed).min(7.5)),
                ("wz", back * (0.4 + 0.12 * speed)),
            ] {
                particles.set(id, name, value);
            }
        }
        lumps
    }

    /// Stops every wheel throwing: the car is not being driven.
    pub fn rest(&mut self, renderer: &mut Renderer) {
        self.thrown = [0.0; 3];
        for id in self.effects.into_iter().flatten() {
            for name in ["mud", "dust", "water"] {
                renderer.particles().set(id, name, 0.0);
            }
        }
    }
}

/// What one wheel throws, as particles: one of these follows each wheel.
///
/// Mud leaves a tread in three sizes. The lugs let go of **clods**, heavy enough to arc
/// and land where gravity takes them, each bursting a little where it lands. Between them
/// the tread draws out **ropes**, strings of wet mud that fly in a line and break up. And
/// everything that tears throws **spatter**, fine and wide. All of it goes the way the
/// tread is going where it leaves the tyre (`tx ty tz`), which is why a spinning wheel
/// roosts up and back and a rolling one lobs its mud up its own flank.
pub const WHEEL_FLING: &str = "\
param mud 0
param dust 0
param water 0
# The way the tread throws what leaves it, from the wheel: to the car's right, up, and
# behind it, in metres a second.
param tx 0
param ty 2.5
param tz 3
# And the way it throws its lumps, which let go of it sooner and so fly flatter.
param cx 0
param cy 1.5
param cz 3
# The way water is thrown out of a creek: sideways, in sheets.
param wx 0
param wy 2
param wz 0.5

# Clods off the lugs: thrown as the tread goes, heavy, falling where they land and lying
# there a moment.
emitter clods
  budget 400
  spawn rate=34*mud
  shape sphere 0.09
  lifetime 1.2 2.0
  velocity cx cy cz cone=13 spread=0.4
  size 0.018 0.042
  gravity
  drag 0.15
  color 0.085 0.04 0.019 1
  size life 0=1 0.8=1 1=0
  collide depth field stick event=landed
  render sprite solid

# Ropes of mud drawn out between the lugs: in a line, close together, breaking up.
emitter ropes
  budget 1400
  spawn rate=420*mud
  shape sphere 0.04
  lifetime 0.35 0.7
  velocity tx ty tz cone=6 spread=0.25
  size 0.011 0.022
  gravity
  drag 1.2
  color 0.1 0.048 0.022 1
  collide depth field kill
  render sprite solid

# Spatter: the fine stuff, every way a tread can throw it, and soon down.
emitter spatter
  budget 800
  spawn rate=280*mud
  shape sphere 0.12
  lifetime 0.25 0.5
  velocity tx ty tz cone=28 spread=0.6
  size 0.006 0.012
  gravity
  drag 2.2
  color 0.12 0.06 0.03 1
  collide depth field kill
  render sprite solid

# Where a clod lands it bursts: a few drops of it, up and out.
emitter burst
  budget 500
  spawn event=landed count=2
  lifetime 0.15 0.35
  velocity 0 1.1 0 cone=65 spread=0.6
  size 0.003 0.007
  gravity
  color 0.1 0.05 0.024 1
  render sprite solid

# Dust off dry ground: it billows up behind the tyre, hangs, drifts with the wind, thins.
emitter dust
  budget 700
  spawn rate=75*dust
  shape sphere 0.3
  lifetime 2.4 4.2
  velocity 0 0.5 1.2 cone=45 spread=0.5
  size 0.3 0.6
  drag 1.3
  wind 0.8
  curl 0.55 scale=1.6 change=0.4
  color 0.6 0.36 0.2 0.26
  color life 0=1 1 1 0 0.1=1 1 1 1 1=1 1 1 0
  size life 0=0.5 1=3.6
  render sprite sort soft=0.5 phase=0.3

# Water thrown out of a creek: sheets of drops to the side and up, brown with the silt.
emitter splash
  budget 2600
  spawn rate=1100*water
  shape sphere 0.25
  lifetime 0.7 1.4
  velocity wx wy wz cone=36 spread=0.55
  size 0.008 0.024
  gravity
  drag 0.2
  color 0.62 0.55 0.45 1
  collide depth field water kill
  render sprite solid phase=0.65

# And the mist that hangs over it.
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

#[cfg(test)]
mod tests {
    use super::*;
    use solstice_game::Surface;

    fn wheel(surface: Surface, slip: f32, sink: f32) -> HudWheel {
        HudWheel {
            at: Vec3::ZERO,
            on_ground: true,
            surface,
            slip,
            sink,
            load: 1.0,
            carried: (0.0, 0.0),
            radius: 0.44,
            tread: 1.0,
        }
    }

    #[test]
    fn the_fling_is_an_effect_the_particles_take() {
        let effect = solstice_render::Effect::parse(WHEEL_FLING).expect("it parses");
        assert_eq!(effect.emitters.len(), 7);
    }

    #[test]
    fn a_spinning_wheel_in_a_bog_roosts_up_and_back() {
        let forward = Vec3::NEG_Z;
        // Standing, wheels spinning at 10 m/s, a hand deep in a bog.
        let spun = thrown(&wheel(Surface::Bog, 10.0, 0.12), forward, 0.0, 0.0, 0.0);
        assert!(spun.mud > 1.0, "{}", spun.mud);
        assert!(spun.tread.z > 6.0 && spun.tread.y > 3.5, "{}", spun.tread);
        // Its lumps go flatter than the rest.
        assert!(spun.lumps.y < spun.tread.y && spun.lumps.z > spun.tread.z);
        // It leaves the tyre behind the contact, off the ground.
        assert!(spun.from.z > 0.15 && spun.from.y > 0.03, "{}", spun.from);
        // Crawling through it without spin throws next to nothing.
        let crawled = thrown(&wheel(Surface::Bog, 0.0, 0.12), forward, 1.0, 1.0, 0.0);
        assert!(crawled.mud < 0.25 * spun.mud, "{}", crawled.mud);
        // Rolling fast with no spin, the mud goes up the flank, slower than the truck.
        let rolled = thrown(&wheel(Surface::Bog, 0.0, 0.1), forward, 10.0, 10.0, 0.0);
        assert!(rolled.mud > 0.5 && rolled.tread.y > 3.5);
        assert!(
            rolled.tread.z < 0.0 && rolled.tread.z > -5.0,
            "{}",
            rolled.tread
        );
        // In reverse the tread goes the other way.
        let reversed = thrown(&wheel(Surface::Bog, -8.0, 0.1), forward, -1.0, 1.0, 0.0);
        assert!(reversed.tread.z < -4.0 && reversed.from.z < -0.15);
    }

    #[test]
    fn what_is_thrown_follows_the_ground_the_weather_and_the_tyre() {
        let forward = Vec3::NEG_Z;
        let at = |surface, wetness| thrown(&wheel(surface, 3.0, 0.01), forward, 8.0, 8.0, wetness);
        // Dry clay smokes and throws no mud; wet, the other way about.
        let (dry, wet) = (at(Surface::Dirt, 0.0), at(Surface::Dirt, 1.0));
        assert!(dry.dust > 0.5 && dry.mud < 0.05, "{dry:?}");
        assert!(wet.dust < 0.01 && wet.mud > 0.15, "{wet:?}");
        // Rock throws nothing, a creek throws water.
        let rock = at(Surface::Rock, 0.0);
        assert_eq!((rock.mud, rock.dust, rock.water), (0.0, 0.0, 0.0));
        let ford = at(Surface::Ford, 0.0);
        assert!(ford.water > 1.0 && ford.mud == 0.0);
        // Deeper in, more mud; a mud tyre more than a road tyre.
        let deep = thrown(&wheel(Surface::Mud, 3.0, 0.09), forward, 8.0, 8.0, 0.0);
        let shallow = thrown(&wheel(Surface::Mud, 3.0, 0.02), forward, 8.0, 8.0, 0.0);
        assert!(deep.mud > shallow.mud * 1.5);
        let tyre = |tread| {
            let wheel = HudWheel {
                tread,
                ..wheel(Surface::Mud, 3.0, 0.06)
            };
            thrown(&wheel, forward, 8.0, 8.0, 0.0).mud
        };
        assert!(tyre(1.18) > tyre(0.8) * 1.6);
        // A tyre out of a bog still throws its mud on rock, and in the air.
        let muddy = HudWheel {
            carried: (0.0, 1.0),
            ..wheel(Surface::Rock, 2.0, 0.0)
        };
        assert!(thrown(&muddy, forward, 8.0, 8.0, 0.0).mud > 0.3);
        let airborne = HudWheel {
            on_ground: false,
            ..muddy
        };
        assert!(thrown(&airborne, forward, 8.0, 8.0, 0.0).mud > 0.3);
    }
}
