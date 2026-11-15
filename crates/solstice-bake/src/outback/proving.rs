//! A proving ground for mud: four lanes of track side by side on a flat of bush, each a
//! different ground, joined end to end into one loop, as a scene file's `proving` asks.
//!
//! ```text
//!            the way back, hard clay
//!        .---------------------------------.
//!        |                                 |
//!        v        ^        |               ^
//!      1 bog    2 wet    3 puddle        4 dry
//!        |      clay       run           clay
//!        |        |        |               |
//!        '--------'        '---------------'
//! ```
//!
//! 1. **A bog**: forty metres of deep mud in a shallow bowl.
//! 2. **A wet clay track**: clay with wallows of mud across it.
//! 3. **A puddle run**: hard clay with five hollows of standing water, rimmed with mud.
//! 4. **Dry ground**: hard, dusty clay.
//!
//! It is for judging what tyres do to soft ground and what they throw (`docs/mud.md`): the
//! truck starts at the top of the bog, facing down it, and the loop is a track, so the game's
//! own driver can be asked to drive it. Nothing is planted on it, and it bakes in seconds.

use super::{draw, ground, ground_textures, Built};
use glam::{Vec2, Vec3};
use rayon::prelude::*;
use solstice_asset::{Heightfield, Material, SceneAsset};
use solstice_builder::{Mesh, Vertex};
use solstice_game::track::Sample;
use solstice_game::{Blocker, SurfaceMap, Track};
use solstice_procgen::noise::fbm;
use solstice_vehicle::Surface;

use super::terrain::smoothstep;

/// Where each lane's middle is along x, and where the lanes begin and end along z.
const LANES: [f32; 4] = [-24.0, -8.0, 8.0, 24.0];
const TOP: f32 = 30.0;
const BOTTOM: f32 = -30.0;
/// The radius of the turns that join them, and how far along z the way back runs.
const TURN: f32 = 8.0;
const BACK: f32 = 50.0;
/// Half the width of the track's tread.
const TREAD: f32 = 2.4;
/// How high the flat stands, so that nothing of it is below zero.
const LEVEL: f32 = 1.0;
/// The land drawn fine and driven on: its least and greatest x and z.
const FINE: (Vec2, Vec2) = (Vec2::new(-34.0, -46.0), Vec2::new(34.0, 58.0));
/// Metres between the fine land's points, and between the points of the bush round it.
const SMALL: f32 = 0.125;
const COARSE: f32 = 1.0;
/// How far out the bush round it is drawn at that spacing, where a wall stands, and how far
/// the country beyond is drawn, at eight times it.
const AROUND: f32 = 64.0;
const BEYOND: f32 = 640.0;
/// The puddles of the third lane: where each is along z, how far to the side of the lane's
/// middle, and how deep, in metres.
const PUDDLES: [(f32, f32, f32); 5] = [
    (21.0, -0.7, 0.17),
    (11.0, 0.6, 0.2),
    (0.0, 0.0, 0.22),
    (-10.0, -0.5, 0.18),
    (-20.0, 0.7, 0.2),
];
/// Half a puddle's hollow across and along, and how far below the ground round it its
/// water stands.
const PUDDLE: Vec2 = Vec2::new(1.7, 2.6);
const BRIM: f32 = 0.05;

/// Metres between the points of the loop's line.
const STEP: f32 = 0.5;

/// Adds the points of a straight from one place to another, the last left for what follows.
fn straight(points: &mut Vec<(Vec2, Vec2)>, from: Vec2, to: Vec2) {
    let along = (to - from).normalize();
    let steps = ((to - from).length() / STEP).round() as usize;
    points.extend((0..steps).map(|k| (from + along * (k as f32 * STEP), along)));
}

/// Adds the points of a turn about `centre`, from one angle to another: radians from +x
/// towards +z.
fn turn(points: &mut Vec<(Vec2, Vec2)>, centre: Vec2, from: f32, to: f32) {
    let steps = ((to - from).abs() * TURN / STEP).round() as usize;
    points.extend((0..steps).map(|k| {
        let angle = from + (to - from) * k as f32 / steps as f32;
        let out = Vec2::new(angle.cos(), angle.sin());
        (centre + out * TURN, out.perp() * (to - from).signum())
    }));
}

