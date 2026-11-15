//! Bulldust Bowl: a showground scraped out of red country a long way from anywhere, for
//! throwing a big truck around, as a scene file's `arena` asks (`docs/arena.md`).
//!
//! ```text
//!                         north
//!     .-------------------------------------------.
//!     |   <-- way off     THE LOOP    <-- run-up  |    an earth bank all round,
//!     |                                           |    with a gate in the south
//!     |  hop  tabletop   double   step-up  -->    |
//!     |                                           |
//!     |  -->        BIG AIR         wrecks        |
//!     |                                           |
//!     |    the wallow               mud run       |
//!     |                                           |
//!     '--------------------.  .-------------------'
//!                          gate
//! ```
//!
//! It is a design, as the mud proving ground is ([`super::proving`]), not country found by
//! the generator: every jump, pit and bank is put where it is and shaped as it is here, and
//! from the one design come the land that is drawn, the grid of heights that is driven on,
//! the map of what a tyre is on, and the ring road that goes round it all, which is the
//! map's track. Outside the bank is open country to drive in: a red plain with a dry creek,
//! a dune and a stony rise, mesas on the skyline, and scrub a long way apart.
//!
//! The jumps and the pits are earth: they are the land. The loop is steel ([`ring`]).

pub mod ring;

use super::flora::{self, Stood};
use super::proving::grid;
use super::terrain::smoothstep;
use super::{chart, draw, ground, Built};
use glam::{Mat4, Vec2, Vec3};
use rayon::prelude::*;
use ring::Ring;
use solstice_asset::{Collide, Heightfield, Instance, Light, Material, SceneAsset};
use solstice_builder::{build_virtual_mesh, BuildOptions, Mesh};
use solstice_game::track::Sample;
use solstice_game::{Blocker, SurfaceMap, Track};
use solstice_import::{append_model, import_gltf, ImportOptions, ModelPlacements};
use solstice_procgen::foliage::Rng;
use solstice_procgen::noise::fbm;
use solstice_vehicle::Surface;
use std::fmt::Write as _;
use std::path::Path;

/// What the map is called.
pub const NAME: &str = "Bulldust Bowl";
/// Its set dressing, made by `tools/blender/arena/build.py`, from the scene's folder.
const MODEL: &str = "../target/outback/arena.glb";
/// How high the arena's floor stands, so that its pits are above zero.
pub const LEVEL: f32 = 2.0;
/// The arena's floor: half its width and its depth, and how round its corners are.
const FLOOR: Vec2 = Vec2::new(124.0, 92.0);
const CORNER: f32 = 50.0;
/// The bank round it: how high, and how far out from the floor's edge its outer foot is.
const BANK: f32 = 3.4;
const BANK_OUT: f32 = 14.0;
/// The gate in the south of it: where along x, and half how wide.
const GATE: f32 = 0.0;
const GATE_HALF: f32 = 8.0;
/// How far inside the floor's edge the ring road runs, and half how wide it is.
const ROAD_IN: f32 = 10.0;
const ROAD_HALF: f32 = 3.5;
/// The land drawn fine: the floor and its bank. Its least and greatest x and z.
const FINE: (Vec2, Vec2) = (Vec2::new(-144.0, -112.0), Vec2::new(144.0, 112.0));
/// Metres between the fine land's points, and between those of the country round it.
const SMALL: f32 = 0.25;
const COARSE: f32 = 1.0;
/// How far out the country that is driven on reaches, where a wall stands, and how far the
/// skyline beyond it is drawn, at eight times the spacing.
pub const AROUND: f32 = 264.0;
const BEYOND: f32 = 1584.0;
/// How steep a mound's sides are: metres out for a metre up.
const SIDE: f32 = 1.25;
/// How far a jump's corners are rounded: half the length its profile is averaged over.
const ROUND: f32 = 0.9;

/// A mound of earth to drive over: where it starts, the way it is driven, half how wide its
/// top is, and its shape along the way, as heights at distances from its start.
pub struct Jump {
    pub name: &'static str,
    pub at: Vec2,
    pub heading: Vec2,
    pub half: f32,
    pub profile: &'static [(f32, f32)],
}

/// The jumps, each a different thing to learn.
///
/// * the hop: a kicker a metre high, to find out what the truck does in the air;
/// * the tabletop: up to a flat top and down the far side, safe to come up short on;
/// * the double: two humps and a gap, which is cleared or is not;
/// * the step-up: a kicker on to a bench higher than itself, and a long roll off it;
/// * big air: a ramp three metres high, a gap with a row of wrecks in it, and a long
///   landing.
pub const JUMPS: [Jump; 5] = [
    Jump {
        name: "hop",
        at: Vec2::new(-88.0, -26.0),
        heading: Vec2::X,
        half: 4.5,
        profile: &[(0.0, 0.0), (6.0, 1.0), (7.6, 0.0)],
    },
    Jump {
        name: "tabletop",
        at: Vec2::new(-66.0, -26.0),
        heading: Vec2::X,
        half: 5.0,
        profile: &[(0.0, 0.0), (9.0, 2.4), (19.0, 2.4), (30.0, 0.0)],
    },
    Jump {
        name: "double",
        at: Vec2::new(-16.0, -26.0),
        heading: Vec2::X,
        half: 5.0,
        profile: &[
            (0.0, 0.0),
            (8.0, 2.2),
            (10.2, 0.25),
            (17.4, 0.25),
            (20.0, 2.2),
            (32.0, 0.0),
        ],
    },
    Jump {
        name: "step-up",
        at: Vec2::new(36.0, -26.0),
        heading: Vec2::X,
        half: 5.5,
        profile: &[
            (0.0, 0.0),
            (3.0, 0.6),
            (6.0, 2.0),
            (7.4, 0.6),
            (10.5, 2.8),
            (26.0, 2.8),
            (42.0, 0.0),
        ],
    },
    Jump {
        name: "big air",
        at: Vec2::new(-46.0, 2.0),
        heading: Vec2::X,
        half: 6.0,
        profile: &[
            (0.0, 0.0),
            (4.0, 0.3),
            (8.0, 1.2),
            (14.0, 3.0),
            (15.6, 0.3),
            (30.0, 0.3),
            (33.0, 3.0),
            (37.0, 3.0),
            (62.0, 0.0),
        ],
    },
];

/// The wallow: a bog in a bowl, with water standing in the middle of it. Its middle, half
/// its length and width, and how deep it is.
const WALLOW: (Vec2, Vec2, f32) = (Vec2::new(-56.0, 44.0), Vec2::new(27.0, 10.0), 0.62);
/// How far below the floor its water stands.
const WALLOW_WATER: f32 = 0.5;
/// The mud run: a lane of wet mud with a row of rollers down it. Its middle, half its
/// length and width, how far apart its rollers are and how high.
const RUN: (Vec2, Vec2) = (Vec2::new(54.0, 44.0), Vec2::new(31.0, 6.0));
const ROLLERS: (f32, f32) = (6.5, 0.46);
/// Where the loop's deck leaves the ground, and the way a truck drives on to it: a yaw in
/// degrees, west.
const LOOP_AT: Vec2 = Vec2::new(14.0, -52.0);
const LOOP_YAW: f32 = 90.0;
/// How far above the floor the loop's plates lie.
const PLATE: f32 = 0.04;
/// The row of wrecks to drive over: where its middle is, and how many there are.
const WRECKS: (Vec2, usize) = (Vec2::new(74.0, 20.0), 7);

