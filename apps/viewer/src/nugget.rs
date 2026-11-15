//! Nuggets, made as the game is played.
//!
//! No map and no file has a nugget in it. A nugget is made when it is found: its shape, what
//! it is made of and its colours are all worked out here from the number it was dug up with
//! (`gold::Nugget::shape`), and it is that nugget every time it is made again.
//!
//! **What a scene has for them.** A scene's geometry is fixed when it is opened, so a map that
//! is to be played is given, as it is read, [`SLOTS`] blank lumps to show nuggets with
//! ([`join_loaded`], [`join_streamed`]): each a ball of some thousands of triangles, built
//! here by the same builder a bake uses, with nothing coarser made from it, so that all of it
//! is in pages that are always loaded. Its bounds are as wide as any nugget will be. A
//! gameplay file, [`FILE`], says which instance and materials each is, and where in which
//! pages its points are.
//!
//! **What a nugget is.** [`Lump::of`] makes one: for each point of the ball, how far out the
//! nugget's skin is that way (lobes, knobs and pits, from noise that is the nugget's own),
//! then the whole drawn out, flattened, pinched, bent and twisted as its `gold::Form` says;
//! and at each point how much of what is seen is clean gold, clay or rock. The game writes
//! those points over the blank's ([`solstice_render::Renderer::rewrite_page`]) and gives its
//! three materials the nugget's colours: its gold, the clay in its hollows, and its rock,
//! which are blended by the shares the points hold (`Material::BLENDS`, with no texture).
use crate::gold::Form;
use glam::{Mat4, Quat, Vec3};
use solstice_asset::gpu::encode_normal;
use solstice_asset::{
    GameFile, Instance, LoadedScene, Material, PackedVertex, PagedScene, VirtualMesh,
};
use std::fmt::Write as _;
use std::sync::OnceLock;

/// How many nuggets can be seen at once: on the ground, in the truck and in the hand.
pub const SLOTS: usize = 40;

/// The gameplay file that says what a scene has to show nuggets with. A `slot INSTANCE
/// MATERIAL` line for each blank, the first of its three materials; and after it a `write
/// PAGE OFFSET FIRST COUNT` line for each run of its points: `COUNT` of them from the
/// `FIRST` of a [`Lump`]'s are `OFFSET` bytes into page `PAGE`.
pub const FILE: &str = "nugget.slots";

/// How far from its middle any point of a nugget can be, in its own lengths: the blanks'
/// bounds, and the grid their points are kept on.
const REACH: f32 = 2.5;

/// How many times the ball's twenty faces are each cut in four: 5,120 triangles.
const CUTS: u32 = 4;

/// The radius of the ball, which a nugget's own lengths are measured against: a plain one
/// is a length across.
const BALL: f32 = 0.5;

/// A ball of triangles, each much like the next: an icosahedron, cut and cut again.
fn ball() -> solstice_builder::Mesh {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut points: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..CUTS {
        let mut between = std::collections::HashMap::new();
        let mut middle = |a: u32, b: u32, points: &mut Vec<Vec3>| {
            *between.entry((a.min(b), a.max(b))).or_insert_with(|| {
                points.push(((points[a as usize] + points[b as usize]) * 0.5).normalize());
                points.len() as u32 - 1
            })
        };
        let mut cut = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let (ab, bc, ca) = (
                middle(a, b, &mut points),
                middle(b, c, &mut points),
                middle(c, a, &mut points),
            );
            cut.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = cut;
    }
    solstice_builder::Mesh {
        vertices: points
            .iter()
            .map(|&way| solstice_builder::Vertex {
                position: way * BALL,
                normal: way,
                uv: glam::Vec2::new(way.x, way.z) * 0.5 + 0.5,
            })
            .collect(),
        indices: faces.into_iter().flatten().collect(),
    }
}

/// The blank every nugget is made from: the ball as the builder cut it into clusters, and
/// which way out from the middle each of its points is.
struct Blank {
    mesh: VirtualMesh,
    /// The ways out there are: one for each place a point is, however many clusters have a
    /// point there.
    ways: Vec<Vec3>,
    /// Which of those each of the mesh's points is, in the order the mesh has them.
    which: Vec<u32>,
}