/// The loop's line: points half a metre apart, each with the way the track runs there.
fn line() -> Vec<(Vec2, Vec2)> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let [a, b, c, d] = LANES;
    let mut points = Vec::new();
    // Down the bog, round, up the wet clay, round, down the puddles, round, up the dry.
    straight(&mut points, Vec2::new(a, TOP), Vec2::new(a, BOTTOM));
    turn(&mut points, Vec2::new(a + TURN, BOTTOM), PI, 2.0 * PI);
    straight(&mut points, Vec2::new(b, BOTTOM), Vec2::new(b, TOP));
    turn(&mut points, Vec2::new(b + TURN, TOP), PI, 0.0);
    straight(&mut points, Vec2::new(c, TOP), Vec2::new(c, BOTTOM));
    turn(&mut points, Vec2::new(c + TURN, BOTTOM), PI, 2.0 * PI);
    straight(&mut points, Vec2::new(d, BOTTOM), Vec2::new(d, BACK - TURN));
    // And the way back across the top, to where it began.
    turn(
        &mut points,
        Vec2::new(d - TURN, BACK - TURN),
        0.0,
        FRAC_PI_2,
    );
    straight(
        &mut points,
        Vec2::new(d - TURN, BACK),
        Vec2::new(a + TURN, BACK),
    );
    turn(&mut points, Vec2::new(a + TURN, BACK - TURN), FRAC_PI_2, PI);
    straight(&mut points, Vec2::new(a, BACK - TURN), Vec2::new(a, TOP));
    points
}

/// What the ground is at a place.
#[derive(Clone, Copy, Debug, Default)]
struct Spot {
    height: f32,
    /// How much of the track's tread this is, and of what covers it how much is clay and
    /// how much mud: the rest is leaf litter.
    tread: f32,
    clay: f32,
    mud: f32,
    /// How much this is the bog, and whether it is under a puddle's water.
    bog: f32,
    water: bool,
}

/// The design: the loop's line, and what follows from it.
struct Ground {
    line: Vec<(Vec2, Vec2)>,
}

impl Ground {
    /// How far a place is from the middle of the track.
    fn off(&self, at: Vec2) -> f32 {
        self.line
            .iter()
            .map(|(point, _)| point.distance_squared(at))
            .fold(f32::MAX, f32::min)
            .sqrt()
    }

    /// How deep a puddle's hollow is at a place, in metres, and how near its middle the
    /// place is: 1 at it, 0 at its rim and beyond.
    fn hollow(at: Vec2) -> (f32, f32) {
        let mut found = (0.0_f32, 0.0_f32);
        for (z, aside, depth) in PUDDLES {
            let from = (at - Vec2::new(LANES[2] + aside, z)) / PUDDLE;
            let near = 1.0 - from.length();
            if near > 0.0 {
                // A dish: flat in its middle, easing up to its rim.
                let dish = smoothstep(0.0, 0.75, near);
                found = (found.0.max(depth * dish), found.1.max(near));
            }
        }
        found
    }

