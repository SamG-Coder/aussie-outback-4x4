//! What grows: the trees, their understorey, the grass, and the stone that stands among them,
//! each kind where it would be.
//!
//! A forest is not trees scattered evenly. Along the creek and up the damp gullies stand the
//! tall white manna gums, with silver wattle and tree ferns under them. The slopes carry the
//! messmate stringybark that most of Victoria's foothill forest is, over tea-tree and bracken.
//! The dry ridges thin out to ironbark, black-trunked, over grass trees and tussock. Clearings
//! are open grass. So every plant is placed by the ground it is on: how damp, how open.
//!
//! Each species is a text file beside this one (`species/`, `grass/`), in the form the
//! procedural library reads, so a tree is changed, or another added, by description.

use super::terrain::{smoothstep, Plan};
use glam::{Mat4, Vec2, Vec3};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use solstice_asset::{Instance, Material, SceneAsset};
use solstice_builder::{build_virtual_mesh, BuildOptions, Mesh, Vertex};
use solstice_game::Blocker;
use solstice_procgen::foliage::grass::Meadow;
use solstice_procgen::foliage::tree::Species;
use solstice_procgen::foliage::Rng;
use solstice_procgen::noise::fbm;

/// A kind of plant, and where it likes to stand.
struct Kind {
    name: &'static str,
    /// Its species, as the text of its file, and how many different ones of it are grown.
    species: &'static str,
    copies: u32,
    /// How well it likes ground this damp (0 a dry ridge, 1 the creek's bank), and this open
    /// (0 under trees, 1 a glade): each a range it is at home in, fading out beyond.
    damp: (f32, f32),
    open: (f32, f32),
    /// How many are tried for each hectare, how far apart two must stand, and how much room it
    /// leaves the track's edge.
    per_hectare: f32,
    apart: f32,
    clear: f32,
    /// How big each is, as a share of the species' own size: from, to.
    size: (f32, f32),
    /// How thick its trunk is at a car's height, in metres, if it is something a car stops at.
    trunk: Option<f32>,
    /// How far round it a car's body has to come to flatten it, in metres, if a car can: a
    /// bush, a fern, or a sapling whose stem is thinner than `snaps` once it is grown to its
    /// size. What a car flattens does not stop it.
    gives: Option<(f32, f32)>,
}

/// A plant a car can drive through and flatten: where it stands, how far round it reaches,
/// and the first of its two instances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breakable {
    pub at: Vec3,
    pub reach: f32,
    pub instance: u32,
}

