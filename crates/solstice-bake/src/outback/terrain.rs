//! The lie of the land: hills, the gullies water has cut in them, a creek with its waterholes,
//! and a four-wheel-drive track found across it all.
//!
//! * **The hills** are noise at several scales about a valley that crosses the whole square,
//!   so that there is always a creek, and it always runs right through.
//! * **Water shapes them.** Every hollow is filled until it spills, each point's rain is sent
//!   down the steepest way, and where enough gathers the land is cut: a spoonful of catchment
//!   makes a dip, a hillside's makes a gully, and the valley's makes the creek. How much has
//!   gathered also says where the ground is damp, which is where the mud is, and what grows.
//! * **Waterholes** are basins cut in the creek's bed, as a creek in summer is a chain of pools.
//!   A basin's rim is raised where the bed below would otherwise lie under its water.
//! * **The track** is found, not drawn: between a few places worth driving to (two fords, the
//!   top of each side's hill, the dampest flat) it takes the easiest line, as a track made by
//!   use does, climbing spurs and following contours. Then it is worn into the land, and
//!   broken by water, as every bush track is:
//!   - two wheel ruts with a strip of litter and grass between, tilted with the hillside it
//!     crosses rather than graded level, and rising and falling along its length;
//!   - on its hills, the rill that rain cuts down it: a gutter a wheel can drop into, which
//!     snakes from one side of the track to the other and bares the stones under the clay;
//!   - washouts where a gully crosses it and has taken the track away with it;
//!   - holes dug by spinning wheels, left and right by turns, which cross a car's axles;
//!   - drainage humps, bog holes in the wet flats, and ledges where it climbs over rock.

use glam::{Vec2, Vec3};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use solstice_procgen::noise::{fbm, ridged};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Metres from the square's middle to the edge of the land the design reaches: beyond what
/// is driven on, so that gullies run out of it rather than ending at a fence.
const MARGIN: f32 = 80.0;
/// How far inside the driven square the track and everything on it keeps from the edge.
const KEEP_IN: f32 = 45.0;
/// Half the width of the track's tread, and how far out its cut and fill reach.
pub const TREAD: f32 = 1.75;
const BATTER: f32 = 5.5;
/// How far from the edge of a main road its cutting and its filling have run out into the
/// land: a road that is made is battered back gently, not cut as a track is worn.
const ROAD_BATTER: f32 = 13.0;
/// Metres between the track's points.
const STEP: f32 = 1.5;
/// Metres between the wheel ruts' middles: a four-wheel drive's track.
const RUTS: f32 = 1.74;

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn gauss(x: f32, width: f32) -> f32 {
    (-(x / width) * (x / width)).exp()
}

/// Values at the points of a square grid centred on the origin.
#[derive(Clone)]
pub struct Grid {
    /// Squares a side: there is one more point than that.
    pub cells: usize,
    pub origin: f32,
    pub spacing: f32,
    pub values: Vec<f32>,
}

impl Grid {
    pub fn new(half: f32, spacing: f32) -> Self {
        let cells = (2.0 * half / spacing).round() as usize;
        Self {
            cells,
            origin: -half,
            spacing,
            values: vec![0.0; (cells + 1) * (cells + 1)],
        }
    }

    pub fn stride(&self) -> usize {
        self.cells + 1
    }

    pub fn place(&self, index: usize) -> (f32, f32) {
        let stride = self.stride();
        (
            self.origin + (index % stride) as f32 * self.spacing,
            self.origin + (index / stride) as f32 * self.spacing,
        )
    }

    /// The point nearest a place, kept on the grid.
    pub fn nearest(&self, x: f32, z: f32) -> usize {
        let last = self.cells as f32;
        let column = ((x - self.origin) / self.spacing).round().clamp(0.0, last) as usize;
        let row = ((z - self.origin) / self.spacing).round().clamp(0.0, last) as usize;
        row * self.stride() + column
    }

    /// The value at a place, between the grid's points.
    pub fn at(&self, x: f32, z: f32) -> f32 {
        let limit = self.cells as f32 - 1.0e-3;
        let u = ((x - self.origin) / self.spacing).clamp(0.0, limit);
        let v = ((z - self.origin) / self.spacing).clamp(0.0, limit);
        let (column, row) = (u as usize, v as usize);
        let (fu, fv) = (u - column as f32, v - row as f32);
        let stride = self.stride();
        let value = |column: usize, row: usize| self.values[row * stride + column];
        let near = value(column, row) + (value(column + 1, row) - value(column, row)) * fu;
        let far =
            value(column, row + 1) + (value(column + 1, row + 1) - value(column, row + 1)) * fu;
        near + (far - near) * fv
    }

    /// How steep the values are at a place: their rise over their run.
    pub fn slope(&self, x: f32, z: f32) -> f32 {
        let step = self.spacing.max(1.5);
        let dx = (self.at(x + step, z) - self.at(x - step, z)) / (2.0 * step);
        let dz = (self.at(x, z + step) - self.at(x, z - step)) / (2.0 * step);
        dx.hypot(dz)
    }

    /// Whether a place is on the grid.
    pub fn holds(&self, x: f32, z: f32) -> bool {
        x.abs() <= -self.origin && z.abs() <= -self.origin
    }
}

/// Where the valley's middle runs: its z for an x. It wanders, as a creek's valley does.
fn valley_axis(x: f32) -> f32 {
    0.22 * x + 105.0 * (x / 240.0 + 0.9).sin() + 38.0 * (x / 95.0 + 2.1).sin() - 20.0
}

/// What kind of country a square of bush is, and what kind of track runs through it. Each of
/// the game's maps is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Country {
    /// What the map is called.
    pub name: &'static str,
    /// Whether it is a hub: a road to drive between trailheads, not a track to get over.
    pub hub: bool,
    /// Where in the hills it is, so that each map has its own, and whether its valley runs
    /// the other way.
    pub shift: Vec3,
    pub turned: bool,
    /// How high its hills and spurs stand, against Wombat Creek's.
    pub relief: f32,
    /// How much more of it is bare rock: taken off what it takes for stone to show.
    pub stone: f32,
    /// How wet it is: what water gathers is multiplied by the one, and the other is how damp
    /// even its dry ground is.
    pub damp: f32,
    pub sodden: f32,
    /// How steep a climb its track takes before going round, against Wombat Creek's.
    pub climbs: f32,
    /// How many bog holes its track may have, and how deep its ruts are worn.
    pub bogs: usize,
    pub rutted: f32,
    /// Whether its track may cross itself. The first two maps' tracks were found before this
    /// was looked for, and are kept as they were found.
    pub crosses: bool,
    /// Whether it is a town: a hub whose camp is a town's yard, with no shed.
    pub town: bool,
}

impl Country {
    /// Wombat Creek Track: a washed-out fire trail in foothill forest.
    pub const TRACK: Self = Self {
        name: "Wombat Creek Track",
        hub: false,
        shift: Vec3::ZERO,
        turned: false,
        relief: 1.0,
        stone: 0.0,
        damp: 1.0,
        sodden: 0.0,
        climbs: 1.0,
        bogs: 9,
        rutted: 1.0,
        crosses: true,
        town: false,
    };
    /// The Crossroads: the hub, with a graded road round it.
    pub const HUB: Self = Self {
        name: "The Crossroads",
        hub: true,
        shift: Vec3::new(3170.0, 0.0, -2260.0),
        turned: true,
        ..Self::TRACK
    };
    /// Granite Ridge: high, steep and stony. A track of slabs and ledges, dry underfoot.
    pub const ROCK: Self = Self {
        name: "Granite Ridge",
        shift: Vec3::new(-5240.0, 0.0, 1730.0),
        relief: 1.55,
        stone: 0.3,
        damp: 0.6,
        climbs: 1.7,
        bogs: 2,
        rutted: 0.7,
        crosses: false,
        ..Self::TRACK
    };
    /// Blackwood Gully: low, wet forest. A track of mud, bog holes and deep ruts.
    pub const MUD: Self = Self {
        name: "Blackwood Gully",
        shift: Vec3::new(1410.0, 0.0, 6890.0),
        turned: true,
        relief: 0.7,
        stone: -0.12,
        damp: 1.9,
        sodden: 0.17,
        climbs: 0.8,
        bogs: 16,
        rutted: 1.5,
        crosses: false,
        ..Self::TRACK
    };

    /// The town: gentler country lower down the same road, with a town where it starts.
    pub const TOWN: Self = Self {
        name: "Ironbark Flat",
        hub: true,
        town: true,
        shift: Vec3::new(-2380.0, 0.0, -4720.0),
        turned: false,
        relief: 0.55,
        stone: -0.1,
        ..Self::TRACK
    };

    /// The country a scene file names: `track`, `hub`, `town`, `rock` or `mud`.
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "track" => Some(Self::TRACK),
            "hub" => Some(Self::HUB),
            "town" => Some(Self::TOWN),
            "rock" => Some(Self::ROCK),
            "mud" => Some(Self::MUD),
            _ => None,
        }
    }
}

/// The land before any water has run on it: its height at a place, in metres. Each country
/// is the same kind of valley among its own hills.
pub fn natural(x: f32, z: f32, country: &Country) -> f32 {
    let x = if country.turned { -x } else { x };
    let p = Vec3::new(x, 0.0, z) + country.shift;
    // The valley: its floor falls to the east, and its sides rise to ridges either side.
    let across = (z - valley_axis(x)) * 0.94;
    let valley = -30.0 * gauss(across, 250.0) - 5.0 * gauss(across, 60.0);
    let fall = -0.017 * x;
    let broad = fbm(p * 0.0019, 4, 901) * 34.0;
    let hills = fbm(p * 0.0058, 4, 902) * 12.0 * country.relief;
    // Spurs running down off the ridges, sharp along their crests.
    let spurs = (ridged(p * 0.0075, 3, 903) - 0.5) * 9.0 * country.relief;
    let knolls = fbm(p * 0.02, 3, 904) * 2.0 * country.relief;
    // Far off, ranges stand all round, so that the land has no edge to see.
    let far = x.hypot(z);
    let around = Vec3::new(x, 0.0, z) / far.max(1.0);
    let ranges = smoothstep(620.0, 1500.0, far).powi(2)
        * 150.0
        * (0.75 + 0.5 * fbm(around * 2.3 + Vec3::new(0.0, 7.0, 0.0), 3, 905));
    90.0 + valley + fall + broad + hills + spurs + knolls + ranges
}

/// The small shapes of the ground between the design's points: hummocks, and lumps.
fn detail(x: f32, z: f32) -> f32 {
    let p = Vec3::new(x, 0.0, z);
    fbm(p * 0.12, 3, 911) * 0.24 + fbm(p * 0.47, 2, 912) * 0.05
}

/// A waterhole in the creek.
#[derive(Clone, Copy, Debug)]
pub struct Pool {
    pub centre: Vec2,
    /// The way the creek runs through it: a unit vector.
    pub along: Vec2,
    /// Half its length along the creek, and half its width across.
    pub radii: Vec2,
    /// The water's height, and its bed's at the deepest.
    pub level: f32,
    pub bed: f32,
}

impl Pool {
    /// How far inside its basin a place is: 1 at the middle, 0 at the rim, below it outside.
    pub fn inside(&self, x: f32, z: f32) -> f32 {
        let d = Vec2::new(x, z) - self.centre;
        let (u, v) = (d.dot(self.along), d.dot(self.along.perp()));
        1.0 - ((u / self.radii.x).powi(2) + (v / self.radii.y).powi(2))
    }

    /// The rectangle its water is drawn over: opposite corners.
    pub fn rectangle(&self) -> (Vec2, Vec2) {
        let reach = Vec2::new(
            (self.along.x * self.radii.x).hypot(self.along.y * self.radii.y),
            (self.along.y * self.radii.x).hypot(self.along.x * self.radii.y),
        ) + Vec2::splat(1.5);
        (self.centre - reach, self.centre + reach)
    }
}

/// A point of the track's middle line.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrailPoint {
    pub at: Vec2,
    /// The track's height here, level across its tread.
    pub height: f32,
    /// The way the track runs: a unit vector.
    pub forward: Vec2,
    /// How far round the loop, in metres.
    pub along: f32,
    /// Its rise over its run, the way it runs.
    pub grade: f32,
    /// How deep its wheel ruts are, in metres.
    pub rut: f32,
    /// How much of it is mud, from 0 to 1, and how much rock.
    pub mud: f32,
    pub rock: f32,
    /// Whether it is crossing a waterhole, from 0 to 1: there the creek's bed is the track.
    pub ford: f32,
    /// How much rain has cut it, from 0 to 1: how deep the rill down it is.
    pub erosion: f32,
    /// How it tilts across: its rise for each metre to the left, with the hillside it is on.
    pub tilt: f32,
    /// How bare it is between its wheel ruts, from 0, grown over, to 1.
    pub wear: f32,
}

/// A hump across the track that turns water off it, and a bog hole in it.
#[derive(Clone, Copy, Debug)]
pub struct Hole {
    pub along: f32,
    pub length: f32,
    pub depth: f32,
}

/// Where a place is, relative to the track.
#[derive(Clone, Copy, Debug)]
pub struct Near {
    /// The point the nearest stretch starts at, and how far along the stretch.
    pub index: usize,
    pub t: f32,
    /// How far round the loop, and how far to the left of the middle line.
    pub along: f32,
    pub offset: f32,
}