    fn spot(&self, x: f32, z: f32) -> Spot {
        let at = Vec2::new(x, z);
        let p = Vec3::new(x, 0.0, z);
        // Level at the edge of the fine land and of the bush round it, so that where one
        // spacing of points meets another every point lies in one plane and no gap opens.
        let fine_edge = (at - FINE.0).min(FINE.1 - at).min_element();
        let inside = smoothstep(1.0, 5.0, fine_edge);
        let reach = x.abs().max(z.abs());
        let around = smoothstep(1.0, 6.0, -fine_edge)
            * (1.0 - smoothstep(AROUND - 8.0, AROUND - 2.0, reach));
        let far = smoothstep(AROUND + 6.0, 380.0, reach);
        let hummocks = 0.05 * fbm(p * 0.21, 3, 4101) + 0.018 * fbm(p * 0.9, 2, 4102);
        let hills = far
            * far
            * (26.0 + 30.0 * fbm(p * 0.004, 4, 4103).abs() + 9.0 * fbm(p * 0.013, 3, 4104));
        let mut spot = Spot {
            height: LEVEL + hummocks * (inside + around) + hills,
            ..Spot::default()
        };
        if fine_edge <= 0.0 {
            return spot;
        }

        let edge = 0.25 * fbm(p * 0.7, 2, 4105);
        let off = self.off(at);
        let tread = 1.0 - smoothstep(TREAD - 0.45, TREAD + 0.45, off + edge);
        if tread <= 0.0 && off > TREAD + 2.0 {
            return spot;
        }
        // Which lane this is, if it is on one of them and not on a turn or the way back.
        let within =
            smoothstep(BOTTOM, BOTTOM + 5.0, z) * (1.0 - smoothstep(TOP - 9.0, TOP - 4.0, z));
        let lane = |index: usize| {
            within * (1.0 - smoothstep(TREAD + 0.2, TREAD + 1.4, (x - LANES[index]).abs()))
        };
        // The track is worn a little into the ground.
        spot.height -= 0.035 * tread;
        spot.tread = tread;

        // 1. The bog: a bowl of deep mud, churned and lumpy, that reaches a pace beyond
        // the tread either side.
        let bog = lane(0) * smoothstep(0.0, 0.35, 0.55 + 0.6 * fbm(p * 0.33, 2, 4106));
        if bog > 0.0 {
            let across = ((x - LANES[0]) / (TREAD + 0.9)).abs().min(1.0);
            let bowl = 1.0 - across.powi(3);
            let churned = 0.035 * fbm(p * 1.4, 3, 4107);
            spot.height -= bog * (0.2 * bowl - churned);
            spot.bog = bog * bowl;
            spot.mud = spot.mud.max(bog);
        }
        // 2. Wet clay: wallows of mud lying across the track where water sits.
        let wallows = lane(1) * tread * smoothstep(-0.3, 0.1, fbm(p * 0.3 + 31.0, 3, 4108));
        if wallows > 0.0 {
            spot.height -= 0.05 * wallows;
            spot.mud = spot.mud.max(0.8 * wallows);
        }
        // 3. The puddles: hollows in hard clay, each with mud round its rim.
        let (hollow, near) = Self::hollow(at);
        if near > 0.0 {
            spot.height -= hollow;
            spot.water = hollow > BRIM + 0.004;
            spot.mud = spot.mud.max(smoothstep(0.0, 0.3, near));
        }
        // 4. Dry ground is clay and nothing else.
        spot.clay = (tread * (1.0 - spot.mud)).clamp(0.0, 1.0);
        spot.mud = spot.mud.clamp(0.0, 1.0);
        spot
    }

    /// What a tyre is on at a place.
    fn surface(&self, x: f32, z: f32) -> Surface {
        let spot = self.spot(x, z);
        if spot.water {
            Surface::Ford
        } else if spot.bog > 0.25 {
            Surface::Bog
        } else if spot.mud > 0.5 {
            Surface::Mud
        } else if spot.tread > 0.5 {
            Surface::Dirt
        } else {
            Surface::Litter
        }
    }
}

/// A grid of points from `low` to `high`, `spacing` apart, as a mesh: each point's height
/// and facing from `height`, and its two shares from `shares`. Squares `skip` says to
/// leave out are left out.
fn grid(
    low: Vec2,
    high: Vec2,
    spacing: f32,
    height: &(dyn Fn(f32, f32) -> f32 + Sync),
    shares: &(dyn Fn(f32, f32) -> Vec2 + Sync),
    skip: &(dyn Fn(Vec2, Vec2) -> bool + Sync),
) -> Mesh {
    let columns = ((high.x - low.x) / spacing).round() as usize;
    let rows = ((high.y - low.y) / spacing).round() as usize;
    let place = |i: usize, j: usize| (low.x + i as f32 * spacing, low.y + j as f32 * spacing);
    let vertices: Vec<Vertex> = (0..(columns + 1) * (rows + 1))
        .into_par_iter()
        .map(|index| {
            let (x, z) = place(index % (columns + 1), index / (columns + 1));
            let reach = spacing.min(0.5);
            let slope_x = (height(x + reach, z) - height(x - reach, z)) / (2.0 * reach);
            let slope_z = (height(x, z + reach) - height(x, z - reach)) / (2.0 * reach);
            Vertex {
                position: Vec3::new(x, height(x, z), z),
                normal: Vec3::new(-slope_x, 1.0, -slope_z).normalize(),
                uv: shares(x, z),
            }
        })
        .collect();
    let mut mesh = Mesh {
        vertices,
        indices: Vec::new(),
    };
    let at = |i: usize, j: usize| (j * (columns + 1) + i) as u32;
    for j in 0..rows {
        for i in 0..columns {
            let (x, z) = place(i, j);
            if skip(Vec2::new(x, z), Vec2::new(x + spacing, z + spacing)) {
                continue;
            }
            mesh.indices.extend_from_slice(&[
                at(i, j),
                at(i, j + 1),
                at(i + 1, j + 1),
                at(i, j),
                at(i + 1, j + 1),
                at(i + 1, j),
            ]);
        }
    }
    mesh
}