/// What is planted, tallest first: a tree takes its place before the scrub under it.
const KINDS: &[Kind] = &[
    Kind {
        name: "manna gum",
        species: include_str!("species/manna_gum.species"),
        copies: 3,
        damp: (0.4, 1.0),
        open: (0.0, 0.55),
        per_hectare: 44.0,
        apart: 10.0,
        clear: 2.6,
        size: (0.72, 1.15),
        trunk: Some(0.78),
        gives: None,
    },
    Kind {
        // A candlebark (Eucalyptus rubida) is a manna gum's cousin of the slopes: the same
        // white trunk, half the height.
        name: "candlebark",
        species: include_str!("species/manna_gum.species"),
        copies: 2,
        damp: (0.14, 0.5),
        open: (0.0, 0.5),
        per_hectare: 46.0,
        apart: 8.0,
        clear: 2.2,
        size: (0.48, 0.78),
        trunk: Some(0.45),
        gives: None,
    },
    Kind {
        name: "messmate stringybark",
        species: include_str!("species/stringybark.species"),
        copies: 4,
        damp: (0.1, 0.6),
        open: (0.0, 0.4),
        per_hectare: 80.0,
        apart: 7.0,
        clear: 2.2,
        size: (0.7, 1.15),
        trunk: Some(0.6),
        gives: None,
    },
    Kind {
        name: "red ironbark",
        species: include_str!("species/ironbark.species"),
        copies: 3,
        damp: (0.0, 0.14),
        open: (0.0, 0.55),
        per_hectare: 46.0,
        apart: 6.5,
        clear: 2.0,
        size: (0.75, 1.2),
        trunk: Some(0.45),
        gives: None,
    },
    Kind {
        name: "gum sapling",
        species: include_str!("species/gum_sapling.species"),
        copies: 3,
        damp: (0.05, 0.85),
        open: (0.0, 0.7),
        per_hectare: 100.0,
        apart: 3.2,
        clear: 1.2,
        size: (0.55, 1.3),
        trunk: Some(0.14),
        gives: Some((0.5, 0.125)),
    },
    Kind {
        name: "silver wattle",
        species: include_str!("species/wattle.species"),
        copies: 3,
        damp: (0.28, 1.0),
        open: (0.0, 0.8),
        per_hectare: 70.0,
        apart: 4.2,
        clear: 1.4,
        size: (0.7, 1.25),
        trunk: Some(0.2),
        gives: Some((0.6, 0.17)),
    },
    Kind {
        name: "tree fern",
        species: include_str!("species/tree_fern.species"),
        copies: 3,
        damp: (0.66, 1.0),
        open: (0.0, 0.5),
        per_hectare: 80.0,
        apart: 2.8,
        clear: 1.2,
        size: (0.6, 1.4),
        trunk: Some(0.3),
        gives: None,
    },
    Kind {
        name: "tea-tree",
        species: include_str!("species/tea_tree.species"),
        copies: 3,
        damp: (0.1, 0.9),
        open: (0.0, 0.75),
        per_hectare: 190.0,
        apart: 2.3,
        clear: 0.9,
        size: (0.7, 1.5),
        trunk: None,
        gives: Some((0.8, 1.0)),
    },
    Kind {
        name: "grass tree",
        species: include_str!("species/grass_tree.species"),
        copies: 3,
        damp: (0.0, 0.22),
        open: (0.0, 0.9),
        per_hectare: 110.0,
        apart: 2.4,
        clear: 0.9,
        size: (0.55, 1.45),
        trunk: Some(0.26),
        gives: Some((0.55, 0.24)),
    },
    Kind {
        name: "bracken",
        species: include_str!("species/bracken.species"),
        copies: 3,
        damp: (0.2, 0.9),
        open: (0.0, 0.6),
        per_hectare: 520.0,
        apart: 1.1,
        clear: 0.3,
        size: (0.7, 1.4),
        trunk: None,
        gives: Some((0.45, 1.0)),
    },
];

/// The grasses, as the text of their files: tussock in the open, kangaroo grass under trees.
const TUSSOCK: &str = include_str!("grass/tussock.grass");
const KANGAROO_GRASS: &str = include_str!("grass/kangaroo_grass.grass");

/// How well a value suits a range: 1 inside it, fading to nothing a way beyond either end.
fn suits(value: f32, (low, high): (f32, f32)) -> f32 {
    smoothstep(low - 0.2, low, value) * (1.0 - smoothstep(high, high + 0.2, value))
}

/// How damp a place is for what grows there: wet where water gathers, and damper low in the
/// valley than up on its ridges.
fn damp(plan: &Plan, x: f32, z: f32, low: f32, high: f32) -> f32 {
    let height = plan.height(x, z);
    let valley = 1.0 - smoothstep(low + (high - low) * 0.12, low + (high - low) * 0.7, height);
    (plan.wet(x, z) * 1.5 + 0.58 * valley).min(1.0)
}

/// A species grown into a scene: its copies, each wood then leaves, and its two materials.
struct Grown {
    first_mesh: u32,
    copies: u32,
    bark: u32,
    leaves: u32,
}