fn blank() -> &'static Blank {
    static BLANK: OnceLock<Blank> = OnceLock::new();
    BLANK.get_or_init(|| {
        let options = solstice_builder::BuildOptions {
            flat: true,
            ..Default::default()
        };
        let mesh = solstice_builder::build_virtual_mesh(&ball(), &options);
        let mut places = std::collections::HashMap::new();
        let mut ways = Vec::new();
        let which = mesh
            .vertices
            .iter()
            .map(|vertex| {
                let grid = [vertex.data[0], vertex.data[1], vertex.data[2]];
                *places.entry(grid).or_insert_with(|| {
                    let at = mesh.quant_origin
                        + Vec3::new(grid[0].into(), grid[1].into(), grid[2].into())
                            * mesh.quant_step;
                    ways.push(at.normalize_or(Vec3::Y));
                    ways.len() as u32 - 1
                })
            })
            .collect();
        Blank { mesh, ways, which }
    })
}

/// One of a scene's blanks: what draws it, and where its points are to be written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub instance: u32,
    /// The first of its three materials.
    pub material: u32,
    /// Each run of its points: the page, how many bytes into it, the first of a [`Lump`]'s
    /// points that goes there, and how many.
    pub writes: Vec<(u32, u32, u32, u32)>,
}

impl Slot {
    /// The slots a scene's gameplay files say it has.
    pub fn read(files: &[GameFile]) -> Vec<Slot> {
        let mut slots: Vec<Slot> = Vec::new();
        for file in files.iter().filter(|file| file.name == FILE) {
            for line in file.text.lines() {
                let mut words = line.split_whitespace();
                let what = words.next();
                let numbers: Vec<u32> = words.filter_map(|word| word.parse().ok()).collect();
                match (what, &numbers[..], slots.last_mut()) {
                    (Some("slot"), &[instance, material], _) => slots.push(Slot {
                        instance,
                        material,
                        writes: Vec::new(),
                    }),
                    (Some("write"), &[page, offset, first, count], Some(slot)) => {
                        slot.writes.push((page, offset, first, count));
                    }
                    _ => {}
                }
            }
        }
        slots
    }
}

/// A scene of nothing but blanks, all of it in pages that are always loaded, and its slots
/// by its own numbers.
fn blanks() -> (LoadedScene, Vec<Slot>) {
    let blank = blank();
    let step = 2.0 * REACH / 65535.0;
    let mut asset = solstice_asset::SceneAsset::default();
    // Put away far under the ground until one is wanted.
    let away = Mat4::from_translation(Vec3::new(0.0, -900.0, 0.0));
    for slot in 0..SLOTS {
        let mut mesh = blank.mesh.clone();
        // A page keeps one copy of points that are byte for byte the same, and every blank's
        // are. Each must have its own, to be written over alone: so each is marked.
        for vertex in &mut mesh.vertices {
            vertex.data[4] = 0;
            vertex.data[5] = slot as u16;
        }
        // Whatever nugget it becomes is within this of its middle.
        for cluster in &mut mesh.clusters {
            (cluster.center, cluster.radius) = ([0.0; 3], REACH);
            (cluster.lod_center, cluster.lod_radius) = ([0.0; 3], REACH);
        }
        for node in &mut mesh.nodes {
            (node.center, node.radius) = ([0.0; 3], REACH);
            (node.lod_center, node.lod_radius) = ([0.0; 3], REACH);
        }
        (mesh.bounds_center, mesh.bounds_radius) = (Vec3::ZERO, REACH);
        (mesh.quant_origin, mesh.quant_step) = (Vec3::splat(-REACH), step);
        (mesh.uv_origin, mesh.uv_step) = ([0.0; 2], [1.0 / 65535.0; 2]);
        asset.meshes.push(mesh);
        asset.instances.push(Instance {
            mesh: slot as u32,
            material: slot as u32 * 3,
            transform: away,
        });
        asset.materials.extend(Lump::colours(0));
    }
    let mut loaded = solstice_builder::paginate(&asset);
    loaded.collision = None;
    debug_assert!(
        loaded.pages.is_empty(),
        "a blank's pages are all always loaded"
    );

    let each = blank.mesh.clusters.len();
    let mut slots: Vec<Slot> = (0..SLOTS)
        .map(|slot| Slot {
            instance: slot as u32,
            material: slot as u32 * 3,
            writes: Vec::new(),
        })
        .collect();
    for set in &loaded.scene.sets {
        for cluster in set.first_cluster..set.first_cluster + set.cluster_count {
            let (slot, own) = (cluster as usize / each, cluster as usize % each);
            let placed = &loaded.scene.clusters[cluster as usize];
            slots[slot].writes.push((
                set.page,
                placed.vertex_offset * 4,
                blank.mesh.clusters[own].vertex_offset,
                placed.vertex_count(),
            ));
        }
    }
    // Runs that lie end to end, in the page and in the lump alike, are one run.
    let size = std::mem::size_of::<PackedVertex>() as u32;
    for slot in &mut slots {
        slot.writes.sort_unstable();
        let mut runs: Vec<(u32, u32, u32, u32)> = Vec::new();
        for &(page, offset, first, count) in &slot.writes {
            match runs.last_mut() {
                Some(run)
                    if run.0 == page
                        && run.1 + run.3 * size == offset
                        && run.2 + run.3 == first =>
                {
                    run.3 += count;
                }
                _ => runs.push((page, offset, first, count)),
            }
        }
        slot.writes = runs;
    }
    (loaded, slots)
}

