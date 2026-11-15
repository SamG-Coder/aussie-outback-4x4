//! Price boards, made as a map is read.
//!
//! A truck for sale in the used-truck yard has a board standing before it that says what is
//! asked for it and how far it has been. No file has one: what is on the lot changes by the
//! day, so each is made here, of a white board on two legs and its words in red blocks, a
//! letter-board's letters, on both its faces (`shared::park_trucks`).
use glam::{Vec2, Vec3};
use solstice_asset::{Instance, LoadedScene, Material};

/// How wide a board is, how tall, how thick, and how high its foot is off the ground.
const WIDE: f32 = 1.1;
const TALL: f32 = 0.62;
const THICK: f32 = 0.04;
const FOOT: f32 = 0.5;

/// What a letter-board has letters for, each five blocks wide and seven tall, from its top.
const LETTERS: [(char, [&str; 7]); 15] = [
    (
        '0',
        [
            " ### ", "#   #", "#  ##", "# # #", "##  #", "#   #", " ### ",
        ],
    ),
    (
        '1',
        [
            "  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### ",
        ],
    ),
    (
        '2',
        [
            " ### ", "#   #", "    #", "   # ", "  #  ", " #   ", "#####",
        ],
    ),
    (
        '3',
        [
            " ### ", "#   #", "    #", "  ## ", "    #", "#   #", " ### ",
        ],
    ),
    (
        '4',
        [
            "   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # ",
        ],
    ),
    (
        '5',
        [
            "#####", "#    ", "#### ", "    #", "    #", "#   #", " ### ",
        ],
    ),
    (
        '6',
        [
            "  ## ", " #   ", "#    ", "#### ", "#   #", "#   #", " ### ",
        ],
    ),
    (
        '7',
        [
            "#####", "    #", "   # ", "  #  ", "  #  ", "  #  ", "  #  ",
        ],
    ),
    (
        '8',
        [
            " ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### ",
        ],
    ),
    (
        '9',
        [
            " ### ", "#   #", "#   #", " ####", "    #", "   # ", " ##  ",
        ],
    ),
    (
        '$',
        [
            "  #  ", " ####", "# #  ", " ### ", "  # #", "#### ", "  #  ",
        ],
    ),
    (
        ',',
        [
            "     ", "     ", "     ", "     ", "     ", "  ## ", "  #  ",
        ],
    ),
    (
        'K',
        [
            "#   #", "#  # ", "# #  ", "##   ", "# #  ", "#  # ", "#   #",
        ],
    ),
    (
        'M',
        [
            "#   #", "## ##", "# # #", "# # #", "#   #", "#   #", "#   #",
        ],
    ),
    (' ', ["     "; 7]),
];

/// A number with a comma at every thousand: 12500 is `12,500`.
pub fn thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A flat face of four corners, facing `normal`, added to a mesh.
fn face(mesh: &mut solstice_builder::Mesh, corners: [Vec3; 4], normal: Vec3) {
    let first = mesh.vertices.len() as u32;
    let mut corners = corners;
    // Wound so that it is seen from the side it faces.
    if (corners[1] - corners[0])
        .cross(corners[2] - corners[0])
        .dot(normal)
        < 0.0
    {
        corners.swap(1, 3);
    }
    for (corner, uv) in corners
        .into_iter()
        .zip([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    {
        mesh.vertices.push(solstice_builder::Vertex {
            position: corner,
            normal,
            uv: Vec2::new(uv.0, uv.1),
        });
    }
    mesh.indices
        .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
}

/// A box, from one corner to the opposite one, added to a mesh.
fn slab(mesh: &mut solstice_builder::Mesh, low: Vec3, high: Vec3) {
    let at = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { high.x } else { low.x },
            if y { high.y } else { low.y },
            if z { high.z } else { low.z },
        )
    };
    for (axis, side) in [
        (0, false),
        (0, true),
        (1, false),
        (1, true),
        (2, false),
        (2, true),
    ] {
        let corner = |a: bool, b: bool| match axis {
            0 => at(side, a, b),
            1 => at(a, side, b),
            _ => at(a, b, side),
        };
        let mut normal = Vec3::ZERO;
        normal[axis] = if side { 1.0 } else { -1.0 };
        face(
            mesh,
            [
                corner(false, false),
                corner(true, false),
                corner(true, true),
                corner(false, true),
            ],
            normal,
        );
    }
}