/// Plants the square, and a way beyond it. `stone` is the material boulders are made of.
/// Returns what a car is stopped by, each tree's trunk and each great boulder, and what it
/// flattens instead: the scrub, the ferns and the thinnest saplings.
pub fn plant(scene: &mut SceneAsset, plan: &Plan, stone: u32) -> (Vec<Blocker>, Vec<Breakable>) {
    // The square's lowest and highest ground, for how high up its valley a place is.
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    let mut probe = -plan.half;
    while probe <= plan.half {
        let mut across = -plan.half;
        while across <= plan.half {
            let height = plan.height(probe, across);
            (low, high) = (low.min(height), high.max(height));
            across += 25.0;
        }
        probe += 25.0;
    }

    // Grow every species' copies, all at once.
    let species: Vec<Species> = KINDS
        .iter()
        .map(|kind| Species::parse(kind.species).unwrap_or_else(|e| panic!("{}: {e}", kind.name)))
        .collect();
    let jobs: Vec<(usize, u32)> = KINDS
        .iter()
        .enumerate()
        .flat_map(|(which, kind)| (0..kind.copies).map(move |copy| (which, copy)))
        .collect();
    let built: Vec<_> = jobs
        .par_iter()
        .map(|&(which, copy)| {
            let plant = species[which].grow(copy);
            let options = BuildOptions::default();
            (
                build_virtual_mesh(&plant.wood, &options),
                build_virtual_mesh(&plant.leaves, &options),
            )
        })
        .collect();
    let mut grown: Vec<Grown> = Vec::new();
    let mut built = built.into_iter();
    // Barks and leaves are shared by the species that have the same.
    let mut barks: FxHashMap<&'static str, u32> = FxHashMap::default();
    let mut foliage: FxHashMap<&'static str, u32> = FxHashMap::default();
    for (which, kind) in KINDS.iter().enumerate() {
        let first_mesh = scene.meshes.len() as u32;
        for _ in 0..kind.copies {
            let (wood, leaves) = built.next().expect("one for each copy");
            scene.meshes.push(wood);
            scene.meshes.push(leaves);
        }
        let texture_size = scene.texture_size;
        let bark = *barks.entry(species[which].bark.name()).or_insert_with(|| {
            scene
                .textures
                .push(species[which].bark.surface(0).bake(texture_size));
            scene
                .materials
                .push(crate::foliage::bark(scene.textures.len() as i32 - 1));
            scene.materials.len() as u32 - 1
        });
        let leaves = *foliage
            .entry(species[which].leaves.kind.name())
            .or_insert_with(|| {
                scene
                    .textures
                    .push(species[which].leaves.kind.atlas(0).bake(texture_size));
                scene
                    .materials
                    .push(crate::foliage::leaves(scene.textures.len() as i32 - 1));
                scene.materials.len() as u32 - 1
            });
        grown.push(Grown {
            first_mesh,
            copies: kind.copies,
            bark,
            leaves,
        });
    }

    let reach = plan.half + 170.0;
    let hectares = (2.0 * reach).powi(2) / 10_000.0;
    let mut blockers = Vec::new();
    let mut breakables = Vec::new();
    // What stands where already, by squares of ground, so that nothing stands in a trunk.
    let mut taken: FxHashMap<(i32, i32), Vec<(Vec2, f32)>> = FxHashMap::default();
    const SQUARE: f32 = 10.0;
    for (which, kind) in KINDS.iter().enumerate() {
        let mut rng = Rng::new(31 + which as u32, 7);
        let mut placed = 0;
        for _ in 0..(kind.per_hectare * hectares) as usize {
            let (x, z) = (rng.range(-reach, reach), rng.range(-reach, reach));
            let (chance, turn, size, copy) = (rng.unit(), rng.unit(), rng.unit(), rng.next_u32());
            let p = Vec2::new(x, z);
            // Not on the track, in water, on rock or on a cliff.
            if plan
                .trail
                .near(x, z)
                .is_some_and(|near| near.offset.abs() < plan.tread + kind.clear)
            {
                continue;
            }
            let cover = plan.cover(x, z);
            if cover.water || cover.rock > 0.5 || cover.tread > 0.0 || plan.slope(x, z) > 0.75 {
                continue;
            }
            // Thinner beyond the square, where nobody walks among them; and in drifts, each
            // kind its own, as plants seed round their parents.
            let beyond = smoothstep(plan.half, reach, x.abs().max(z.abs()));
            let drift =
                0.55 + 0.9 * fbm(Vec3::new(x * 0.017, which as f32 * 7.0, z * 0.017), 2, 771);
            let liking =
                suits(damp(plan, x, z, low, high), kind.damp) * suits(plan.open(x, z), kind.open);
            if chance > liking * drift.clamp(0.0, 1.0) * (1.0 - 0.6 * beyond) {
                continue;
            }
            let key = ((x / SQUARE).floor() as i32, (z / SQUARE).floor() as i32);
            let crowded = (-1..=1).any(|dz| {
                (-1..=1).any(|dx| {
                    taken.get(&(key.0 + dx, key.1 + dz)).is_some_and(|near| {
                        near.iter()
                            .any(|&(at, room)| at.distance(p) < room.min(kind.apart))
                    })
                })
            });
            if crowded {
                continue;
            }
            taken.entry(key).or_default().push((p, kind.apart));
            let scale = kind.size.0 + (kind.size.1 - kind.size.0) * size;
            let height = plan.height(x, z);
            let transform = Mat4::from_translation(Vec3::new(x, height - 0.06, z))
                * Mat4::from_rotation_y(turn * std::f32::consts::TAU)
                * Mat4::from_scale(Vec3::splat(scale));
            let plant = &grown[which];
            let first_instance = scene.instances.len() as u32;
            let mesh = plant.first_mesh + (copy % plant.copies) * 2;
            for (mesh, material) in [(mesh, plant.bark), (mesh + 1, plant.leaves)] {
                scene.instances.push(Instance {
                    mesh,
                    material,
                    transform,
                });
            }
            placed += 1;
            let inside = x.abs().max(z.abs()) < plan.half;
            // Scrub, ferns and whippy saplings go down under a truck; a trunk stops it.
            let flattened = kind
                .gives
                .filter(|&(_, snaps)| kind.trunk.is_none_or(|trunk| trunk * scale < snaps));
            if let (Some((reach, _)), true) = (flattened, inside) {
                breakables.push(Breakable {
                    at: Vec3::new(x, height, z),
                    reach: reach * scale,
                    instance: first_instance,
                });
            } else if let (Some(trunk), true) = (kind.trunk, inside) {
                let width = trunk * scale;
                blockers.push(Blocker {
                    centre: Vec3::new(x, height + 2.4, z),
                    size: Vec3::new(width, 5.0, width),
                    yaw_degrees: turn * 90.0,
                });
            }
        }
        println!("    {placed} {}", kind.name);
    }

    grass(scene, plan, low, high);
    blockers.extend(boulders(scene, plan, stone));
    // Fallen timber wears the stringybark's bark, lying still.
    if let Some(&bark) = barks.get("stringybark") {
        let lying = Material {
            flags: 0,
            ..scene.materials[bark as usize]
        };
        scene.materials.push(lying);
        let lying = scene.materials.len() as u32 - 1;
        blockers.extend(logs(scene, plan, lying));
    }
    (blockers, breakables)
}