/// The track: a loop, or, if it is `open`, a line from one place to another.
pub struct Trail {
    pub points: Vec<TrailPoint>,
    /// Whether it is not a loop: its last point does not lead back to its first.
    pub open: bool,
    /// How many of its points are the track itself, if not all of them: those after are a
    /// road off it to the edge of the map, a line of its own (a hub's way out).
    pub main: usize,
    /// The point of the track a road leaves it at, if one does. For a way either side of
    /// it the track's own banks run out as far as a road's.
    pub junction: Option<usize>,
    /// The first of its points that are a made road, a main road to drive fast, if any are:
    /// the road to a town, all of it, or a hub's road to the edge. Its banks run out
    /// further into the land than a track's, so it is looked for from further off.
    pub made: usize,
    pub length: f32,
    /// Drainage humps, by how far round the loop each is, and which way the hill falls there.
    pub bars: Vec<(f32, f32)>,
    pub holes: Vec<Hole>,
    /// Washouts, where a gully has cut across the track: how far round, and how deep.
    pub washes: Vec<(f32, f32)>,
    /// Holes dug by wheels, on one side or the other: how far round, and which side (1 is
    /// the left).
    pub moguls: Vec<(f32, f32)>,
    /// Which stretches pass near each square of ground, to find the nearest quickly.
    squares: FxHashMap<(i32, i32), Vec<u32>>,
}

/// The size of the squares the track's stretches are sorted into.
const SQUARE: f32 = 10.0;

impl Trail {
    fn index(&mut self) {
        self.squares.clear();
        for index in 0..self.points.len() {
            let a = self.points[index].at;
            let key = ((a.x / SQUARE).floor() as i32, (a.y / SQUARE).floor() as i32);
            let reach = if self.wide(index) { 2 } else { 1 };
            for dz in -reach..=reach {
                for dx in -reach..=reach {
                    self.squares
                        .entry((key.0 + dx, key.1 + dz))
                        .or_default()
                        .push(index as u32);
                }
            }
        }
    }

    /// Whether the stretch that starts at a point has a road's banks, which run out a long
    /// way into the land: a made road's, and the track's own for sixty metres either side
    /// of where a road leaves it.
    pub fn wide(&self, index: usize) -> bool {
        if index >= self.made {
            return true;
        }
        let main = self.main_points().len();
        self.junction.is_some_and(|junction| {
            let apart = index.abs_diff(junction);
            index < main && apart.min(main - apart) as f32 * STEP < 60.0
        })
    }

    /// The nearest of the track to a place, if it is within about ten metres.
    pub fn near(&self, x: f32, z: f32) -> Option<Near> {
        self.near_among(x, z, |_| true)
    }

    /// The bench at a place, of every stretch of the track and its road that is near it,
    /// each by how much of the place is its own: how high it is, and how much the ground is
    /// held to that, from 0 to 1. `batter` is how far from a stretch's middle its cutting has
    /// run out into the land, by the point it starts at.
    ///
    /// The nearest stretch alone will not do where there are two to be near: where a road
    /// leaves the track, or bends back beside itself, the ground between would be the one's
    /// up to a line and the other's beyond it, with a step along the line.
    fn bench(
        &self,
        x: f32,
        z: f32,
        tread: f32,
        batter: impl Fn(usize) -> f32,
    ) -> Option<(f32, f32)> {
        let key = ((x / SQUARE).floor() as i32, (z / SQUARE).floor() as i32);
        let stretches = self.squares.get(&key)?;
        let p = Vec2::new(x, z);
        let (mut level, mut weight, mut most) = (0.0_f32, 0.0_f32, 0.0_f32);
        for &index in stretches {
            let index = index as usize;
            let (a, b) = (&self.points[index], &self.points[self.after(index)]);
            let run = b.at - a.at;
            let t = ((p - a.at).dot(run) / run.length_squared().max(1.0e-6)).clamp(0.0, 1.0);
            let on = a.at + run * t;
            let away = p.distance(on);
            let ford = a.ford + (b.ford - a.ford) * t;
            let held = (1.0 - smoothstep(tread, batter(index), away)) * (1.0 - ford);
            if held <= 0.0 {
                continue;
            }
            // (Tilted with its hillside, across its tread.)
            let forward = (a.forward + (b.forward - a.forward) * t).normalize_or(a.forward);
            let aside = (p - on).dot(Vec2::new(forward.y, -forward.x));
            let high = a.height
                + (b.height - a.height) * t
                + aside.clamp(-tread, tread) * (a.tilt + (b.tilt - a.tilt) * t);
            // A place on one stretch's tread is that stretch's, and hardly another's.
            let share = held.powi(4);
            level += high * share;
            weight += share;
            most = most.max(held);
        }
        (weight > 0.0).then(|| (level / weight, most))
    }

    /// The nearest to a place of those of its stretches that are wanted.
    fn near_among(&self, x: f32, z: f32, wanted: impl Fn(usize) -> bool) -> Option<Near> {
        let key = ((x / SQUARE).floor() as i32, (z / SQUARE).floor() as i32);
        let stretches = self.squares.get(&key)?;
        let p = Vec2::new(x, z);
        let mut best: Option<(f32, Near)> = None;
        for &index in stretches {
            let index = index as usize;
            if !wanted(index) {
                continue;
            }
            let (a, b) = (&self.points[index], &self.points[self.after(index)]);
            let run = b.at - a.at;
            let along = (p - a.at).dot(run) / run.length_squared().max(1.0e-6);
            let t = along.clamp(0.0, 1.0);
            let on = a.at + run * t;
            let distance = p.distance_squared(on);
            if best.is_none_or(|(least, _)| distance < least) {
                let forward = (a.forward + (b.forward - a.forward) * t).normalize_or(a.forward);
                // To the left of the way it runs, seen from above with +x east and +z south.
                let left = Vec2::new(forward.y, -forward.x);
                best = Some((
                    distance,
                    Near {
                        index,
                        t,
                        along: a.along + run.length() * t,
                        // (Past the end of a stretch a place is as far off as it is, not as
                        // far to one side: the end of a road is round, and does not run on
                        // as a bench for as far as it is looked for.)
                        offset: {
                            let aside = (p - on).dot(left);
                            if (0.0..=1.0).contains(&along) {
                                aside
                            } else {
                                distance.sqrt().copysign(aside)
                            }
                        },
                    },
                ));
            }
        }
        best.map(|(_, near)| near)
    }

    /// A value of the track's points at a place along a stretch.
    fn mix(&self, near: &Near, value: impl Fn(&TrailPoint) -> f32) -> f32 {
        let (a, b) = (
            &self.points[near.index],
            &self.points[self.after(near.index)],
        );
        value(a) + (value(b) - value(a)) * near.t
    }

    /// The point after one: round to the first from the last, unless the track is open,
    /// where the last is the end of it.
    fn after(&self, index: usize) -> usize {
        let main = self.main_points().len();
        if index >= main {
            // The road off it: a line, to its end.
            (index + 1).min(self.points.len() - 1)
        } else if index + 1 < main {
            index + 1
        } else if self.open {
            index
        } else {
            0
        }
    }

    /// The points of the track itself, without any road off it.
    pub fn main_points(&self) -> &[TrailPoint] {
        match self.main {
            0 => &self.points,
            main => &self.points[..main],
        }
    }

    /// How far round the loop from one place along it to another, the short way: signed.
    /// Along a track that is not a loop there is only the one way.
    pub fn apart(&self, from: f32, to: f32) -> f32 {
        if self.open {
            return to - from;
        }
        // (A road off the track is numbered on from the track's end, and is nowhere near
        // anything that is on the track.)
        if from > self.length || to > self.length {
            return if from > self.length && to > self.length {
                to - from
            } else {
                1.0e9
            };
        }
        let d = (to - from).rem_euclid(self.length);
        if d > self.length * 0.5 {
            d - self.length
        } else {
            d
        }
    }
}

/// A way out of a map at its edge: a main road runs off it there, and whoever drives down it
/// is in the next map, on the road in.
#[derive(Clone, Copy, Debug)]
pub struct WayOut {
    /// Where on the road a truck is as good as gone, and how near to there.
    pub at: Vec2,
    pub radius: f32,
    /// Where a truck that comes in by it is put: a little way in, on the road, facing in.
    pub arrive: Vec2,
    pub yaw_degrees: f32,
}

/// What the ground is covered with at a place: shares of each, the rest leaf litter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cover {
    /// The track's clay.
    pub clay: f32,
    pub mud: f32,
    /// Dry grass, in the clearings.
    pub grass: f32,
    pub rock: f32,
    /// How deep a bog hole is here, as a share of its deepest, and whether it is under a
    /// waterhole's water.
    pub bog: f32,
    pub water: bool,
    /// How much of the track's tread this is.
    pub tread: f32,
    /// How much this is the bed of a rill or a washout: stones in wet silt.
    pub scour: f32,
    /// Something is built here: a hub's shed, on its slab. Nothing grows.
    pub built: bool,
}

/// The whole design.
pub struct Plan {
    /// Half the width of the square that is driven on.
    pub half: f32,
    /// Whether it is a hub: a road to drive between trailheads, not a track to get over.
    pub hub: bool,
    /// What kind of country it is.
    pub country: Country,
    /// Half the width of what is driven on: a track's, one truck wide, or a road's, two.
    pub tread: f32,
    /// The trailheads: rings on the road, each in a clearing.
    pub places: Vec<Vec2>,
    /// The land once water has cut it, with the waterholes and the camp.
    land: Grid,
    /// How damp the ground is, from 0 to 1: how much water gathers there.
    wet: Grid,
    pub pools: Vec<Pool>,
    pub trail: Trail,
    /// Where the track starts: the camp, a level clearing.
    pub camp: Vec2,
    pub camp_height: f32,
    /// The creek's line, from its mouth upstream.
    pub creek: Vec<Vec2>,
    /// A hub's shed, beside the road at the camp; or, where the hub is a town, the town,
    /// beside the middle of its street.
    pub shed: Option<super::shed::Shed>,
    pub town: Option<super::town::Town>,
    street: Option<Street>,
    /// A hub's gate: a trailhead that is not on its road, but beside its shed, where a
    /// track of its own leaves from. The last of `places`, if there is one.
    pub gate: Option<Vec2>,
    /// Whether a main road is to run to the edge of the map, and where it does: a hub's from
    /// beside its shed, a town's from the end of its street.
    edge: bool,
    pub way_out: Option<WayOut>,
}

/// How far round a level clearing the camp is.
const CAMP: f32 = 16.0;
/// A town's street: how long it is, before the buildings, and how deep the block is that
/// they stand on.
const STREET: f32 = 150.0;
const BLOCK: f32 = 46.0;

/// A town's street, along the front of its buildings: where the track to the town ends.
/// Where its middle is, and the way it runs, with the block on its left.
#[derive(Clone, Copy, Debug)]
struct Street {
    middle: Vec2,
    along: Vec2,
    /// How high it is: a street is level from end to end, and so is the yard beside it.
    height: f32,
}

impl Street {
    /// Whether a place is on the street itself, not the lane behind or the ends between.
    fn on(&self, at: Vec2) -> bool {
        let off = at - self.middle;
        off.dot(self.along).abs() < STREET * 0.5 + 6.0
            && off.dot(Vec2::new(self.along.y, -self.along.x)).abs() < 4.0
    }
}

/// How far past the edge of the map a road that leaves it is drawn, in metres.
const BEYOND_EDGE: f32 = 56.0;
/// Half the width of a hub's road: two trucks pass on it.
pub const ROAD: f32 = 3.4;
/// How far round a trailhead's ring is.
pub const RING: f32 = 7.0;

