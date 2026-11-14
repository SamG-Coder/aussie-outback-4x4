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

mod draw;
mod flora;
pub mod terrain;

use glam::Vec3;
use rayon::prelude::*;
use solstice_asset::{Heightfield, Material, SceneAsset};
use solstice_game::track::Sample;
use solstice_game::{Blocker, SurfaceMap, Track};
use solstice_procgen::surface::TILE_METRES;
use solstice_vehicle::Surface;
use terrain::{Plan, TREAD};

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

/// Builds the bush into the scene: the land `size` metres across, centred on the origin.
pub fn build(scene: &mut SceneAsset, size: f32) -> Result<Built, String> {
    println!("outback: {size:.0} m of bush");
    let plan = Plan::new(size);
    let half = size * 0.5;
    let drawn = half + BEYOND;

    println!("  textures...");
    let first = scene.textures.len() as i32;
    for name in ["gum_litter", "red_clay", "bush_mud", "tussock", "granite"] {
        scene
            .textures
            .push(crate::textures::surface(name, scene.texture_size));
    }
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
        .points
        .iter()
        .step_by(2)
        .map(|point| Sample {
            position: Vec3::new(point.at.x, plan.height(point.at.x, point.at.y), point.at.y),
            left: Vec3::new(point.forward.y, 0.0, -point.forward.x),
            half_left: TREAD,
            half_right: TREAD,
            left_bands: Vec::new(),
            right_bands: Vec::new(),
        })
        .collect();
    let sectors = vec![samples.len() / 3, samples.len() * 2 / 3];
    let track = Track::new("Wombat Creek Track", samples, sectors)?;

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
    })
}

#[cfg(test)]
mod tests {
    use super::terrain::{picture, Plan};

    /// Writes a picture of the design to `target/outback/plan.ppm`, to look at.
    #[test]
    #[ignore]
    fn draw_the_plan() {
        let plan = Plan::new(1000.0);
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