/// A fallen trunk lying along x, its middle at the origin: a little crooked, a little
/// tapered, never quite round.
fn log_mesh(length: f32, radius: f32, seed: u32) -> Mesh {
    const ROUND: usize = 14;
    let rings = (length / 0.3).ceil() as usize;
    let mut mesh = Mesh::default();
    let bend = |t: f32| {
        Vec3::new(
            0.0,
            fbm(Vec3::new(t * 1.7, seed as f32, 0.0), 2, 791) * radius * 0.5,
            fbm(Vec3::new(t * 1.3, seed as f32, 5.0), 2, 792) * radius * 0.9,
        )
    };
    for ring in 0..=rings {
        let t = ring as f32 / rings as f32;
        let x = (t - 0.5) * length;
        let thick = radius * (1.12 - 0.3 * t);
        for k in 0..=ROUND {
            let angle = k as f32 / ROUND as f32 * std::f32::consts::TAU;
            let out = Vec3::new(0.0, angle.cos(), angle.sin());
            let lump = 1.0
                + 0.1
                    * fbm(
                        Vec3::new(x * 1.2, out.y * 1.5, out.z * 1.5 + seed as f32),
                        2,
                        793,
                    );
            mesh.vertices.push(Vertex {
                position: Vec3::new(x, 0.0, 0.0) + bend(t) + out * thick * lump,
                normal: out,
                // Bark is laid in metres: round it, and along it.
                uv: glam::Vec2::new(angle * radius, x),
            });
        }
    }
    let at = |ring: usize, k: usize| (ring * (ROUND + 1) + k) as u32;
    for ring in 0..rings {
        for k in 0..ROUND {
            mesh.indices.extend_from_slice(&[
                at(ring, k),
                at(ring, k + 1),
                at(ring + 1, k + 1),
                at(ring, k),
                at(ring + 1, k + 1),
                at(ring + 1, k),
            ]);
        }
    }
    // Its broken ends.
    for (ring, facing) in [(0, -1.0), (rings, 1.0)] {
        let t = ring as f32 / rings as f32;
        let middle = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            position: Vec3::new((t - 0.5) * length + facing * radius * 0.15, 0.0, 0.0) + bend(t),
            normal: Vec3::X * facing,
            uv: glam::Vec2::ZERO,
        });
        for k in 0..ROUND {
            let (a, b) = (at(ring, k), at(ring, k + 1));
            if facing > 0.0 {
                mesh.indices.extend_from_slice(&[middle, b, a]);
            } else {
                mesh.indices.extend_from_slice(&[middle, a, b]);
            }
        }
    }
    mesh
}

