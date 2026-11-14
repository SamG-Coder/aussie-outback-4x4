//! The ground as it is drawn: squares of land half a metre across, cut sixteen times finer
//! along the track, where the ruts are, and coloured by what covers them.
//!
//! * **Three kinds of ground at a time.** A ground material blends three textures by shares
//!   kept at each point of the mesh ([`Material::BLENDS`]). The bush has more than three:
//!   leaf litter under everything, the track's clay, mud, dry grass and bare rock. So each
//!   square of land goes to one of three meshes, by what is there: the track with mud, the
//!   track with rock, or the bush, with grass and rock. Where two meet, the points they share
//!   show only what both have, so no seam is seen.
//! * **Fine where the track is.** A wheel rut is a hand across. Squares near the track are cut
//!   into sixteen, with their heights from the design itself; along an edge shared with an
//!   uncut square the points lie on that square's straight edge, so no gap opens.
//! * **In pieces.** The land is built as meshes 128 m square. A mesh keeps its points to a
//!   65,535th of its size, which over a whole kilometre is coarser than a rut's edge.

use super::terrain::{Cover, Plan};
use glam::{Mat4, Vec2, Vec3};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use solstice_asset::{Instance, SceneAsset};
use solstice_builder::{build_virtual_mesh, BuildOptions, Mesh, Vertex};

/// Metres between the points of the land as it is drawn, and how many smaller squares a side
/// a square by the track is cut into.
pub const SPACING: f32 = 0.5;
const FINE: i32 = 4;
/// Squares a side in each mesh: 128 m.
const PIECE: i32 = 256;
/// The meshes a square of land can go to.
const TRACK_MUD: usize = 0;
const TRACK_ROCK: usize = 1;
const BUSH: usize = 2;
/// What each mesh's two shares are, after the leaf litter under them all.
const CLAY: u8 = 1;
const MUD: u8 = 2;
const GRASS: u8 = 4;
const ROCK: u8 = 8;
const SHOWS: [u8; 3] = [CLAY | MUD, CLAY | ROCK, GRASS | ROCK];

