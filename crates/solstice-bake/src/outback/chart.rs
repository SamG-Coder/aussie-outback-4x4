//! A map of the map: the design drawn as a map is drawn, to find the way by.
//!
//! Not a picture of the ground from above. Paper, tinted by what grows on it; the shape of
//! the land in contour lines and a little shading; water in blue; the road cased and filled,
//! a four-wheel-drive track a broken red line; what is built, in black; and each trailhead a
//! ring. North is up: the game's -z. It is drawn from the design, so that it is the land
//! that is driven on, and written as a picture the game's pages show (`ui/maps/`), with how
//! much ground it covers, for the pages to put the truck on it.
use super::terrain::{Plan, RING};
use glam::Vec2;
use rayon::prelude::*;

/// How many dots the picture is across, and down.
const EDGE: usize = 1024;
/// How far apart its contour lines are, in metres of height, and every which one is heavy.
const CONTOUR: f32 = 5.0;
const HEAVY: f32 = 25.0;

/// The picture, as a PNG file's bytes, and how far from the middle of the map its edge is,
/// in metres.
pub struct Chart {
    pub png: Vec<u8>,
    pub half: f32,
}

pub(super) type Colour = [f32; 3];

pub(super) fn mix(a: Colour, b: Colour, t: f32) -> Colour {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub(super) const PAPER: Colour = [0.925, 0.89, 0.78];
pub(super) const FOREST: Colour = [0.80, 0.85, 0.70];
pub(super) const OPEN: Colour = [0.93, 0.90, 0.75];
pub(super) const ROCK: Colour = [0.80, 0.77, 0.72];
pub(super) const WATER: Colour = [0.55, 0.72, 0.83];
pub(super) const SHORE: Colour = [0.30, 0.48, 0.62];
pub(super) const LINE: Colour = [0.55, 0.40, 0.26];
pub(super) const ROAD: Colour = [0.93, 0.70, 0.42];
pub(super) const CASING: Colour = [0.30, 0.20, 0.13];
pub(super) const TRACK: Colour = [0.66, 0.20, 0.13];
pub(super) const BUILT: Colour = [0.16, 0.14, 0.13];
pub(super) const RING_OF: Colour = [0.86, 0.45, 0.10];
pub(super) const INK: Colour = [0.12, 0.10, 0.09];

/// Draws the design as a map.
pub fn draw(plan: &Plan) -> Chart {
    // As far as the fine land is drawn: a little beyond the square that is driven on.
    let half = plan.half + 12.0;
    let pace = 2.0 * half / EDGE as f32;
    let place = |column: usize, row: usize| (column as f32 * pace - half, row as f32 * pace - half);
    // The land's height at the corner of every dot.
    let stride = EDGE + 1;
    let heights: Vec<f32> = (0..stride * stride)
        .into_par_iter()
        .map(|index| {
            let (x, z) = place(index % stride, index / stride);
            plan.height(x, z)
        })
        .collect();
    // What is built, as boxes on the ground: their middles, half their sizes, and how turned.
    let built: Vec<(Vec2, Vec2, Vec2)> = plan
        .town
        .iter()
        .flat_map(|town| town.blockers())
        .filter(|blocker| blocker.size.y >= 3.5)
        .map(|blocker| {
            let turn = blocker.yaw_degrees.to_radians();
            (
                Vec2::new(blocker.centre.x, blocker.centre.z),
                Vec2::new(blocker.size.x, blocker.size.z) * 0.5,
                Vec2::new(turn.cos(), turn.sin()),
            )
        })
        .collect();
    let mut dots = vec![0_u8; EDGE * EDGE * 3];
    dots.par_chunks_mut(EDGE * 3)
        .enumerate()
        .for_each(|(row, line)| {
            for column in 0..EDGE {
                let (x, z) = place(column, row);
                let (x, z) = (x + pace * 0.5, z + pace * 0.5);
                let (here, east, south) = (
                    heights[row * stride + column],
                    heights[row * stride + column + 1],
                    heights[(row + 1) * stride + column],
                );
                let cover = plan.cover(x, z);
                // The ground: forest, or open grass, or stone; and water.
                let mut colour = mix(FOREST, OPEN, plan.open(x, z));
                colour = mix(colour, ROCK, cover.rock * 0.8);
                // Lit from the north-west, as maps are, so that hills stand up off the paper.
                let (dx, dz) = ((east - here) / pace, (south - here) / pace);
                let lit = (0.55 * dx + 0.55 * dz) / (1.0 + dx * dx + dz * dz).sqrt();
                colour = mix(
                    colour,
                    if lit > 0.0 {
                        [1.0, 0.98, 0.9]
                    } else {
                        [0.35, 0.30, 0.28]
                    },
                    (lit.abs() * 1.3).min(0.36),
                );
                // Contours, where the height passes a line between this dot and the next.
                let crosses = |every: f32| {
                    let band = |height: f32| (height / every).floor();
                    band(here) != band(east) || band(here) != band(south)
                };
                if crosses(HEAVY) {
                    colour = mix(colour, LINE, 0.9);
                } else if crosses(CONTOUR) {
                    colour = mix(colour, LINE, 0.42);
                }
                // A grid, a hundred metres to the square.
                let grid = |a: f32| (a / 100.0).floor() != ((a + pace) / 100.0).floor();
                if grid(x - pace * 0.5) || grid(z - pace * 0.5) {
                    colour = mix(colour, INK, 0.13);
                }
                // The creek, and its waterholes.
                let wet = plan.wet(x, z);
                colour = mix(colour, WATER, smooth(0.86, 0.95, wet) * 0.75);
                if cover.water {
                    colour = WATER;
                } else if cover.mud > 0.6 && wet > 0.6 {
                    colour = mix(colour, SHORE, 0.3);
                }
                // The road, cased and filled; or a track, a broken line.
                if let Some(near) = plan.trail.near(x, z) {
                    let off = near.offset.abs();
                    if plan.hub {
                        if off < plan.tread {
                            colour = ROAD;
                        } else if off < plan.tread + (1.6 * pace).max(1.1) {
                            colour = CASING;
                        }
                    } else if off < (2.2 * pace).max(2.4) && (near.along / 9.0).fract() < 0.68 {
                        colour = if cover.water {
                            mix(TRACK, WATER, 0.45)
                        } else {
                            TRACK
                        };
                    }
                }
                // What is built: a hub's shed, a town's buildings.
                let at = Vec2::new(x, z);
                let shed = plan
                    .shed
                    .as_ref()
                    .is_some_and(|shed| shed.covers(x, z, 0.0));
                let town = built.iter().any(|(middle, size, turn)| {
                    let off = at - *middle;
                    let along = Vec2::new(
                        off.x * turn.x - off.y * turn.y,
                        off.x * turn.y + off.y * turn.x,
                    );
                    along.x.abs() < size.x && along.y.abs() < size.y
                });
                if shed || town {
                    colour = BUILT;
                }
                // Trailheads: a ring, and a dot in it.
                for ring in &plan.places {
                    let away = ring.distance(at);
                    if (away - RING).abs() < (1.5 * pace).max(1.3) || away < (2.0 * pace).max(1.6) {
                        colour = RING_OF;
                    }
                }
                // Beyond what is driven on the paper shows through, and a line marks the edge.
                let out = x.abs().max(z.abs()) - plan.half;
                if out > 0.0 {
                    colour = mix(colour, PAPER, 0.55);
                }
                if out.abs() < pace * 0.9 {
                    colour = mix(colour, INK, 0.7);
                }
                for (channel, value) in colour.iter().enumerate() {
                    line[column * 3 + channel] =
                        (value.clamp(0.0, 1.0).powf(1.0 / 1.05) * 255.0 + 0.5) as u8;
                }
            }
        });
    furniture(&mut dots, pace);
    Chart {
        png: png(EDGE as u32, EDGE as u32, &dots),
        half,
    }
}

/// A map of a design that is not a square of bush's (`super::arena`), drawn dot by dot: `dot`
/// is given the middle of each dot on the ground and how wide a dot is, in metres, and says
/// its colour. It is finished as any map is, with a bar for its scale and an arrow north.
pub(super) fn paint(half: f32, dot: &(dyn Fn(f32, f32, f32) -> Colour + Sync)) -> Chart {
    let pace = 2.0 * half / EDGE as f32;
    let mut dots = vec![0_u8; EDGE * EDGE * 3];
    dots.par_chunks_mut(EDGE * 3)
        .enumerate()
        .for_each(|(row, line)| {
            for column in 0..EDGE {
                let (x, z) = (
                    (column as f32 + 0.5) * pace - half,
                    (row as f32 + 0.5) * pace - half,
                );
                let colour = dot(x, z, pace);
                for (channel, value) in colour.iter().enumerate() {
                    line[column * 3 + channel] =
                        (value.clamp(0.0, 1.0).powf(1.0 / 1.05) * 255.0 + 0.5) as u8;
                }
            }
        });
    furniture(&mut dots, pace);
    Chart {
        png: png(EDGE as u32, EDGE as u32, &dots),
        half,
    }
}

fn smooth(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a map has in its corners: a bar a hundred metres long, in quarters, and an arrow
/// to the north.
fn furniture(dots: &mut [u8], pace: f32) {
    let mut dot = |column: usize, row: usize, colour: Colour| {
        if column < EDGE && row < EDGE {
            for (channel, value) in colour.iter().enumerate() {
                dots[(row * EDGE + column) * 3 + channel] = (value * 255.0) as u8;
            }
        }
    };
    // The bar, low on the left.
    let long = (100.0 / pace) as usize;
    let (left, top) = (34, EDGE - 46);
    for row in 0..9 {
        for column in 0..=long {
            let frame = row == 0 || row == 8 || column == 0 || column == long;
            let quarter = (column * 4 / long.max(1)).is_multiple_of(2);
            dot(
                left + column,
                top + row,
                if frame || quarter {
                    INK
                } else {
                    [0.97, 0.95, 0.88]
                },
            );
        }
    }
    // The arrow, high on the right: its left half dark, its right half light.
    let (middle, tip, tall, wide) = (EDGE - 52, 30, 46, 15.0);
    for row in 0..tall {
        let reach = (row as f32 / tall as f32 * wide) as usize;
        let notch = row > tall * 3 / 4;
        for across in 0..=reach {
            if notch && across < (row - tall * 3 / 4) {
                continue;
            }
            dot(middle - across, tip + row, INK);
            dot(
                middle + across,
                tip + row,
                if across == reach {
                    INK
                } else {
                    [0.97, 0.95, 0.88]
                },
            );
        }
    }
}

/// A picture as a PNG file: eight bits each of red, green and blue.
fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rgb.len() + height as usize);
    for line in rgb.chunks(width as usize * 3) {
        // Each line as it is: no filter.
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut file = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut part = |name: &[u8; 4], body: &[u8]| {
        file.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let from = file.len();
        file.extend_from_slice(name);
        file.extend_from_slice(body);
        let check = crc(&file[from..]);
        file.extend_from_slice(&check.to_be_bytes());
    };
    let mut head = Vec::new();
    head.extend_from_slice(&width.to_be_bytes());
    head.extend_from_slice(&height.to_be_bytes());
    // Eight bits a channel, colour, and nothing else out of the ordinary.
    head.extend_from_slice(&[8, 2, 0, 0, 0]);
    part(b"IHDR", &head);
    part(
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 8),
    );
    part(b"IEND", &[]);
    file
}