impl Plan {
    /// Designs a square of bush `size` metres across, of some country: a four-wheel-drive
    /// track through it, or, for a hub, a graded road round it with `trailheads` trailheads;
    /// and, if `gate`, one more beside its shed, off the road.
    pub fn new(size: f32, country: Country, trailheads: usize, gate: bool, edge: bool) -> Self {
        let half = size * 0.5;
        let hub = country.hub;
        let reach = half + MARGIN;
        println!("  the hills...");
        let mut land = Grid::new(reach, 1.0);
        let count = land.values.len();
        let heights: Vec<f32> = (0..count)
            .into_par_iter()
            .map(|index| {
                let (x, z) = land.place(index);
                natural(x, z, &country)
            })
            .collect();
        land.values = heights;

        println!("  running water over them...");
        // Water is routed over the land with the ground's own unevenness on it: on a slope
        // that was perfectly even it would run ruler-straight along the grid.
        let mut uneven = land.clone();
        uneven.values = (0..count)
            .into_par_iter()
            .map(|index| {
                let (x, z) = land.place(index);
                let p = Vec3::new(x, 0.0, z);
                land.values[index] + fbm(p * 0.045, 3, 921) * 1.1 + fbm(p * 0.14, 2, 922) * 0.3
            })
            .collect();
        let flow = Flow::new(&uneven);
        let cut = flow.cut(&land);
        // Towards the design's edge the land goes back to its own shape, which is all there is
        // beyond it.
        let stride = land.stride();
        for index in 0..count {
            let (x, z) = land.place(index);
            let edge = reach - x.abs().max(z.abs());
            land.values[index] -= cut[index] * smoothstep(0.0, 40.0, edge);
        }
        let mut wet = Grid::new(reach, 1.0);
        wet.values = cut
            .iter()
            .map(|&depth| (depth / 2.6 * country.damp + country.sodden).min(1.0))
            .collect();

        // The creek: from where most water leaves, upstream by the greater branch.
        let creek_cells = flow.main_stem(40_000.0);
        let creek: Vec<Vec2> = creek_cells
            .iter()
            .map(|&index| {
                let (x, z) = land.place(index);
                Vec2::new(x, z)
            })
            .collect();
        println!("    the creek runs {} m", creek.len());

        // Waterholes along it, well apart, where it is inside the square.
        let mut pools: Vec<Pool> = Vec::new();
        let inside = half - 85.0;
        let mut since = 1.0e9_f32;
        for k in 12..creek.len().saturating_sub(12) {
            since += creek[k].distance(creek[k - 1]);
            let here = creek[k];
            // As many as a scene's still water has room for: a creek in summer is a chain of
            // long pools with bars of stone and sand between.
            if here.x.abs() > inside || here.y.abs() > inside || since < 105.0 || pools.len() == 8 {
                continue;
            }
            // Upstream is further along the list: the creek runs the other way.
            let along = (creek[k - 10] - creek[k + 10]).normalize_or(Vec2::X);
            let floor = land.at(here.x, here.y);
            let long = 13.5 + 4.5 * fbm(Vec3::new(here.x * 0.05, 3.0, here.y * 0.05), 2, 931);
            pools.push(Pool {
                centre: here,
                along,
                radii: Vec2::new(long, long * 0.5),
                level: floor - 0.1,
                bed: floor - 0.8,
            });
            since = 0.0;
        }
        // From the west, upstream, to the east.
        pools.sort_by(|a, b| a.centre.x.total_cmp(&b.centre.x));
        for pool in &pools {
            let (low, high) = pool.rectangle();
            let (first, last) = (land.nearest(low.x, low.y), land.nearest(high.x, high.y));
            let (c0, r0) = (first % stride, first / stride);
            let (c1, r1) = (last % stride, last / stride);
            for row in r0.saturating_sub(24)..=(r1 + 24).min(land.cells) {
                for column in c0.saturating_sub(24)..=(c1 + 24).min(land.cells) {
                    let index = row * stride + column;
                    let (x, z) = land.place(index);
                    let inside = pool.inside(x, z);
                    let d = (Vec2::new(x, z) - pool.centre).abs();
                    let reach = (high - low) * 0.5;
                    // How far inside the rectangle the water is drawn over, in metres.
                    let within = (reach.x - d.x).min(reach.y - d.y);
                    let height = &mut land.values[index];
                    if inside > 0.0 {
                        // The basin: a bowl, steeper at its rim than in its middle.
                        let bowl = pool.level + 0.1 - (0.9 * inside.powf(0.8));
                        *height = height.min(bowl);
                    } else {
                        // Its banks: no lower than the water, wherever the water is drawn,
                        // and sloping back to the land at one in three, not standing as a cut.
                        let rim = pool.level + 0.14 + 0.25 * smoothstep(0.0, -0.6, inside);
                        let held = smoothstep(-3.0, 0.0, within);
                        *height = *height + (rim.max(*height) - *height) * held;
                        // Close by the water, that is; a way back the land is its own again.
                        let out = ((1.0 - inside).sqrt() - 1.0) * pool.radii.y;
                        let sloped = height.min(pool.level + 0.24 + out * 0.34);
                        *height = sloped + (*height - sloped) * smoothstep(3.0, 11.0, out);
                    }
                }
            }
        }
        println!("    {} waterholes", pools.len());

        let mut plan = Self {
            half,
            hub,
            country,
            tread: if hub { ROAD } else { TREAD },
            places: vec![Vec2::ZERO; trailheads],
            land,
            wet,
            pools,
            trail: Trail {
                points: Vec::new(),
                open: false,
                main: 0,
                junction: None,
                made: usize::MAX,
                length: 0.0,
                bars: Vec::new(),
                holes: Vec::new(),
                washes: Vec::new(),
                moguls: Vec::new(),
                squares: FxHashMap::default(),
            },
            camp: Vec2::ZERO,
            camp_height: 0.0,
            creek,
            shed: None,
            town: None,
            street: None,
            gate: None,
            edge,
            way_out: None,
        };
        println!("  finding the track...");
        plan.route();
        // A hub has a shed at its camp, beside the road where a truck starts on it.
        if hub {
            let start = plan
                .trail
                .main_points()
                .iter()
                .min_by(|a, b| {
                    a.at.distance(plan.camp)
                        .total_cmp(&b.at.distance(plan.camp))
                })
                .copied()
                .unwrap_or_default();
            if country.town {
                // The town is beside the middle of its street, at the street's own height.
                let (at, forward) = plan.street.map_or((start.at, start.forward), |street| {
                    (street.middle, street.along)
                });
                let height = plan
                    .trail
                    .near(at.x, at.y)
                    .map_or(plan.camp_height, |near| {
                        plan.trail.points[near.index].height
                    });
                plan.town = Some(super::town::Town::beside(at, forward, height));
            } else {
                let shed = super::shed::Shed::beside(start.at, start.forward, plan.camp_height);
                if gate {
                    plan.gate = Some(shed.gate());
                    plan.places.push(shed.gate());
                    println!("    a gate beside the shed at {}", shed.gate());
                }
                plan.shed = Some(shed);
            }
        }
        // A track has a gate too, the way back to the hub: a trailhead beside where a truck
        // starts on it, off the track, on whichever side is the more level and the drier.
        if gate && !hub {
            let start = plan
                .trail
                .points
                .iter()
                .min_by(|a, b| {
                    a.at.distance(plan.camp)
                        .total_cmp(&b.at.distance(plan.camp))
                })
                .copied()
                .unwrap_or_default();
            let left = Vec2::new(start.forward.y, -start.forward.x);
            let reach = plan.tread + RING + 2.5;
            let rough = |at: Vec2| plan.land.slope(at.x, at.y) + 0.5 * plan.wet(at.x, at.y);
            let (a, b) = (start.at + left * reach, start.at - left * reach);
            let at = if rough(a) <= rough(b) { a } else { b };
            println!("    a gate beside the start at {at}");
            plan.gate = Some(at);
            plan.places.push(at);
        }
        plan
    }

    /// How far from the track's middle its cutting has run out into the land.
    fn batter(&self) -> f32 {
        self.tread + (BATTER - TREAD)
    }

    /// The same, of the stretch of it that starts at a point: further for a main road, the
    /// road to a town or a hub's road to the edge of its map.
    fn batter_at(&self, index: usize) -> f32 {
        if self.trail.wide(index) {
            self.tread + ROAD_BATTER
        } else {
            self.batter()
        }
    }

    /// The land's height at a place before the track is worn into it.
    fn untracked(&self, x: f32, z: f32) -> f32 {
        if self.land.holds(x, z) {
            self.land.at(x, z)
        } else {
            natural(x, z, &self.country)
        }
    }

    pub fn wet(&self, x: f32, z: f32) -> f32 {
        if self.wet.holds(x, z) {
            self.wet.at(x, z)
        } else {
            0.0
        }
    }

    /// How steep the land is at a place, without its small shapes.
    pub fn slope(&self, x: f32, z: f32) -> f32 {
        let step = 1.5;
        let dx = (self.untracked(x + step, z) - self.untracked(x - step, z)) / (2.0 * step);
        let dz = (self.untracked(x, z + step) - self.untracked(x, z - step)) / (2.0 * step);
        dx.hypot(dz)
    }

    /// The waterhole a place is in, if any, and how far inside.
    fn pool_at(&self, x: f32, z: f32) -> Option<(&Pool, f32)> {
        self.pools
            .iter()
            .map(|pool| (pool, pool.inside(x, z)))
            .find(|(_, inside)| *inside > -0.35)
    }

    /// How much of open, grassy ground a place is, from 0, under trees, to 1.
    pub fn open(&self, x: f32, z: f32) -> f32 {
        let p = Vec3::new(x, 0.0, z);
        let glades = smoothstep(0.2, 0.42, fbm(p * 0.0075, 3, 941));
        let camp = 1.0 - smoothstep(CAMP * 0.8, CAMP * 1.7, Vec2::new(x, z).distance(self.camp));
        let trailheads = self
            .places
            .iter()
            .map(|place| 1.0 - smoothstep(RING * 1.3, RING * 2.6, Vec2::new(x, z).distance(*place)))
            .fold(0.0, f32::max);
        glades.max(camp).max(trailheads)
    }

    /// How much of bare rock a place off the track is.
    fn outcrop(&self, x: f32, z: f32) -> f32 {
        let p = Vec3::new(x, 0.0, z);
        let stone = fbm(p * 0.016, 3, 951) + 0.5 * fbm(p * 0.07, 2, 952);
        let dry = 1.0 - self.wet(x, z);
        let steep = smoothstep(0.42, 0.7, self.slope(x, z));
        let shows = self.country.stone;
        (smoothstep(0.6 - shows, 0.78 - shows, stone * dry) + steep).min(1.0)
    }

