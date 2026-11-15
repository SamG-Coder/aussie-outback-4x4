//! The loop: a steel deck bent up, over and back down, that a truck drives round.
//!
//! Seen from the side it is a teardrop, not a circle: it leaves the ground on a wide curve
//! and tightens as it rises, as a roller coaster's does, so that a truck is not squashed
//! flat at the bottom at the speed it needs to get over the top. Its curve at the top is
//! [`TOP`] metres in radius, which a truck 3.6 m high goes round with room over its roof and
//! a long-tailed one without dragging its tail; at the bottom [`BOTTOM`]; and it eases out
//! of the flat over [`EASE`] metres of run.
//!
//! A truck comes off the loop where it went on, in the side view, so the deck is skewed: it
//! slides [`SHIFT`] metres to the right between going on and coming off, as a corkscrew does,
//! and the way out lies beside the way in. The deck is a strip bent in one direction only,
//! on which a straight line stays straight, so a truck that holds its wheel straight would
//! stay on the middle of it all the way round, if it were a point.
//!
//! It is not a point. Going round a skewed loop a truck pitches and rolls at once, and a
//! thing longer than it is wide, turned about two of its axes together, wants to turn about
//! the third: its tyres hold it, but they give a little, and by the top it points a few
//! degrees off its line. So the deck is a trough: flat in the middle, [`FLAT`] wide, with a
//! shoulder curving up either side of it. A wheel that wanders on to a shoulder is pushed
//! back by it, the front wheel first, which steers the truck back to the middle. Nothing
//! touches its body.
//!
//! It is driven on what is drawn. The deck and its rails collide as the triangles they are
//! made of (`Collide::Auto`), and a truck's wheels look for the ground along the truck's own
//! down, whichever way up it is (`solstice_vehicle::Vehicle::substep`), so nothing here is
//! special to a loop. The frame that holds it up is looked at, and stops a truck as boxes.

use glam::{Mat4, Vec2, Vec3};
use solstice_builder::{Mesh, Vertex};
use solstice_game::Blocker;

/// The radius of its curve at the top and at the bottom, in metres, and over how much of its
/// run it eases out of the flat into the bottom's curve.
pub const TOP: f32 = 7.5;
pub const BOTTOM: f32 = 18.0;
pub const EASE: f32 = 6.0;
/// How wide the flat of the deck is, and its shoulders: how far out each reaches, and the
/// radius it curves up on.
pub const FLAT: f32 = 6.5;
pub const SHOULDER: (f32, f32) = (1.3, 1.6);
/// How wide the deck is between its rails.
pub const WIDE: f32 = FLAT + 2.0 * SHOULDER.0;
/// How far to the right the deck has slid by the time a truck comes off it.
pub const SHIFT: f32 = 15.0;
/// The rail along the top of each shoulder: how wide, and how high it stands off it.
pub const RAIL: (f32, f32) = (0.24, 0.3);
/// The flat plate a truck drives on to it by, and off it by: how long.
pub const LEAD: f32 = 6.0;
/// How thick the deck is.
const SLAB: f32 = 0.12;
/// Metres between the stations the deck is made at.
const STEP: f32 = 0.4;
/// How far outside the deck's edges the two frames that hold it stand, and how high a part
/// of the deck is before it is carried by beams that reach across to them: under it, a truck
/// on the way in or the way out has room to pass.
const FRAME_OUT: f32 = 1.4;
const CLEAR: f32 = 5.0;

/// A place along the loop, seen from the side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// How far along the deck it is, from where the deck leaves the ground.
    pub run: f32,
    /// How far along the ground that is, and how high.
    pub along: f32,
    pub up: f32,
    /// The way the deck runs there: radians up from the level, round to a full turn.
    pub turn: f32,
}

impl Station {
    /// The way the deck faces there: toward the truck on it.
    fn facing(&self) -> Vec2 {
        Vec2::new(-self.turn.sin(), self.turn.cos())
    }
}

/// How sharply the deck curves where it runs at `turn`, `run` metres on.
fn curve(turn: f32, run: f32) -> f32 {
    let ease = (run / EASE).clamp(0.0, 1.0);
    let ease = ease * ease * (3.0 - 2.0 * ease);
    (1.0 / BOTTOM + (1.0 / TOP - 1.0 / BOTTOM) * (turn * 0.5).sin()) * ease
}