/// Says in a scene's gameplay files what it now has to show nuggets with: `slots`, by the
/// numbers they have once joined.
fn tell(
    files: &mut Vec<GameFile>,
    slots: &[Slot],
    joined: solstice_asset::join::Joined,
    pinned: u32,
) {
    let mut text = String::from("# what nuggets are shown with (apps/viewer/src/nugget.rs)\n");
    for slot in slots {
        let _ = writeln!(
            text,
            "slot {} {}",
            slot.instance + joined.instances,
            slot.material + joined.materials
        );
        for &(page, offset, first, count) in &slot.writes {
            // The pages are all always loaded, and those follow the scene's own such.
            let _ = writeln!(text, "write {} {offset} {first} {count}", page + pinned);
        }
    }
    files.retain(|file| file.name != FILE);
    files.push(GameFile {
        name: FILE.to_owned(),
        text,
    });
}

/// Gives a scene that is all in memory the blanks nuggets are shown with.
pub fn join_loaded(scene: &mut LoadedScene) -> Result<(), String> {
    let (blanks, slots) = blanks();
    let pinned = scene.scene.pinned_pages;
    let joined = scene.join(blanks).map_err(|error| error.to_string())?;
    tell(&mut scene.scene.gameplay, &slots, joined, pinned);
    Ok(())
}

/// Gives a scene that is read as it is drawn the blanks nuggets are shown with: they are
/// all in pages that are always loaded, and nothing of them is ever asked of its file.
pub fn join_streamed(scene: &mut PagedScene) -> Result<(), String> {
    let (blanks, slots) = blanks();
    let pinned = scene.pinned_pages;
    let joined =
        solstice_asset::join::join(scene, blanks.scene, 0).map_err(|error| error.to_string())?;
    tell(&mut scene.gameplay, &slots, joined, pinned);
    Ok(())
}

/// What a nugget is made of, besides gold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Gold and nothing else: water-worn, with clay in its hollows.
    Gold,
    /// Gold with quartz still in it.
    Quartz,
    /// Gold in dark ironstone.
    Ironstone,
    /// A piece of the reef itself: quartz with gold through it in veins.
    Reef,
}

impl Kind {
    /// The kind of the nugget with this number: most are gold and nothing else.
    pub fn of(shape: u32) -> Self {
        match Form::chance(shape, 120) {
            kind if kind < 0.42 => Self::Gold,
            kind if kind < 0.72 => Self::Quartz,
            kind if kind < 0.88 => Self::Ironstone,
            _ => Self::Reef,
        }
    }
}