    /// The ground's height at a place, with the track worn into it, and level under a
    /// hub's shed.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let height = self.worn(x, z);
        // (And level in the gate beside it.)
        let gated = self.gate.map_or(0.0, |gate| {
            1.0 - smoothstep(RING + 1.5, RING + 8.0, gate.distance(Vec2::new(x, z)))
        });
        match (&self.shed, &self.town) {
            (Some(shed), _) => height + (shed.height - height) * shed.held(x, z).max(gated),
            (_, Some(town)) => height + (town.height - height) * town.held(x, z),
            // (A track's gate is level with its camp.)
            _ => height + (self.camp_height - height) * gated,
        }
    }

    /// The ground's height at a place, with the track worn into it.
    fn worn(&self, x: f32, z: f32) -> f32 {
        let land = self.untracked(x, z);
        let lumps = detail(x, z);
        let Some(near) = self.trail.near(x, z) else {
            return land + lumps * self.lumpiness(x, z);
        };
        let trail = &self.trail;
        let ford = trail.mix(&near, |p| p.ford);
        let d = near.offset;
        let s = near.along;
        // Where a gully has washed the track out, its channel is cut through the bench.
        let wash = self.wash(s);
        // The bench: cut across the hillside but still tilted with it, eased back to the land
        // over its cut and fill, and rising and falling along its length as nothing graded does.
        let mut held =
            (1.0 - smoothstep(self.tread, self.batter_at(near.index), d.abs())) * (1.0 - ford);
        // (A main road is made to be driven fast: it is smooth.)
        let smooth = if self.hub { 0.2 } else { 1.0 };
        let swell = (0.1 * fbm(Vec3::new(s * 0.1, 0.0, 9.0), 2, 966)
            + 0.05 * fbm(Vec3::new(s * 0.33, 0.0, 9.0), 1, 967))
            * smooth;
        let mut level = trail.mix(&near, |p| p.height)
            + d.clamp(-self.tread, self.tread) * trail.mix(&near, |p| p.tilt)
            + swell;
        // By a made road the ground is all the nearby road's, not the nearest stretch's: so
        // where a road leaves the track, or bends back beside itself, the one runs into the
        // other with no step between them, and neither is cut through the other's bank.
        if trail.made != usize::MAX {
            let batter = |index: usize| self.batter_at(index);
            match trail.bench(x, z, self.tread, batter) {
                Some((all, most)) => (level, held) = (all + swell, most),
                None => held = 0.0,
            }
        }
        let mut height = land + (level - land) * held + lumps * (1.0 - held) * self.lumpiness(x, z);
        let tread = 1.0 - smoothstep(self.tread - 0.35, self.tread + 0.55, d.abs());
        let reach = 1.0 - smoothstep(self.tread + 0.6, self.batter() * 0.85, d.abs());
        if reach <= 0.0 && held <= 0.0 {
            return height;
        }
        let p = Vec3::new(x, 0.0, z);
        let (rut, mud, rock, erosion) = (
            trail.mix(&near, |p| p.rut),
            trail.mix(&near, |p| p.mud),
            trail.mix(&near, |p| p.rock),
            trail.mix(&near, |p| p.erosion),
        );
        let dry = 1.0 - ford;
        // The wheel ruts wander a little within the tread, as each driver picks a line.
        let line = d - self.wander(s);
        let wheels = gauss(line - RUTS * 0.5, 0.2) + gauss(line + RUTS * 0.5, 0.2);
        let crown = gauss(line, 0.42);
        height += (-rut * wheels + 0.35 * rut * crown) * dry;
        // Spoil thrown to the edges, and the tread's own roughness: stones and hoof-sized dips.
        height += 0.06 * gauss(d.abs() - (self.tread + 0.2), 0.45) * dry;
        height += (fbm(p * 0.8, 2, 962) * 0.03 + fbm(p * 2.6, 2, 963) * 0.009) * tread * smooth;
        // Hollows that hold a puddle, on the flatter stretches.
        let flat = 1.0 - smoothstep(0.05, 0.14, trail.mix(&near, |p| p.grade.abs()));
        let hollow = (fbm(p * 0.33, 2, 964) - 0.16).max(0.0) * smooth;
        height -= hollow * 0.36 * tread * flat * dry * (0.4 + 0.6 * mud);

        // The rill: rain runs down a hill's track and cuts a gutter in it, which snakes from
        // one wheel rut to the other and back. Its sides crumble, and stones stand in its bed.
        if erosion > 0.02 {
            let (gutter, bed) = self.rill(s, d, erosion);
            let stones = (fbm(p * 1.1, 2, 968) - 0.05).max(0.0) * 0.16;
            height += (-gutter + stones * bed) * dry;
        }
        // A washout: the gully's own channel, straight through the track, with a rough bed.
        if wash.0 > 0.0 {
            let rubble = fbm(p * 0.7, 2, 969) * 0.07;
            height += (-wash.0 + rubble * wash.1) * reach * dry;
        }
        // Holes dug by wheels that spun, left and right by turns: with one wheel in a hole the
        // one across the axle from it hangs in the air.
        for &(at, side) in &trail.moguls {
            let along = trail.apart(at, s);
            if along.abs() < 3.5 {
                let hole = gauss(along, 0.95) * gauss(line - side * RUTS * 0.5, 0.5);
                let heap = gauss(along - 1.9, 0.8) * gauss(line - side * RUTS * 0.5, 0.55);
                height += (-0.32 * hole + 0.1 * heap) * dry;
            }
        }
        // Drainage humps across the hills: a mound, with a dip above it where water gathers
        // and is turned off the track.
        for &(at, uphill) in &trail.bars {
            let along = trail.apart(at, s);
            if along.abs() < 7.0 {
                height +=
                    (0.4 * gauss(along, 1.4) - 0.14 * gauss(along - uphill * 2.7, 1.2)) * reach;
            }
        }
        // Bog holes: a bowl the width of the tread, deepest under the wheels.
        height -= self.bog(s, line) * dry;
        // Ledges of rock: the track climbs them as steps, each a little askew.
        if rock > 0.01 {
            const LEDGE: f32 = 3.1;
            let skew = 0.6 * (d * 0.9 + (s * 0.21).sin() * 2.0).sin();
            let f = ((s + skew) / LEDGE).rem_euclid(1.0);
            let riser = smoothstep(0.8, 1.0, f) - f;
            let climbing = trail.mix(&near, |p| p.grade).signum();
            let stones = (fbm(p * 0.75, 2, 965) - 0.1).max(0.0) * 0.36;
            height += (0.36 * riser * climbing + stones) * rock * tread;
        }
        height
    }

    /// How far the wheel ruts are to the left of the track's middle, this far round.
    fn wander(&self, s: f32) -> f32 {
        0.28 * fbm(Vec3::new(s * 0.035, 0.0, 5.0), 2, 961)
    }

    /// The rill at a place along the track and across it, where rain has cut it this much:
    /// how deep its gutter is there, in metres, and how much of its bed this is, 0 to 1.
    fn rill(&self, s: f32, d: f32, erosion: f32) -> (f32, f32) {
        // It snakes across the tread, a turn every forty metres or so, never quite the same.
        let line = 1.05 * (s * 0.075 + 1.6 * (s * 0.019).sin()).sin();
        let width = 0.3 + 0.24 * erosion;
        let depth = 0.08 + 0.4 * erosion;
        let across = ((d - line).abs() / width).min(1.0);
        let bed = (1.0 - across).powf(1.25);
        // Its banks slump towards it.
        let slump = 0.07 * erosion * gauss(d - line, width * 2.4);
        (depth * bed + slump, smoothstep(0.25, 0.8, bed))
    }

    /// The washout at a place along the track, if it is in one: how deep the gully's channel
    /// is there, in metres, and how much of a washout it is, 0 to 1.
    fn wash(&self, s: f32) -> (f32, f32) {
        let mut found = (0.0_f32, 0.0_f32);
        for &(at, depth) in &self.trail.washes {
            let along = self.trail.apart(at, s).abs();
            if along < 3.6 {
                // A channel with a flat, narrow bed and banks that have slumped in.
                let bank = 1.0 - smoothstep(0.3, 2.7, along);
                found = (
                    found.0.max(depth * bank),
                    found.1.max(1.0 - smoothstep(1.4, 3.6, along)),
                );
            }
        }
        found
    }

    /// How deep a bog hole is at a place along the track and across it, in metres.
    fn bog(&self, s: f32, line: f32) -> f32 {
        let mut deepest = 0.0_f32;
        for hole in &self.trail.holes {
            let along = self.trail.apart(hole.along, s) / (hole.length * 0.5);
            if along.abs() >= 1.0 {
                continue;
            }
            let across = (line / (self.tread + 0.3)).abs();
            if across >= 1.0 {
                continue;
            }
            let bowl = (1.0 - along * along).powi(2) * (1.0 - across.powi(3));
            let wheels = gauss(line - RUTS * 0.5, 0.3) + gauss(line + RUTS * 0.5, 0.3);
            deepest = deepest.max(hole.depth * bowl * (0.72 + 0.28 * wheels));
        }
        deepest
    }

    /// How much of the forest floor's hummocks a place has: none in water or on stone.
    fn lumpiness(&self, x: f32, z: f32) -> f32 {
        match self.pool_at(x, z) {
            Some((_, inside)) => smoothstep(0.0, -0.35, inside),
            None => 1.0,
        }
    }

    /// What covers the ground at a place.
    pub fn cover(&self, x: f32, z: f32) -> Cover {
        let mut cover = Cover::default();
        let p = Vec3::new(x, 0.0, z);
        let wet = self.wet(x, z);
        if let Some((pool, inside)) = self.pool_at(x, z) {
            // A waterhole's bed and its banks are mud and silt.
            cover.mud = smoothstep(-0.35, -0.05, inside);
            cover.water = self.untracked(x, z) < pool.level - 0.02 && inside > 0.0;
        }
        // The creek's own bed, and the wettest of the gullies: damp earth.
        let damp = smoothstep(0.62, 0.9, wet + 0.1 * fbm(p * 0.2, 2, 971));
        cover.mud = cover.mud.max(damp * 0.85);
        cover.rock = self.outcrop(x, z) * (1.0 - cover.mud);
        cover.grass = self.open(x, z) * (1.0 - cover.rock) * (1.0 - cover.mud);
        // Round a shed the ground is worn bare by coming and going, and on its slab nothing
        // grows at all.
        if let Some(shed) = &self.shed {
            cover.built = shed.covers(x, z, 1.2);
            cover.grass *= 1.0 - shed.held(x, z);
        }
        // A town's yard is bare clay, worn by a hundred years of coming and going, and nothing
        // grows on it or close round it.
        if let Some(town) = &self.town {
            let held = town.held(x, z);
            // Its street too, and a few paces beyond: nothing lies fallen across a town.
            cover.built = town.covers(x, z, 9.0);
            cover.grass *= 1.0 - held;
            cover.rock *= 1.0 - held;
            cover.mud *= 1.0 - held;
            cover.clay = held * (1.0 - cover.rock - cover.mud).max(0.0);
        }
        let Some(near) = self.trail.near(x, z) else {
            return cover;
        };
        let trail = &self.trail;
        let edge = 0.3 * fbm(p * 0.6, 2, 972);
        let tread = 1.0 - smoothstep(self.tread - 0.5, self.tread + 0.5, near.offset.abs() + edge);
        if tread <= 0.0 {
            return cover;
        }
        let ford = trail.mix(&near, |p| p.ford);
        let line = near.offset - self.wander(near.along);
        let bog = self.bog(near.along, line);
        let mud = (trail.mix(&near, |p| p.mud) + smoothstep(0.03, 0.2, bog)).min(1.0);
        let erosion = trail.mix(&near, |p| p.erosion);
        // Scoured beds, the rill's and a washout's, are stones in silt.
        let wash = self.wash(near.along);
        let scour = if erosion > 0.02 {
            self.rill(near.along, near.offset, erosion).1
        } else {
            0.0
        }
        .max(smoothstep(0.1, 0.35, wash.0));
        let rock = (trail.mix(&near, |p| p.rock) + scour * 0.55).min(1.0);
        // Bare where the wheels run; between them, and at the edges, as bare as the track is
        // worn. A track seldom driven is two lines in the litter.
        let wheels = (gauss(line - RUTS * 0.5, 0.46) + gauss(line + RUTS * 0.5, 0.46)).min(1.0);
        let wear = trail.mix(&near, |p| p.wear);
        let ragged = 0.5 + 0.5 * fbm(p * 0.9, 2, 973);
        let bare = (wheels.max(wear * (0.55 + 0.45 * ragged)) + mud + scour).min(1.0);
        let worn = tread * (1.0 - ford) * bare;
        cover.tread = tread;
        cover.bog = bog;
        cover.scour = scour * tread;
        // On the tread: clay, with mud where it is wet and stone where it crosses rock.
        cover.grass *= 1.0 - worn;
        cover.rock = cover.rock * (1.0 - worn) + worn * rock;
        cover.mud = (cover.mud * (1.0 - worn) + worn * mud * (1.0 - rock)).min(1.0 - cover.rock);
        cover.clay = (worn * (1.0 - mud) * (1.0 - rock)).min(1.0 - cover.rock - cover.mud);
        cover
    }

    /// Finds the track, and what it is like along its length.
    fn route(&mut self) {
        let half = self.half;
        let tread = self.tread;
        let climbs = self.country.climbs;
        let crossing = self.country.crosses;
        let keep = half - KEEP_IN;
        let land = &self.land;
        // Which side of the creek each place is on: flooded out from beside the first ford,
        // with the creek as a wall.
        let coarse = 2.0;
        let mut side = Grid::new(half, coarse);
        let stride = side.stride();
        let mut wall = vec![false; side.values.len()];
        for at in &self.creek {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let (x, z) = (at.x + dx as f32 * coarse, at.y + dz as f32 * coarse);
                    if side.holds(x, z) {
                        wall[side.nearest(x, z)] = true;
                    }
                }
            }
        }
        // The creek rises somewhere inside the square: carry its line on from its head to the
        // edge, the way it was heading, so that it parts the square in two.
        if self.creek.len() > 40 {
            let head = self.creek[self.creek.len() - 1];
            let heading = (head - self.creek[self.creek.len() - 40]).normalize_or(Vec2::NEG_X);
            let mut at = head;
            while side.holds(at.x, at.y) {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let (x, z) = (at.x + dx as f32 * coarse, at.y + dz as f32 * coarse);
                        if side.holds(x, z) {
                            wall[side.nearest(x, z)] = true;
                        }
                    }
                }
                at += heading * coarse * 0.5;
            }
        }
        let fords = if self.pools.len() >= 2 {
            vec![self.pools[0], self.pools[self.pools.len() - 1]]
        } else {
            self.pools.clone()
        };
        let across = |pool: &Pool| pool.along.perp();
        let flood = |from: Vec2, mark: f32, side: &mut Grid| {
            let start = side.nearest(from.x, from.y);
            if wall[start] || side.values[start] != 0.0 {
                return;
            }
            let mut queue = vec![start];
            side.values[start] = mark;
            while let Some(index) = queue.pop() {
                let (column, row) = (index % stride, index / stride);
                let mut visit = |c: usize, r: usize| {
                    let next = r * stride + c;
                    if !wall[next] && side.values[next] == 0.0 {
                        side.values[next] = mark;
                        queue.push(next);
                    }
                };
                if column > 0 {
                    visit(column - 1, row);
                }
                if column < stride - 1 {
                    visit(column + 1, row);
                }
                if row > 0 {
                    visit(column, row - 1);
                }
                if row < stride - 1 {
                    visit(column, row + 1);
                }
            }
        };
        let Some(first) = fords.first().copied() else {
            return;
        };
        let last = *fords.last().expect("one at least");
        let out = first.radii.y + 17.0;
        flood(first.centre + across(&first) * out, 1.0, &mut side);
        flood(first.centre - across(&first) * out, 2.0, &mut side);
        let side_of = |p: Vec2| side.values[side.nearest(p.x, p.y)];

        // The places worth driving to. On each side: the best of some measure, well inside
        // the square and well away from what has been chosen already.
        let mut chosen: Vec<Vec2> = vec![first.centre, last.centre];
        let mut best = |which: f32, apart: f32, score: &dyn Fn(f32, f32) -> f32| -> Vec2 {
            let mut found = (f32::MIN, Vec2::ZERO);
            for index in (0..side.values.len()).step_by(3) {
                if side.values[index] != which {
                    continue;
                }
                let (x, z) = side.place(index);
                if x.abs() > keep - 25.0 || z.abs() > keep - 25.0 {
                    continue;
                }
                let p = Vec2::new(x, z);
                if chosen.iter().any(|c| c.distance(p) < apart) {
                    continue;
                }
                let value = score(x, z);
                if value > found.0 {
                    found = (value, p);
                }
            }
            chosen.push(found.1);
            found.1
        };
        let wet = &self.wet;
        let high = |x: f32, z: f32| land.at(x, z);
        let flat_and_wet = |x: f32, z: f32| {
            let level = 1.0 - smoothstep(0.03, 0.14, land.slope(x, z));
            wet.at(x, z).min(0.7) * level + level * 0.2
        };
        // The camp: level ground near the first ford.
        let near_first = first.centre;
        let camp_score = |x: f32, z: f32| {
            let away = Vec2::new(x, z).distance(near_first);
            if !(55.0..150.0).contains(&away) {
                return f32::MIN;
            }
            -land.slope(x, z) * 100.0 - wet.at(x, z) * 30.0 - away * 0.02
        };
        // A town is not driven round a valley. It has one road, a street along the front of
        // its buildings and a lane back round behind them, on the most level, driest ground
        // there is, clear of the creek; the street runs across the slope, so that it is
        // level, with the buildings on its uphill side.
        let street = self.country.town.then(|| {
            let reach = STREET * 0.5 + BLOCK;
            let level = |x: f32, z: f32| {
                let mut slope = land.slope(x, z) * 2.0;
                for (dx, dz) in [(-45.0, 0.0), (45.0, 0.0), (0.0, -45.0), (0.0, 45.0)] {
                    slope += land.slope(x + dx, z + dz);
                }
                slope / 6.0
            };
            let mut found = (f32::MIN, Vec2::ZERO);
            for index in (0..side.values.len()).step_by(3) {
                let (x, z) = side.place(index);
                if x.abs().max(z.abs()) > keep - reach {
                    continue;
                }
                let p = Vec2::new(x, z);
                let water = self
                    .pools
                    .iter()
                    .map(|pool| pool.centre.distance(p) - pool.radii.x)
                    .fold(f32::MAX, f32::min);
                let value = -level(x, z) * 100.0
                    - wet.at(x, z) * 40.0
                    - smoothstep(reach + 30.0, reach, water) * 60.0;
                if value > found.0 {
                    found = (value, p);
                }
            }
            let block = found.1;
            let rise = Vec2::new(
                land.at(block.x + 10.0, block.y) - land.at(block.x - 10.0, block.y),
                land.at(block.x, block.y + 10.0) - land.at(block.x, block.y - 10.0),
            );
            let mut along = Vec2::new(-rise.y, rise.x)
                .try_normalize()
                .unwrap_or(Vec2::X);
            // The block is on the street's left: uphill of it.
            if Vec2::new(along.y, -along.x).dot(rise) < 0.0 {
                along = -along;
            }
            let middle = block - Vec2::new(along.y, -along.x) * BLOCK * 0.5;
            // As high as the land under it is, taken along its whole length.
            let height = (0..=16)
                .map(|k| {
                    let at = middle + along * STREET * (k as f32 / 16.0 - 0.5);
                    land.at(at.x, at.y)
                })
                .sum::<f32>()
                / 17.0;
            Street {
                middle,
                along,
                height,
            }
        });
        // A town is come to by a main road from the edge of its map: from whichever place on
        // the edge is on the town's side of the creek, so that the road crosses no water, a
        // fair run from the town, on ground that is level and dry.
        let gate = street.map(|street| {
            let town_side = side_of(street.middle);
            let ends = [
                street.middle - street.along * STREET * 0.5,
                street.middle + street.along * STREET * 0.5,
            ];
            let rim = half - 4.0;
            let mut found = (f32::MIN, Vec2::new(-rim, street.middle.y.clamp(-rim, rim)));
            let mut along = -rim + 30.0;
            while along <= rim - 30.0 {
                for (p, inward) in [
                    (Vec2::new(-rim, along), Vec2::X),
                    (Vec2::new(rim, along), Vec2::NEG_X),
                    (Vec2::new(along, -rim), Vec2::Y),
                    (Vec2::new(along, rim), Vec2::NEG_Y),
                ] {
                    let inside = p + inward * 40.0;
                    let run = p.distance(ends[0]).min(p.distance(ends[1]));
                    let water = self
                        .pools
                        .iter()
                        .map(|pool| pool.centre.distance(inside) - pool.radii.x)
                        .fold(f32::MAX, f32::min);
                    if side_of(inside) != town_side
                        || town_side == 0.0
                        || run < 150.0
                        || water < 45.0
                    {
                        continue;
                    }
                    let value = -(run - 260.0).abs() * 0.3
                        - (land.slope(inside.x, inside.y) + land.slope(p.x, p.y)) * 120.0
                        - wet.at(inside.x, inside.y) * 60.0;
                    if value > found.0 {
                        found = (value, p);
                    }
                }
                along += 6.0;
            }
            found.1
        });
        let camp = match gate {
            Some(gate) => gate,
            None => best(2.0, 30.0, &camp_score),
        };
        let summit_a = best(1.0, 170.0, &high);
        let lookout_a = best(1.0, 230.0, &high);
        let bog_flat = best(2.0, 170.0, &flat_and_wet);
        let summit_b = best(2.0, 200.0, &high);
        self.camp = camp;
        self.street = street;
        let sides = |which: f32| side.values.iter().filter(|&&v| v == which).count();
        println!(
            "    sides {} and {} of {}; fords {} and {}",
            sides(1.0),
            sides(2.0),
            side.values.len(),
            first.centre,
            last.centre
        );
        println!(
            "    camp {camp}, hills {summit_a} and {lookout_a}, wet flat {bog_flat}, hill {summit_b}"
        );

        // Level the camp.
        let pad = land.at(camp.x, camp.y);
        self.camp_height = pad;
        let stride_land = self.land.stride();
        for row in 0..=self.land.cells {
            for column in 0..=self.land.cells {
                let index = row * stride_land + column;
                let (x, z) = self.land.place(index);
                let away = Vec2::new(x, z).distance(camp);
                if away < CAMP * 2.2 {
                    let held = 1.0 - smoothstep(CAMP, CAMP * 2.2, away);
                    let height = &mut self.land.values[index];
                    *height += (pad - *height) * held;
                }
            }
        }
        let land = &self.land;

        // The first ford is the nearer to the camp's side of the two that bound each leg.
        let ford_ends = |pool: &Pool, to_side: f32| -> Vec2 {
            let step = across(pool) * (pool.radii.y + 17.0);
            if side_of(pool.centre + step) == to_side {
                pool.centre + step
            } else {
                pool.centre - step
            }
        };
        // Round the loop: the camp, over the first ford, the far side's two hills, back over
        // the second ford, the wet flat, this side's hill, and home.
        let (a, b) = (1.0, 2.0);
        let legs: Vec<(Vec2, Vec2, bool)> = vec![
            (camp, ford_ends(&first, b), false),
            (ford_ends(&first, b), ford_ends(&first, a), true),
            (ford_ends(&first, a), summit_a, false),
            (summit_a, lookout_a, false),
            (lookout_a, ford_ends(&last, a), false),
            (ford_ends(&last, a), ford_ends(&last, b), true),
            (ford_ends(&last, b), bog_flat, false),
            (bog_flat, summit_b, false),
            (summit_b, camp, false),
        ];

        // Each leg by the easiest way: a search over the land, where a step costs its length,
        // and more the steeper it climbs, the steeper the hillside it crosses, and the wetter
        // the ground. Where a leg has gone, the next keeps off, so the loop does not double
        // back along itself.
        // A town's track is not a loop. It runs from the gate to the nearer end of the
        // street, by the ford if the creek is between them, and then down the street.
        let open = street.is_some();
        let ends = street.map(|street| {
            let (a, b) = (
                street.middle - street.along * STREET * 0.5,
                street.middle + street.along * STREET * 0.5,
            );
            if camp.distance(a) <= camp.distance(b) {
                (a, b)
            } else {
                (b, a)
            }
        });
        // Where the town stands, and its street beyond its near end: no track is found there.
        let built = |p: Vec2| {
            let (Some(street), Some((begin, end))) = (&street, ends) else {
                return false;
            };
            let gone = (p - begin).dot((end - begin).normalize_or(Vec2::X));
            let aside = (p - street.middle).dot(Vec2::new(street.along.y, -street.along.x));
            // (Across the street from the block is the used-truck yard.)
            gone > 6.0 && gone < STREET + 25.0 && aside > -36.0 && aside < BLOCK + 30.0
        };
        let legs = match ends {
            Some((begin, _)) => {
                let (here, there) = (side_of(camp), side_of(begin));
                if here != there && here != 0.0 && there != 0.0 {
                    // Over whichever ford is the less out of the way.
                    let by = |pool: &Pool| camp.distance(pool.centre) + pool.centre.distance(begin);
                    let ford = if by(&first) <= by(&last) { first } else { last };
                    vec![
                        (camp, ford_ends(&ford, here), false),
                        (ford_ends(&ford, here), ford_ends(&ford, there), true),
                        (ford_ends(&ford, there), begin, false),
                    ]
                } else {
                    vec![(camp, begin, false)]
                }
            }
            None => legs,
        };
        let mut used = vec![0.0_f32; side.values.len()];
        let pools = &self.pools;
        let mut line: Vec<Vec2> = Vec::new();
        // The fords' own runs, down one bank and up the other, are track before any leg is
        // found, so that none is found across them.
        if !crossing {
            for (from, to, _) in legs.iter().filter(|leg| leg.2) {
                let steps = (from.distance(*to) / coarse).ceil() as usize;
                for k in 0..=steps {
                    let at = from.lerp(*to, k as f32 / steps as f32);
                    for dz in -8..=8_i32 {
                        for dx in -8..=8_i32 {
                            let (ux, uz) = (at.x + dx as f32 * coarse, at.y + dz as f32 * coarse);
                            if side.holds(ux, uz) {
                                let away = (dx as f32).hypot(dz as f32) * coarse;
                                let mark = &mut used[side.nearest(ux, uz)];
                                *mark = mark.max(7.0 * (1.0 - smoothstep(6.0, 16.0, away)));
                            }
                        }
                    }
                }
            }
        }
        for (from, to, straight) in &legs {
            if *straight {
                // A ford: straight across the waterhole.
                let steps = (from.distance(*to) / coarse).ceil() as usize;
                for k in 0..steps {
                    line.push(from.lerp(*to, k as f32 / steps as f32));
                }
                continue;
            }
            let cost = |index: usize, next: usize, length: f32| -> f32 {
                let (x0, z0) = side.place(index);
                let (x1, z1) = side.place(next);
                // (A road that comes in from the edge of the map starts there.)
                let bound = if open { half - 2.0 } else { keep };
                if x1.abs() > bound || z1.abs() > bound {
                    return f32::INFINITY;
                }
                let p = Vec2::new(x1, z1);
                if built(p) {
                    return f32::INFINITY;
                }
                let near_end = p.distance(*from).min(p.distance(*to));
                // Not through a waterhole, nor along the creek's bed, but to reach a ford.
                if near_end > 20.0 {
                    if wall[next] || pools.iter().any(|pool| pool.inside(x1, z1) > -1.2) {
                        return f32::INFINITY;
                    }
                }
                let grade = (land.at(x1, z1) - land.at(x0, z0)).abs() / length;
                let climb = ((grade - 0.05 * climbs).max(0.0) / (0.11 * climbs)).powi(2) * 1.6;
                let hillside = (land.slope(x1, z1) / 0.4).powi(2);
                let soggy = if near_end > 25.0 {
                    smoothstep(0.45, 0.8, wet.at(x1, z1)) * 5.0
                } else {
                    0.0
                };
                let edge = if open {
                    0.0
                } else {
                    smoothstep(keep - 40.0, keep, x1.abs().max(z1.abs())) * 6.0
                };
                let crowded = if near_end > 30.0 { used[next] } else { 0.0 };
                // Where an earlier leg itself runs, not at all: a track that crossed itself
                // would meet itself at two heights, and one would be a pit in the other.
                if crowded > 6.9 && !crossing {
                    return f32::INFINITY;
                }
                length * (1.0 + climb + hillside + soggy + edge + crowded)
            };
            let path = search(&side, *from, *to, &cost);
            // Mark what it used, and a way either side.
            for &index in &path {
                let (x, z) = side.place(index);
                for dz in -8..=8_i32 {
                    for dx in -8..=8_i32 {
                        let (ux, uz) = (x + dx as f32 * coarse, z + dz as f32 * coarse);
                        if side.holds(ux, uz) {
                            let away = (dx as f32).hypot(dz as f32) * coarse;
                            let mark = &mut used[side.nearest(ux, uz)];
                            *mark = mark.max(7.0 * (1.0 - smoothstep(6.0, 16.0, away)));
                        }
                    }
                }
            }
            line.extend(path.iter().map(|&index| {
                let (x, z) = side.place(index);
                Vec2::new(x, z)
            }));
        }

        // A town's road comes from beyond the edge of the map: it is drawn a way out past
        // where it is driven from, straight, so that it is seen to come from somewhere.
        if open {
            let outward = if camp.x.abs() > camp.y.abs() {
                Vec2::new(camp.x.signum(), 0.0)
            } else {
                Vec2::new(0.0, camp.y.signum())
            };
            let beyond = (BEYOND_EDGE / coarse) as usize;
            let mut whole: Vec<Vec2> = (0..beyond)
                .map(|k| camp + outward * (BEYOND_EDGE - k as f32 * coarse))
                .collect();
            whole.append(&mut line);
            line = whole;
        }
        // Then a town's street, from its near end to its far one, where the track ends.
        if let Some((begin, end)) = ends {
            let steps = (begin.distance(end) / coarse).ceil() as usize;
            for k in 0..=steps {
                line.push(begin.lerp(end, k as f32 / steps as f32));
            }
        }

        // The line found steps from point to point of a grid: take every few of its points
        // and draw a smooth curve through them, round the loop.
        let mut knots: Vec<Vec2> = line.iter().step_by(5).copied().collect();
        if let (true, Some(last)) = (open, line.last()) {
            knots.push(*last);
        }
        let count = knots.len();
        let curve = |k: usize, t: f32| -> Vec2 {
            let p = |i: isize| {
                let at = k as isize + i;
                if open {
                    knots[at.clamp(0, count as isize - 1) as usize]
                } else {
                    knots[at.rem_euclid(count as isize) as usize]
                }
            };
            let (p0, p1, p2, p3) = (p(-1), p(0), p(1), p(2));
            0.5 * ((2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
        };
        let mut fine: Vec<Vec2> = Vec::new();
        for k in 0..count {
            for step in 0..8 {
                fine.push(curve(k, step as f32 / 8.0));
            }
        }
        // And walk it at an even pace.
        let mut points: Vec<Vec2> = vec![fine[0]];
        let mut left = STEP;
        for k in 0..fine.len() {
            // (A track that is not a loop ends at its last point.)
            if open && k + 1 == fine.len() {
                break;
            }
            let (a, b) = (fine[k], fine[(k + 1) % fine.len()]);
            let run = a.distance(b);
            let mut gone = 0.0;
            while run - gone >= left {
                gone += left;
                points.push(a.lerp(b, gone / run));
                left = STEP;
            }
            left -= run - gone;
        }
        // The last point would lie on the first.
        if !open && points.len() > 2 && points[points.len() - 1].distance(points[0]) < STEP * 0.6 {
            points.pop();
        }
        let n = points.len();
        // The point so many on from one: round the loop, or, along a track that is not one,
        // no further than its ends.
        let wrap = |i: isize| {
            if open {
                i.clamp(0, n as isize - 1) as usize
            } else {
                i.rem_euclid(n as isize) as usize
            }
        };
        // A hub's road, and the road to a town, is graded: a track is rough.
        let rough = !self.hub;
        let street_at = |p: Vec2| street.is_some_and(|street| street.on(p));
        // No bend tighter than a truck can turn. Each point is drawn towards the middle of
        // those a few metres either side of it, over and over, which opens a hairpin out to a
        // loop; in a waterhole, and a car's length either side, the line stays as it is.
        let pinned: Vec<bool> = (0..n)
            .map(|i| {
                (-2..=2_isize).any(|k| {
                    let at = points[wrap(i as isize + k)];
                    pools.iter().any(|pool| pool.inside(at.x, at.y) > 0.0)
                })
            })
            .collect();
        // (A main road's bends are opened out a good deal further: it is driven fast.)
        let fast = self.country.town;
        for _ in 0..if fast { 220 } else { 40 } {
            let before = points.clone();
            for i in 0..n {
                if pinned[i] {
                    continue;
                }
                let mut middle = Vec2::ZERO;
                for k in -3..=3_isize {
                    middle += before[wrap(i as isize + k)];
                }
                points[i] = before[i].lerp(middle / 7.0, 0.6);
            }
        }

        // Its height: the land's along it, smoothed, so that it climbs evenly and does not
        // follow every hummock.
        let raw: Vec<f32> = points
            .iter()
            .map(|p| match &self.street {
                Some(street) if street.on(*p) => street.height,
                _ => land.at(p.x, p.y),
            })
            .collect();
        let mut height = raw.clone();
        // A road is graded: it keeps an even climb over more of the land's ups and downs.
        for _ in 0..if rough {
            2
        } else if fast {
            40
        } else {
            9
        } {
            let before = height.clone();
            for i in 0..n {
                let mut sum = 0.0;
                for k in -4..=4_isize {
                    sum += before[wrap(i as isize + k)];
                }
                height[i] = sum / 9.0;
            }
        }
        // Fords: in a waterhole the creek's bed is the track, and the track runs down to it
        // and up out of it on a ramp cut through the bank, no steeper than a wet tyre climbs.
        // The ramp is cut where the bank stands above it and filled where the land falls
        // away below, as below the bar at a waterhole's foot. Its grade is held gentlest by
        // the water and let go little by little over sixty metres, so that it runs out into
        // the track's own grade with no step where it ends.
        const RAMP: f32 = 0.14;
        let in_pool: Vec<bool> = points
            .iter()
            .map(|p| pools.iter().any(|pool| pool.inside(p.x, p.y) > -0.04))
            .collect();
        let from_pool: Vec<f32> = (0..n)
            .map(|i| {
                (-40..=40_isize)
                    .filter(|&k| in_pool[wrap(i as isize + k)])
                    .map(|k| k.unsigned_abs() as f32 * STEP)
                    .fold(f32::INFINITY, f32::min)
            })
            .collect();
        let grade_at = |i: usize| RAMP + (0.5 - RAMP) * smoothstep(24.0, 60.0, from_pool[i]);
        for i in 0..n {
            if in_pool[i] {
                height[i] = raw[i];
            }
        }
        for _ in 0..3 {
            for i in 0..n {
                if from_pool[i].is_finite() && !in_pool[i] {
                    let (before, step) = (height[wrap(i as isize - 1)], grade_at(i) * STEP);
                    height[i] = height[i].clamp(before - step, before + step);
                }
            }
            for i in (0..n).rev() {
                if from_pool[i].is_finite() && !in_pool[i] {
                    let (after, step) = (height[wrap(i as isize + 1)], grade_at(i) * STEP);
                    height[i] = height[i].clamp(after - step, after + step);
                }
            }
        }
        // Beside a waterhole the track is never cut below the water: wherever the water is
        // drawn, and a car's width beyond, it stands a hand above it, or the water would be
        // seen hanging over the cutting.
        for i in 0..n {
            if in_pool[i] {
                continue;
            }
            let at = points[i];
            for pool in pools.iter() {
                let (low, high) = pool.rectangle();
                let margin = tread + 4.0;
                if at.x > low.x - margin
                    && at.x < high.x + margin
                    && at.y > low.y - margin
                    && at.y < high.y + margin
                {
                    height[i] = height[i].max(pool.level + 0.16);
                }
            }
        }
        // How the hillside it is on tilts it: a share of the land's own fall across it,
        // evened out along its length.
        let mut tilt: Vec<f32> = points
            .iter()
            .enumerate()
            .map(|(i, at)| {
                let forward = (points[wrap(i as isize + 1)] - points[wrap(i as isize - 1)])
                    .normalize_or(Vec2::X);
                let left = Vec2::new(forward.y, -forward.x) * (tread + 1.0);
                let fall = (land.at(at.x + left.x, at.y + left.y)
                    - land.at(at.x - left.x, at.y - left.y))
                    / (2.0 * (tread + 1.0));
                (fall * 0.42).clamp(-0.13, 0.13)
            })
            .collect();
        for _ in 0..2 {
            let before = tilt.clone();
            for i in 0..n {
                let mut sum = 0.0;
                for k in -4..=4_isize {
                    sum += before[wrap(i as isize + k)];
                }
                tilt[i] = sum / 9.0;
            }
        }
        let mut trail = Trail {
            points: Vec::with_capacity(n),
            open,
            main: 0,
            junction: None,
            // (A hub's road and the road to a town are made roads from end to end.)
            made: if self.hub { 0 } else { usize::MAX },
            length: 0.0,
            bars: Vec::new(),
            holes: Vec::new(),
            washes: Vec::new(),
            moguls: Vec::new(),
            squares: FxHashMap::default(),
        };
        let mut along = 0.0;
        for i in 0..n {
            let (before, after) = (points[wrap(i as isize - 1)], points[wrap(i as isize + 1)]);
            let forward = (after - before).normalize_or(Vec2::X);
            let run = before.distance(after).max(1.0e-3);
            let grade = (height[wrap(i as isize + 1)] - height[wrap(i as isize - 1)]) / run;
            let at = points[i];
            let ford = pools
                .iter()
                // The track is its own right down to the water's edge, where its ramp ends.
                .map(|pool| smoothstep(-0.14, 0.06, pool.inside(at.x, at.y)))
                .fold(0.0, f32::max);
            trail.points.push(TrailPoint {
                at,
                height: height[i],
                forward,
                along,
                grade,
                ford,
                // Level at the camp, and by a waterhole, where a tilt would dip one edge
                // of it under the water.
                tilt: tilt[i]
                    * (1.0 - smoothstep(CAMP * 2.0, CAMP, at.distance(camp)))
                    * smoothstep(12.0, 30.0, from_pool[i]),
                ..TrailPoint::default()
            });
            along += at.distance(points[wrap(i as isize + 1)]);
        }
        trail.length = along;

        // What it is like along its length.
        for i in 0..n {
            let point = trail.points[i];
            let p = Vec3::new(point.at.x, 0.0, point.at.y);
            let damp = wet.at(point.at.x, point.at.y);
            let steep = smoothstep(0.08, 0.24, point.grade.abs());
            // Mud where water gathers and the track is not steep enough to shed it, and on
            // the way into and out of a ford.
            let near_ford = (-12..=12_isize)
                .map(|k| trail.points[wrap(i as isize + k)].ford)
                .fold(0.0, f32::max)
                * (1.0 - point.ford);
            let patchy = smoothstep(0.05, 0.3, fbm(p * 0.03, 3, 981));
            let mud =
                (smoothstep(0.3, 0.62, damp + 0.25 * patchy) * (1.0 - steep)).max(near_ford * 0.9);
            // Rock where the land is stone and the track is steep or on a crest.
            let stone = self.outcrop(point.at.x, point.at.y);
            let rock = (stone * (0.35 + 0.65 * steep)).max(
                smoothstep(0.12, 0.2, point.grade.abs())
                    * smoothstep(-0.2, 0.1, fbm(p * 0.025, 2, 982)),
            ) * (1.0 - mud)
                * (1.0 - point.ford);
            let rut = (0.05 + 0.1 * steep + 0.13 * mud + 0.05 * fbm(p * 0.05, 2, 983))
                * (1.0 - 0.75 * rock);
            // Rain cuts a hill's track, here more and there less, and not where it is stone
            // or a bog already; and never at the camp.
            let away = smoothstep(CAMP * 1.5, CAMP * 3.0, point.at.distance(camp));
            let patchy = smoothstep(-0.25, 0.3, fbm(p * 0.018, 3, 985));
            let erosion = smoothstep(0.045, 0.17, point.grade.abs())
                * patchy
                * (1.0 - mud)
                * (1.0 - point.ford)
                * away;
            // How bare it is between the wheels: more where it is steep, wet or cut.
            let wear =
                (0.22 + 0.5 * steep + 0.6 * mud + 0.5 * erosion + 0.35 * fbm(p * 0.03, 2, 986))
                    .clamp(0.08, 1.0)
                    .max(1.0 - away);
            let point = &mut trail.points[i];
            point.mud = mud;
            point.rock = smoothstep(0.3, 0.7, rock);
            point.rut = (rut * self.country.rutted).max(0.02);
            point.erosion = erosion * (1.0 - 0.6 * point.rock);
            point.wear = wear;
            // A hub's road is graded now and then: worn bare, with shallow wheel tracks, and
            // only its wettest stretches soft. Getting to a track is not the hard part.
            if !rough || street_at(point.at) {
                point.mud = smoothstep(0.55, 1.0, mud) * 0.6;
                point.rut = (rut * 0.22).max(0.012);
                point.tilt *= 0.4;
                point.erosion *= 0.2;
                point.wear = wear.max(0.8);
            }
            // A main road is better kept still, a hub's as the road to a town: hard, level
            // across, with no ledge of rock in it and no rut to speak of.
            if self.hub {
                point.mud = 0.0;
                point.rock = 0.0;
                point.rut = 0.006;
                point.tilt = 0.0;
                point.erosion = 0.0;
                point.wear = 1.0;
            }
        }

        // Drainage humps on the hills, one every forty or fifty metres.
        let mut since = 0.0;
        for i in 0..n {
            let point = trail.points[i];
            since += STEP;
            if point.grade.abs() > 0.11 && point.ford == 0.0 && point.rock < 0.3 && since > 58.0 {
                // Clear of the camp, where the track starts.
                if point.at.distance(camp) > CAMP * 2.0 {
                    trail.bars.push((point.along, -point.grade.signum()));
                    since = 0.0;
                }
            }
        }
        // Bog holes in the wet flats: where the track is muddy and level. If the land gives
        // too few, the muddiest stretches get them anyway: it is what the track is for.
        let mut wanted: Vec<(f32, usize)> = (0..n)
            .filter(|&i| {
                let point = &trail.points[i];
                point.ford == 0.0
                    && point.grade.abs() < 0.07
                    && !street_at(point.at)
                    && point.at.distance(camp) > CAMP * 2.5
                    && pools
                        .iter()
                        .all(|pool| pool.inside(point.at.x, point.at.y) < -3.0)
            })
            .map(|i| {
                let point = &trail.points[i];
                (point.mud + 0.4 * wet.at(point.at.x, point.at.y), i)
            })
            .collect();
        wanted.sort_by(|a, b| b.0.total_cmp(&a.0));
        // A road has no bog holes, washouts or wheel-dug holes: those are what a track is for.
        if !rough {
            wanted.clear();
        }
        let mut holes: Vec<Hole> = Vec::new();
        for &(score, i) in &wanted {
            let most = self.country.bogs;
            if holes.len() >= most || (score < 0.55 && holes.len() >= most.div_ceil(2)) {
                break;
            }
            let at = trail.points[i].along;
            if holes
                .iter()
                .any(|hole| trail.apart(hole.along, at).abs() < 26.0)
            {
                continue;
            }
            let size = 0.5 + 0.5 * fbm(Vec3::new(at * 0.37, 1.0, 0.0), 1, 984);
            holes.push(Hole {
                along: at,
                length: 7.5 + 4.5 * size,
                depth: 0.34 + 0.2 * size,
            });
        }
        // Mud round each hole, whatever the land said.
        for hole in &holes {
            for point in &mut trail.points {
                let away = (point.along - hole.along).abs();
                let away = if open {
                    away
                } else {
                    away.min(trail.length - away)
                };
                let muddy =
                    1.0 - smoothstep(hole.length * 0.5 + 1.0, hole.length * 0.5 + 9.0, away);
                point.mud = point.mud.max(muddy);
                point.rock *= 1.0 - muddy;
            }
        }
        trail.holes = holes;

        // Washouts: where the track crosses a gully, away from the fords and the bog holes,
        // the gully has taken the track with it. The wetter the gully, the deeper.
        let mut crossings: Vec<(f32, usize)> = (0..n)
            .filter(|&i| {
                let point = &trail.points[i];
                let damp = wet.at(point.at.x, point.at.y);
                let peak = (-6..=6_isize).all(|k| {
                    let other = trail.points[wrap(i as isize + k)].at;
                    wet.at(other.x, other.y) <= damp
                });
                damp > 0.2
                    && peak
                    && !street_at(point.at)
                    && point.at.distance(camp) > CAMP * 3.0
                    && pools
                        .iter()
                        .all(|pool| pool.inside(point.at.x, point.at.y) < -6.0)
            })
            .map(|i| (wet.at(trail.points[i].at.x, trail.points[i].at.y), i))
            .collect();
        crossings.sort_by(|a, b| b.0.total_cmp(&a.0));
        if !rough {
            crossings.clear();
        }
        let mut washes: Vec<(f32, f32)> = Vec::new();
        for &(damp, i) in &crossings {
            let at = trail.points[i].along;
            let clear = washes
                .iter()
                .all(|&(other, _)| trail.apart(other, at).abs() > 45.0)
                && trail
                    .holes
                    .iter()
                    .all(|hole| trail.apart(hole.along, at).abs() > hole.length + 8.0);
            if clear && washes.len() < 14 {
                washes.push((at, 0.34 + 0.26 * smoothstep(0.2, 0.8, damp)));
            }
        }
        // Nothing else is built in a washout: no hump, no rill, and the ruts end at its lip.
        let kept: Vec<(f32, f32)> = trail
            .bars
            .iter()
            .copied()
            .filter(|&(bar, _)| {
                washes
                    .iter()
                    .all(|&(wash, _)| trail.apart(wash, bar).abs() > 9.0)
            })
            .collect();
        trail.bars = kept;
        for &(wash, _) in &washes {
            for point in &mut trail.points {
                let d = (point.along - wash).abs();
                let d = if open { d } else { d.min(trail.length - d) };
                let near = 1.0 - smoothstep(2.5, 6.0, d);
                point.erosion *= 1.0 - near;
                point.wear = point.wear.max(near);
            }
        }
        trail.washes = washes;

        // Wheel-dug holes, left and right by turns, in runs of five or six: on the climbs
        // that are neither stone nor already cut by a rill.
        let mut moguls: Vec<(f32, f32)> = Vec::new();
        let mut run_ends = -1.0e9_f32;
        let mut runs = 0;
        for i in 0..n {
            let point = trail.points[i];
            let fits = (0.075..0.2).contains(&point.grade.abs())
                && point.rock < 0.2
                && point.mud < 0.3
                && point.erosion < 0.35
                && point.ford == 0.0
                && !street_at(point.at)
                && point.at.distance(camp) > CAMP * 3.0;
            let clear = trail
                .washes
                .iter()
                .all(|&(wash, _)| trail.apart(wash, point.along).abs() > 26.0)
                && trail
                    .bars
                    .iter()
                    .all(|&(bar, _)| !(-6.0..24.0).contains(&trail.apart(bar, point.along)));
            if fits && clear && point.along - run_ends > 170.0 && runs < 7 && rough {
                let count = 5 + (runs % 2);
                for k in 0..count {
                    let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                    moguls.push((point.along + k as f32 * 2.7, side));
                }
                run_ends = point.along + count as f32 * 2.7;
                runs += 1;
            }
        }
        trail.moguls = moguls;
        // Nothing is dug across a main road to turn the rain off: it is driven fast.
        if self.hub {
            trail.bars.clear();
        }
        // The trailheads: spread evenly round the road from the camp, each at the most level
        // place within sixty metres of where it falls, clear of the fords and their ramps.
        let home = (0..n)
            .min_by(|&a, &b| {
                trail.points[a]
                    .at
                    .distance(camp)
                    .total_cmp(&trail.points[b].at.distance(camp))
            })
            .unwrap_or(0);
        let count = self.places.len();
        // A track that is not a loop has its one trailhead at the gate, where it starts:
        // and a truck that has come in by it starts a little way on, clear of the ring.
        if open && self.edge {
            // It is come into from the edge of the map, and left by it: a truck that is a
            // dozen metres from the edge is as good as gone; one that comes in is put three
            // times as far in, on the road, facing the town.
            let inside = |p: Vec2, by: f32| p.x.abs().max(p.y.abs()) < half - by;
            let first = |by: f32| {
                (0..n)
                    .find(|&i| inside(trail.points[i].at, by))
                    .unwrap_or(0)
            };
            let (zone, arrive) = (trail.points[first(12.0)], trail.points[first(38.0)]);
            self.way_out = Some(WayOut {
                at: zone.at,
                radius: self.tread + 4.5,
                arrive: arrive.at,
                yaw_degrees: (-arrive.forward.x).atan2(-arrive.forward.y).to_degrees(),
            });
            self.camp = arrive.at;
        } else if open {
            let ring = ((RING + 3.0) / STEP) as usize;
            let clear = ring + ((RING + 7.0) / STEP) as usize;
            if let Some(place) = self.places.first_mut() {
                *place = trail.points[ring.min(n - 1)].at;
            }
            self.camp = trail.points[clear.min(n - 1)].at;
        }
        for (k, place) in self.places.iter_mut().enumerate().filter(|_| !open) {
            let middle = home + (k + 1) * n / (count + 1);
            let reach = (60.0 / STEP) as isize;
            let best = (-reach..=reach)
                .map(|step| wrap(middle as isize + step))
                .filter(|&i| !from_pool[i].is_finite() && trail.points[i].ford == 0.0)
                .min_by(|&a, &b| {
                    trail.points[a]
                        .grade
                        .abs()
                        .total_cmp(&trail.points[b].grade.abs())
                })
                .unwrap_or(wrap(middle as isize));
            *place = trail.points[best].at;
        }
        if count > 0 {
            println!("    trailheads at {:?}", self.places);
        }
        // A hub's way out: a main road from its road, by the shed, to the nearest edge of the
        // map, clear of the shed and of the water, and on past the edge a way.
        if self.edge && self.hub && !open {
            let start = trail.points[home];
            let left = Vec2::new(start.forward.y, -start.forward.x);
            // (Where the shed stands: `shed.rs`.)
            let shed = start.at + left * 12.5;
            let rim = half - 4.0;
            let (end, outward) = [
                (Vec2::new(-rim, start.at.y), Vec2::NEG_X),
                (Vec2::new(rim, start.at.y), Vec2::X),
                (Vec2::new(start.at.x, -rim), Vec2::NEG_Y),
                (Vec2::new(start.at.x, rim), Vec2::Y),
            ]
            .into_iter()
            .min_by(|a, b| a.0.distance(start.at).total_cmp(&b.0.distance(start.at)))
            .expect("four of them");
            // It leaves the road within a short way of the shed, wherever is nearest the edge
            // without the shed standing between.
            let passes = |from: Vec2| {
                let run = end - from;
                let t = ((shed - from).dot(run) / run.length_squared().max(1.0)).clamp(0.0, 1.0);
                (from + run * t).distance(shed) < 16.0
            };
            let reach = (45.0 / STEP) as isize;
            let junction = (-reach..=reach)
                .map(|k| wrap(home as isize + k))
                .min_by(|&a, &b| {
                    let score = |i: usize| {
                        let at = trail.points[i].at;
                        at.distance(end) + if passes(at) { 1000.0 } else { 0.0 }
                    };
                    score(a).total_cmp(&score(b))
                })
                .unwrap_or(home);
            let from = trail.points[junction];
            let cost = |index: usize, next: usize, length: f32| -> f32 {
                let (x0, z0) = side.place(index);
                let (x1, z1) = side.place(next);
                let p = Vec2::new(x1, z1);
                if p.distance(shed) < 16.0 || wall[next] {
                    return f32::INFINITY;
                }
                if pools.iter().any(|pool| pool.inside(x1, z1) > -1.2) {
                    return f32::INFINITY;
                }
                // Not along the road it leaves, nor back across it.
                if p.distance(from.at) > 20.0 && used[next] > 2.0 {
                    return f32::INFINITY;
                }
                let grade = (land.at(x1, z1) - land.at(x0, z0)).abs() / length;
                let climb = ((grade - 0.05).max(0.0) / 0.11).powi(2) * 1.6;
                // (A road is made where the ground is easy: it keeps off a hillside.)
                let hillside = (land.slope(x1, z1) / 0.2).powi(2) * 2.0;
                let soggy = smoothstep(0.45, 0.8, wet.at(x1, z1)) * 5.0;
                length * (1.0 + climb + hillside + soggy)
            };
            // To whichever place along that edge, within a hundred metres or so of the
            // nearest, is come to over the gentlest ground.
            let along_edge = Vec2::new(outward.y, outward.x).abs();
            let rough = |path: &[usize]| -> f32 {
                path.iter()
                    .map(|&index| {
                        let (x, z) = side.place(index);
                        0.02 + land.slope(x, z)
                    })
                    .sum()
            };
            let (path, end) = (-4..=4)
                .map(|k| end + along_edge * (k as f32 * 30.0))
                .filter(|end| end.x.abs().max(end.y.abs()) <= rim + 0.5)
                .map(|end| (search(&side, from.at, end, &cost), end))
                .filter(|(path, _)| path.len() >= 4)
                .min_by(|a, b| rough(&a.0).total_cmp(&rough(&b.0)))
                .unwrap_or((Vec::new(), end));
            if path.len() < 4 {
                println!("    no way was found from the shed to the edge of the map");
            } else {
                let mut line: Vec<Vec2> = vec![from.at];
                line.extend(path.iter().map(|&index| {
                    let (x, z) = side.place(index);
                    Vec2::new(x, z)
                }));
                line.push(end + outward * BEYOND_EDGE);
                // At an even pace, and its corners opened out; its ends stay where they are.
                let mut road: Vec<Vec2> = vec![line[0]];
                let mut left_over = STEP;
                for pair in line.windows(2) {
                    let run = pair[0].distance(pair[1]);
                    let mut gone = 0.0;
                    while run - gone >= left_over {
                        gone += left_over;
                        road.push(pair[0].lerp(pair[1], gone / run));
                        left_over = STEP;
                    }
                    left_over -= run - gone;
                }
                let count = road.len();
                for _ in 0..90 {
                    let before = road.clone();
                    for i in 3..count.saturating_sub(3) {
                        let middle: Vec2 = (i - 3..=i + 3).map(|k| before[k]).sum::<Vec2>() / 7.0;
                        road[i] = before[i].lerp(middle, 0.6);
                    }
                }
                // Its height: the land's, evened out, from the road's own where it leaves it.
                let mut high: Vec<f32> = road.iter().map(|p| land.at(p.x, p.y)).collect();
                for _ in 0..48 {
                    let before = high.clone();
                    for i in 0..count {
                        let (low, top) = (i.saturating_sub(4), (i + 4).min(count - 1));
                        high[i] = before[low..=top].iter().sum::<f32>() / (top - low + 1) as f32;
                    }
                }
                for (i, height) in high.iter_mut().enumerate() {
                    let eased = smoothstep(0.0, 14.0, i as f32);
                    *height = from.height + (*height - from.height) * eased;
                }
                // And it climbs and falls no steeper than a road does: one in eight.
                for i in 1..count {
                    let most = 0.125 * road[i].distance(road[i - 1]);
                    high[i] = high[i].clamp(high[i - 1] - most, high[i - 1] + most);
                }
                // For as long as it runs beside the road it leaves it is as high as that road
                // is there, whatever that road does: the two are one piece of ground until
                // they have parted, with no bank between them.
                for i in 0..count {
                    let (beside, away) = (-70..=70_isize)
                        .map(|k| wrap(junction as isize + k))
                        .map(|j| (j, trail.points[j].at.distance(road[i])))
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .unwrap_or((junction, 0.0));
                    let with = 1.0 - smoothstep(tread * 2.0, 26.0, away);
                    high[i] += (trail.points[beside].height - high[i]) * with;
                }
                // It is numbered on from the road's end, and well clear of it.
                let mut along = trail.length + 1000.0;
                trail.main = n;
                trail.junction = Some(junction);
                for i in 0..count {
                    let (before, after) = (road[i.saturating_sub(1)], road[(i + 1).min(count - 1)]);
                    let run = before.distance(after).max(1.0e-3);
                    let grade = (high[(i + 1).min(count - 1)] - high[i.saturating_sub(1)]) / run;
                    trail.points.push(TrailPoint {
                        at: road[i],
                        height: high[i],
                        forward: (after - before).normalize_or(outward),
                        along,
                        grade,
                        rut: 0.01,
                        wear: 1.0,
                        ..TrailPoint::default()
                    });
                    along += road[i].distance(road[(i + 1).min(count - 1)]);
                }
                // Driven off the edge by it, a truck is gone a dozen metres short of the edge;
                // one that comes in by it is put three times as far in, facing the shed.
                let spur = &trail.points[n..];
                let out = |p: Vec2, by: f32| p.x.abs().max(p.y.abs()) > half - by;
                let first = |by: f32| spur.iter().position(|point| out(point.at, by)).unwrap_or(0);
                let (zone, arrive) = (spur[first(12.0)], spur[first(38.0).max(4)]);
                self.way_out = Some(WayOut {
                    at: zone.at,
                    radius: self.tread + 4.5,
                    arrive: arrive.at,
                    yaw_degrees: arrive.forward.x.atan2(arrive.forward.y).to_degrees(),
                });
                println!(
                    "    a road of {:.0} m from the shed to the edge of the map at {end}",
                    along - trail.length - 1000.0 - BEYOND_EDGE
                );
            }
        }
        trail.index();
        println!(
            "    a {} of {:.0} m: {} drainage humps, {} bog holes, {} washouts, {} runs of wheel holes",
            if open { "track" } else { "loop" },
            trail.length,
            trail.bars.len(),
            trail.holes.len(),
            trail.washes.len(),
            runs
        );
        let cut = trail.points.iter().filter(|p| p.erosion > 0.3).count() as f32 * STEP;
        let stone = trail.points.iter().filter(|p| p.rock > 0.5).count() as f32 * STEP;
        let soft = trail.points.iter().filter(|p| p.mud > 0.5).count() as f32 * STEP;
        println!("    {cut:.0} m of it cut by rain, {stone:.0} m over rock, {soft:.0} m of mud");
        self.trail = trail;
    }

    /// Where a car starts: on the track at the camp, and the way it faces, as a yaw in degrees
    /// (zero looks along -z).
    pub fn start(&self) -> (Vec3, f32) {
        let point = self
            .trail
            .main_points()
            .iter()
            .min_by(|a, b| {
                a.at.distance(self.camp)
                    .total_cmp(&b.at.distance(self.camp))
            })
            .copied()
            .unwrap_or_default();
        let height = self.height(point.at.x, point.at.y);
        let yaw = (-point.forward.x).atan2(-point.forward.y).to_degrees();
        (Vec3::new(point.at.x, height, point.at.y), yaw)
    }
}