/// The area under a line through `points`, from its first point to `x`: before the first
/// point and after the last, the line is level.
fn area(points: &[(f32, f32)], x: f32) -> f32 {
    let (first, last) = (points[0], points[points.len() - 1]);
    if x <= first.0 {
        return (x - first.0) * first.1;
    }
    let mut sum = 0.0;
    for pair in points.windows(2) {
        let ((a, low), (b, high)) = (pair[0], pair[1]);
        if x <= a {
            break;
        }
        let to = x.min(b);
        let at = low + (high - low) * (to - a) / (b - a);
        sum += (low + at) * 0.5 * (to - a);
    }
    sum + (x - last.0).max(0.0) * last.1
}

/// The height of a line through `points` at a distance along it: level before its first
/// point and after its last.
fn line(points: &[(f32, f32)], along: f32) -> f32 {
    let (first, last) = (points[0], points[points.len() - 1]);
    if along <= first.0 {
        return first.1;
    }
    for pair in points.windows(2) {
        let ((a, low), (b, high)) = (pair[0], pair[1]);
        if along <= b {
            return low + (high - low) * (along - a) / (b - a);
        }
    }
    last.1
}

/// The height of a profile at a distance along it. Where it turns upward, at the foot of a
/// ramp, it is rounded: there it is the average of the line through its points over a
/// little way each side. Where it turns downward, at a lip or a crest, it is the line
/// itself: a jump's lip is sharp, and sends a truck off at the slope it was climbing.
pub fn eased(points: &[(f32, f32)], along: f32) -> f32 {
    let rounded = (area(points, along + ROUND) - area(points, along - ROUND)) / (2.0 * ROUND);
    rounded.max(line(points, along))
}

/// The lesser of two heights, rounded where they meet.
fn lesser(a: f32, b: f32, round: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / round).clamp(0.0, 1.0);
    b + (a - b) * h - round * h * (1.0 - h)
}

/// How far a place is outside a rectangle with rounded corners, about the origin: less than
/// nothing, inside it.
fn outside(at: Vec2, half: Vec2, corner: f32) -> f32 {
    let q = at.abs() - (half - Vec2::splat(corner));
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - corner
}

impl Jump {
    /// How long it is, from where it starts to where it ends.
    pub fn length(&self) -> f32 {
        self.profile.last().map_or(0.0, |last| last.0)
    }

    /// A place on the map, as so far along it and so far to its right.
    fn local(&self, at: Vec2) -> (f32, f32) {
        let off = at - self.at;
        (off.dot(self.heading), off.dot(-self.heading.perp()))
    }

    /// A place so far along it, on the map.
    pub fn along(&self, along: f32) -> Vec2 {
        self.at + self.heading * along
    }

    /// How high it stands over the floor at a place.
    fn height(&self, at: Vec2) -> f32 {
        let (along, across) = self.local(at);
        if along < -ROUND || along > self.length() + ROUND {
            return 0.0;
        }
        let high = eased(self.profile, along);
        if high <= 1.0e-4 {
            return 0.0;
        }
        // A flat top as wide as it is said to be, and sides that fall away from it.
        let side = (self.half + SIDE * high - across.abs()) / SIDE;
        lesser(high, side, 0.35).max(0.0)
    }
}

/// What the ground is at a place.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spot {
    pub height: f32,
    /// Of the arena's own ground, how much is mud and how much loose dust: the rest is
    /// hard red clay.
    pub mud: f32,
    pub dust: f32,
    /// Of the country's, how much is dry grass and how much stone: the rest is red sand.
    pub grass: f32,
    pub rock: f32,
    /// How much this is the bog, whether water stands on it, and how much it is loose sand:
    /// the creek's bed, the dune.
    pub bog: f32,
    pub water: bool,
    pub sand: f32,
    /// How much it is something built of earth: a jump, the bank.
    pub built: f32,
}

/// The design: where the loop stands, the ground it stands over, and the ring road.
pub struct Design {
    pub ring: Ring,
    /// The ground under the loop, as squares half a metre across over the fine land.
    hard: Vec<bool>,
    /// The ring road's line: points a metre apart, each with the way the road runs there.
    pub road: Vec<(Vec2, Vec2)>,
}

/// The squares of the fine land that `hard` is kept in: how many across.
const HARD: f32 = 0.5;
fn hard_columns() -> usize {
    ((FINE.1.x - FINE.0.x) / HARD) as usize
}

impl Default for Design {
    fn default() -> Self {
        Self::new()
    }
}

impl Design {
    pub fn new() -> Self {
        let ring = Ring::standing(Vec3::new(LOOP_AT.x, LEVEL + PLATE, LOOP_AT.y), LOOP_YAW);
        let columns = hard_columns();
        let rows = ((FINE.1.y - FINE.0.y) / HARD) as usize;
        let mut hard = vec![false; columns * rows];
        for at in ring.footprint() {
            let cell = ((at - FINE.0) / HARD).floor();
            if cell.x >= 0.0
                && cell.y >= 0.0
                && (cell.x as usize) < columns
                && (cell.y as usize) < rows
            {
                hard[cell.y as usize * columns + cell.x as usize] = true;
            }
        }
        Self {
            ring,
            hard,
            road: road(),
        }
    }

    /// Whether a place is under the loop: hard standing, which keeps no rut.
    pub fn hard(&self, x: f32, z: f32) -> bool {
        let cell = ((Vec2::new(x, z) - FINE.0) / HARD).floor();
        let columns = hard_columns();
        cell.x >= 0.0
            && cell.y >= 0.0
            && (cell.x as usize) < columns
            && self
                .hard
                .get(cell.y as usize * columns + cell.x as usize)
                .is_some_and(|hard| *hard)
    }