/// A line of words in blocks, on one face of the board: its middle `height` up the board,
/// each block `block` across, facing along z to `side` (1 or -1), and read from there.
fn line(mesh: &mut solstice_builder::Mesh, words: &str, height: f32, block: f32, side: f32) {
    let count = words.chars().count() as f32;
    // A letter is five blocks and a gap of one before the next.
    let wide = (count * 6.0 - 1.0) * block;
    let proud = side * (THICK * 0.5 + 0.004);
    for (index, letter) in words.chars().enumerate() {
        let Some((_, rows)) = LETTERS.iter().find(|(known, _)| *known == letter) else {
            continue;
        };
        for (row, blocks) in rows.iter().enumerate() {
            for (column, _) in blocks.chars().enumerate().filter(|(_, on)| *on == '#') {
                // From the left of whoever reads it: who looks back along z at the face, with x
                // to their right on the near face and to their left on the far one.
                let along = -wide * 0.5 + (index as f32 * 6.0 + column as f32) * block;
                let (from, to) = (along * side, (along + block) * side);
                let top = height + (3.5 - row as f32) * block;
                face(
                    mesh,
                    [
                        Vec3::new(from, top - block, proud),
                        Vec3::new(to, top - block, proud),
                        Vec3::new(to, top, proud),
                        Vec3::new(from, top, proud),
                    ],
                    Vec3::new(0.0, 0.0, side),
                );
            }
        }
    }
}

/// A price board, as a scene of its own: a white board on two legs, standing on the ground
/// at its origin and facing along z both ways, with what is asked in big red blocks and,
/// under it, how far the truck has been in smaller ones.
pub fn board(price: u64, km: f32) -> LoadedScene {
    let mut frame = solstice_builder::Mesh::default();
    slab(
        &mut frame,
        Vec3::new(-WIDE * 0.5, FOOT, -THICK * 0.5),
        Vec3::new(WIDE * 0.5, FOOT + TALL, THICK * 0.5),
    );
    for side in [-1.0, 1.0] {
        let x = side * (WIDE * 0.5 - 0.09);
        slab(
            &mut frame,
            Vec3::new(x - 0.025, 0.0, -0.025),
            Vec3::new(x + 0.025, FOOT, 0.025),
        );
        // And a foot under each, so that it stands.
        slab(
            &mut frame,
            Vec3::new(x - 0.03, 0.0, -0.24),
            Vec3::new(x + 0.03, 0.035, 0.24),
        );
    }
    let mut words = solstice_builder::Mesh::default();
    let asked = format!("${}", thousands(price));
    let been = format!("{} KM", thousands(km.max(0.0).round() as u64));
    // As big as fits across the board, and no bigger than is easy to read.
    let fit = |words: &str, most: f32| {
        (WIDE * 0.88 / (words.chars().count() as f32 * 6.0 - 1.0)).min(most)
    };
    for side in [-1.0, 1.0] {
        line(
            &mut words,
            &asked,
            FOOT + TALL * 0.6,
            fit(&asked, 0.034),
            side,
        );
        line(
            &mut words,
            &been,
            FOOT + TALL * 0.17,
            fit(&been, 0.013),
            side,
        );
    }
    let plain = |colour: [f32; 3], roughness: f32| Material {
        base_color: [colour[0], colour[1], colour[2], 1.0],
        emissive: [0.0; 3],
        flags: 0,
        roughness,
        metallic: 0.0,
        uv_scale: 1.0,
        normal_strength: 0.0,
        texture_layer: -1,
        macro_variation: 0.0,
        texture_tier: 0,
        texture_slot: 0,
    };
    let mut asset = solstice_asset::SceneAsset::default();
    let options = solstice_builder::BuildOptions::default();
    for (index, mesh) in [frame, words].iter().enumerate() {
        asset
            .meshes
            .push(solstice_builder::build_virtual_mesh(mesh, &options));
        asset.instances.push(Instance {
            mesh: index as u32,
            material: index as u32,
            transform: glam::Mat4::IDENTITY,
        });
    }
    asset.materials.push(plain([0.86, 0.85, 0.80], 0.55));
    asset.materials.push(plain([0.62, 0.035, 0.03], 0.5));
    let mut loaded = solstice_builder::paginate(&asset);
    loaded.collision = None;
    loaded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_says_what_is_asked_in_letters_it_has() {
        assert_eq!(thousands(950), "950");
        assert_eq!(thousands(4300), "4,300");
        assert_eq!(thousands(1_250_000), "1,250,000");
        // Every letter is five blocks by seven, and a price is made of none but them.
        for (letter, rows) in LETTERS {
            assert!(rows.iter().all(|row| row.chars().count() == 5), "{letter}");
        }
        let words = format!("${} {} KM", thousands(12_500), thousands(184_000));
        assert!(words
            .chars()
            .all(|letter| LETTERS.iter().any(|(known, _)| *known == letter)));
        // A board is two things to draw, its frame and its words, and stands on the ground.
        let board = board(12_500, 184_000.0);
        assert_eq!(board.scene.instances.len(), 2);
        assert_eq!(board.scene.materials.len(), 2);
        // The words face both ways, each wound to be seen from its own side.
        let mut mesh = solstice_builder::Mesh::default();
        line(&mut mesh, "1", 1.0, 0.03, 1.0);
        line(&mut mesh, "1", 1.0, 0.03, -1.0);
        for triangle in mesh.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices[triangle[i] as usize]);
            let wound = (b.position - a.position).cross(c.position - a.position);
            assert!(wound.dot(a.normal) > 0.0);
        }
    }
}