/// The cheapest way over a grid from one place to another, as the grid's points in order.
fn search(
    grid: &Grid,
    from: Vec2,
    to: Vec2,
    cost: &dyn Fn(usize, usize, f32) -> f32,
) -> Vec<usize> {
    #[derive(PartialEq)]
    struct Open(f32, usize);
    impl Eq for Open {}
    impl PartialOrd for Open {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Open {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
        }
    }
    let (start, goal) = (grid.nearest(from.x, from.y), grid.nearest(to.x, to.y));
    let stride = grid.stride() as isize;
    let mut reached = vec![f32::INFINITY; grid.values.len()];
    let mut came = vec![u32::MAX; grid.values.len()];
    let mut open = BinaryHeap::new();
    reached[start] = 0.0;
    open.push(Reverse(Open(0.0, start)));
    let remaining = |index: usize| {
        let (x, z) = grid.place(index);
        Vec2::new(x, z).distance(to)
    };
    // Sixteen ways from each point: to its neighbours, and a knight's move beyond, so that
    // a track can run at more angles than eight.
    const MOVES: [(isize, isize); 16] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
        (2, 1),
        (2, -1),
        (-2, 1),
        (-2, -1),
        (1, 2),
        (1, -2),
        (-1, 2),
        (-1, -2),
    ];
    while let Some(Reverse(Open(_, index))) = open.pop() {
        if index == goal {
            break;
        }
        let (column, row) = (index as isize % stride, index as isize / stride);
        for (dx, dz) in MOVES {
            let (c, r) = (column + dx, row + dz);
            if c < 0 || r < 0 || c >= stride || r >= stride {
                continue;
            }
            let next = (r * stride + c) as usize;
            let length = (dx as f32).hypot(dz as f32) * grid.spacing;
            let step = cost(index, next, length);
            if !step.is_finite() {
                continue;
            }
            let total = reached[index] + step;
            if total < reached[next] {
                reached[next] = total;
                came[next] = index as u32;
                open.push(Reverse(Open(total + remaining(next), next)));
            }
        }
    }
    let mut path = Vec::new();
    let mut at = goal;
    if came[goal] == u32::MAX && goal != start {
        // No way was found: straight there, rather than no track at all.
        let steps = (from.distance(to) / grid.spacing).ceil() as usize;
        return (0..steps)
            .map(|k| {
                let p = from.lerp(to, k as f32 / steps as f32);
                grid.nearest(p.x, p.y)
            })
            .collect();
    }
    while at != start {
        path.push(at);
        at = came[at] as usize;
    }
    path.push(start);
    path.reverse();
    // The goal is the next leg's start.
    path.pop();
    path
}