/// The check PNG keeps on each of its parts.
fn crc(bytes: &[u8]) -> u32 {
    let mut check = 0xFFFF_FFFF_u32;
    for &byte in bytes {
        check ^= u32::from(byte);
        for _ in 0..8 {
            check = if check & 1 == 1 {
                (check >> 1) ^ 0xEDB8_8320
            } else {
                check >> 1
            };
        }
    }
    !check
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_a_png_whose_parts_check() {
        let dots: Vec<u8> = (0..4 * 3 * 3).map(|i| (i * 7) as u8).collect();
        let file = png(4, 3, &dots);
        assert_eq!(
            &file[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        // Each part: its length, its name, its body, and a check of the name and body.
        let (mut at, mut names) = (8, Vec::new());
        while at < file.len() {
            let long = u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
            let check = u32::from_be_bytes(file[at + 8 + long..at + 12 + long].try_into().unwrap());
            assert_eq!(check, crc(&file[at + 4..at + 8 + long]));
            names.push(file[at + 4..at + 8].to_vec());
            at += 12 + long;
        }
        assert_eq!(
            names,
            [b"IHDR".to_vec(), b"IDAT".to_vec(), b"IEND".to_vec()]
        );
        // The check is the one PNG uses: of "IEND" alone it is a number everyone knows.
        assert_eq!(crc(b"IEND"), 0xAE42_6082);
        // And the dots come back out of it as they went in.
        let long = u32::from_be_bytes(file[33..37].try_into().unwrap()) as usize;
        let raw =
            miniz_oxide::inflate::decompress_to_vec_zlib(&file[41..41 + long]).expect("it unpacks");
        let back: Vec<u8> = raw.chunks(13).flat_map(|line| line[1..].to_vec()).collect();
        assert_eq!(back, dots);
    }
}