/// Fallen timber: a eucalypt forest's floor is strewn with it. Limbs and whole trunks lie
/// where they came down, through the bush and, here and there, across the track, where a car
/// has to climb over them. Returns what a car meets: every log thick enough to matter.
fn logs(scene: &mut SceneAsset, plan: &Plan, bark: u32) -> Vec<Blocker> {
    // Each its length and how thick it is at its butt.
    const SIZES: [(f32, f32); 4] = [(2.6, 0.085), (4.6, 0.15), (6.5, 0.21), (5.2, 0.3)];
    let first = scene.meshes.len() as u32;
    let meshes: Vec<_> = SIZES
        .par_iter()
        .enumerate()
        .map(|(seed, &(length, radius))| {
            build_virtual_mesh(
                &log_mesh(length, radius, seed as u32),
                &BuildOptions::default(),
            )
        })
        .collect();
    scene.meshes.extend(meshes);
    let mut blockers = Vec::new();
    let mut lay = |scene: &mut SceneAsset, size: usize, x: f32, z: f32, yaw: f32, stops: bool| {
        let (length, radius) = SIZES[size];
        // Lying on the slope it fell on: its two ends at the ground's height.
        let along = Vec2::new(yaw.cos(), -yaw.sin()) * (length * 0.5);
        let (a, b) = (
            plan.height(x - along.x, z - along.y),
            plan.height(x + along.x, z + along.y),
        );
        let middle = (a + b) * 0.5 + radius * 0.72;
        let tilt = ((b - a) / length).atan();
        scene.instances.push(Instance {
            mesh: first + size as u32,
            material: bark,
            transform: Mat4::from_translation(Vec3::new(x, middle, z))
                * Mat4::from_rotation_y(yaw)
                * Mat4::from_rotation_z(tilt),
        });
        if stops {
            blockers.push(Blocker {
                centre: Vec3::new(x, middle - radius * 0.1, z),
                size: Vec3::new(length * 0.94, radius * 1.7, radius * 1.7),
                yaw_degrees: yaw.to_degrees(),
            });
        }
    };

    // Across the track: on its gentler, drier stretches, well apart, each at its own angle.
    let trail = &plan.trail;
    let mut rng = Rng::new(71, 9);
    let (mut across, mut last) = (0, -1.0e9_f32);
    for point in &trail.points {
        let clear = trail
            .holes
            .iter()
            .all(|hole| trail_apart(plan, hole.along, point.along) > 30.0)
            && trail
                .washes
                .iter()
                .all(|&(wash, _)| trail_apart(plan, wash, point.along) > 30.0)
            && trail
                .bars
                .iter()
                .all(|&(bar, _)| trail_apart(plan, bar, point.along) > 14.0)
            && trail
                .moguls
                .iter()
                .all(|&(hole, _)| trail_apart(plan, hole, point.along) > 20.0);
        if point.along - last < 230.0
            || !clear
            || point.grade.abs() > 0.09
            || point.tilt.abs() > 0.05
            || point.mud > 0.3
            || point.rock > 0.2
            || point.erosion > 0.3
            || point.ford > 0.0
            || point.at.distance(plan.camp) < 60.0
            || across == 9
        {
            continue;
        }
        // The way the track runs, as a yaw, and the log at a slant across it.
        let heading = (-point.forward.y).atan2(point.forward.x);
        let slant = std::f32::consts::FRAC_PI_2 + rng.range(-0.45, 0.45);
        let size = 1 + rng.index(2);
        lay(scene, size, point.at.x, point.at.y, heading + slant, true);
        last = point.along;
        across += 1;
    }

    // Through the bush: where there are trees to have dropped them.
    let reach = plan.half + 40.0;
    let mut fallen = 0;
    for _ in 0..5200 {
        let (x, z) = (rng.range(-reach, reach), rng.range(-reach, reach));
        let (yaw, pick) = (rng.range(0.0, std::f32::consts::TAU), rng.unit());
        let cover = plan.cover(x, z);
        if cover.water
            || cover.rock > 0.4
            || plan.open(x, z) > 0.55
            || plan.slope(x, z) > 0.45
            || plan
                .trail
                .near(x, z)
                .is_some_and(|near| near.offset.abs() < plan.tread + 4.5)
        {
            continue;
        }
        let size = if pick < 0.5 {
            0
        } else if pick < 0.8 {
            1
        } else if pick < 0.94 {
            2
        } else {
            3
        };
        let inside = x.abs().max(z.abs()) < plan.half;
        lay(scene, size, x, z, yaw, size >= 1 && inside);
        fallen += 1;
    }
    println!("    {across} logs across the track, {fallen} through the bush");
    blockers
}