    pub fn spot(&self, x: f32, z: f32) -> Spot {
        let at = Vec2::new(x, z);
        let p = Vec3::new(x, 0.0, z);
        // Level at the edge of the fine land and at the edge of what is driven on, so that
        // where one spacing of points meets another every point lies in one plane.
        let fine_edge = (at - FINE.0).min(FINE.1 - at).min_element();
        let inside = smoothstep(1.0, 5.0, fine_edge);
        let reach = x.abs().max(z.abs());
        let around = smoothstep(1.0, 6.0, -fine_edge)
            * (1.0 - smoothstep(AROUND - 8.0, AROUND - 2.0, reach));
        let far = smoothstep(AROUND + 6.0, 520.0, reach);
        let mut spot = Spot {
            height: LEVEL,
            ..Spot::default()
        };
        if far > 0.0 {
            // The skyline: flat-topped hills with scree at their feet, a long way off.
            let rise = fbm(p * 0.0021, 4, 5103);
            let mesa = 58.0 * smoothstep(0.1, 0.17, rise) + 22.0 * smoothstep(-0.12, 0.1, rise);
            spot.height += far * far * (mesa + 7.0 * fbm(p * 0.011, 3, 5104).abs());
            spot.rock = far * smoothstep(0.02, 0.12, rise);
            spot.grass = far * 0.5 * smoothstep(-0.1, 0.3, fbm(p * 0.02, 3, 5110));
            return spot;
        }
        if fine_edge <= 0.0 {
            return self.country(spot, at, around);
        }

        // The floor, a little uneven, and the bank round it.
        let out = outside(at, FLOOR, CORNER);
        spot.height += inside * 0.03 * fbm(p * 0.33, 3, 5101);
        let gate = if z > 0.0 {
            smoothstep(GATE_HALF, GATE_HALF + 6.0, (x - GATE).abs())
        } else {
            1.0
        };
        let bank = eased(
            &[(-0.5, 0.0), (5.0, BANK), (7.5, BANK), (BANK_OUT, 0.0)],
            out,
        ) * gate
            * (1.0 + 0.07 * fbm(p * 0.09, 2, 5102));
        spot.height += inside * bank;
        spot.built = (bank / BANK).min(1.0);
        // Loose dust: on the bank, and lying in drifts on the floor where nothing is driven.
        let drift = smoothstep(0.05, 0.5, fbm(p * 0.045, 3, 5105));
        spot.dust = (0.75 * smoothstep(0.2, 1.4, bank) + 0.55 * drift).min(1.0);
        if out > 1.0 {
            // Outside the bank's foot the country begins.
            spot.grass = smoothstep(BANK_OUT - 2.0, BANK_OUT + 4.0, out)
                * smoothstep(-0.05, 0.4, fbm(p * 0.035, 3, 5110));
            return spot;
        }

        // The jumps: packed hard, with the dust swept off them.
        let jump = JUMPS.iter().map(|jump| jump.height(at)).fold(0.0, f32::max);
        if jump > 0.0 {
            spot.height += jump;
            spot.built = spot.built.max(smoothstep(0.0, 0.3, jump));
            spot.dust *= 1.0 - 0.8 * smoothstep(0.02, 0.4, jump);
        }
        // The ring road and the lanes are swept too.
        let road = self.off_road(at);
        spot.dust *= 0.25 + 0.75 * smoothstep(ROAD_HALF - 0.5, ROAD_HALF + 2.5, road);

        // The wallow: a bowl of deep mud, churned and lumpy, with water standing in it.
        let (middle, half, deep) = WALLOW;
        let from = outside(at - middle, half, half.y);
        if from < 3.0 {
            let bowl = smoothstep(0.0, -6.0, from);
            let churned = 0.05 * fbm(p * 1.3, 3, 5107) * smoothstep(0.5, -1.5, from);
            spot.height -= deep * bowl - churned;
            spot.bog = smoothstep(-0.3, -2.2, from);
            spot.mud = spot.mud.max(smoothstep(2.6, -0.6, from));
            spot.water = deep * bowl - churned > WALLOW_WATER + 0.03;
        }
        // The mud run: wet mud, a hand deep, with rollers across it.
        let (middle, half) = RUN;
        let from = outside(at - middle, half, half.y);
        if from < 3.0 {
            let within = smoothstep(0.0, -2.5, from);
            let ends = smoothstep(half.x - 3.0, half.x - 9.0, (x - middle.x).abs());
            let wave = 0.5 - 0.5 * (std::f32::consts::TAU * (x - middle.x) / ROLLERS.0).cos();
            spot.height +=
                within * (ends * ROLLERS.1 * wave - 0.14) + 0.03 * fbm(p * 1.1, 2, 5108) * within;
            spot.mud = spot.mud.max(smoothstep(2.2, -0.5, from));
        }
        spot.mud = spot.mud.clamp(0.0, 1.0);
        spot.dust = (spot.dust * (1.0 - spot.mud)).clamp(0.0, 1.0);
        spot
    }

    /// The country outside the fine land: a red plain that rolls a little, a dry creek down
    /// its east side, a dune along its west, a stony rise in its north-west, and a station
    /// track south from the gate.
    fn country(&self, mut spot: Spot, at: Vec2, around: f32) -> Spot {
        let (x, z) = (at.x, at.y);
        let p = Vec3::new(x, 0.0, z);
        let plain = 1.5 * fbm(p * 0.007, 3, 5201) + 0.4 * fbm(p * 0.024, 3, 5202);
        // The creek: a sandy bed a truck's length wide, cut a metre and a half down.
        let bed = (x - creek(z)).abs();
        let cut = 1.0 - smoothstep(3.5, 11.0, bed);
        spot.sand = 1.0 - smoothstep(5.0, 13.0, bed);
        // The dune: one long ridge of red sand, steeper on its east face.
        let off = x - dune(z);
        let ridge = if off < 0.0 {
            (-(off / 22.0).powi(2)).exp()
        } else {
            (-(off / 11.0).powi(2)).exp()
        } * (0.75 + 0.25 * fbm(Vec3::new(0.0, 0.0, z * 0.02), 2, 5203));
        spot.sand = spot.sand.max(smoothstep(0.08, 0.4, ridge));
        // The rise: a stony knoll with ledges round it.
        let knoll = at.distance(RISE);
        let stone = smoothstep(52.0, 12.0, knoll);
        let ledges =
            (stone * 4.0).floor() / 4.0 + smoothstep(0.55, 0.95, (stone * 4.0).fract()) / 4.0;
        spot.rock = smoothstep(0.12, 0.4, stone)
            .max(0.8 * smoothstep(0.48, 0.66, fbm(p * 0.021, 3, 5204)) * (1.0 - spot.sand));
        // The station track: two wheel ruts' worth of bare clay from the gate to the edge.
        let track = (x - station(z)).abs();
        let worn = if z > FINE.1.y - 4.0 {
            1.0 - smoothstep(1.6, 3.4, track)
        } else {
            0.0
        };
        spot.height += around
            * (plain * (1.0 - 0.6 * worn) - 1.5 * cut
                + 5.5 * ridge
                + 8.0 * ledges
                + 0.06 * fbm(p * 0.4, 2, 5205));
        spot.grass = smoothstep(-0.05, 0.4, fbm(p * 0.035, 3, 5210))
            * (1.0 - spot.sand)
            * (1.0 - spot.rock)
            * (1.0 - worn);
        spot.dust = worn;
        spot
    }

    /// How far a place is from the middle of the ring road, or of a lane to a jump.
    fn off_road(&self, at: Vec2) -> f32 {
        let ring = (outside(at, FLOOR, CORNER) + ROAD_IN).abs();
        let lanes = JUMPS
            .iter()
            .map(|jump| {
                let (along, across) = jump.local(at);
                if (-40.0..jump.length() + 40.0).contains(&along) {
                    across.abs()
                } else {
                    f32::MAX
                }
            })
            .fold(f32::MAX, f32::min);
        ring.min(lanes)
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.spot(x, z).height
    }

    /// What a tyre is on at a place.
    pub fn surface(&self, x: f32, z: f32) -> Surface {
        let spot = self.spot(x, z);
        if self.hard(x, z) {
            Surface::Concrete
        } else if spot.bog > 0.25 {
            Surface::Bog
        } else if spot.water {
            Surface::Ford
        } else if spot.mud > 0.5 {
            Surface::Mud
        } else if spot.rock > 0.5 {
            Surface::Rock
        } else if spot.sand > 0.5 {
            Surface::Sand
        } else {
            Surface::Dirt
        }
    }

    /// Where a truck starts: inside the gate, on the ring road, looking up the arena.
    pub fn start(&self) -> (Vec3, f32) {
        let (at, _) = self.road[0];
        (Vec3::new(at.x, self.height(at.x, at.y), at.y), 0.0)
    }
}

/// Where the creek's bed is along x, at a place along z; and the dune's crest; and the
/// station track's line.
fn creek(z: f32) -> f32 {
    198.0 + 20.0 * (z * 0.021).sin() + 8.0 * (z * 0.053 + 1.0).sin()
}

fn dune(z: f32) -> f32 {
    -206.0 + 13.0 * (z * 0.017).sin()
}

fn station(z: f32) -> f32 {
    GATE + 7.0 * ((z - FINE.1.y) * 0.03).sin()
}

/// The middle of the stony rise.
const RISE: Vec2 = Vec2::new(-150.0, -196.0);