/// Where water runs over a grid of heights, and how much.
struct Flow {
    stride: usize,
    /// For each point, the one its water runs to, or itself at the edge.
    down: Vec<u32>,
    /// How much has gathered at each: the points whose water passes through it.
    gathered: Vec<f32>,
}

impl Flow {
    fn new(land: &Grid) -> Self {
        let stride = land.stride();
        let count = land.values.len();
        // Fill every hollow until it spills (Barnes, Lehman and Mulla 2014): from the edge
        // inwards, lowest first, no point may be lower than the one it was reached from.
        #[derive(PartialEq)]
        struct Wet(f32, u32);
        impl Eq for Wet {}
        impl PartialOrd for Wet {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Wet {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
            }
        }
        let mut filled = land.values.clone();
        let mut seen = vec![false; count];
        let mut heap = BinaryHeap::new();
        for row in 0..stride {
            for column in 0..stride {
                if row == 0 || column == 0 || row == stride - 1 || column == stride - 1 {
                    let index = row * stride + column;
                    seen[index] = true;
                    heap.push(Reverse(Wet(filled[index], index as u32)));
                }
            }
        }
        let neighbours = |index: usize| {
            let (column, row) = ((index % stride) as isize, (index / stride) as isize);
            (-1..=1_isize)
                .flat_map(move |dz| (-1..=1_isize).map(move |dx| (dx, dz)))
                .filter(|&(dx, dz)| dx != 0 || dz != 0)
                .filter_map(move |(dx, dz)| {
                    let (c, r) = (column + dx, row + dz);
                    (c >= 0 && r >= 0 && c < stride as isize && r < stride as isize).then(|| {
                        (
                            (r * stride as isize + c) as usize,
                            if dx != 0 && dz != 0 {
                                std::f32::consts::SQRT_2
                            } else {
                                1.0
                            },
                        )
                    })
                })
        };
        // In the order the flood reaches them, which is from low to high.
        let mut order: Vec<u32> = Vec::with_capacity(count);
        while let Some(Reverse(Wet(height, index))) = heap.pop() {
            order.push(index);
            for (next, _) in neighbours(index as usize) {
                if !seen[next] {
                    seen[next] = true;
                    // A hair higher, so that a filled hollow still drains across its flat.
                    filled[next] = filled[next].max(height + 1.0e-4);
                    heap.push(Reverse(Wet(filled[next], next as u32)));
                }
            }
        }
        // Each point's water runs to the neighbour it falls most steeply to.
        let down: Vec<u32> = (0..count)
            .into_par_iter()
            .map(|index| {
                let mut best = (0.0_f32, index);
                for (next, length) in neighbours(index) {
                    let fall = (filled[index] - filled[next]) / length;
                    if fall > best.0 {
                        best = (fall, next);
                    }
                }
                best.1 as u32
            })
            .collect();
        // From the highest down, each hands on what it has gathered.
        let mut gathered = vec![1.0_f32; count];
        for &index in order.iter().rev() {
            let to = down[index as usize];
            if to != index {
                gathered[to as usize] += gathered[index as usize];
            }
        }
        Self {
            stride,
            down,
            gathered,
        }
    }