/// How far apart two places along the track are, the short way round, in metres.
fn trail_apart(plan: &Plan, from: f32, to: f32) -> f32 {
    let d = (to - from).rem_euclid(plan.trail.length);
    d.min(plan.trail.length - d)
}

/// A patch of grass laid on the ground at a place: sheared to the ground's slope, so that
/// every blade grows straight up from it, and turned about its middle.
fn patch(plan: &Plan, x: f32, z: f32, size: f32, turn: f32) -> Mat4 {
    let half = size * 0.5;
    let height = |x: f32, z: f32| plan.height(x, z);
    let slope_x = (height(x + half, z) - height(x - half, z)) / size;
    let slope_z = (height(x, z + half) - height(x, z - half)) / size;
    let shear = Mat4::from_cols(
        glam::Vec4::new(1.0, slope_x, 0.0, 0.0),
        glam::Vec4::Y,
        glam::Vec4::new(0.0, slope_z, 1.0, 0.0),
        glam::Vec4::W,
    );
    Mat4::from_translation(Vec3::new(x, height(x, z), z))
        * shear
        * Mat4::from_rotation_y(turn)
        * Mat4::from_translation(Vec3::new(-half, 0.0, -half))
}

/// The grass: tussock thick in the clearings and thin along the creek, kangaroo grass in
/// tufts under the open forest of the slopes and ridges.
fn grass(scene: &mut SceneAsset, plan: &Plan, low: f32, high: f32) {
    const COPIES: u32 = 3;
    let breeds = [("tussock", TUSSOCK), ("kangaroo grass", KANGAROO_GRASS)];
    let mut first = [0_u32; 2];
    let mut materials = [0_u32; 2];
    let mut size = 3.0;
    for (which, (name, text)) in breeds.iter().enumerate() {
        let meadow = Meadow::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        size = meadow.size;
        first[which] = scene.meshes.len() as u32;
        let meshes: Vec<_> = (0..COPIES)
            .into_par_iter()
            .map(|seed| {
                // In drifts and clumps, with bare ground between, each patch its own: grass
                // sown evenly is a paddock.
                let mesh = Meadow {
                    seed,
                    ..meadow.clone()
                }
                .mesh_where(&|u, v| {
                    let p = Vec3::new(u * 0.55, seed as f32 * 3.0 + which as f32 * 11.0, v * 0.55);
                    smoothstep(-0.28, 0.2, fbm(p, 3, 783))
                });
                build_virtual_mesh(&mesh, &BuildOptions::default())
            })
            .collect();
        scene.meshes.extend(meshes);
        scene
            .textures
            .push(meadow.atlas(0).bake(scene.texture_size));
        scene
            .materials
            .push(crate::foliage::blades(scene.textures.len() as i32 - 1));
        materials[which] = scene.materials.len() as u32 - 1;
    }
    let reach = plan.half + 90.0;
    let cells = (2.0 * reach / size) as i32;
    let mut rng = Rng::new(53, 3);
    let mut placed = [0; 2];
    for j in 0..cells {
        for i in 0..cells {
            let (x, z) = (
                (i as f32 + 0.5) * size - reach,
                (j as f32 + 0.5) * size - reach,
            );
            let (chance, turn, copy) = (rng.unit(), rng.index(4), rng.next_u32());
            if plan
                .trail
                .near(x, z)
                .is_some_and(|near| near.offset.abs() < plan.tread + size * 0.75)
            {
                continue;
            }
            let cover = plan.cover(x, z);
            if cover.water || cover.rock > 0.45 || cover.mud > 0.4 || plan.slope(x, z) > 0.6 {
                continue;
            }
            let open = plan.open(x, z);
            let dry = 1.0 - damp(plan, x, z, low, high);
            // Tussock where it is open; under trees, kangaroo grass where the ground is dry
            // enough for light to reach it, in drifts.
            let drift = smoothstep(-0.1, 0.35, fbm(Vec3::new(x * 0.03, 3.0, z * 0.03), 2, 781));
            let which = if chance < smoothstep(0.25, 0.6, open) {
                0
            } else if chance < 0.22 + 0.5 * dry * drift {
                1
            } else {
                continue;
            };
            scene.instances.push(Instance {
                mesh: first[which] + copy % COPIES,
                material: materials[which],
                transform: patch(plan, x, z, size, turn as f32 * std::f32::consts::FRAC_PI_2),
            });
            placed[which] += 1;
        }
    }
    println!(
        "    {} patches of tussock, {} of kangaroo grass",
        placed[0], placed[1]
    );
}

