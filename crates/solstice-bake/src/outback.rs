//! A square kilometre of Victorian bush with a four-wheel-drive track through it, built from
//! nothing but its size, as a scene file's `outback SIZE` asks.
//!
//! * [`terrain`] designs it: the hills, the gullies and creek that water cuts in them, the
//!   waterholes, and a track found across it all and worn into the land, with its ruts, its
//!   bog holes, its drainage humps and its ledges of rock.
//! * [`draw`] makes the ground from the design: fine where the track is, and coloured by what
//!   covers it, leaf litter, clay, mud, dry grass and rock.
//! * [`flora`] plants it, each kind where it would grow.
//!
//! From the same design come what is driven on: the ground as a grid of heights; a map of what
//! the ground is made of, square by square, which is what a tyre feels (`solstice_game::ground`);
//! the track's line, for timing a lap of it and for putting a car back on it; and walls at
//! the square's edge. So what is drawn and what is driven on agree.

mod chart;
mod draw;
mod flora;
mod shed;
pub mod terrain;
mod town;

use glam::{Mat4, Vec3};
use rayon::prelude::*;
use solstice_asset::{Heightfield, Instance, Material, SceneAsset};
use solstice_builder::{build_virtual_mesh, BuildOptions, Mesh, Vertex};
use solstice_game::track::Sample;
use solstice_game::{Blocker, SurfaceMap, Track};
use solstice_procgen::surface::TILE_METRES;
use solstice_vehicle::Surface;
pub use terrain::Country;
use terrain::{Plan, RING};

/// How far beyond the square that is driven on the fine land is drawn, in metres, and how far
/// the coarse country beyond that reaches.
const BEYOND: f32 = 12.0;
const COUNTRY: f32 = 1536.0;

/// What the bush gives a scene besides what is drawn.
pub struct Built {
    /// The land, as a grid of heights to drive on.
    pub ground: Heightfield,
    /// The square's edge, and every tree's trunk.
    pub blockers: Vec<Blocker>,
    /// The track's line, as a `.track` file's text, and the map of the ground, as a
    /// `.surfaces` file's.
    pub track: String,
    pub surfaces: String,
    /// Where a car starts, and the way it faces, as a yaw in degrees.
    pub start: (Vec3, f32),
    /// The waterholes: each its least x and z, its greatest, and its water's height.
    pub ponds: Vec<[f32; 5]>,
    /// The plants a car flattens, as a file's text: a line each, `X Y Z REACH INSTANCE`,
    /// the last being the first of the plant's two instances.
    pub plants: String,
    /// What is kept in the map, as a `.items` file's text: a hub's chest, `chest X Y Z YAW`,
    /// the middle of its floor, inside, and how it is turned, in radians. (Nuggets are in no
    /// file: the game makes each as it is found.)
    pub items: String,
    /// A hub's trailheads, as a `.places` file's text: a line each, `place MAP X Y Z RADIUS`.
    /// Empty for a track.
    pub places: String,
    /// Where the map of it is, as a `.chart` file's text: `chart FILE HALF`, the picture
    /// among the game's pages and how far from the middle of the map its edge is, in
    /// metres. Empty if there were no pages to put a picture among.
    pub chart: String,
}

/// A ground material over a texture layer, darkened as it is seen in the field.
fn ground(scene: &mut SceneAsset, layer: i32, tone: [f32; 3], blends: bool) {
    scene.materials.push(Material {
        base_color: [tone[0], tone[1], tone[2], 1.0],
        roughness: 1.0,
        uv_scale: 1.0 / TILE_METRES,
        normal_strength: 1.0,
        texture_layer: layer,
        macro_variation: 1.0,
        flags: if blends { Material::BLENDS } else { 0 },
        ..Default::default()
    });
}

/// The textures the ground is laid with, added to a scene: leaf litter, clay, mud, dry grass
/// and granite, in that order. Returns the layer of the first.
fn ground_textures(scene: &mut SceneAsset) -> i32 {
    let first = scene.textures.len() as i32;
    for name in ["gum_litter", "red_clay", "bush_mud", "tussock", "granite"] {
        scene
            .textures
            .push(crate::textures::surface(name, scene.texture_size));
    }
    first
}

/// What every square of bush is made of that is the same in all of them, with none of it put
/// anywhere: the ground's textures, and the plants, grasses, boulders and fallen timber with
/// theirs. For the file a game's maps share (`lends outback` in a scene's text): a map baked
/// beside that file leaves these out of its own (`crate::lend`).
pub fn stock(scene: &mut SceneAsset) {
    println!("outback: what every square of bush is made of");
    ground_textures(scene);
    flora::stock(scene);
}