/// Builds the proving ground into the scene.
pub fn build(scene: &mut SceneAsset) -> Result<Built, String> {
    println!("outback: a proving ground for mud");
    let design = Ground { line: line() };
    let height = |x: f32, z: f32| design.spot(x, z).height;

    println!("  textures...");
    let first = ground_textures(scene);
    let (litter, clay, mud, grass, rock) = (first, first + 1, first + 2, first + 3, first + 4);
    // The track's ground: leaf litter, clay and mud. And the bush's: litter, grass, rock.
    let mut sets = [0_u32; 3];
    for (set, pair) in [[clay, mud], [clay, rock], [grass, rock]]
        .iter()
        .enumerate()
    {
        sets[set] = scene.materials.len() as u32;
        ground(scene, litter, [1.0, 0.96, 0.9], true);
        for &layer in pair {
            ground(scene, layer, [1.0; 3], false);
            if layer == mud {
                scene.materials.last_mut().expect("just added").flags |= Material::MUD;
            }
        }
    }

    println!("  land...");
    let none = |_: f32, _: f32| Vec2::ZERO;
    // Dry grass in the bush round about, in patches, and none where it meets the track's
    // ground, which has none to meet it with.
    let grassy = |x: f32, z: f32| {
        let at = Vec2::new(x, z);
        let from_fine = -(at - FINE.0).min(FINE.1 - at).min_element();
        let patches = smoothstep(-0.1, 0.35, fbm(Vec3::new(x, 0.0, z) * 0.06, 3, 4109));
        Vec2::new(patches * smoothstep(1.5, 7.0, from_fine), 0.0)
    };
    let fine = grid(
        FINE.0,
        FINE.1,
        SMALL,
        &height,
        &|x, z| {
            let spot = design.spot(x, z);
            let both = (spot.clay + spot.mud).max(1.0);
            Vec2::new(spot.clay / both, spot.mud / both)
        },
        &|_, _| false,
    );
    let around = grid(
        Vec2::splat(-AROUND),
        Vec2::splat(AROUND),
        COARSE,
        &height,
        &grassy,
        // Not under the fine land.
        &|low, high| low.cmpge(FINE.0).all() && high.cmple(FINE.1).all(),
    );
    let beyond = grid(
        Vec2::splat(-BEYOND),
        Vec2::splat(BEYOND),
        COARSE * 8.0,
        &height,
        &none,
        &|low, high| low.cmpge(Vec2::splat(-AROUND)).all() && high.cmple(Vec2::splat(AROUND)).all(),
    );
    let land = vec![(0, fine), (2, around), (2, beyond)];
    let triangles: usize = land.iter().map(|(_, mesh)| mesh.triangle_count()).sum();
    println!("    {} meshes, {triangles} triangles", land.len());
    draw::place(scene, land, sets);

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
    // What each half metre of the fine land is made of; beyond it, the nearest of it, which
    // is the bush's floor.
    let (columns, rows) = (
        ((FINE.1.x - FINE.0.x) / spacing).round() as u32,
        ((FINE.1.y - FINE.0.y) / spacing).round() as u32,
    );
    let map = SurfaceMap {
        origin: FINE.0.to_array(),
        spacing,
        columns,
        rows,
        squares: (0..columns * rows)
            .into_par_iter()
            .map(|index| {
                let (x, z) = (
                    FINE.0.x + ((index % columns) as f32 + 0.5) * spacing,
                    FINE.0.y + ((index / columns) as f32 + 0.5) * spacing,
                );
                design.surface(x, z).index()
            })
            .collect(),
    };

    // The loop, as a track: every other point of its line.
    let samples: Vec<Sample> = design
        .line
        .iter()
        .step_by(2)
        .map(|(at, forward)| Sample {
            position: Vec3::new(at.x, height(at.x, at.y), at.y),
            left: Vec3::new(forward.y, 0.0, -forward.x),
            half_left: TREAD,
            half_right: TREAD,
            left_bands: Vec::new(),
            right_bands: Vec::new(),
        })
        .collect();
    let sectors = vec![samples.len() / 3, samples.len() * 2 / 3];
    let track = Track::new("Mud Proving Ground", samples, sectors)?;

    // A wall at each edge of the bush that is driven on.
    let blockers = [
        (0.0, -AROUND, true),
        (0.0, AROUND, true),
        (-AROUND, 0.0, false),
        (AROUND, 0.0, false),
    ]
    .into_iter()
    .map(|(x, z, long_x)| Blocker {
        centre: Vec3::new(x, LEVEL, z),
        size: if long_x {
            Vec3::new(2.0 * AROUND + 4.0, 400.0, 2.0)
        } else {
            Vec3::new(2.0, 400.0, 2.0 * AROUND + 4.0)
        },
        yaw_degrees: 0.0,
    })
    .collect();
    // Each puddle's water: a level sheet over its hollow, a little below its rim.
    let ponds = PUDDLES
        .iter()
        .map(|&(z, aside, _)| {
            let middle = Vec2::new(LANES[2] + aside, z);
            let level = height(middle.x + PUDDLE.x + 0.4, z) - 0.035 - BRIM;
            [
                middle.x - PUDDLE.x,
                middle.y - PUDDLE.y,
                middle.x + PUDDLE.x,
                middle.y + PUDDLE.y,
                level,
            ]
        })
        .collect();
    let start = Vec3::new(LANES[0], height(LANES[0], TOP), TOP);
    Ok(Built {
        ground,
        blockers,
        track: track.to_text(),
        surfaces: map.to_text(),
        start: (start, 0.0),
        ponds,
        plants: String::new(),
        items: String::new(),
        places: String::new(),
        chart: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loop_closes_and_runs_down_each_lane() {
        let line = line();
        // Half a metre a step, all the way round to where it began.
        for (k, (at, forward)) in line.iter().enumerate() {
            let (next, _) = line[(k + 1) % line.len()];
            let step = next - *at;
            assert!((0.3..0.7).contains(&step.length()), "at {k}: {step}");
            assert!(step.normalize().dot(*forward) > 0.9, "at {k}");
        }
        for (lane, down) in LANES.into_iter().zip([-1.0, 1.0, -1.0, 1.0]) {
            let on = line
                .iter()
                .find(|(at, _)| (at.x - lane).abs() < 0.01 && at.y.abs() < 0.3)
                .expect("the line runs down the lane");
            assert!((on.1.y - down).abs() < 1.0e-3);
        }
    }

    #[test]
    fn each_lane_is_the_ground_it_is_meant_to_be() {
        let design = Ground { line: line() };
        // Down the middle of each lane, what a tyre is on.
        let along = |lane: usize, aside: f32| -> Vec<Surface> {
            (-20..=20)
                .map(|z| design.surface(LANES[lane] + aside, z as f32))
                .collect()
        };
        let count = |surfaces: &[Surface], surface: Surface| {
            surfaces.iter().filter(|&&s| s == surface).count()
        };
        let bog = along(0, 0.87);
        assert!(count(&bog, Surface::Bog) > 30, "{bog:?}");
        let wet = along(1, 0.87);
        assert!(
            count(&wet, Surface::Mud) > 8 && count(&wet, Surface::Dirt) > 8,
            "{wet:?}"
        );
        let puddles = along(2, 0.0);
        assert!(count(&puddles, Surface::Ford) >= 8, "{puddles:?}");
        assert!(count(&puddles, Surface::Dirt) > 10);
        let dry = along(3, 0.87);
        assert_eq!(count(&dry, Surface::Dirt), dry.len());
        // Off the track, the bush's floor.
        assert_eq!(design.surface(16.0, 0.0), Surface::Litter);
        // The bog is a bowl, and a puddle's water lies below its rim.
        assert!(design.spot(LANES[0], 0.0).height < LEVEL - 0.12);
        let (z, aside, depth) = PUDDLES[2];
        let floor = design.spot(LANES[2] + aside, z);
        assert!(floor.water && floor.height < LEVEL - depth * 0.8);
    }

    #[test]
    fn the_land_is_level_where_fine_meets_coarse() {
        let design = Ground { line: line() };
        // Along each edge of the fine land, and of the bush round it: one height.
        for k in 0..200 {
            let t = k as f32 / 199.0;
            let x = FINE.0.x + (FINE.1.x - FINE.0.x) * t;
            let z = FINE.0.y + (FINE.1.y - FINE.0.y) * t;
            for (x, z) in [(x, FINE.0.y), (x, FINE.1.y), (FINE.0.x, z), (FINE.1.x, z)] {
                assert_eq!(design.spot(x, z).height, LEVEL, "at {x} {z}");
            }
            let at = -AROUND + 2.0 * AROUND * t;
            for (x, z) in [(at, -AROUND), (at, AROUND), (-AROUND, at), (AROUND, at)] {
                assert!(
                    (design.spot(x, z).height - LEVEL).abs() < 1.0e-4,
                    "at {x} {z}"
                );
            }
        }
    }
}