/// The ring road's line: a metre a point, from inside the gate, west along the south side
/// and so round, back to where it began.
fn road() -> Vec<(Vec2, Vec2)> {
    let half = FLOOR - Vec2::splat(ROAD_IN);
    let corner = CORNER - ROAD_IN;
    let (straight_x, straight_z) = (half.x - corner, half.y - corner);
    let quarter = std::f32::consts::FRAC_PI_2 * corner;
    // A straight, a point a metre from one place to another, the last left for what
    // follows; and a corner, a quarter turn about its middle from an angle, radians from +x
    // towards +z.
    fn line(points: &mut Vec<(Vec2, Vec2)>, from: Vec2, to: Vec2) {
        let along = (to - from).normalize();
        let steps = (to - from).length().round() as usize;
        points.extend((0..steps).map(|k| (from + along * k as f32, along)));
    }
    let turn = |points: &mut Vec<(Vec2, Vec2)>, middle: Vec2, from: f32| {
        let steps = quarter.round() as usize;
        points.extend((0..steps).map(|k| {
            let angle = from + std::f32::consts::FRAC_PI_2 * k as f32 / steps as f32;
            let out = Vec2::new(angle.cos(), angle.sin());
            (middle + out * corner, out.perp())
        }));
    };
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut points = Vec::new();
    line(
        &mut points,
        Vec2::new(GATE, half.y),
        Vec2::new(-straight_x, half.y),
    );
    turn(&mut points, Vec2::new(-straight_x, straight_z), FRAC_PI_2);
    line(
        &mut points,
        Vec2::new(-half.x, straight_z),
        Vec2::new(-half.x, -straight_z),
    );
    turn(&mut points, Vec2::new(-straight_x, -straight_z), PI);
    line(
        &mut points,
        Vec2::new(-straight_x, -half.y),
        Vec2::new(straight_x, -half.y),
    );
    turn(&mut points, Vec2::new(straight_x, -straight_z), 1.5 * PI);
    line(
        &mut points,
        Vec2::new(half.x, -straight_z),
        Vec2::new(half.x, straight_z),
    );
    turn(&mut points, Vec2::new(straight_x, straight_z), 0.0);
    line(
        &mut points,
        Vec2::new(straight_x, half.y),
        Vec2::new(GATE, half.y),
    );
    points
}

/// A plain material: paint, or steel.
fn plain(scene: &mut SceneAsset, colour: [f32; 3], roughness: f32, metallic: f32) -> u32 {
    scene.materials.push(Material {
        base_color: [colour[0], colour[1], colour[2], 1.0],
        roughness,
        metallic,
        ..Default::default()
    });
    scene.materials.len() as u32 - 1
}

/// A mesh put in the scene where it was made, of one material, colliding as said.
fn put(scene: &mut SceneAsset, mesh: &Mesh, material: u32, transform: Mat4, collide: Collide) {
    // (Whatever of the scene came before this collides as it was going to.)
    scene.collide.resize(scene.instances.len(), Collide::None);
    scene.instances.push(Instance {
        mesh: scene.meshes.len() as u32,
        material,
        transform,
    });
    scene.collide.push(collide);
    scene
        .meshes
        .push(build_virtual_mesh(mesh, &BuildOptions::default()));
}

/// The set dressing: each piece of the model by its name, where it stands and how it is
/// turned (a yaw in degrees), and what of it stops a truck: boxes, each its middle and its
/// size in the piece's own space.
struct Prop {
    piece: &'static str,
    at: Vec2,
    yaw: f32,
    /// Lifted this far off the ground it stands on.
    lift: f32,
}

/// What stops a truck at each kind of piece: boxes in the piece's own space, each its
/// middle and its full size. Measured from `tools/blender/arena/build.py`.
fn solid(piece: &str) -> &'static [([f32; 3], [f32; 3])] {
    match piece {
        "grandstand" => &[([0.0, 2.2, 0.0], [24.0, 4.4, 6.4])],
        "tower" => &[([0.0, 7.0, 0.0], [0.5, 14.0, 0.5])],
        "container" => &[([0.0, 1.3, 0.0], [6.06, 2.6, 2.44])],
        "stack" => &[([0.0, 0.9, 0.0], [1.5, 1.8, 1.5])],
        "tyre" => &[([0.0, 0.5, 0.0], [1.8, 1.0, 0.7])],
        "drum" => &[([0.0, 0.45, 0.0], [0.6, 0.9, 0.6])],
        "wreck" | "wreck_b" | "wreck_r" => &[
            ([0.0, 0.55, 0.0], [1.75, 1.1, 4.3]),
            ([0.0, 1.2, 0.3], [1.5, 0.35, 1.9]),
        ],
        "arch" => &[
            ([-GATE_HALF - 0.4, 3.5, 0.0], [0.7, 7.0, 0.7]),
            ([GATE_HALF + 0.4, 3.5, 0.0], [0.7, 7.0, 0.7]),
        ],
        "box" => &[([0.0, 3.2, 0.0], [5.0, 6.4, 4.0])],
        "windmill" => &[([0.0, 5.0, 0.0], [1.6, 10.0, 1.6])],
        "tank" => &[([0.0, 2.6, 0.0], [3.4, 5.2, 3.4])],
        "sign" => &[([0.0, 1.5, 0.0], [0.25, 3.0, 0.25])],
        _ => &[],
    }
}

impl Design {
    /// Where every piece of the set dressing stands.
    fn props(&self) -> Vec<Prop> {
        let mut props = Vec::new();
        let mut put = |piece: &'static str, x: f32, z: f32, yaw: f32| {
            props.push(Prop {
                piece,
                at: Vec2::new(x, z),
                yaw,
                lift: 0.0,
            });
        };
        // The gate: an arch over it with the ground's name on it, the announcer's box
        // beside it, and a stand each side of the gate, backed on to the bank.
        put("arch", GATE, FLOOR.y + 5.5, 0.0);
        put("box", GATE + 19.0, FLOOR.y - 1.6, 180.0);
        put("grandstand", -34.0, FLOOR.y - 2.4, 180.0);
        put("grandstand", 48.0, FLOOR.y - 2.4, 180.0);
        // Lamps on towers, on the bank's crest, all the way round.
        for (x, z) in [
            (-84.0, 98.2),
            (84.0, 98.2),
            (-84.0, -98.2),
            (84.0, -98.2),
            (-130.2, 40.0),
            (-130.2, -40.0),
            (130.2, 40.0),
            (130.2, -40.0),
        ] {
            // (Its lamps look along its own -z: it is turned to face the middle.)
            put("tower", x, z, x.atan2(z).to_degrees());
        }
        // The paddock, inside the gate to the west: containers, drums, stacks of tyres.
        put("container", -22.0, 72.5, 90.0);
        put("container", -22.0, 69.7, 90.0);
        put("container", -30.0, 71.1, 78.0);
        for (x, z) in [(-16.5, 74.0), (-15.6, 74.4), (-16.2, 73.1), (-26.0, 75.0)] {
            put("drum", x, z, 0.0);
        }
        // Tyres: stacks at the foot of each jump's lane, and half-buried ones along the
        // pits' edges and the loop's run-up, to steer by.
        for jump in &JUMPS {
            for side in [-1.0, 1.0] {
                let at = jump.along(-9.0) + jump.heading.perp() * (side * (jump.half + 3.2));
                put("stack", at.x, at.y, side * 20.0);
            }
        }
        let (middle, half, _) = WALLOW;
        for k in 0..7 {
            let x = middle.x - half.x + 3.0 + k as f32 * (2.0 * half.x - 6.0) / 6.0;
            for side in [-1.0, 1.0] {
                put("tyre", x, middle.y + side * (half.y + 2.6), 0.0);
            }
        }
        let (middle, half) = RUN;
        for k in 0..8 {
            let x = middle.x - half.x + 2.0 + k as f32 * (2.0 * half.x - 4.0) / 7.0;
            for side in [-1.0, 1.0] {
                put("tyre", x, middle.y + side * (half.y + 2.4), 0.0);
            }
        }
        for k in 0..7 {
            let (at, _) = self.ring.approach(14.0 + k as f32 * 10.0);
            for side in [-1.0, 1.0] {
                put("tyre", at.x, at.z + side * (ring::WIDE * 0.5 + 1.6), 0.0);
            }
        }
        // The loop's board: what speed to bring to it.
        let (at, _) = self.ring.approach(26.0);
        put("sign", at.x, at.z + ring::WIDE * 0.5 + 4.0, 90.0);
        // The wrecks: a row of them, side by side, to drive over; and another in the gap of
        // the big jump, to fly over.
        let (middle, count) = WRECKS;
        for k in 0..count {
            let x = middle.x + (k as f32 - (count - 1) as f32 * 0.5) * 2.05;
            let piece = ["wreck", "wreck_b", "wreck_r"][k % 3];
            put(piece, x, middle.y, if k % 2 == 0 { 0.0 } else { 180.0 });
        }
        let air = &JUMPS[4];
        for k in 0..6 {
            let at = air.along(17.6 + k as f32 * 2.1);
            let piece = ["wreck_r", "wreck", "wreck_b"][k % 3];
            put(piece, at.x, at.y, if k % 2 == 0 { 180.0 } else { 0.0 });
        }
        // Outside: the station's windmill and tank, by the track south of the gate.
        put("windmill", 26.0, 156.0, 30.0);
        put("tank", 33.0, 151.0, 0.0);
        props
    }

    /// What of the set dressing stops a truck.
    fn prop_blockers(&self) -> Vec<Blocker> {
        let mut blockers = Vec::new();
        for prop in self.props() {
            let foot = self.height(prop.at.x, prop.at.y) + prop.lift;
            let turn = Mat4::from_rotation_y(prop.yaw.to_radians());
            for (middle, size) in solid(prop.piece) {
                let off = turn.transform_vector3(Vec3::from_array(*middle));
                blockers.push(Blocker {
                    centre: Vec3::new(prop.at.x + off.x, foot + off.y, prop.at.y + off.z),
                    size: Vec3::from_array(*size),
                    yaw_degrees: prop.yaw,
                });
            }
        }
        blockers
    }
}