/// What a tyre is on at a place.
fn surface_at(plan: &Plan, x: f32, z: f32) -> Surface {
    let cover = plan.cover(x, z);
    if cover.water {
        Surface::Ford
    } else if cover.tread > 0.5 {
        if cover.bog > 0.1 {
            Surface::Bog
        } else if cover.mud > 0.5 {
            Surface::Mud
        } else if cover.rock > 0.5 {
            Surface::Rock
        } else {
            Surface::Dirt
        }
    } else if cover.rock > 0.5 {
        Surface::Rock
    } else if cover.mud > 0.5 {
        Surface::Mud
    } else if cover.grass > 0.5 {
        // Dry tussock on dry soil: nothing like the mown, damp verge of a circuit. A tyre
        // with tread grips it as it grips the forest's floor.
        Surface::Litter
    } else {
        Surface::Litter
    }
}

/// The glow of a trailhead's ring, as radiance: ochre, and bright enough to be seen by day.
const RING_GLOW: [f32; 3] = [3.0, 1.0, 0.1];

/// A box, added to a mesh: its middle, and half its width, height and depth.
fn boxed(mesh: &mut Mesh, centre: Vec3, half: Vec3) {
    for (normal, across, up) in [
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ] {
        let first = mesh.vertices.len() as u32;
        for (a, u) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            mesh.vertices.push(Vertex {
                position: centre + (normal + across * a + up * u) * half,
                normal,
                uv: glam::Vec2::new(a * 0.5 + 0.5, u * 0.5 + 0.5),
            });
        }
        mesh.indices
            .extend([0, 1, 2, 0, 2, 3].map(|corner| first + corner));
    }
}

/// What a map says of the things that are found and kept in it, as a `.items` file's text.
///
/// No nugget is in a map, or in any file. There is gold in the ground wherever the game says
/// there is when a map is entered, and a nugget is made when it is dug up, by the game, from
/// a number of its own (`apps/viewer/src/nugget.rs`). So all there is to say here is where a
/// hub's chest is.
fn items(plan: &Plan) -> String {
    use std::fmt::Write as _;
    let mut text = String::from("# what is kept here: a hub's chest\n");
    if let Some(shed) = &plan.shed {
        let (chest, yaw) = shed.chest();
        let _ = writeln!(
            text,
            "chest {:.3} {:.3} {:.3} {yaw:.4}",
            chest.x, chest.y, chest.z
        );
    }
    text
}

/// Marks the trailheads: at each a ring on the ground, glowing, to drive into, and round it
/// short posts with a glowing cap, left out where the road comes through. Returns the
/// trailheads as a `.places` file's text, each leading to the map of its name.
fn trailheads(scene: &mut SceneAsset, plan: &Plan, leads: &[String]) -> String {
    use std::fmt::Write as _;
    let glow = scene.materials.len() as u32;
    scene.materials.push(Material {
        base_color: [0.9, 0.42, 0.1, 1.0],
        emissive: RING_GLOW,
        roughness: 0.5,
        ..Default::default()
    });
    scene.materials.push(Material {
        base_color: [0.16, 0.12, 0.09, 1.0],
        roughness: 0.9,
        ..Default::default()
    });
    let mut text = String::from("# MAP X Y Z RADIUS: the rings that lead to other maps\n");
    for (place, map) in plan.places.iter().zip(leads) {
        let centre = Vec3::new(place.x, plan.height(place.x, place.y), place.y);
        let _ = writeln!(
            text,
            "place {map} {:.2} {:.2} {:.2} {RING}",
            centre.x, centre.y, centre.z
        );
        // The ring: a low kerb that follows the ground, wheel ruts and all.
        let mut ring = Mesh::default();
        const AROUND: u32 = 160;
        const WIDE: f32 = 0.42;
        for step in 0..AROUND {
            let angle = step as f32 / AROUND as f32 * std::f32::consts::TAU;
            let out = Vec3::new(angle.cos(), 0.0, angle.sin());
            for (radius, lift, normal) in [
                (RING - WIDE, -0.25, -out),
                (RING - WIDE, 0.09, Vec3::Y),
                (RING, 0.09, Vec3::Y),
                (RING, -0.25, out),
            ] {
                let at = centre + out * radius;
                ring.vertices.push(Vertex {
                    position: Vec3::new(at.x, plan.height(at.x, at.z) + lift, at.z),
                    normal,
                    uv: glam::Vec2::new(step as f32 / 8.0, radius),
                });
            }
            let (here, next) = (step * 4, (step + 1) % AROUND * 4);
            for side in 0..3 {
                ring.indices.extend([
                    here + side,
                    next + side,
                    next + side + 1,
                    here + side,
                    next + side + 1,
                    here + side + 1,
                ]);
            }
        }
        // The posts, a pace outside it.
        let mut posts = Mesh::default();
        const POSTS: u32 = 14;
        for step in 0..POSTS {
            let angle = (step as f32 + 0.5) / POSTS as f32 * std::f32::consts::TAU;
            let at = centre + Vec3::new(angle.cos(), 0.0, angle.sin()) * (RING + 0.9);
            let on_road = plan
                .trail
                .near(at.x, at.z)
                .is_some_and(|near| near.offset.abs() < plan.tread + 1.6);
            // A gate beside a shed is open on the shed's side, where it is driven into from.
            let from_camp = plan.gate.is_some_and(|gate| {
                let out = glam::Vec2::new(angle.cos(), angle.sin());
                let home = plan.camp - gate;
                gate.distance(*place) < 0.5 && out.dot(home.normalize_or_zero()) > 0.6
            });
            if on_road || from_camp {
                continue;
            }
            let foot = plan.height(at.x, at.z);
            boxed(
                &mut posts,
                Vec3::new(at.x, foot + 0.4, at.z),
                Vec3::new(0.07, 0.6, 0.07),
            );
            boxed(
                &mut ring,
                Vec3::new(at.x, foot + 1.06, at.z),
                Vec3::new(0.085, 0.07, 0.085),
            );
        }
        for (mesh, material) in [(ring, glow), (posts, glow + 1)] {
            if mesh.indices.is_empty() {
                continue;
            }
            scene.instances.push(Instance {
                mesh: scene.meshes.len() as u32,
                material,
                transform: Mat4::IDENTITY,
            });
            scene
                .meshes
                .push(build_virtual_mesh(&mesh, &BuildOptions::default()));
        }
    }
    text
}