/// A number from 0 to 1 at each corner of a grid of unit cubes, from the corner and a seed.
fn corner(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut mixed = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (z as u32).wrapping_mul(0xCB1A_B31F)
        ^ seed.wrapping_mul(0x9E37_79B1);
    mixed ^= mixed >> 15;
    mixed = mixed.wrapping_mul(0x2C1B_3C6D);
    mixed ^= mixed >> 12;
    mixed = mixed.wrapping_mul(0x297A_2D39);
    mixed ^= mixed >> 15;
    (mixed >> 8) as f32 / (1 << 24) as f32
}

/// Smooth noise from 0 to 1: those corners' numbers, eased between.
fn noise(at: Vec3, seed: u32) -> f32 {
    let cell = at.floor();
    let within = at - cell;
    let ease = within * within * (Vec3::splat(3.0) - 2.0 * within);
    let (x, y, z) = (cell.x as i32, cell.y as i32, cell.z as i32);
    let along = |y: i32, z: i32| {
        let (near, far) = (corner(x, y, z, seed), corner(x + 1, y, z, seed));
        near + (far - near) * ease.x
    };
    let low = along(y, z) + (along(y + 1, z) - along(y, z)) * ease.y;
    let high = along(y, z + 1) + (along(y + 1, z + 1) - along(y, z + 1)) * ease.y;
    low + (high - low) * ease.z
}

/// Noise of several sizes laid over one another, each half the size and half the strength
/// of the last: from 0 to 1, and most of it near a half.
fn rough(at: Vec3, layers: u32, seed: u32) -> f32 {
    let (mut sum, mut strength, mut total, mut at) = (0.0, 1.0, 0.0, at);
    for layer in 0..layers {
        sum += noise(at, seed.wrapping_add(layer * 7919)) * strength;
        total += strength;
        strength *= 0.5;
        at = at * 2.03 + Vec3::splat(17.3);
    }
    sum / total
}

fn ease(from: f32, to: f32, value: f32) -> f32 {
    let within = ((value - from) / (to - from)).clamp(0.0, 1.0);
    within * within * (3.0 - 2.0 * within)
}

/// Everything about a nugget that is its own, from its number.
struct Make {
    form: Form,
    kind: Kind,
    seed: u32,
    /// How strong its lobes, its knobs and its pits are, and how big each is.
    lobes: (f32, f32),
    knobs: (f32, f32),
    pits: (f32, f32),
    /// How much clay is in its hollows.
    clay: f32,
    /// How much of it is rock, for one with rock in it: what the rock's noise must pass.
    rock: f32,
    /// How wide the veins of gold are, in a piece of reef.
    veins: f32,
}

impl Make {
    fn of(shape: u32) -> Self {
        let chance = |which: u32| Form::chance(shape, which);
        let form = Form::of(shape);
        Self {
            form,
            kind: Kind::of(shape),
            seed: shape,
            lobes: (
                (0.2 + 0.4 * chance(130)) * form.knobs,
                0.9 + 0.9 * chance(131),
            ),
            knobs: (
                (0.1 + 0.2 * chance(132)) * form.knobs,
                1.6 + 1.3 * chance(133),
            ),
            pits: (0.05 + 0.1 * chance(134), 2.4 + 1.2 * chance(135)),
            clay: 0.45 + 1.0 * chance(136),
            rock: 0.62 - 0.17 * chance(137),
            veins: 0.06 + 0.07 * chance(138),
        }
    }