/// A point's shares in a mesh: the second and third of its three kinds of ground.
fn shares(kind: usize, cover: &Cover, allowed: u8) -> Vec2 {
    let take = |what: u8, share: f32| if allowed & what != 0 { share } else { 0.0 };
    let (a, b) = match kind {
        TRACK_MUD => (take(CLAY, cover.clay), take(MUD, cover.mud)),
        TRACK_ROCK => (take(CLAY, cover.clay), take(ROCK, cover.rock)),
        _ => (take(GRASS, cover.grass), take(ROCK, cover.rock)),
    };
    let (a, b) = (a.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
    let both = (a + b).max(1.0);
    Vec2::new(a / both, b / both)
}

/// One mesh being put together: its points, found again by where they are on the fine grid.
#[derive(Default)]
struct Piece {
    mesh: Mesh,
    points: FxHashMap<(i32, i32), u32>,
}

impl Piece {
    fn point(&mut self, key: (i32, i32), make: impl FnOnce() -> Vertex) -> u32 {
        *self.points.entry(key).or_insert_with(|| {
            self.mesh.vertices.push(make());
            self.mesh.vertices.len() as u32 - 1
        })
    }

    fn square(&mut self, corners: [u32; 4]) {
        // Corners are given as (0,0), (0,1), (1,1), (1,0): both triangles face up.
        let [a, b, c, d] = corners;
        self.mesh.indices.extend_from_slice(&[a, b, c, a, c, d]);
    }
}

/// The land within the square that is driven on, as meshes for each kind of ground: each with
/// which kind it is.
pub fn land(plan: &Plan, half: f32) -> Vec<(usize, Mesh)> {
    let cells = (2.0 * half / SPACING).round() as i32;
    let pieces = (cells + PIECE - 1) / PIECE;
    let place = |i: i32, j: i32| (i as f32 * SPACING - half, j as f32 * SPACING - half);
    (0..pieces * pieces)
        .into_par_iter()
        .flat_map_iter(|index| {
            let (first_i, first_j) = ((index % pieces) * PIECE, (index / pieces) * PIECE);
            let (wide, deep) = (PIECE.min(cells - first_i), PIECE.min(cells - first_j));
            // The points of this piece and one square beyond, for the squares beside it.
            let stride = (wide + 3) as usize;
            let at =
                |i: i32, j: i32| ((j - first_j + 1) * stride as i32 + (i - first_i + 1)) as usize;
            let mut heights = vec![0.0_f32; stride * (deep + 3) as usize];
            let mut covers = vec![Cover::default(); heights.len()];
            for j in first_j - 1..=first_j + deep + 1 {
                for i in first_i - 1..=first_i + wide + 1 {
                    let (x, z) = place(i, j);
                    heights[at(i, j)] = plan.height(x, z);
                    covers[at(i, j)] = plan.cover(x, z);
                }
            }
            // Each square's kind, and whether it is cut fine: this piece's, and the ring of
            // squares round it.
            let square_stride = (wide + 2) as usize;
            let square_at = |i: i32, j: i32| {
                ((j - first_j + 1) * square_stride as i32 + (i - first_i + 1)) as usize
            };
            let mut kinds = vec![(BUSH, false); square_stride * (deep + 2) as usize];
            for j in first_j - 1..=first_j + deep {
                for i in first_i - 1..=first_i + wide {
                    let corners = [
                        &covers[at(i, j)],
                        &covers[at(i + 1, j)],
                        &covers[at(i, j + 1)],
                        &covers[at(i + 1, j + 1)],
                    ];
                    let (x, z) = place(i, j);
                    let by_track = plan
                        .trail
                        .near(x + SPACING * 0.5, z + SPACING * 0.5)
                        .is_some_and(|near| near.offset.abs() < plan.tread + 1.9);
                    let any = |of: &dyn Fn(&Cover) -> f32| corners.iter().any(|c| of(c) > 0.02);
                    let (muddy, rocky) = (any(&|c| c.mud), any(&|c| c.rock));
                    let tracked = by_track || any(&|c| c.clay);
                    let kind = if tracked || muddy {
                        if rocky && !muddy {
                            TRACK_ROCK
                        } else {
                            TRACK_MUD
                        }
                    } else {
                        BUSH
                    };
                    kinds[square_at(i, j)] = (kind, by_track);
                }
            }
            // What a point may show: only what every square it belongs to has.
            let allowed_at = |i: i32, j: i32| {
                let mut allowed = CLAY | MUD | GRASS | ROCK;
                for (di, dj) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
                    allowed &= SHOWS[kinds[square_at(i + di, j + dj)].0];
                }
                allowed
            };
            // A point of the coarse grid: its height from the grid, its facing from its
            // neighbours.
            let coarse = |i: i32, j: i32, kind: usize| -> Vertex {
                let (x, z) = place(i, j);
                let slope_x = (heights[at(i + 1, j)] - heights[at(i - 1, j)]) / (2.0 * SPACING);
                let slope_z = (heights[at(i, j + 1)] - heights[at(i, j - 1)]) / (2.0 * SPACING);
                Vertex {
                    position: Vec3::new(x, heights[at(i, j)], z),
                    normal: Vec3::new(-slope_x, 1.0, -slope_z).normalize(),
                    uv: shares(kind, &covers[at(i, j)], allowed_at(i, j)),
                }
            };

            let mut made: [Piece; 3] = Default::default();
            let small = SPACING / FINE as f32;
            for j in first_j..first_j + deep {
                for i in first_i..first_i + wide {
                    let (kind, fine) = kinds[square_at(i, j)];
                    let piece = &mut made[kind];
                    if !fine {
                        let corners =
                            [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)].map(|(ci, cj)| {
                                piece.point((ci * FINE, cj * FINE), || coarse(ci, cj, kind))
                            });
                        piece.square(corners);
                        continue;
                    }
                    // Cut fine: its own grid of heights, with one beyond each edge for the
                    // facing of the points along them.
                    let (x0, z0) = place(i, j);
                    let side = (FINE + 3) as usize;
                    let mut fine_heights = [0.0_f32; 49];
                    for b in -1..=FINE + 1 {
                        for a in -1..=FINE + 1 {
                            fine_heights[(b + 1) as usize * side + (a + 1) as usize] =
                                plan.height(x0 + a as f32 * small, z0 + b as f32 * small);
                        }
                    }
                    let fine_at =
                        |a: i32, b: i32| fine_heights[(b + 1) as usize * side + (a + 1) as usize];
                    // Whether the square across each edge is cut fine too, and what it shows.
                    let beside = |di: i32, dj: i32| kinds[square_at(i + di, j + dj)];
                    let mut index = [[0_u32; 5]; 5];
                    for b in 0..=FINE {
                        for a in 0..=FINE {
                            let key = (i * FINE + a, j * FINE + b);
                            let on_i = a == 0 || a == FINE;
                            let on_j = b == 0 || b == FINE;
                            let vertex = if on_i && on_j {
                                // A corner: the coarse grid's own point.
                                let (ci, cj) = (i + a / FINE, j + b / FINE);
                                piece.point(key, || coarse(ci, cj, kind))
                            } else {
                                // The square across the edge this point is on, if it is on one.
                                let across = if on_i {
                                    Some(beside(if a == 0 { -1 } else { 1 }, 0))
                                } else if on_j {
                                    Some(beside(0, if b == 0 { -1 } else { 1 }))
                                } else {
                                    None
                                };
                                match across {
                                    Some((_, false)) => {
                                        // Beside an uncut square: on its straight edge.
                                        let (from, to, t) = if on_i {
                                            let ci = i + a / FINE;
                                            ((ci, j), (ci, j + 1), b as f32 / FINE as f32)
                                        } else {
                                            let cj = j + b / FINE;
                                            ((i, cj), (i + 1, cj), a as f32 / FINE as f32)
                                        };
                                        piece.point(key, || {
                                            let (p, q) = (
                                                coarse(from.0, from.1, kind),
                                                coarse(to.0, to.1, kind),
                                            );
                                            Vertex {
                                                position: p.position.lerp(q.position, t),
                                                normal: p.normal.lerp(q.normal, t).normalize(),
                                                uv: p.uv.lerp(q.uv, t),
                                            }
                                        })
                                    }
                                    other => {
                                        let allowed = SHOWS[kind]
                                            & other.map_or(SHOWS[kind], |(kind, _)| SHOWS[kind]);
                                        piece.point(key, || {
                                            let (x, z) =
                                                (x0 + a as f32 * small, z0 + b as f32 * small);
                                            let slope_x = (fine_at(a + 1, b) - fine_at(a - 1, b))
                                                / (2.0 * small);
                                            let slope_z = (fine_at(a, b + 1) - fine_at(a, b - 1))
                                                / (2.0 * small);
                                            Vertex {
                                                position: Vec3::new(x, fine_at(a, b), z),
                                                normal: Vec3::new(-slope_x, 1.0, -slope_z)
                                                    .normalize(),
                                                uv: shares(kind, &plan.cover(x, z), allowed),
                                            }
                                        })
                                    }
                                }
                            };
                            index[b as usize][a as usize] = vertex;
                        }
                    }
                    for b in 0..FINE as usize {
                        for a in 0..FINE as usize {
                            piece.square([
                                index[b][a],
                                index[b + 1][a],
                                index[b + 1][a + 1],
                                index[b][a + 1],
                            ]);
                        }
                    }
                }
            }
            made.into_iter()
                .enumerate()
                .filter(|(_, piece)| !piece.mesh.indices.is_empty())
                .map(|(kind, piece)| (kind, piece.mesh))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The country beyond the square that is driven on, out to the ranges: one coarse mesh, of
/// the bush's kind, lying a little under the fine land where the two overlap.
pub fn country(plan: &Plan, inner: f32, outer: f32) -> Mesh {
    const STEP: f32 = 8.0;
    let cells = (2.0 * outer / STEP).round() as i32;
    let place = |i: i32, j: i32| (i as f32 * STEP - outer, j as f32 * STEP - outer);
    // Under the fine land's edge by a square, so that no gap shows between them.
    let hidden = inner - STEP * 1.5;
    let points: Vec<Vertex> = (0..(cells + 1) * (cells + 1))
        .into_par_iter()
        .map(|index| {
            let (i, j) = (index % (cells + 1), index / (cells + 1));
            let (x, z) = place(i, j);
            let height = |x: f32, z: f32| plan.height(x, z);
            let slope_x = (height(x + 2.0, z) - height(x - 2.0, z)) / 4.0;
            let slope_z = (height(x, z + 2.0) - height(x, z - 2.0)) / 4.0;
            let sunk = if x.abs().max(z.abs()) < inner + STEP {
                0.35
            } else {
                0.0
            };
            Vertex {
                position: Vec3::new(x, height(x, z) - sunk, z),
                normal: Vec3::new(-slope_x, 1.0, -slope_z).normalize(),
                uv: shares(BUSH, &plan.cover(x, z), GRASS | ROCK),
            }
        })
        .collect();
    let mut mesh = Mesh {
        vertices: points,
        indices: Vec::new(),
    };
    let at = |i: i32, j: i32| (j * (cells + 1) + i) as u32;
    for j in 0..cells {
        for i in 0..cells {
            let (x, z) = place(i, j);
            let (cx, cz) = (x + STEP * 0.5, z + STEP * 0.5);
            if cx.abs().max(cz.abs()) < hidden {
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

/// Adds meshes to the scene, each once where it was made, with the material of its kind.
pub fn place(scene: &mut SceneAsset, meshes: Vec<(usize, Mesh)>, materials: [u32; 3]) {
    let built: Vec<_> = meshes
        .par_iter()
        .map(|(_, mesh)| build_virtual_mesh(mesh, &BuildOptions::default()))
        .collect();
    for ((kind, _), mesh) in meshes.iter().zip(built) {
        scene.instances.push(Instance {
            mesh: scene.meshes.len() as u32,
            material: materials[*kind],
            transform: Mat4::IDENTITY,
        });
        scene.meshes.push(mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_shows_only_what_its_mesh_has_and_what_it_may() {
        let cover = Cover {
            clay: 0.5,
            mud: 0.3,
            grass: 0.6,
            rock: 0.2,
            ..Cover::default()
        };
        let all = CLAY | MUD | GRASS | ROCK;
        assert_eq!(shares(TRACK_MUD, &cover, all), Vec2::new(0.5, 0.3));
        assert_eq!(shares(TRACK_ROCK, &cover, all), Vec2::new(0.5, 0.2));
        assert_eq!(shares(BUSH, &cover, all), Vec2::new(0.6, 0.2));
        // Where the track's mesh meets the bush's, the points they share show neither's own.
        let shared = SHOWS[TRACK_MUD] & SHOWS[BUSH];
        assert_eq!(shares(TRACK_MUD, &cover, shared), Vec2::ZERO);
        assert_eq!(shares(BUSH, &cover, shared), Vec2::ZERO);
        let shared = SHOWS[TRACK_ROCK] & SHOWS[BUSH];
        assert_eq!(shares(TRACK_ROCK, &cover, shared), Vec2::new(0.0, 0.2));
        assert_eq!(shares(BUSH, &cover, shared), Vec2::new(0.0, 0.2));
        // Shares never come to more than the whole.
        let much = Cover {
            clay: 0.9,
            mud: 0.9,
            ..Cover::default()
        };
        let both = shares(TRACK_MUD, &much, all);
        assert!((both.x + both.y - 1.0).abs() < 1.0e-6);
    }
}