/// Plants the country outside the bank: scrub a long way apart, gums along the creek,
/// spinifex in rings and drifts, and stone on the rise. Returns what stops a truck, and
/// what it flattens, as a `.plants` file's lines.
fn plant(scene: &mut SceneAsset, design: &Design, stone: u32) -> (Vec<Blocker>, String) {
    let stock = flora::stock(scene);
    let mut blockers = Vec::new();
    let mut plants = String::from("# X Y Z REACH INSTANCE: what a car flattens\n");
    let reach = AROUND + 150.0;
    let hectares = (2.0 * reach).powi(2) / 10_000.0;
    // Each kind: how many are tried for a hectare, how far apart they stand, and how well
    // it likes a place, given how near the creek it is and how stony.
    type Liking = fn(f32, f32) -> f32;
    let kinds: [(&str, f32, f32, Liking); 6] = [
        // River gums, white-trunked, along the creek's banks and nowhere else.
        ("manna gum", 5.0, 13.0, |creek, _| {
            smoothstep(34.0, 20.0, creek) * smoothstep(7.0, 12.0, creek)
        }),
        ("red ironbark", 3.2, 16.0, |creek, stone| {
            0.35 + 0.4 * smoothstep(60.0, 25.0, creek) + 0.3 * stone
        }),
        // Mulga and its like: wattles, well apart.
        ("silver wattle", 7.0, 9.0, |_, _| 0.6),
        ("gum sapling", 5.0, 7.0, |creek, _| {
            0.25 + 0.6 * smoothstep(50.0, 15.0, creek)
        }),
        ("grass tree", 5.0, 6.0, |_, stone| 0.2 + 0.8 * stone),
        ("tea-tree", 16.0, 3.4, |creek, _| {
            0.3 + 0.5 * smoothstep(40.0, 12.0, creek)
        }),
    ];
    let mut taken: Vec<(Vec2, f32)> = Vec::new();
    for (which, (name, per_hectare, apart, liking)) in kinds.iter().enumerate() {
        let mut rng = Rng::new(131 + which as u32, 9);
        let sizes = flora::Stock::sizes(name);
        let mut placed = 0;
        for _ in 0..(per_hectare * hectares) as usize {
            let (x, z) = (rng.range(-reach, reach), rng.range(-reach, reach));
            let (chance, turn, size, copy) = (rng.unit(), rng.unit(), rng.unit(), rng.next_u32());
            let at = Vec2::new(x, z);
            // Not in the arena or on its bank, in the creek's bed, on the dune's sand or on
            // the station track.
            if outside(at, FLOOR, CORNER) < BANK_OUT + 6.0 {
                continue;
            }
            let spot = design.spot(x, z);
            let creek = (x - creek(z)).abs();
            if spot.sand > 0.3 || (x - station(z)).abs() < 6.0 && z > 0.0 {
                continue;
            }
            // In drifts, each kind its own, and thinner beyond what is driven on.
            let drift =
                0.45 + 0.9 * fbm(Vec3::new(x * 0.013, which as f32 * 7.0, z * 0.013), 2, 5301);
            let beyond = smoothstep(AROUND, reach, x.abs().max(z.abs()));
            if chance > liking(creek, spot.rock) * drift.clamp(0.0, 1.0) * (1.0 - 0.5 * beyond) {
                continue;
            }
            if taken
                .iter()
                .any(|&(other, room)| other.distance(at) < room.min(*apart))
            {
                continue;
            }
            taken.push((at, *apart));
            let scale = sizes.0 + (sizes.1 - sizes.0) * size;
            let transform = Mat4::from_translation(Vec3::new(x, spot.height - 0.06, z))
                * Mat4::from_rotation_y(turn * std::f32::consts::TAU)
                * Mat4::from_scale(Vec3::splat(scale));
            let Some((first, stood)) = stock.stand(scene, name, copy, transform, scale) else {
                continue;
            };
            placed += 1;
            if x.abs().max(z.abs()) >= AROUND {
                continue;
            }
            match stood {
                Stood::Scrub(reach) => {
                    let _ = writeln!(
                        plants,
                        "{x:.2} {:.2} {z:.2} {reach:.2} {first}",
                        spot.height
                    );
                }
                Stood::Trunk(width) => blockers.push(Blocker {
                    centre: Vec3::new(x, spot.height + 2.4, z),
                    size: Vec3::new(width, 5.0, width),
                    yaw_degrees: turn * 90.0,
                }),
                Stood::Nothing => {}
            }
        }
        println!("    {placed} {name}");
    }

    // Spinifex: tussock in patches, where the design says the ground is grassed.
    let size = stock.patch();
    let cells = (2.0 * (AROUND + 60.0) / size) as i32;
    let mut rng = Rng::new(157, 3);
    let mut tufts = 0;
    for j in 0..cells {
        for i in 0..cells {
            let (x, z) = (
                (i as f32 + 0.5) * size - AROUND - 60.0,
                (j as f32 + 0.5) * size - AROUND - 60.0,
            );
            let (chance, turn, copy) = (rng.unit(), rng.index(4), rng.next_u32());
            let at = Vec2::new(x, z);
            if outside(at, FLOOR, CORNER) < BANK_OUT + 2.0 {
                continue;
            }
            let spot = design.spot(x, z);
            if chance > 0.7 * spot.grass {
                continue;
            }
            let half = size * 0.5;
            let transform = Mat4::from_translation(Vec3::new(x, spot.height, z))
                * Mat4::from_rotation_y(turn as f32 * std::f32::consts::FRAC_PI_2)
                * Mat4::from_translation(Vec3::new(-half, 0.0, -half));
            stock.tussock(scene, copy, transform);
            tufts += 1;
        }
    }
    println!("    {tufts} patches of spinifex");

    // Stone: on the rise and wherever the ground is stony, and a few along the creek.
    let mut rng = Rng::new(167, 5);
    let mut stones = 0;
    for _ in 0..90_000 {
        let (x, z) = (
            rng.range(-AROUND - 40.0, AROUND + 40.0),
            rng.range(-AROUND - 40.0, AROUND + 40.0),
        );
        let (chance, pick, turn, tip, grow) = (
            rng.unit(),
            rng.unit(),
            rng.range(0.0, std::f32::consts::TAU),
            rng.range(-0.4, 0.4),
            rng.range(0.8, 1.25),
        );
        let at = Vec2::new(x, z);
        if outside(at, FLOOR, CORNER) < BANK_OUT + 4.0 {
            continue;
        }
        let spot = design.spot(x, z);
        let creek = (x - creek(z)).abs();
        let liking = 0.22 * smoothstep(0.4, 0.9, spot.rock)
            + 0.02 * smoothstep(13.0, 8.0, creek) * smoothstep(3.0, 6.0, creek);
        if chance > liking {
            continue;
        }
        let size = match pick {
            p if p < 0.5 => 0,
            p if p < 0.82 => 1,
            p if p < 0.95 => 2,
            _ => 3,
        };
        let transform = |radius: f32| {
            Mat4::from_translation(Vec3::new(x, spot.height - radius * 0.3, z))
                * Mat4::from_rotation_y(turn)
                * Mat4::from_rotation_x(tip)
                * Mat4::from_scale(Vec3::splat(grow))
        };
        // (Its size is known before it is set: the sizes are the stock's own.)
        let radius = [0.28, 0.6, 1.2, 2.3][size] * grow;
        stock.boulder(scene, size, rng.next_u32(), stone, transform(radius));
        stones += 1;
        if size >= 2 && x.abs().max(z.abs()) < AROUND {
            blockers.push(Blocker {
                centre: Vec3::new(x, spot.height + radius * 0.1, z),
                size: Vec3::new(radius * 1.5, radius * 1.1, radius * 1.5),
                yaw_degrees: turn.to_degrees(),
            });
        }
    }
    println!("    {stones} boulders");
    (blockers, plants)
}