    /// What the nugget is, a way out from its middle: where its skin is, and how much of
    /// what is seen there is clay, and how much rock.
    fn at(&self, way: Vec3) -> (Vec3, f32, f32) {
        let (seed, form) = (self.seed, &self.form);
        // The noise is the nugget's own: a different part of it for each.
        let from = Vec3::new(
            Form::chance(seed, 140) * 61.0,
            Form::chance(seed, 141) * 61.0,
            Form::chance(seed, 142) * 61.0,
        );
        // Its shape before anything stands out of it: the ball drawn out, flattened, thicker
        // at one end and pinched at its middle, in the ball's own lengths.
        let girth = form.girth(way.x);
        let axes = Vec3::new(form.long, form.thick * girth, form.wide * girth);
        let smooth = way * axes;
        // Which way its skin faces there, and how far anything can stand out of it or sink
        // into it: less in a thin one, which would otherwise be pushed through itself.
        let outward = (way / axes).normalize_or(way);
        let depth = (axes.min_element() * 0.85).clamp(0.15, 1.0);
        // Its lobes, knobs and pits are as big one way as another across the nugget as it
        // will be, and not drawn out with it: so the noise is read where the point will be.
        // A long one has fewer points to its length, and its knobs are no finer than those
        // can show.
        let fine = (1.6 / form.long).min(1.0);
        let lobe = rough(smooth * self.lobes.1 + from, 3, seed ^ 0x51) - 0.5;
        // Knobs are round on top with creases between them, as gold that has been rolled is.
        let knob = (2.0 * rough(smooth * self.knobs.1 * fine + from, 2, seed ^ 0x52) - 1.0).abs();
        let pit = ease(
            0.56,
            0.7,
            rough(smooth * self.pits.1 * fine + from, 2, seed ^ 0x53),
        );
        let relief = (2.0 * self.lobes.0 * lobe + self.knobs.0 * (knob - 0.35) - self.pits.0 * pit)
            .clamp(-0.7, 0.9);

        // What it is made of there.
        let patches = rough(smooth * 1.7 + from, 3, seed ^ 0x54);
        let rock = match self.kind {
            Kind::Gold => 0.0,
            Kind::Quartz | Kind::Ironstone => {
                ease(self.rock - 0.07, self.rock + 0.07, patches + 0.12 * pit)
            }
            Kind::Reef => {
                let vein = 1.0 - ease(self.veins * 0.5, self.veins, (patches - 0.5).abs());
                let bleb = ease(0.6, 0.66, rough(smooth * 2.6 + from, 2, seed ^ 0x55));
                1.0 - vein.max(bleb)
            }
        };
        // Clay lies in the pits and the creases, and not evenly: here a hollow is full of
        // it, there one has been washed clean.
        let crease = ease(0.28, 0.06, knob);
        let stained = ease(0.38, 0.58, rough(smooth * 1.9 + from, 2, seed ^ 0x56));
        let clay = ((pit + crease * 0.9) * (0.35 + 0.65 * stained) * self.clay).clamp(0.0, 1.0);

        // Then what stands out of it; and the whole bent, arched and twisted along its length.
        let lump = (smooth + outward * relief * depth) * BALL;
        let along = (lump.x / (BALL * form.long)).clamp(-1.2, 1.2);
        let turned = Quat::from_rotation_y(form.bend * along)
            * Quat::from_rotation_z(form.arch * along)
            * Quat::from_rotation_x(form.twist * along);
        ((turned * lump).clamp_length_max(REACH * 0.98), clay, rock)
    }
}

/// A nugget, made: the points to write over a blank's, and its three materials.
#[derive(Clone, Debug, PartialEq)]
pub struct Lump {
    /// Its points, in the order the blank has them.
    pub vertices: Vec<PackedVertex>,
    /// Its gold, the clay in its hollows, and its rock.
    pub materials: [Material; 3],
}

impl Lump {
    /// The nugget with this number.
    pub fn of(shape: u32) -> Self {
        let (blank, make) = (blank(), Make::of(shape));
        let step = 2.0 * REACH / 65535.0;
        // A little way round from each point, both ways, to find which way its skin faces.
        const ROUND: f32 = 0.035;
        let made: Vec<PackedVertex> = blank
            .ways
            .iter()
            .map(|&way| {
                let (at, clay, rock) = make.at(way);
                let side = way.any_orthonormal_vector();
                let other = way.cross(side);
                let (to_side, ..) = make.at((way + side * ROUND).normalize());
                let (to_other, ..) = make.at((way + other * ROUND).normalize());
                let normal = (to_side - at).cross(to_other - at).normalize_or(way);
                let grid = ((at + Vec3::splat(REACH)) / step)
                    .round()
                    .clamp(Vec3::ZERO, Vec3::splat(65535.0));
                let share = |share: f32| (share.clamp(0.0, 1.0) * 65535.0).round() as u16;
                PackedVertex {
                    data: [
                        grid.x as u16,
                        grid.y as u16,
                        grid.z as u16,
                        encode_normal(normal.to_array()),
                        share(clay),
                        share(rock),
                    ],
                }
            })
            .collect();
        Self {
            vertices: blank
                .which
                .iter()
                .map(|&which| made[which as usize])
                .collect(),
            materials: Self::colours(shape),
        }
    }