/// The loop from the side, from where it leaves the ground to where it meets it again:
/// stations [`STEP`] apart. The second half is the first, turned about.
pub fn profile() -> Vec<Station> {
    // The first half, in fine steps, to where the deck runs level again, upside down.
    const FINE: f32 = 0.004;
    let per = (STEP / FINE).round() as usize;
    let mut half = Vec::new();
    let (mut at, mut turn, mut count) = (Vec2::ZERO, 0.0_f32, 0_usize);
    while turn < std::f32::consts::PI {
        if count.is_multiple_of(per) {
            half.push(Station {
                run: count as f32 * FINE,
                along: at.x,
                up: at.y,
                turn,
            });
        }
        turn += curve(turn, count as f32 * FINE) * FINE;
        at += Vec2::new(turn.cos(), turn.sin()) * FINE;
        count += 1;
    }
    let top = Station {
        run: count as f32 * FINE,
        along: at.x,
        up: at.y,
        turn: std::f32::consts::PI,
    };
    // (The last of them may be all but at the top: the top takes its place.)
    if half
        .last()
        .is_some_and(|last| top.run - last.run < STEP * 0.5)
    {
        half.pop();
    }
    let mut all = half.clone();
    all.push(top);
    all.extend(half.iter().rev().map(|station| Station {
        run: 2.0 * top.run - station.run,
        along: 2.0 * top.along - station.along,
        up: station.up,
        turn: std::f32::consts::TAU - station.turn,
    }));
    all
}

/// How far a shoulder stands off the flat of the deck, so far across from the deck's
/// middle, and how steeply it is climbing there, as the sine of its slope: nothing, on the
/// flat.
fn shoulder(across: f32) -> (f32, f32) {
    let out = (across.abs() - FLAT * 0.5).clamp(0.0, SHOULDER.0);
    let (_, radius) = SHOULDER;
    (
        radius - (radius * radius - out * out).sqrt(),
        (out / radius) * across.signum(),
    )
}

/// The loop as it stands: its side view, and how it lies on the ground.
pub struct Ring {
    pub stations: Vec<Station>,
    /// The loop's own space to the map's: its origin is the middle of the deck where it
    /// leaves the ground, and a truck drives on to it along its -z.
    pub placed: Mat4,
}

/// What the loop is made of, to draw: its deck and its rails, which are driven on, and the
/// frame that holds it up.
pub struct Meshes {
    pub deck: Mesh,
    pub rails: Mesh,
    pub frame: Mesh,
}

/// A box-section member from one place to another, `thick` across, in lengths of two
/// metres and a half: the renderer draws small triangles, and a side as long as a beam
/// may not be drawn at all.
fn member(mesh: &mut Mesh, from: Vec3, to: Vec3, thick: f32) {
    let along = to - from;
    let length = along.length();
    if length < 1.0e-3 {
        return;
    }
    let along = along / length;
    let aside = if along.y.abs() > 0.95 {
        Vec3::X
    } else {
        along.cross(Vec3::Y).normalize()
    };
    let over = aside.cross(along);
    let half = thick * 0.5;
    let pieces = (length / 2.5).ceil().max(1.0) as usize;
    for (normal, across) in [
        (aside, over),
        (-aside, -over),
        (over, -aside),
        (-over, aside),
    ] {
        for piece in 0..pieces {
            let (start, end) = (
                from + along * (length * piece as f32 / pieces as f32),
                from + along * (length * (piece + 1) as f32 / pieces as f32),
            );
            let first = mesh.vertices.len() as u32;
            for (place, side) in [(start, -1.0), (end, -1.0), (end, 1.0), (start, 1.0)] {
                mesh.vertices.push(Vertex {
                    position: place + normal * half + across * (side * half),
                    normal,
                    uv: Vec2::ZERO,
                });
            }
            mesh.indices
                .extend([0, 1, 2, 0, 2, 3].map(|corner| first + corner));
        }
    }
}