/// The map of it: the floor and what is on it, the bank, and the country round it.
fn charted(design: &Design) -> chart::Chart {
    use chart::{mix, BUILT, CASING, INK, LINE, PAPER, ROAD, ROCK, TRACK, WATER};
    const SAND: chart::Colour = [0.93, 0.80, 0.62];
    const CLAY: chart::Colour = [0.90, 0.72, 0.56];
    const MUD: chart::Colour = [0.55, 0.40, 0.30];
    const STEEL: chart::Colour = [0.86, 0.30, 0.12];
    let half = AROUND + 12.0;
    let frame = design.ring.blockers();
    chart::paint(half, &|x, z, pace| {
        let at = Vec2::new(x, z);
        let spot = design.spot(x, z);
        let out = outside(at, FLOOR, CORNER);
        // Paper, tinted by what the ground is.
        let mut colour = if out < 0.0 {
            CLAY
        } else {
            mix(PAPER, SAND, 0.6)
        };
        colour = mix(colour, [0.82, 0.84, 0.66], spot.grass * 0.6);
        colour = mix(colour, ROCK, spot.rock * 0.8);
        colour = mix(colour, [0.96, 0.86, 0.66], spot.sand * 0.8);
        colour = mix(colour, MUD, spot.mud * 0.85);
        // Lit from the north-west, so that what is built of earth stands up off the paper.
        let (east, south) = (design.height(x + pace, z), design.height(x, z + pace));
        let (dx, dz) = ((east - spot.height) / pace, (south - spot.height) / pace);
        let lit = (0.55 * dx + 0.55 * dz) / (1.0 + dx * dx + dz * dz).sqrt();
        colour = mix(
            colour,
            if lit > 0.0 {
                [1.0, 0.98, 0.9]
            } else {
                [0.35, 0.30, 0.28]
            },
            (lit.abs() * 1.6).min(0.42),
        );
        // A contour every metre in the arena, where a metre is a jump; every five outside.
        let every = if out < BANK_OUT { 1.0 } else { 5.0 };
        let band = |height: f32| (height / every).floor();
        if band(spot.height) != band(east) || band(spot.height) != band(south) {
            colour = mix(colour, LINE, if out < BANK_OUT { 0.3 } else { 0.45 });
        }
        let grid = |a: f32| (a / 100.0).floor() != ((a + pace) / 100.0).floor();
        if grid(x - pace * 0.5) || grid(z - pace * 0.5) {
            colour = mix(colour, INK, 0.13);
        }
        if spot.water {
            colour = WATER;
        }
        // The ring road, cased and filled, and the station track, a broken line.
        if out < 0.0 {
            let off = (out + ROAD_IN).abs();
            if off < ROAD_HALF {
                colour = mix(colour, ROAD, 0.8);
            } else if off < ROAD_HALF + (1.2 * pace).max(0.8) {
                colour = CASING;
            }
        } else if z > FLOOR.y
            && (x - station(z)).abs() < (1.6 * pace).max(1.4)
            && (z / 9.0).fract() < 0.68
        {
            colour = TRACK;
        }
        // The loop: its deck, in the colour of its paint, between its two frames.
        if design.hard(x, z) {
            colour = STEEL;
        }
        let framed = frame.iter().take(2).any(|blocker| {
            let turn = blocker.yaw_degrees.to_radians();
            let off = at - Vec2::new(blocker.centre.x, blocker.centre.z);
            let along = Vec2::new(
                off.x * turn.cos() - off.y * turn.sin(),
                off.x * turn.sin() + off.y * turn.cos(),
            );
            along.x.abs() < blocker.size.x * 0.5 && along.y.abs() < (1.2 * pace).max(0.9)
        });
        if framed {
            colour = BUILT;
        }
        // What is built: the stands, the box, the containers.
        let built = design.props().iter().any(|prop| {
            matches!(prop.piece, "grandstand" | "box" | "container" | "tank")
                && solid(prop.piece).iter().any(|(_, size)| {
                    let turn = prop.yaw.to_radians();
                    let off = at - prop.at;
                    let along = Vec2::new(
                        off.x * turn.cos() - off.y * turn.sin(),
                        off.x * turn.sin() + off.y * turn.cos(),
                    );
                    along.x.abs() < size[0] * 0.5 && along.y.abs() < size[2] * 0.5
                })
        });
        if built {
            colour = BUILT;
        }
        // Beyond what is driven on the paper shows through, and a line marks the edge.
        let beyond = x.abs().max(z.abs()) - AROUND;
        if beyond > 0.0 {
            colour = mix(colour, PAPER, 0.55);
        }
        if beyond.abs() < pace * 0.9 {
            colour = mix(colour, INK, 0.7);
        }
        colour
    })
}