    /// The bytes of some of its points, as a page holds them.
    pub fn bytes(&self, first: u32, count: u32) -> Option<Vec<u8>> {
        let points = self
            .vertices
            .get(first as usize..(first + count) as usize)?;
        Some(bytemuck::cast_slice(points).to_vec())
    }

    /// The three materials of the nugget with this number: its gold, the clay in its
    /// hollows, and its rock. The first says that the three are blended.
    pub fn colours(shape: u32) -> [Material; 3] {
        let chance = |which: u32| Form::chance(shape, which);
        let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
        let plain = |colour: [f32; 3], roughness: f32, metallic: f32, flags: u32| Material {
            base_color: [colour[0], colour[1], colour[2], 1.0],
            emissive: [0.0; 3],
            flags,
            roughness,
            metallic,
            uv_scale: 1.0,
            normal_strength: 0.0,
            texture_layer: -1,
            macro_variation: 0.0,
            texture_tier: 0,
            texture_slot: 0,
        };
        // Gold from deep yellow to pale, and some of it red with copper.
        let deep = [1.0, 0.60, 0.11];
        let tone = chance(150);
        let gold = if tone < 0.5 {
            mix(deep, [1.0, 0.78, 0.36], tone * 2.0)
        } else {
            mix(deep, [1.0, 0.46, 0.10], (tone - 0.5) * 2.0)
        };
        let gold = gold.map(|part| part * (0.82 + 0.18 * chance(151)));
        // Clay from red to brown to nearly black, with the gold showing through it a little.
        let earth = mix(
            mix([0.30, 0.10, 0.03], [0.12, 0.07, 0.04], chance(152)),
            [0.04, 0.03, 0.03],
            chance(153).powi(3),
        );
        let clay = mix(earth, gold, 0.12);
        // Its rock: quartz white, smoky or stained with rust; ironstone black to red-brown.
        let (rock, rock_rough, rock_metal) = match Kind::of(shape) {
            Kind::Ironstone => (
                mix([0.045, 0.035, 0.035], [0.17, 0.07, 0.04], chance(154)),
                0.82,
                0.08,
            ),
            _ => {
                let white = mix([0.74, 0.73, 0.69], [0.28, 0.27, 0.27], chance(154).powi(2));
                (
                    mix(white, [0.58, 0.33, 0.16], 0.75 * chance(155).powi(2)),
                    0.55 + 0.3 * chance(156),
                    0.0,
                )
            }
        };
        [
            plain(gold, 0.3 + 0.2 * chance(157), 1.0, Material::BLENDS),
            plain(clay, 0.85, 0.3, 0),
            plain(rock, rock_rough, rock_metal, 0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(vertex: &PackedVertex) -> Vec3 {
        let step = 2.0 * REACH / 65535.0;
        Vec3::new(
            vertex.data[0].into(),
            vertex.data[1].into(),
            vertex.data[2].into(),
        ) * step
            - Vec3::splat(REACH)
    }

    #[test]
    fn a_scene_is_given_blanks_that_are_always_loaded_and_each_has_its_own_points() {
        let (loaded, slots) = blanks();
        let scene = &loaded.scene;
        assert!(loaded.pages.is_empty() && scene.pinned_pages as usize == scene.pages.len());
        assert_eq!(
            (scene.instances.len(), scene.materials.len(), slots.len()),
            (SLOTS, SLOTS * 3, SLOTS)
        );
        // Every point of a lump has a place in each slot, once; and no two slots, or runs,
        // have any byte of a page between them.
        let points = blank().mesh.vertices.len() as u32;
        let size = std::mem::size_of::<PackedVertex>() as u32;
        let mut taken: Vec<(u32, u32, u32)> = Vec::new();
        for slot in &slots {
            let mut firsts: Vec<(u32, u32)> = slot
                .writes
                .iter()
                .map(|&(_, _, first, count)| (first, count))
                .collect();
            firsts.sort_unstable();
            let mut next = 0;
            for (first, count) in firsts {
                assert_eq!(first, next);
                next += count;
            }
            assert_eq!(next, points);
            for &(page, offset, _, count) in &slot.writes {
                let info = scene.pages[page as usize];
                assert!(offset % 4 == 0 && (offset + count * size) as usize <= info.vertex_bytes());
                taken.push((page, offset, offset + count * size));
            }
        }
        taken.sort_unstable();
        assert!(taken
            .windows(2)
            .all(|pair| pair[0].0 != pair[1].0 || pair[0].2 <= pair[1].1));
        // Its bounds are as wide as any nugget, and its points are kept on a grid that wide.
        let mesh = &scene.meshes[SLOTS - 1];
        assert_eq!(
            (mesh.quant_origin, mesh.uv_origin),
            (Vec3::splat(-REACH), [0.0; 2])
        );
        assert!(scene.nodes.iter().all(|node| node.radius == REACH));
        assert!(scene.clusters.iter().all(|cluster| cluster.radius == REACH));
    }

    #[test]
    fn joined_to_a_map_the_slots_are_told_by_the_maps_numbers() {
        // A map of one triangle, with a material and a page that is always loaded.
        let mut map = solstice_asset::SceneAsset::default();
        let triangle = solstice_builder::Mesh {
            vertices: [Vec3::ZERO, Vec3::X, Vec3::Z]
                .iter()
                .map(|&position| solstice_builder::Vertex {
                    position,
                    normal: Vec3::Y,
                    uv: glam::Vec2::new(position.x, position.z),
                })
                .collect(),
            indices: vec![0, 2, 1],
        };
        map.meshes.push(solstice_builder::build_virtual_mesh(
            &triangle,
            &Default::default(),
        ));
        map.materials.push(Material::default());
        map.instances.push(Instance {
            mesh: 0,
            material: 0,
            transform: Mat4::IDENTITY,
        });
        let mut loaded = solstice_builder::paginate(&map);
        let pinned = loaded.scene.pinned_pages;
        assert!(pinned >= 1);
        join_loaded(&mut loaded).expect("they join");
        let slots = Slot::read(&loaded.scene.gameplay);
        assert_eq!(slots.len(), SLOTS);
        let (alone, _) = blanks();
        for (index, slot) in slots.iter().enumerate() {
            assert_eq!(
                (slot.instance, slot.material),
                (1 + index as u32, 1 + index as u32 * 3)
            );
            let instance = loaded.scene.instances[slot.instance as usize];
            assert_eq!(
                (instance.mesh, instance.material),
                (1 + index as u32, slot.material)
            );
            assert!(loaded.scene.materials[slot.material as usize].flags & Material::BLENDS != 0);
            // Its points are where they were in the blanks' own pages, which follow the map's.
            for &(page, offset, _, count) in &slot.writes {
                let theirs = alone.scene.pinned_page(page - pinned);
                let ours = loaded.scene.pinned_page(page);
                let bytes = offset as usize..offset as usize + count as usize * 12;
                assert_eq!(ours[bytes.clone()], theirs[bytes]);
            }
        }
        // And one that is read as it is drawn is told the same.
        let mut streamed = solstice_builder::paginate(&map).scene;
        join_streamed(&mut streamed).expect("they join");
        assert_eq!(Slot::read(&streamed.gameplay), slots);
    }

    #[test]
    fn a_nugget_is_made_from_its_number_and_is_the_same_every_time() {
        let lump = Lump::of(77_123);
        assert_eq!(lump, Lump::of(77_123));
        assert_ne!(lump.vertices, Lump::of(77_124).vertices);
        assert_eq!(lump.vertices.len(), blank().mesh.vertices.len());
        // Points that are one point of the ball are one point of the nugget, to the bit:
        // its clusters meet without a crack.
        let blank = blank();
        for (index, &which) in blank.which.iter().enumerate().skip(1) {
            if let Some(before) = blank.which[..index]
                .iter()
                .position(|&other| other == which)
            {
                assert_eq!(lump.vertices[index], lump.vertices[before]);
                break;
            }
        }
        // It is all within reach of its middle.
        assert!(lump
            .vertices
            .iter()
            .all(|vertex| at(vertex).length() < REACH));
        // The skin of one that is not bent or pinched faces outward nearly everywhere.
        let plain = (0..9000_u32)
            .find(|&shape| {
                let form = Form::of(shape);
                form.bend.abs() + form.arch.abs() + form.twist.abs() + form.pinch < 0.15
            })
            .expect("some are plain");
        let lump = Lump::of(plain);
        let outward = lump
            .vertices
            .iter()
            .filter(|vertex| {
                let normal = Vec3::from_array(solstice_asset::gpu::decode_normal(vertex.data[3]));
                normal.dot(at(vertex).normalize_or_zero()) > 0.0
            })
            .count();
        assert!(
            outward * 100 > lump.vertices.len() * 93,
            "{outward} of {}",
            lump.vertices.len()
        );
        // It is about as long, as thick and as wide as its form says: what it lies as.
        for shape in [5_u32, 900, 31_337] {
            let form = Form::of(shape);
            if form.bend.abs() + form.arch.abs() + form.twist.abs() > 0.3 {
                continue;
            }
            let lump = Lump::of(shape);
            let (mut least, mut most) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for vertex in &lump.vertices {
                least = least.min(at(vertex));
                most = most.max(at(vertex));
            }
            let across = most - least;
            assert!(
                across.x > form.long * 0.55 && across.x < form.long * 1.9,
                "{across} {form:?}"
            );
            assert!(
                across.y > form.thick * 0.4 && across.y < form.thick * 1.9,
                "{across} {form:?}"
            );
        }
    }

    #[test]
    fn nuggets_are_of_every_kind_and_colour_and_no_two_are_alike() {
        let shapes: Vec<u32> = (0..400_u32)
            .map(|n| n.wrapping_mul(2_654_435_761).rotate_left(9))
            .collect();
        let count = |kind: Kind| {
            shapes
                .iter()
                .filter(|&&shape| Kind::of(shape) == kind)
                .count()
        };
        assert!(
            count(Kind::Gold) > count(Kind::Quartz) && count(Kind::Quartz) > count(Kind::Ironstone)
        );
        assert!(count(Kind::Reef) > 20);
        // Gold and nothing else has no rock at any point; a piece of reef is mostly rock,
        // with gold through it; the others are some of each.
        let rock = |shape: u32| {
            let lump = Lump::of(shape);
            lump.vertices
                .iter()
                .filter(|vertex| vertex.data[5] > 32767)
                .count() as f32
                / lump.vertices.len() as f32
        };
        let of = |kind: Kind| {
            shapes
                .iter()
                .copied()
                .filter(move |&shape| Kind::of(shape) == kind)
                .take(6)
        };
        assert!(of(Kind::Gold).all(|shape| rock(shape) == 0.0));
        for shape in of(Kind::Reef) {
            let share = rock(shape);
            assert!(share > 0.3 && share < 0.97, "{share}");
        }
        for shape in of(Kind::Quartz).chain(of(Kind::Ironstone)) {
            let share = rock(shape);
            assert!(share > 0.04 && share < 0.9, "{share}");
        }
        // Their golds are many, all of them yellow; their materials have no texture, and
        // the first says the three are blended.
        let golds: Vec<[f32; 4]> = shapes
            .iter()
            .map(|&shape| Lump::colours(shape)[0].base_color)
            .collect();
        assert!(golds
            .iter()
            .all(|gold| gold[0] > gold[1] && gold[1] > gold[2]));
        let (palest, deepest) = golds
            .iter()
            .fold((0.0_f32, 1.0_f32), |(most, least), gold| {
                (most.max(gold[2]), least.min(gold[2]))
            });
        assert!(palest > deepest * 2.5);
        let colours = Lump::colours(9);
        assert!(colours.iter().all(|colour| colour.texture_layer < 0));
        assert!(
            colours[0].flags & Material::BLENDS != 0
                && colours[0].metallic == 1.0
                && colours[2].metallic < 0.2
        );
    }
}