/// Stone: boulders where the land breaks into outcrop, and along the creek. The great ones
/// stop a car.
fn boulders(scene: &mut SceneAsset, plan: &Plan, stone: u32) -> Vec<Blocker> {
    // Each size is made at its size, so that its stone is at the right scale.
    const SIZES: [f32; 4] = [0.28, 0.6, 1.2, 2.3];
    const EACH: u32 = 3;
    let first = scene.meshes.len() as u32;
    let jobs: Vec<(usize, u32)> = (0..SIZES.len())
        .flat_map(|size| (0..EACH).map(move |seed| (size, seed)))
        .collect();
    let meshes: Vec<_> = jobs
        .par_iter()
        .map(|&(size, seed)| {
            let radius = SIZES[size];
            let mut mesh = crate::meshes::rock(
                if radius > 1.0 { 5 } else { 4 },
                60 + size as u32 * 7 + seed,
            );
            for vertex in &mut mesh.vertices {
                vertex.position *= radius;
                vertex.uv *= radius;
            }
            build_virtual_mesh(&mesh, &BuildOptions::default())
        })
        .collect();
    scene.meshes.extend(meshes);
    let mut rng = Rng::new(67, 5);
    let reach = plan.half + 60.0;
    let mut blockers = Vec::new();
    let mut placed = 0;
    for _ in 0..260_000 {
        let (x, z) = (rng.range(-reach, reach), rng.range(-reach, reach));
        let (chance, pick, turn, tip, grow) = (
            rng.unit(),
            rng.unit(),
            rng.range(0.0, std::f32::consts::TAU),
            rng.range(-0.4, 0.4),
            rng.range(0.8, 1.25),
        );
        let cover = plan.cover(x, z);
        if cover.water {
            continue;
        }
        let near = plan.trail.near(x, z).map(|near| near.offset.abs());
        // On outcrop, thick; in a scoured gully's bed and beside the track's ledges, small.
        let by_track = near.is_some_and(|offset| offset < plan.tread + 0.4);
        let liking = if by_track {
            cover.scour * 0.25
        } else {
            smoothstep(0.35, 0.9, cover.rock) * 0.5
                + smoothstep(0.75, 0.95, plan.wet(x, z)) * 0.05
                + near.map_or(0.0, |offset| 0.012 * (1.0 - smoothstep(2.0, 5.0, offset)))
        };
        if chance > liking {
            continue;
        }
        let size = if by_track {
            0
        } else if pick < 0.45 {
            0
        } else if pick < 0.78 {
            1
        } else if pick < 0.94 {
            2
        } else {
            3
        };
        // Nothing a car cannot drive over within its own width of the track.
        if size >= 2 && near.is_some_and(|offset| offset < plan.tread + 2.6) {
            continue;
        }
        let radius = SIZES[size] * grow;
        let height = plan.height(x, z);
        scene.instances.push(Instance {
            mesh: first + size as u32 * EACH + rng.index(EACH as usize) as u32,
            material: stone,
            transform: Mat4::from_translation(Vec3::new(x, height - radius * 0.3, z))
                * Mat4::from_rotation_y(turn)
                * Mat4::from_rotation_x(tip)
                * Mat4::from_scale(Vec3::splat(grow)),
        });
        placed += 1;
        if size >= 2 && x.abs().max(z.abs()) < plan.half {
            blockers.push(Blocker {
                centre: Vec3::new(x, height + radius * 0.1, z),
                size: Vec3::new(radius * 1.5, radius * 1.1, radius * 1.5),
                yaw_degrees: turn.to_degrees(),
            });
        }
    }
    println!("    {placed} boulders");
    blockers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plant_is_at_home_in_its_range_and_fades_out_beyond_it() {
        assert_eq!(suits(0.5, (0.3, 0.7)), 1.0);
        assert_eq!(suits(0.3, (0.3, 0.7)), 1.0);
        assert_eq!(suits(0.0, (0.3, 0.7)), 0.0);
        assert_eq!(suits(1.0, (0.3, 0.7)), 0.0);
        let edge = suits(0.8, (0.3, 0.7));
        assert!(edge > 0.0 && edge < 1.0);
        // A range that reaches an end takes everything to that end.
        assert_eq!(suits(0.0, (0.0, 0.4)), 1.0);
        assert_eq!(suits(1.0, (0.6, 1.0)), 1.0);
    }

    #[test]
    fn every_species_and_grass_reads_and_grows() {
        for kind in KINDS {
            let species =
                Species::parse(kind.species).unwrap_or_else(|e| panic!("{}: {e}", kind.name));
            let plant = species.grow(0);
            let top = plant
                .wood
                .vertices
                .iter()
                .map(|v| v.position.y)
                .fold(0.0, f32::max);
            let leaves = plant.leaves.triangle_count();
            println!(
                "{}: {top:.1} m tall, {leaves} leaf and {} wood triangles",
                kind.name,
                plant.wood.triangle_count()
            );
            assert!(top > 0.1 && leaves > 500, "{}", kind.name);
            assert!(leaves < 1_600_000, "{}: {leaves} leaf triangles", kind.name);
        }
        for text in [TUSSOCK, KANGAROO_GRASS] {
            assert!(Meadow::parse(text).unwrap().mesh().triangle_count() > 1000);
        }
    }
}