/// A strip of surface through rows of points, each row from one side to the other, with
/// the way the surface faces at each point.
fn strip(mesh: &mut Mesh, rows: &[Vec<(Vec3, Vec3)>]) {
    let across = rows.first().map_or(0, Vec::len);
    if across < 2 || rows.len() < 2 {
        return;
    }
    let first = mesh.vertices.len() as u32;
    for (index, row) in rows.iter().enumerate() {
        for (place, &(position, normal)) in row.iter().enumerate() {
            mesh.vertices.push(Vertex {
                position,
                normal,
                uv: Vec2::new(place as f32, index as f32),
            });
        }
    }
    let at = |row: usize, place: usize| first + (row * across + place) as u32;
    for row in 0..rows.len() - 1 {
        for place in 0..across - 1 {
            // Wound so that it faces the way it is said to.
            let (a, b, c) = (
                rows[row][place].0,
                rows[row + 1][place].0,
                rows[row][place + 1].0,
            );
            let faces = (b - a).cross(c - a).dot(rows[row][place].1) > 0.0;
            let quad = [
                at(row, place),
                at(row + 1, place),
                at(row + 1, place + 1),
                at(row, place + 1),
            ];
            let corners = if faces {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            mesh.indices.extend(corners.map(|corner| quad[corner]));
        }
    }
}

impl Ring {
    /// The loop with the middle of its deck leaving the ground at `at`, for a truck that
    /// drives on to it facing `yaw_degrees` (0 is along -z, 90 along -x).
    pub fn standing(at: Vec3, yaw_degrees: f32) -> Self {
        Self {
            stations: profile(),
            placed: Mat4::from_translation(at) * Mat4::from_rotation_y(yaw_degrees.to_radians()),
        }
    }

    /// How long the deck is, from leaving the ground to meeting it again.
    pub fn length(&self) -> f32 {
        self.stations.last().map_or(0.0, |last| last.run)
    }

    /// How high its top is.
    pub fn height(&self) -> f32 {
        self.stations
            .iter()
            .map(|station| station.up)
            .fold(0.0, f32::max)
    }

    /// The angle the deck is skewed by: a truck drives on to it this far off square, and
    /// holds its line.
    pub fn skew(&self) -> f32 {
        (SHIFT / self.length()).atan()
    }

    /// The loop's own directions: the way its side view runs along the ground, and across
    /// it, to the right. A truck drives on to it along -z, a little across both.
    fn axes(&self) -> (Vec3, Vec3) {
        let (sin, cos) = self.skew().sin_cos();
        let (travel, right) = (Vec3::NEG_Z, Vec3::X);
        (travel * cos - right * sin, travel * sin + right * cos)
    }

    /// A place in the loop's own space: so far along its side view, so high, so far across.
    fn local(&self, along: f32, up: f32, across: f32) -> Vec3 {
        let (run, aside) = self.axes();
        run * along + Vec3::Y * up + aside * across
    }

    /// The flat of the deck `run` metres along it and so far across from its middle, in the
    /// loop's own space, with the way the flat faces there. Before the deck leaves the
    /// ground and after it meets it again, the plate it lies on.
    fn on_flat(&self, run: f32, across: f32) -> (Vec3, Vec3) {
        let length = self.length();
        let slide = self.skew().tan();
        let (along, up, facing) = if run <= 0.0 {
            (run, 0.0, Vec2::Y)
        } else if run >= length {
            let last = self.stations.last().expect("it has stations");
            (last.along + (run - length), 0.0, Vec2::Y)
        } else {
            let index = self
                .stations
                .partition_point(|station| station.run <= run)
                .clamp(1, self.stations.len() - 1);
            let (a, b) = (self.stations[index - 1], self.stations[index]);
            let t = (run - a.run) / (b.run - a.run);
            let facing = a.facing().lerp(b.facing(), t).normalize();
            (
                a.along + (b.along - a.along) * t,
                a.up + (b.up - a.up) * t,
                facing,
            )
        };
        let (axis, _) = self.axes();
        (
            self.local(along, up, run * slide + across),
            axis * facing.x + Vec3::Y * facing.y,
        )
    }

    /// How much of their height the shoulders and the rails stand `run` metres along: they
    /// rise out of the plate a truck drives on by, and sink into the one it drives off by,
    /// so that neither end is a wall and a truck can drive on from a little to one side.
    fn raised(&self, run: f32) -> f32 {
        let from_end = (run + LEAD).min(self.length() + LEAD - run);
        let t = ((from_end - 0.8) / 5.0).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// The deck's surface `run` metres along it and so far across from its middle, in the
    /// loop's own space, with the way it faces there: on a shoulder, up it and in.
    pub fn on_deck(&self, run: f32, across: f32) -> (Vec3, Vec3) {
        let (at, facing) = self.on_flat(run, across);
        let (lift, slope) = shoulder(across);
        let raised = self.raised(run);
        let (_, aside) = self.axes();
        let slope = slope * raised;
        (
            at + facing * (lift * raised),
            (facing * (1.0 - slope * slope).sqrt() - aside * slope).normalize(),
        )
    }

    /// The same, on the map.
    pub fn on_map(&self, run: f32, across: f32) -> (Vec3, Vec3) {
        let (at, facing) = self.on_deck(run, across);
        (
            self.placed.transform_point3(at),
            self.placed.transform_vector3(facing),
        )
    }

    /// Where a truck is put to drive on to it, `back` metres before the deck leaves the
    /// ground, on the map: its place on the ground, and the way it faces, a yaw in degrees.
    pub fn approach(&self, back: f32) -> (Vec3, f32) {
        let at = self.placed.transform_point3(Vec3::Z * back);
        let ahead = self.placed.transform_vector3(Vec3::NEG_Z);
        (at, (-ahead.x).atan2(-ahead.z).to_degrees())
    }

    /// Where a truck that has come off it is, `on` metres past where the deck meets the
    /// ground again, on the map.
    pub fn way_off(&self, on: f32) -> Vec3 {
        self.on_map(self.length() + on, 0.0).0
    }

    /// The runs the deck is made at: its lead on, the loop, and its lead off.
    fn runs(&self) -> Vec<f32> {
        let leads = (LEAD / STEP).round() as usize;
        let length = self.length();
        let mut runs: Vec<f32> = (0..leads)
            .map(|k| -LEAD + k as f32 * LEAD / leads as f32)
            .collect();
        runs.extend(self.stations.iter().map(|station| station.run));
        runs.extend((1..=leads).map(|k| length + k as f32 * LEAD / leads as f32));
        runs
    }

    /// Where the deck's points are across it: its flat in four, and each shoulder in seven.
    fn across() -> Vec<f32> {
        let half = FLAT * 0.5;
        let mut across: Vec<f32> = (0..7)
            .map(|k| -half - SHOULDER.0 * (7 - k) as f32 / 7.0)
            .collect();
        across.extend((0..=4).map(|k| -half + FLAT * k as f32 / 4.0));
        across.extend((1..=7).map(|k| half + SHOULDER.0 * k as f32 / 7.0));
        across
    }

    /// The loop, in its own space.
    pub fn meshes(&self) -> Meshes {
        let runs = self.runs();
        let across = Self::across();
        let edge = WIDE * 0.5;
        let (_, aside) = self.axes();
        // The deck: its top, flat and shoulders, and its underside.
        let mut deck = Mesh::default();
        let top: Vec<Vec<(Vec3, Vec3)>> = runs
            .iter()
            .map(|&run| across.iter().map(|&w| self.on_deck(run, w)).collect())
            .collect();
        strip(&mut deck, &top);
        let under: Vec<Vec<(Vec3, Vec3)>> = runs
            .iter()
            .map(|&run| {
                [-edge, 0.0, edge]
                    .iter()
                    .map(|&w| {
                        let (at, facing) = self.on_flat(run, w);
                        (at - facing * SLAB, -facing)
                    })
                    .collect()
            })
            .collect();
        strip(&mut deck, &under);
        // Its two edges, from the top of each shoulder down to its underside; and its ends.
        for side in [-1.0, 1.0] {
            let sides: Vec<Vec<(Vec3, Vec3)>> = runs
                .iter()
                .map(|&run| {
                    let out = aside * side;
                    let (flat, facing) = self.on_flat(run, side * edge);
                    vec![
                        (self.on_deck(run, side * edge).0, out),
                        (flat - facing * SLAB, out),
                    ]
                })
                .collect();
            strip(&mut deck, &sides);
        }
        for (end, out) in [(0, Vec3::Z), (runs.len() - 1, Vec3::NEG_Z)] {
            let run = runs[end];
            let ends: Vec<Vec<(Vec3, Vec3)>> = [0.0, SLAB]
                .iter()
                .map(|&down| {
                    [-edge, 0.0, edge]
                        .iter()
                        .map(|&w| {
                            let (at, facing) = self.on_flat(run, w);
                            (at - facing * down, out)
                        })
                        .collect()
                })
                .collect();
            strip(&mut deck, &ends);
        }

        // The rails: a box section along the top of each shoulder.
        let mut rails = Mesh::default();
        for side in [-1.0, 1.0] {
            let (inner, outer) = (side * edge, side * (edge + RAIL.0));
            // (A rail stands off the shoulder's top the way the flat faces, not the
            // shoulder: its sides are upright to the deck.)
            let corner = |run: f32, across: f32, up: bool| {
                let foot = self.on_deck(run, side * edge).0;
                let (_, facing) = self.on_flat(run, across);
                let lift = if up { RAIL.1 * self.raised(run) } else { 0.0 };
                foot + aside * (across - side * edge) + facing * lift
            };
            let face = |from: (f32, bool), to: (f32, bool), flat: bool, out: f32| {
                runs.iter()
                    .map(|&run| {
                        let normal = if flat {
                            self.on_flat(run, 0.0).1
                        } else {
                            aside * out
                        };
                        vec![
                            (corner(run, from.0, from.1), normal),
                            (corner(run, to.0, to.1), normal),
                        ]
                    })
                    .collect::<Vec<_>>()
            };
            // Toward the lane, over the top, and outside.
            strip(
                &mut rails,
                &face((inner, false), (inner, true), false, -side),
            );
            strip(&mut rails, &face((inner, true), (outer, true), true, 0.0));
            strip(
                &mut rails,
                &face((outer, true), (outer, false), false, side),
            );
        }

        Meshes {
            deck,
            rails,
            frame: self.frame(),
        }
    }

    /// Where the two frames stand, across: left of the way on, and right of the way off.
    fn frames(&self) -> [f32; 2] {
        let half = WIDE * 0.5 + RAIL.0 + FRAME_OUT;
        [-half, SHIFT + half]
    }

    /// The deck's outside, a little way off it, at a station: where what carries it runs.
    fn outside(station: &Station, off: f32) -> Vec2 {
        Vec2::new(station.along, station.up) - station.facing() * off
    }

    /// The stations at which something holds the deck up, every `every` of them.
    fn held(&self, every: usize) -> impl Iterator<Item = &Station> {
        self.stations.iter().step_by(every)
    }

    /// The frame: a hoop either side of the whole loop, legs from them to the ground, beams
    /// across between them under the deck where it is high, and trestles under it where it
    /// is low.
    fn frame(&self) -> Mesh {
        let mut mesh = Mesh::default();
        let frames = self.frames();
        let slide = self.skew().tan();
        // The hoops, and a sill under each.
        for side in frames {
            let ring: Vec<Vec2> = self
                .held(3)
                .map(|station| Self::outside(station, SLAB + 0.25))
                .filter(|at| at.y > 0.2)
                .collect();
            for pair in ring.windows(2) {
                member(
                    &mut mesh,
                    self.local(pair[0].x, pair[0].y, side),
                    self.local(pair[1].x, pair[1].y, side),
                    0.32,
                );
            }
            let (low, high) = ring.iter().fold((f32::MAX, f32::MIN), |(low, high), at| {
                (low.min(at.x), high.max(at.x))
            });
            member(
                &mut mesh,
                self.local(low - 0.6, 0.12, side),
                self.local(high + 0.6, 0.12, side),
                0.3,
            );
        }
        // Where the deck is high: a beam across under it, and a leg down from each end.
        for station in self.held(6) {
            let at = Self::outside(station, SLAB + 0.25);
            if at.y < CLEAR {
                continue;
            }
            member(
                &mut mesh,
                self.local(at.x, at.y, frames[0]),
                self.local(at.x, at.y, frames[1]),
                0.34,
            );
            for side in frames {
                member(
                    &mut mesh,
                    self.local(at.x, at.y, side),
                    self.local(at.x, 0.0, side),
                    0.26,
                );
            }
        }
        // Where it is low: a trestle under it, a post under each edge and a bar between.
        for station in self.held(4) {
            let at = Self::outside(station, SLAB + 0.12);
            if at.y < 0.5 || at.y >= CLEAR {
                continue;
            }
            let middle = station.run * slide;
            let (left, right) = (middle - WIDE * 0.5, middle + WIDE * 0.5);
            member(
                &mut mesh,
                self.local(at.x, at.y, left - 0.4),
                self.local(at.x, at.y, right + 0.4),
                0.26,
            );
            for edge in [left, right] {
                member(
                    &mut mesh,
                    self.local(at.x, at.y, edge),
                    self.local(at.x, 0.0, edge),
                    0.22,
                );
            }
        }
        mesh
    }

    /// What of the frame stops a truck, on the map: the two frames, each as a wall, and each
    /// trestle's posts.
    pub fn blockers(&self) -> Vec<Blocker> {
        let mut blockers = Vec::new();
        let (axis, _) = self.axes();
        let axis = self.placed.transform_vector3(axis);
        // A box whose long side lies along the loop's side view.
        let yaw_degrees = (-axis.z).atan2(axis.x).to_degrees();
        let (low, high) = self
            .stations
            .iter()
            .map(|station| Self::outside(station, SLAB + 0.25))
            .filter(|at| at.y > 0.2)
            .fold((f32::MAX, f32::MIN), |(low, high), at| {
                (low.min(at.x), high.max(at.x))
            });
        let tall = self.height() + 1.0;
        for side in self.frames() {
            let middle = self.local((low + high) * 0.5, tall * 0.5, side);
            blockers.push(Blocker {
                centre: self.placed.transform_point3(middle),
                size: Vec3::new(high - low + 1.2, tall, 0.4),
                yaw_degrees,
            });
        }
        let slide = self.skew().tan();
        for station in self.held(4) {
            let at = Self::outside(station, SLAB + 0.12);
            if at.y < 0.5 || at.y >= CLEAR {
                continue;
            }
            let middle = station.run * slide;
            for edge in [middle - WIDE * 0.5, middle + WIDE * 0.5] {
                let post = self.local(at.x, at.y * 0.5, edge);
                blockers.push(Blocker {
                    centre: self.placed.transform_point3(post),
                    size: Vec3::new(0.3, at.y, 0.3),
                    yaw_degrees,
                });
            }
        }
        blockers
    }

    /// The ground the loop stands over, on the map, as places a little way apart: the deck's
    /// own shadow. It is hard standing.
    pub fn footprint(&self) -> Vec<Vec2> {
        let mut places = Vec::new();
        let length = self.length();
        let half = WIDE * 0.5 + RAIL.0 + 0.5;
        let mut run = -LEAD - 1.0;
        while run <= length + LEAD + 1.0 {
            let mut across = -half;
            while across <= half {
                let at = self.placed.transform_point3(self.on_flat(run, across).0);
                places.push(Vec2::new(at.x, at.z));
                across += 0.5;
            }
            run += 0.5;
        }
        places
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loop_closes_on_the_ground_and_is_a_teardrop() {
        let ring = Ring::standing(Vec3::ZERO, 0.0);
        let (first, last) = (ring.stations[0], *ring.stations.last().unwrap());
        // It leaves the ground level and meets it level, a full turn on.
        assert!(first.up == 0.0 && first.turn == 0.0);
        assert!(last.up.abs() < 0.02, "{}", last.up);
        assert!((last.turn - std::f32::consts::TAU).abs() < 1.0e-4);
        // Stations are a step apart, all the way round, and nowhere doubled.
        for pair in ring.stations.windows(2) {
            let step = pair[1].run - pair[0].run;
            assert!((STEP * 0.5..STEP * 1.6).contains(&step), "{step}");
        }
        // Taller than it is wide, with its top curved as tightly as it is meant to be.
        let (low, high) = ring
            .stations
            .iter()
            .fold((f32::MAX, f32::MIN), |(low, high), station| {
                (low.min(station.along), high.max(station.along))
            });
        assert!(
            ring.height() > high - low,
            "{} {}",
            ring.height(),
            high - low
        );
        assert!((18.0..22.0).contains(&ring.height()), "{}", ring.height());
        let top = ring.stations.len() / 2;
        let (a, b) = (ring.stations[top - 1], ring.stations[top + 1]);
        let radius = (b.run - a.run) / (b.turn - a.turn);
        assert!((radius - TOP).abs() < 0.2, "{radius}");
        // A truck 3.6 m high, upside down at the top, has its roof clear of the deck below
        // it by more than its own height again.
        assert!(ring.height() - 2.0 * 3.6 > 3.6);
        // And one 3.9 m wide has more than a metre of flat either side of it.
        assert!(FLAT > 3.9 + 2.0);
    }

    #[test]
    fn the_way_off_lies_clear_of_the_way_on() {
        let ring = Ring::standing(Vec3::ZERO, 0.0);
        let length = ring.length();
        // Wherever the deck is low enough to meet a truck on another part of it, the two
        // parts are a deck's width and both rails apart, and more.
        let room = WIDE + 2.0 * RAIL.0 + 0.5;
        let low: Vec<(Vec3, f32)> = (0..=(length / 0.25) as usize)
            .map(|k| k as f32 * 0.25)
            .map(|run| (ring.on_deck(run, 0.0).0, run))
            .filter(|(at, _)| at.y < 4.6)
            .collect();
        for (a, run_a) in &low {
            for (b, run_b) in &low {
                if (run_a - run_b).abs() < 25.0 {
                    continue;
                }
                let (run, aside) = ring.axes();
                let apart = (*a - *b).dot(run).abs();
                if apart < 4.0 {
                    let across = (*a - *b).dot(aside).abs();
                    assert!(across > room, "at {run_a} and {run_b}: {across}");
                }
            }
        }
        // It comes off beside where it went on, to the right, heading as it was: two places
        // on the plate it leaves by are in line with the way it came.
        let (off, _) = ring.on_deck(length + 1.0, 0.0);
        let (further, _) = ring.on_deck(length + 4.0, 0.0);
        assert!((9.0..16.0).contains(&off.x), "{off}");
        assert!(
            (further.x - off.x).abs() < 1.0e-3 && further.z < off.z,
            "{off} {further}"
        );
    }

    #[test]
    fn the_deck_is_a_trough_that_faces_whoever_is_on_it() {
        let ring = Ring::standing(Vec3::new(10.0, 2.0, -5.0), 90.0);
        let meshes = ring.meshes();
        // Every triangle of the deck faces the way its points say they do.
        for triangle in meshes.deck.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| meshes.deck.vertices[triangle[k] as usize]);
            let faces = (b.position - a.position).cross(c.position - a.position);
            assert!(faces.dot(a.normal) > 0.0);
        }
        // Half way up, its flat is flat and its shoulders stand up off it and lean in.
        let run = ring.length() * 0.25;
        let (middle, facing) = ring.on_deck(run, 0.0);
        let (edge, _) = ring.on_deck(run, FLAT * 0.5);
        assert!((edge - middle).dot(facing).abs() < 1.0e-3);
        for side in [-1.0, 1.0] {
            let (top, leans) = ring.on_deck(run, side * WIDE * 0.5);
            let lift = (top - middle).dot(facing);
            assert!((0.6..0.75).contains(&lift), "{lift}");
            let inward = (middle - top).normalize();
            assert!(
                leans.dot(inward) > 0.5 && leans.dot(facing) > 0.5,
                "{leans}"
            );
        }
        // Where a truck drives on, there are no shoulders yet: the plate is flat.
        let (on, _) = ring.on_deck(-LEAD, WIDE * 0.5);
        assert!(on.y.abs() < 1.0e-4, "{on}");
        // On the map, a truck drives on to it heading along -x, from 30 m back.
        let (from, yaw) = ring.approach(30.0);
        assert!(
            (from - Vec3::new(40.0, 2.0, -5.0)).length() < 1.0e-3,
            "{from}"
        );
        assert!((yaw - 90.0).abs() < 1.0e-3, "{yaw}");
        // At the top it faces down.
        let (top, facing) = ring.on_map(ring.length() * 0.5, 0.0);
        assert!(top.y > 19.0 && facing.y < -0.99, "{top} {facing}");
        assert!(!meshes.rails.indices.is_empty() && !meshes.frame.indices.is_empty());
        assert!(ring.blockers().len() > 6);
    }
}