    /// How deep water has cut the land at each point, in metres: nothing where little
    /// gathers, a gully where a hillside drains, the creek's bed where the valley does, each
    /// with banks that slope back to the land.
    fn cut(&self, land: &Grid) -> Vec<f32> {
        let stride = self.stride;
        let count = land.values.len();
        let mut depth: Vec<f32> = self
            .gathered
            .par_iter()
            .map(|&gathered| 0.95 * (gathered.log10() - 2.9).max(0.0).powf(1.3))
            .collect();
        // Banks: no steeper than they stand. Two sweeps, each way, as a distance is spread.
        const BANK: f32 = 0.3;
        let relax = |depth: &mut Vec<f32>, index: usize, from: usize, length: f32| {
            let through = depth[from] - BANK * length;
            if through > depth[index] {
                depth[index] = through;
            }
        };
        let diagonal = std::f32::consts::SQRT_2;
        for _ in 0..2 {
            for row in 1..stride - 1 {
                for column in 1..stride - 1 {
                    let index = row * stride + column;
                    relax(&mut depth, index, index - 1, 1.0);
                    relax(&mut depth, index, index - stride, 1.0);
                    relax(&mut depth, index, index - stride - 1, diagonal);
                    relax(&mut depth, index, index - stride + 1, diagonal);
                }
            }
            for row in (1..stride - 1).rev() {
                for column in (1..stride - 1).rev() {
                    let index = row * stride + column;
                    relax(&mut depth, index, index + 1, 1.0);
                    relax(&mut depth, index, index + stride, 1.0);
                    relax(&mut depth, index, index + stride + 1, diagonal);
                    relax(&mut depth, index, index + stride - 1, diagonal);
                }
            }
        }
        // A bed is not a knife's cut: round it.
        for _ in 0..3 {
            let before = depth.clone();
            depth = (0..count)
                .into_par_iter()
                .map(|index| {
                    let (column, row) = (index % stride, index / stride);
                    if column == 0 || row == 0 || column == stride - 1 || row == stride - 1 {
                        return before[index];
                    }
                    let mut sum = 0.0;
                    for dz in 0..3 {
                        for dx in 0..3 {
                            sum += before[(row + dz - 1) * stride + column + dx - 1];
                        }
                    }
                    sum / 9.0
                })
                .collect();
        }
        depth
    }