/// Builds the bush into the scene: the land `size` metres across, centred on the origin, of the
/// country asked for. A hub has a road round it, and on the road a trailhead for each of the
/// maps `leads` names, in the order they are come to, and a shed at its camp, whose model is
/// found from `base`, the folder the scene's text is in.
pub fn build(
    scene: &mut SceneAsset,
    size: f32,
    country: Country,
    leads: &[String],
    base: &std::path::Path,
) -> Result<Built, String> {
    println!("outback: {size:.0} m of bush: {}", country.name);
    // The last map may be `+NAME`, a gate: a trailhead off the road, beside a hub's shed or a
    // track's start. Or `>NAME`, a way out: a main road to the edge of the map, which whoever
    // drives down is in that map; from a hub's shed, or from a town.
    let gate = leads.last().is_some_and(|last| last.starts_with('+'));
    let edge = leads.last().is_some_and(|last| last.starts_with('>'));
    let leads: Vec<String> = leads
        .iter()
        .map(|lead| lead.trim_start_matches(['+', '>']).to_owned())
        .collect();
    let leads = &leads[..];
    let plan = Plan::new(
        size,
        country,
        leads.len() - usize::from(gate || edge),
        gate,
        edge,
    );
    let half = size * 0.5;
    let drawn = half + BEYOND;

    println!("  textures...");
    let first = ground_textures(scene);
    let (litter, clay, mud, grass, rock) = (first, first + 1, first + 2, first + 3, first + 4);
    // Three materials in a row for each of the ground's three meshes: the leaf litter under
    // all of them, then that mesh's own two.
    // The granite here weathers warm: buff and brown, not the blue-grey of a quarry's.
    let tones = [
        (litter, [1.0, 0.96, 0.9]),
        (clay, [1.0; 3]),
        (mud, [1.0; 3]),
        (grass, [1.0; 3]),
        (rock, [0.62, 0.52, 0.42]),
    ];
    let tone = |layer: i32| {
        tones
            .iter()
            .find(|t| t.0 == layer)
            .map_or([1.0; 3], |t| t.1)
    };
    let mut sets = [0_u32; 3];
    for (set, pair) in [[clay, mud], [clay, rock], [grass, rock]]
        .iter()
        .enumerate()
    {
        sets[set] = scene.materials.len() as u32;
        ground(scene, litter, tone(litter), true);
        for &layer in pair {
            ground(scene, layer, tone(layer), false);
        }
    }

    println!("  land...");
    let land = draw::land(&plan, drawn);
    let triangles: usize = land.iter().map(|(_, mesh)| mesh.triangle_count()).sum();
    println!("    {} meshes, {triangles} triangles", land.len());
    draw::place(scene, land, sets);
    draw::place(scene, vec![(2, draw::country(&plan, drawn, COUNTRY))], sets);

    println!("  planting...");
    // Boulders are the rock of the ground's own third kind, on its own.
    let (mut blockers, breakables) = flora::plant(scene, &plan, sets[1] + 2);
    let mut plants = String::from("# X Y Z REACH INSTANCE: what a car flattens\n");
    for plant in &breakables {
        use std::fmt::Write as _;
        let _ = writeln!(
            plants,
            "{:.2} {:.2} {:.2} {:.2} {}",
            plant.at.x, plant.at.y, plant.at.z, plant.reach, plant.instance
        );
    }
    println!("    {} of them a car can flatten", breakables.len());
    let mut places = if leads.is_empty() {
        String::new()
    } else {
        trailheads(scene, &plan, leads)
    };
    // A way out at the edge of the map: where a truck is as good as gone, how near, and
    // where one that comes in by it is put, and facing which way.
    if let (Some(way), Some(to)) = (&plan.way_out, leads.last()) {
        use std::fmt::Write as _;
        let high = |at: glam::Vec2| plan.height(at.x, at.y);
        let _ = writeln!(
            places,
            "place edge:{to} {:.2} {:.2} {:.2} {:.1} {:.2} {:.2} {:.2} {:.1}",
            way.at.x,
            high(way.at),
            way.at.y,
            way.radius,
            way.arrive.x,
            high(way.arrive),
            way.arrive.y,
            way.yaw_degrees
        );
        println!(
            "  the way out to {to} is at {:.0} {:.0}; coming in, a truck is put at {:.2} {:.2} {:.2} facing {:.1}",
            way.at.x,
            way.at.y,
            way.arrive.x,
            high(way.arrive),
            way.arrive.y,
            way.yaw_degrees
        );
    }
    // A hub's shed: the model, what of it stops a truck, and its bay as a place to stop in.
    if let Some(shed) = &plan.shed {
        use std::fmt::Write as _;
        shed.build(scene, base)?;
        blockers.extend(shed.blockers());
        let (bay, round) = shed.bay();
        let _ = writeln!(
            places,
            "place shed {:.2} {:.2} {:.2} {round}",
            bay.x, bay.y, bay.z
        );
    }
    // A town: its buildings, what of them stops a truck, and the gold buyer's as a place to
    // stop at.
    if let Some(town) = &plan.town {
        use std::fmt::Write as _;
        town.build(scene, base)?;
        blockers.extend(town.blockers());
        let (buyer, round) = town.buyer();
        let _ = writeln!(
            places,
            "place buyer {:.2} {:.2} {:.2} {round}",
            buyer.x, buyer.y, buyer.z
        );
        // Its doors: each a place somebody walks up to, on foot.
        for (what, at) in town.doors() {
            let _ = writeln!(
                places,
                "place door:{what} {:.2} {:.2} {:.2} 1.7",
                at.x, at.y, at.z
            );
        }
    }
    let items = items(&plan);

    // For finding what a car has run into: `OUTBACK_NEAR=x,z` lists what stops one there.
    if let Some(near) = std::env::var("OUTBACK_NEAR").ok().and_then(|text| {
        let (x, z) = text.split_once(',')?;
        Some(glam::Vec2::new(
            x.trim().parse().ok()?,
            z.trim().parse().ok()?,
        ))
    }) {
        for blocker in &blockers {
            let at = glam::Vec2::new(blocker.centre.x, blocker.centre.z);
            if at.distance(near) < 9.0 {
                println!(
                    "    near {near}: a blocker at {} of size {}, {:.1} m off",
                    blocker.centre,
                    blocker.size,
                    at.distance(near)
                );
            }
        }
        for step in -6..=6 {
            let (x, z) = (near.x + step as f32 * 0.5, near.y - step as f32 * 0.5);
            println!("    ground at {x:.1} {z:.1}: {:.2}", plan.height(x, z));
        }
    }
    // A map of it, among the game's pages, which are beside the scenes' folder: named for
    // the country it is.
    let pages = base.join("../ui");
    let chart = if pages.is_dir() {
        println!("  its map...");
        let drawn = chart::draw(&plan);
        let name: String = country
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let folder = pages.join("maps");
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let file = folder.join(format!("{name}.png"));
        std::fs::write(&file, &drawn.png).map_err(|e| format!("{}: {e}", file.display()))?;
        format!("# FILE HALF: the map of it among the pages, and how far out its edge is\nchart maps/{name}.png {}\n", drawn.half)
    } else {
        String::new()
    };
    println!("  the ground to drive on...");
    let cells = (2.0 * drawn / draw::SPACING).round() as u32;
    let heights: Vec<f32> = (0..(cells + 1) * (cells + 1))
        .into_par_iter()
        .map(|index| {
            let (column, row) = (index % (cells + 1), index / (cells + 1));
            plan.height(
                column as f32 * draw::SPACING - drawn,
                row as f32 * draw::SPACING - drawn,
            )
        })
        .collect();
    let ground = Heightfield {
        columns: cells + 1,
        rows: cells + 1,
        origin: [-drawn, -drawn],
        spacing: [draw::SPACING, draw::SPACING],
        heights,
    };
    // What each square metre of it is made of.
    let squares = (2.0 * drawn).round() as u32;
    let map = SurfaceMap {
        origin: [-drawn, -drawn],
        spacing: 1.0,
        columns: squares,
        rows: squares,
        squares: (0..squares * squares)
            .into_par_iter()
            .map(|index| {
                let (x, z) = (
                    (index % squares) as f32 + 0.5 - drawn,
                    (index / squares) as f32 + 0.5 - drawn,
                );
                surface_at(&plan, x, z).index()
            })
            .collect(),
    };

    // The track's line: every other point of the design's is plenty.
    let samples: Vec<Sample> = plan
        .trail
        .main_points()
        .iter()
        .step_by(2)
        .map(|point| Sample {
            position: Vec3::new(point.at.x, plan.height(point.at.x, point.at.y), point.at.y),
            left: Vec3::new(point.forward.y, 0.0, -point.forward.x),
            half_left: plan.tread,
            half_right: plan.tread,
            left_bands: Vec::new(),
            right_bands: Vec::new(),
        })
        .collect();
    let sectors = vec![samples.len() / 3, samples.len() * 2 / 3];
    let track = Track::new(country.name, samples, sectors)?;

    // A wall at each edge of the square, well above and below the land.
    let middle = plan.height(0.0, 0.0);
    for (x, z, long_x) in [
        (0.0, -half, true),
        (0.0, half, true),
        (-half, 0.0, false),
        (half, 0.0, false),
    ] {
        blockers.push(Blocker {
            centre: Vec3::new(x, middle, z),
            size: if long_x {
                Vec3::new(size + 4.0, 400.0, 2.0)
            } else {
                Vec3::new(2.0, 400.0, size + 4.0)
            },
            yaw_degrees: 0.0,
        });
    }
    let ponds = plan
        .pools
        .iter()
        .map(|pool| {
            let (low, high) = pool.rectangle();
            [low.x, low.y, high.x, high.y, pool.level]
        })
        .collect();
    Ok(Built {
        ground,
        blockers,
        track: track.to_text(),
        surfaces: map.to_text(),
        start: plan.start(),
        ponds,
        plants,
        items,
        places,
        chart,
    })
}