/// Builds the arena into the scene. `base` is the folder the scene's text is in.
pub fn build(scene: &mut SceneAsset, base: &Path) -> Result<Built, String> {
    println!("outback: {NAME}, a showground in red country");
    let design = Design::new();
    let height = |x: f32, z: f32| design.height(x, z);

    println!("  textures...");
    let first = scene.textures.len() as i32;
    for name in ["red_clay", "bush_mud", "sand", "tussock", "granite"] {
        scene
            .textures
            .push(crate::textures::surface(name, scene.texture_size));
    }
    let (clay, mud, sand, grass, rock) = (first, first + 1, first + 2, first + 3, first + 4);
    // The arena's own ground: hard red clay, with mud and loose dust on it. And the
    // country's: red sand, with dry grass and red stone.
    const DUST: [f32; 3] = [1.0, 0.66, 0.47];
    const STONE: [f32; 3] = [0.78, 0.5, 0.38];
    let floor = scene.materials.len() as u32;
    ground(scene, clay, [1.0, 0.93, 0.88], true);
    ground(scene, mud, [1.0; 3], false);
    scene.materials.last_mut().expect("just added").flags |= Material::MUD;
    ground(scene, sand, DUST, false);
    let country = scene.materials.len() as u32;
    ground(scene, sand, DUST, true);
    ground(scene, grass, [1.0, 0.95, 0.85], false);
    ground(scene, rock, STONE, false);
    let stone = country + 2;

    println!("  land...");
    let fine = grid(
        FINE.0,
        FINE.1,
        SMALL,
        &height,
        &|x, z| {
            let spot = design.spot(x, z);
            Vec2::new(spot.mud, spot.dust)
        },
        &|_, _| false,
    );
    let outer = |x: f32, z: f32| {
        let spot = design.spot(x, z);
        Vec2::new(spot.grass, spot.rock)
    };
    let around = grid(
        Vec2::splat(-AROUND),
        Vec2::splat(AROUND),
        COARSE,
        &height,
        &outer,
        // Not under the fine land.
        &|low, high| low.cmpge(FINE.0).all() && high.cmple(FINE.1).all(),
    );
    let beyond = grid(
        Vec2::splat(-BEYOND),
        Vec2::splat(BEYOND),
        COARSE * 8.0,
        &height,
        &outer,
        &|low, high| low.cmpge(Vec2::splat(-AROUND)).all() && high.cmple(Vec2::splat(AROUND)).all(),
    );
    let land = vec![(0, fine), (1, around), (1, beyond)];
    let triangles: usize = land.iter().map(|(_, mesh)| mesh.triangle_count()).sum();
    println!("    {} meshes, {triangles} triangles", land.len());
    draw::place(scene, land, [floor, country, country]);

    println!("  the loop...");
    let meshes = design.ring.meshes();
    let steel = plain(scene, [0.34, 0.35, 0.37], 0.55, 0.9);
    let paint = plain(scene, [0.93, 0.66, 0.08], 0.5, 0.2);
    let frame = plain(scene, [0.62, 0.13, 0.08], 0.6, 0.4);
    let placed = design.ring.placed;
    // The deck and its rails are driven on as they are drawn; the frame is looked at.
    put(scene, &meshes.deck, steel, placed, Collide::Auto);
    put(scene, &meshes.rails, paint, placed, Collide::Auto);
    put(scene, &meshes.frame, frame, placed, Collide::None);
    println!(
        "    {:.1} m of deck, {:.1} m high, {:.1} m wide between its rails, skewed {:.1} degrees",
        design.ring.length(),
        design.ring.height(),
        ring::WIDE,
        design.ring.skew().to_degrees()
    );

    println!("  set dressing...");
    let path = base.join(MODEL);
    if !path.exists() {
        return Err(format!(
            "{} is missing: the arena's set dressing is made by `blender -b --python tools/blender/arena/build.py -- target/outback/arena.glb`",
            path.display()
        ));
    }
    let options = ImportOptions {
        texture_size: scene.texture_size,
        ..Default::default()
    };
    let model = import_gltf(&path, &options).map_err(|e| format!("{}: {e}", path.display()))?;
    let props = design.props();
    let parts = props
        .iter()
        .map(|prop| {
            let foot = height(prop.at.x, prop.at.y) + prop.lift;
            (
                prop.piece.to_owned(),
                Mat4::from_translation(Vec3::new(prop.at.x, foot, prop.at.y))
                    * Mat4::from_rotation_y(prop.yaw.to_radians()),
            )
        })
        .collect();
    append_model(
        scene,
        model,
        &ModelPlacements {
            whole: Vec::new(),
            parts,
        },
    )?;
    // A lamp at the head of each tower, for the night.
    for prop in props.iter().filter(|prop| prop.piece == "tower") {
        let inward = -prop.at.normalize_or_zero() * 3.0;
        scene.lights.push(Light {
            position: [
                prop.at.x + inward.x,
                height(prop.at.x, prop.at.y) + 13.4,
                prop.at.y + inward.y,
            ],
            range: 95.0,
            intensity: [26.0, 24.0, 19.0],
            _pad: 0,
        });
    }

    println!("  planting...");
    let (mut blockers, plants) = plant(scene, &design, stone);
    blockers.extend(design.ring.blockers());
    blockers.extend(design.prop_blockers());

    // A map of it, among the game's pages, which are beside the scenes' folder.
    let pages = base.join("../ui");
    let chart = if pages.is_dir() {
        println!("  its map...");
        let drawn = charted(&design);
        let folder = pages.join("maps");
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let file = folder.join("bulldust-bowl.png");
        std::fs::write(&file, &drawn.png).map_err(|e| format!("{}: {e}", file.display()))?;
        format!(
            "# FILE HALF: the map of it among the pages, and how far out its edge is\nchart maps/bulldust-bowl.png {}\n",
            drawn.half
        )
    } else {
        String::new()
    };

    println!("  the ground to drive on...");
    let spacing = 0.5;
    let cells = (2.0 * AROUND / spacing).round() as u32;
    let heights: Vec<f32> = (0..(cells + 1) * (cells + 1))
        .into_par_iter()
        .map(|index| {
            let (column, row) = (index % (cells + 1), index / (cells + 1));
            height(
                column as f32 * spacing - AROUND,
                row as f32 * spacing - AROUND,
            )
        })
        .collect();
    let ground = Heightfield {
        columns: cells + 1,
        rows: cells + 1,
        origin: [-AROUND, -AROUND],
        spacing: [spacing, spacing],
        heights,
    };
    let map = SurfaceMap {
        origin: [-AROUND, -AROUND],
        spacing,
        columns: cells,
        rows: cells,
        squares: (0..cells * cells)
            .into_par_iter()
            .map(|index| {
                let (x, z) = (
                    ((index % cells) as f32 + 0.5) * spacing - AROUND,
                    ((index / cells) as f32 + 0.5) * spacing - AROUND,
                );
                design.surface(x, z).index()
            })
            .collect(),
    };

    // The ring road, as the map's track: every other point of its line.
    let samples: Vec<Sample> = design
        .road
        .iter()
        .step_by(2)
        .map(|(at, forward)| Sample {
            position: Vec3::new(at.x, height(at.x, at.y), at.y),
            left: Vec3::new(forward.y, 0.0, -forward.x),
            half_left: ROAD_HALF,
            half_right: ROAD_HALF,
            left_bands: Vec::new(),
            right_bands: Vec::new(),
        })
        .collect();
    let sectors = vec![samples.len() / 3, samples.len() * 2 / 3];
    let track = Track::new(NAME, samples, sectors)?;

    // A wall at each edge of the country that is driven on.
    for (x, z, long_x) in [
        (0.0, -AROUND, true),
        (0.0, AROUND, true),
        (-AROUND, 0.0, false),
        (AROUND, 0.0, false),
    ] {
        blockers.push(Blocker {
            centre: Vec3::new(x, LEVEL, z),
            size: if long_x {
                Vec3::new(2.0 * AROUND + 4.0, 400.0, 2.0)
            } else {
                Vec3::new(2.0, 400.0, 2.0 * AROUND + 4.0)
            },
            yaw_degrees: 0.0,
        });
    }
    // The wallow's water: a level sheet over its bowl.
    let (middle, half, _) = WALLOW;
    let ponds = vec![[
        middle.x - half.x,
        middle.y - half.y,
        middle.x + half.x,
        middle.y + half.y,
        LEVEL - WALLOW_WATER,
    ]];
    let (from, yaw) = design.ring.approach(90.0);
    println!(
        "  the loop is driven on to from {:.1} {:.1} {:.1}, facing {yaw:.1}",
        from.x,
        height(from.x, from.z),
        from.z
    );
    Ok(Built {
        ground,
        blockers,
        track: track.to_text(),
        surfaces: map.to_text(),
        start: design.start(),
        ponds,
        plants,
        items: String::new(),
        places: String::new(),
        chart,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_is_rounded_and_keeps_its_heights() {
        let table = JUMPS[1].profile;
        // Level before it and after it, and its top is its top.
        assert_eq!(eased(table, -3.0), 0.0);
        assert!((eased(table, 14.0) - 2.4).abs() < 1.0e-4);
        assert!(eased(table, 33.0).abs() < 1.0e-4);
        // The foot of every ramp is rounded: nowhere does a slope steepen sharply. (A lip
        // is sharp, and is meant to be.)
        for jump in &JUMPS {
            let slope = |a: f32| (eased(jump.profile, a + 0.25) - eased(jump.profile, a)) / 0.25;
            let mut along = -2.0;
            while along < jump.length() + 2.0 {
                let bend = slope(along + 0.5) - slope(along);
                assert!(bend < 0.8, "{} at {along}: {bend}", jump.name);
                along += 0.25;
            }
            // And its lip is where its profile says, as high as it says.
            let (lip, high) = jump
                .profile
                .windows(2)
                .find(|pair| pair[1].1 < pair[0].1)
                .map(|pair| pair[0])
                .expect("it has a lip");
            assert!(
                (eased(jump.profile, lip) - high).abs() < 1.0e-4,
                "{}",
                jump.name
            );
        }
    }

    #[test]
    fn the_arena_is_what_it_is_meant_to_be() {
        let design = Design::new();
        // The floor is level, the bank stands round it, and the gate is a way through it.
        assert!((design.height(0.0, 20.0) - LEVEL).abs() < 0.1);
        assert!(design.height(0.0, -FLOOR.y - 6.0) > LEVEL + BANK * 0.85);
        assert!(design.height(FLOOR.x + 6.0, 0.0) > LEVEL + BANK * 0.85);
        assert!((design.height(GATE, FLOOR.y + 6.0) - LEVEL).abs() < 0.15);
        // Each jump stands as high as it says, on its middle, and nothing off its side.
        for jump in &JUMPS {
            let top = jump.profile.iter().map(|point| point.1).fold(0.0, f32::max);
            let mut most = 0.0_f32;
            let mut along = 0.0;
            while along < jump.length() {
                let at = jump.along(along);
                most = most.max(design.height(at.x, at.y) - LEVEL);
                along += 0.5;
            }
            assert!((most - top).abs() < 0.25, "{}: {most} of {top}", jump.name);
            let beside = jump.along(jump.length() * 0.5) + jump.heading.perp() * (jump.half + 9.0);
            assert!(
                (design.height(beside.x, beside.y) - LEVEL).abs() < 0.1,
                "{}",
                jump.name
            );
        }
        // The wallow is a bog with water in its middle; the mud run is mud, with rollers.
        let (middle, half, deep) = WALLOW;
        assert_eq!(design.surface(middle.x, middle.y), Surface::Bog);
        assert!(design.spot(middle.x, middle.y).water);
        assert!(design.height(middle.x, middle.y) < LEVEL - deep * 0.8);
        assert_eq!(
            design.surface(middle.x - half.x + 4.0, middle.y),
            Surface::Bog
        );
        let (middle, _) = RUN;
        assert_eq!(design.surface(middle.x, middle.y), Surface::Mud);
        let (crest, dip) = (
            design.height(middle.x + ROLLERS.0 * 0.5, middle.y),
            design.height(middle.x, middle.y),
        );
        assert!(crest - dip > ROLLERS.1 * 0.8, "{crest} {dip}");
        // Under the loop is hard standing; the floor is dirt; the country has sand in it.
        let (on, _) = design.ring.on_map(2.0, 0.0);
        assert_eq!(design.surface(on.x, on.z), Surface::Concrete);
        assert_eq!(design.surface(0.0, 30.0), Surface::Dirt);
        assert_eq!(design.surface(creek(-150.0), -150.0), Surface::Sand);
        // The loop's deck starts on the floor, and its frames are inside the bank.
        assert!((on.y - LEVEL - PLATE).abs() < 0.01, "{on}");
        for blocker in design.ring.blockers() {
            let at = Vec2::new(blocker.centre.x, blocker.centre.z);
            assert!(outside(at, FLOOR, CORNER) < -ROAD_IN + ROAD_HALF, "{at}");
        }
    }

    #[test]
    fn the_land_is_level_where_fine_meets_coarse() {
        let design = Design::new();
        for k in 0..400 {
            let t = k as f32 / 399.0;
            let x = FINE.0.x + (FINE.1.x - FINE.0.x) * t;
            let z = FINE.0.y + (FINE.1.y - FINE.0.y) * t;
            for (x, z) in [(x, FINE.0.y), (x, FINE.1.y), (FINE.0.x, z), (FINE.1.x, z)] {
                assert_eq!(design.height(x, z), LEVEL, "at {x} {z}");
            }
            let at = -AROUND + 2.0 * AROUND * t;
            for (x, z) in [(at, -AROUND), (at, AROUND), (-AROUND, at), (AROUND, at)] {
                assert!((design.height(x, z) - LEVEL).abs() < 1.0e-4, "at {x} {z}");
            }
        }
    }

    #[test]
    fn the_ring_road_goes_round_and_nothing_stands_on_it() {
        let design = Design::new();
        let road = &design.road;
        for (k, (at, forward)) in road.iter().enumerate() {
            let (next, _) = road[(k + 1) % road.len()];
            let step = next - *at;
            assert!((0.5..1.6).contains(&step.length()), "at {k}: {step}");
            assert!(step.normalize().dot(*forward) > 0.9, "at {k}");
            // Level, clear of every jump and pit, and nothing built on it.
            assert!((design.height(at.x, at.y) - LEVEL).abs() < 0.12, "at {at}");
            assert_eq!(design.surface(at.x, at.y), Surface::Dirt, "at {at}");
        }
        for blocker in design.prop_blockers().iter().chain(&design.ring.blockers()) {
            let at = Vec2::new(blocker.centre.x, blocker.centre.z);
            let reach = blocker.size.x.max(blocker.size.z) * 0.5;
            let off = (outside(at, FLOOR, CORNER) + ROAD_IN).abs();
            let inside = outside(at, FLOOR, CORNER) < 0.0;
            assert!(!inside || off > ROAD_HALF + reach.min(3.5), "{at}: {off}");
        }
        // The truck starts on it, inside the gate, looking north.
        let (start, yaw) = design.start();
        assert!((start.x - GATE).abs() < 0.01 && start.z > FLOOR.y - ROAD_IN - 0.5);
        assert_eq!(yaw, 0.0);
    }
}