    /// The main stream: from where the most water leaves the land, upstream by the greater
    /// branch at each meeting, for as long as at least `least` has gathered.
    fn main_stem(&self, least: f32) -> Vec<usize> {
        let count = self.gathered.len();
        let mouth = (0..count)
            .max_by(|&a, &b| self.gathered[a].total_cmp(&self.gathered[b]))
            .unwrap_or(0);
        // The greatest of the points that drain to each.
        let mut feeder = vec![u32::MAX; count];
        for index in 0..count {
            let to = self.down[index] as usize;
            if to != index
                && (feeder[to] == u32::MAX
                    || self.gathered[index] > self.gathered[feeder[to] as usize])
            {
                feeder[to] = index as u32;
            }
        }
        let mut stem = vec![mouth];
        let mut at = mouth;
        while feeder[at] != u32::MAX && self.gathered[feeder[at] as usize] >= least {
            at = feeder[at] as usize;
            stem.push(at);
        }
        stem
    }
}

/// A picture of the design from above, for looking at while changing it: a `.ppm` file's bytes,
/// two metres to a pixel. The land is shaded by its slope, the creek and waterholes are blue,
/// the track is drawn by what it is made of, and the camp is white.
pub fn picture(plan: &Plan) -> Vec<u8> {
    picture_of(plan, Vec2::ZERO, plan.half + 40.0, 2.0)
}

/// A picture of part of the design: `half` metres either side of `middle`, `scale` metres to
/// a pixel, with a contour drawn every half metre when it is close enough to see them.
pub fn picture_of(plan: &Plan, middle: Vec2, half: f32, scale: f32) -> Vec<u8> {
    let size = (2.0 * half / scale) as usize;
    let mut bytes = format!("P6\n{size} {size}\n255\n").into_bytes();
    let pixels: Vec<[u8; 3]> = (0..size * size)
        .into_par_iter()
        .map(|index| {
            let (x, z) = (
                (index % size) as f32 * scale - half + middle.x,
                (index / size) as f32 * scale - half + middle.y,
            );
            let h = plan.height(x, z);
            let light = ((plan.height(x - 2.0, z - 2.0) - plan.height(x + 2.0, z + 2.0)) * 0.22
                + 0.62)
                .clamp(0.15, 1.0);
            let cover = plan.cover(x, z);
            let tall = ((h - 40.0) / 90.0).clamp(0.0, 1.0);
            let mut color = Vec3::new(0.42 + 0.3 * tall, 0.5 + 0.2 * tall, 0.3 + 0.2 * tall);
            color = color.lerp(Vec3::new(0.75, 0.72, 0.4), cover.grass * 0.7);
            color = color.lerp(Vec3::new(0.6, 0.6, 0.62), cover.rock);
            color = color.lerp(Vec3::new(0.3, 0.38, 0.7), plan.wet(x, z) * 0.6);
            color *= light;
            if cover.tread > 0.3 {
                color = Vec3::new(0.85, 0.5, 0.25);
                color = color.lerp(Vec3::new(0.35, 0.2, 0.1), cover.mud);
                color = color.lerp(Vec3::new(0.95, 0.95, 0.95), cover.rock);
                if cover.bog > 0.1 {
                    color = Vec3::new(0.9, 0.1, 0.6);
                }
                if cover.scour > 0.3 {
                    color = Vec3::new(0.1, 0.9, 0.9);
                }
            }
            if cover.water {
                color = Vec3::new(0.1, 0.3, 0.95);
            }
            if x.abs().max(z.abs()) > plan.half && (x.abs().max(z.abs()) - plan.half) < 3.0 {
                color = Vec3::new(1.0, 0.0, 0.0);
            }
            if Vec2::new(x, z).distance(plan.camp) < 5.0 {
                color = Vec3::ONE;
            }
            if scale < 1.0 {
                let line = (h * 2.0).fract();
                if !(0.06..0.94).contains(&line) {
                    color *= 0.55;
                }
                if plan
                    .trail
                    .near(x, z)
                    .is_some_and(|near| near.offset.abs() < 0.12)
                {
                    color = Vec3::new(1.0, 1.0, 0.2);
                }
            }
            (color * 255.0)
                .to_array()
                .map(|c| c.clamp(0.0, 255.0) as u8)
        })
        .collect();
    for pixel in pixels {
        bytes.extend_from_slice(&pixel);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_reads_between_its_points() {
        let mut grid = Grid::new(4.0, 2.0);
        assert_eq!(grid.cells, 4);
        for index in 0..grid.values.len() {
            let (x, z) = grid.place(index);
            grid.values[index] = x + 10.0 * z;
        }
        assert!((grid.at(1.0, -1.0) - (1.0 - 10.0)).abs() < 1.0e-4);
        assert_eq!(grid.nearest(-4.0, -4.0), 0);
        assert_eq!(grid.place(grid.nearest(1.9, 0.2)), (2.0, 0.0));
        assert!((grid.slope(0.0, 0.0) - 101.0_f32.sqrt()).abs() < 1.0e-3);
    }

    #[test]
    fn water_runs_downhill_and_gathers() {
        // A trough falling to the east: everything gathers along its middle.
        let mut land = Grid::new(20.0, 1.0);
        for index in 0..land.values.len() {
            let (x, z) = land.place(index);
            land.values[index] = z.abs() * 0.5 - x * 0.1;
        }
        // A pit in it, which must fill and spill rather than swallow the stream.
        let pit = land.nearest(0.0, 0.0);
        land.values[pit] -= 3.0;
        let flow = Flow::new(&land);
        let mouth = land.nearest(20.0, 0.0);
        let side = land.nearest(20.0, 15.0);
        assert!(
            flow.gathered[mouth] > 600.0,
            "at the mouth: {}",
            flow.gathered[mouth]
        );
        assert!(flow.gathered[side] < 30.0);
        let stem = flow.main_stem(100.0);
        assert!(stem.len() > 25, "{} points", stem.len());
        // It runs up the trough's middle.
        for &index in &stem {
            assert!(land.place(index).1.abs() <= 2.0);
        }
        let cut = flow.cut(&land);
        assert!(cut[mouth] > cut[side]);
    }
}