#[cfg(test)]
mod tests {
    use super::terrain::{picture, Plan};

    /// Writes a picture of the design to `target/outback/plan.ppm`, to look at.
    #[test]
    #[ignore]
    fn draw_the_plan() {
        // `OUTBACK_COUNTRY=hub`, `rock` or `mud` draws that design in place of the track's.
        let country = std::env::var("OUTBACK_COUNTRY")
            .ok()
            .and_then(|name| super::Country::named(&name))
            .unwrap_or(super::Country::TRACK);
        // `OUTBACK_SIZE=700` draws it that many metres across, as a town is.
        let size = std::env::var("OUTBACK_SIZE")
            .ok()
            .and_then(|size| size.parse().ok())
            .unwrap_or(1000.0);
        let trailheads = match (country.hub, country.town) {
            (true, false) => 3,
            _ => 0,
        };
        // (`OUTBACK_GATE=1` gives a track its gate; a hub and a town have a way out.)
        let gate = !country.hub && std::env::var("OUTBACK_GATE").is_ok();
        let plan = Plan::new(size, country, trailheads, gate, country.hub);
        println!("trailheads {:?}, way out {:?}", plan.places, plan.way_out);
        let (start, yaw) = plan.start();
        println!("start {start} yaw {yaw}");
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/outback");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("plan.ppm"), picture(&plan)).unwrap();
        // And a closer look at one place, if `OUTBACK_VIEW=x,z,half` asks for one.
        if let Ok(view) = std::env::var("OUTBACK_VIEW") {
            let numbers: Vec<f32> = view
                .split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect();
            if let [x, z, half] = numbers[..] {
                let close = super::terrain::picture_of(&plan, glam::Vec2::new(x, z), half, 0.2);
                std::fs::write(folder.join("close.ppm"), close).unwrap();
            }
        }
    }
}
