//! A truck's damage, as it is drawn and driven.
//!
//! `damage.rs` decides what a blow does: this takes the blows the game measured, keeps the
//! player's truck's [`Damage`], and makes it seen and felt. Each piece that is hurt is
//! moved from where it is when sound ([`damage::pose`]): pushed in and crumpled, hanging
//! and swinging, or, torn off, taken off the truck and left lying where it fell (the
//! piece's own instances are what lies there: nothing is made). Lamps that are broken are
//! out, and the driving is told what is wrong with the truck ([`Damage::handicap`]).
//!
//! A scene may have **repair bays**: an actor whose file is `repair_bay.actor`, wherever it
//! is put. A truck that stops in one for a moment is mended. (A crash-test scene's way of
//! starting again: the game's own repairs are a shop's to do, with [`Wrecks::repair`].)

use crate::damage::{
    self, Change, Damage, Does, Fallen, Hang, Hit, Mount, Placed, Stage, Swing, Table,
};
use crate::kit::Fitted;
use glam::{Mat4, Vec3};
use solstice_asset::GameFile;
use solstice_game::scene::ActorLine;
use solstice_game::{Play, GAME_FILE};
use solstice_render::Renderer;

/// The actor file that marks a repair bay, and how near its middle a truck must stop to be
/// mended, in metres, unless its line in the scene says (`radius=6`); and for how long.
const BAY_FILE: &str = "repair_bay.actor";
const BAY_RADIUS: f32 = 4.5;
const BAY_SECONDS: f32 = 0.8;
/// How far over a truck is that is on its side: how much of its up is still up. And how
/// long it lies still there, in seconds, before it is stood in the nearest bay.
const ON_ITS_SIDE: f32 = 0.35;
/// How long what a blow did is shown beside the truck's instruments, in seconds.
const SHOWN_SECONDS: f32 = 8.0;
const LIES_SECONDS: f32 = 2.5;

/// What has become of a piece that is no longer drawn on the truck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gone {
    No,
    /// Not drawn at all: broken glass.
    Hidden,
    /// Off the truck, and lying where it fell.
    Fallen,
}

/// A piece on the truck.
struct Piece {
    name: String,
    instances: Vec<u32>,
    /// The box round it, in the truck's own space.
    bounds: (Vec3, Vec3),
    /// The kit's hinge for it, if it swings: where, whether upright, and how far, in degrees.
    hinge: Option<(Vec3, bool, f32)>,
    /// The piece it rides on, by its place among the pieces: a spare wheel on a door goes
    /// where the door goes.
    on: Option<usize>,
    /// How it is moved from where it is when sound, as of the last frame.
    shown: Mat4,
    /// How it hangs and how far it has swung, while it hangs loose.
    swing: Option<(Hang, Swing)>,
    gone: Gone,
}

/// A piece that has come off: how it fell, and the instances that draw it, each with
/// where it was in the truck's own space when it came off.
struct Lying {
    piece: usize,
    fallen: Fallen,
    instances: Vec<(u32, Mat4)>,
}

/// The player's truck's damage.
pub struct Wrecks {
    pub table: Table,
    damage: Damage,
    pieces: Vec<Piece>,
    /// The same pieces as blows are reckoned against them.
    placed: Vec<Placed>,
    /// What the truck was wearing when its pieces were listed: they are listed again when
    /// that changes.
    built: Option<String>,
    lying: Vec<Lying>,
    /// The scene's repair bays: each where it is, how far round it reaches, and the way a
    /// truck stood in it faces, a yaw in degrees.
    bays: Vec<(Vec3, f32, f32)>,
    /// How long the truck has stood in a repair bay, in seconds; and where it last was,
    /// and how long it has lain still there on its side or its roof.
    stood: f32,
    lain: (Vec3, f32),
    /// The instances of lamps that are out, and how many of its head lamps still light,
    /// of how many it has.
    dark: Vec<u32>,
    lit: (u32, u32),
    /// Whether the damage has changed since it was last drawn.
    stale: bool,
    /// What blows have changed since this was last asked for.
    told: Vec<Change>,
    /// The last blow that did anything, in words, and how long ago it was, in seconds.
    last: (String, f32),
    /// Damage to put on the truck once its pieces are listed: as words say it, and blows.
    staged: Option<String>,
    struck: Vec<Hit>,
}

impl Wrecks {
    /// With the scene's repair bays, if it has any.
    pub fn new(files: &[GameFile]) -> Self {
        let game = files.iter().find(|file| file.name == GAME_FILE);
        let mut bays = Vec::new();
        for line in game.map(|game| game.text.lines()).into_iter().flatten() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some((&"actor", rest)) = words.split_first() else {
                continue;
            };
            let Ok(placed) = ActorLine::parse(rest) else {
                continue;
            };
            if placed.file.rsplit('/').next() != Some(BAY_FILE) {
                continue;
            }
            let said = placed.settings.iter().find(|(name, _)| name == "radius");
            let radius = said.and_then(|(_, value)| value.parse::<f32>().ok());
            bays.push((
                placed.position,
                radius.unwrap_or(BAY_RADIUS),
                placed.yaw_degrees,
            ));
        }
        Self {
            table: Table::built_in(),
            damage: Damage::default(),
            pieces: Vec::new(),
            placed: Vec::new(),
            built: None,
            lying: Vec::new(),
            bays,
            stood: 0.0,
            lain: (Vec3::ZERO, 0.0),
            dark: Vec::new(),
            lit: (0, 0),
            stale: false,
            told: Vec::new(),
            last: (String::new(), f32::MAX),
            staged: None,
            struck: Vec::new(),
        }
    }

    /// The truck's damage: plain data, for a kept game and a repair shop.
    pub fn damage(&self) -> &Damage {
        &self.damage
    }

    /// Puts damage on the truck: what it had when it was last driven.
    pub fn set_damage(&mut self, damage: Damage) {
        self.damage = damage;
        self.stale = true;
    }

    /// Damage as words say it, for a picture (`bumper_front=dented,door_FL=loose`): put on
    /// once the truck's pieces are known, the next frame. A mistake in it is logged then.
    pub fn stage(&mut self, words: &str) {
        self.staged = Some(words.to_owned());
    }

    /// Hits the truck, as the game would have measured it: the next frame.
    pub fn strike(&mut self, hit: Hit) {
        self.struck.push(hit);
    }

    /// Mends every piece: what hangs is straight, and what fell off is back on.
    pub fn repair(&mut self) {
        self.damage.mend_all();
        self.stale = true;
    }

    /// Mends one piece. Whether it was hurt.
    pub fn mend(&mut self, piece: &str) -> bool {
        let mended = self.damage.mend(piece);
        self.stale |= mended;
        mended
    }

    /// What blows have changed since this was last asked: for whoever tells the player.
    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.told)
    }

    /// What the last blow did, to show beside the truck's instruments for a few seconds
    /// after it: how hard it was, and each piece it changed.
    pub fn told(&self) -> String {
        if self.last.1 > SHOWN_SECONDS {
            return String::new();
        }
        format!("   |   {}", self.last.0)
    }

    /// Whether an instance is a lamp that is out.
    pub fn is_dark(&self, instance: u32) -> bool {
        self.dark.contains(&instance)
    }

    /// The share of its head lamps that still light: 1 for a truck with none of its own
    /// pieces to say.
    pub fn beam(&self) -> f32 {
        match self.lit {
            (_, 0) => 1.0,
            (lit, all) => lit as f32 / all as f32,
        }
    }

    /// Lists the truck's pieces afresh, as it is now fitted. Whatever was moved or taken
    /// off is put back first: the damage is kept, and is drawn again on the new list.
    fn list(&mut self, play: &mut Play, kit: &Fitted, renderer: &mut Renderer) {
        for index in 0..self.pieces.len() {
            self.restore(index, play, renderer);
        }
        let car = play
            .car_pose()
            .map(|(at, turned)| Mat4::from_rotation_translation(turned, at).inverse());
        self.pieces.clear();
        for (name, instances) in kit.fitted() {
            // Its box: as its truck's files have it, or else about where the renderer
            // draws it, which is rounder than it is.
            let bounds = kit.piece_box(name).or_else(|| {
                let car = car?;
                let mut found: Option<(Vec3, Vec3)> = None;
                for instance in instances.clone() {
                    let (middle, radius) = renderer.instance_bounds(instance)?;
                    let (middle, reach) = (car.transform_point3(middle), Vec3::splat(radius * 0.6));
                    let (low, high) = found.get_or_insert((middle - reach, middle + reach));
                    (*low, *high) = (low.min(middle - reach), high.max(middle + reach));
                }
                found
            });
            let Some(bounds) = bounds else { continue };
            let hinge = kit.kit.hinges.iter().find(|hinge| hinge.piece == name);
            self.pieces.push(Piece {
                name: name.to_owned(),
                instances: instances.collect(),
                bounds,
                hinge: hinge.map(|hinge| (hinge.at, hinge.upright, hinge.opens)),
                on: None,
                shown: Mat4::IDENTITY,
                swing: None,
                gone: Gone::No,
            });
        }
        for (rider, carrier) in &kit.kit.rides {
            let at = |name: &String| self.pieces.iter().position(|piece| piece.name == *name);
            if let (Some(rider), Some(carrier)) = (at(rider), at(carrier)) {
                self.pieces[rider].on = Some(carrier);
            }
        }
        self.placed = self
            .pieces
            .iter()
            .map(|piece| Placed {
                name: piece.name.clone(),
                low: piece.bounds.0,
                high: piece.bounds.1,
            })
            .collect();
        self.stale = true;
    }

    /// Puts a piece back as it is when sound: on the truck, shown, and where it belongs.
    fn restore(&mut self, index: usize, play: &mut Play, renderer: &mut Renderer) {
        let piece = &mut self.pieces[index];
        let back = piece.shown.inverse();
        match piece.gone {
            Gone::No => {
                if piece.shown != Mat4::IDENTITY {
                    for &instance in &piece.instances {
                        play.move_within(instance, back);
                    }
                }
            }
            Gone::Hidden => {
                for &instance in &piece.instances {
                    renderer.set_instance_hidden(instance, false);
                    play.move_within(instance, back);
                }
            }
            Gone::Fallen => {
                // Each instance was taken off as it was then shown.
                let lying = self.lying.iter().position(|lying| lying.piece == index);
                for (instance, was) in lying
                    .map(|at| self.lying.remove(at).instances)
                    .unwrap_or_default()
                {
                    play.attach(instance, back * was);
                }
            }
        }
        (piece.shown, piece.swing, piece.gone) = (Mat4::IDENTITY, None, Gone::No);
    }

    /// A frame: the truck takes the blows the game measured, and its damage is drawn.
    /// `kit` is the player's truck, if it is one built of pieces: one that is not has
    /// nothing to break.
    pub fn frame(
        &mut self,
        play: &mut Play,
        kit: Option<&Fitted>,
        renderer: &mut Renderer,
        seconds: f32,
    ) {
        let blows = play.take_impacts();
        let Some(kit) = kit else {
            return;
        };
        let wearing = kit.build().to_text();
        if self.built.as_deref() != Some(wearing.as_str()) {
            self.list(play, kit, renderer);
            self.built = Some(wearing);
        }
        if let Some(words) = self.staged.take() {
            match Damage::staged(&self.table, &self.placed, &words) {
                Ok(damage) => self.set_damage(damage),
                Err(error) => log::error!("damage `{words}`: {error}"),
            }
        }
        let hits = blows.iter().map(Hit::from).chain(self.struck.drain(..));
        for hit in hits.collect::<Vec<_>>() {
            let before = self.damage.hurts.len();
            let changes = self.damage.strike(&self.table, &self.placed, &hit);
            // A piece's stage may be as it was and its dent deeper: it is drawn again.
            self.stale |=
                !changes.is_empty() || self.damage.hurts.len() != before || hit.force > 10.0;
            for change in &changes {
                log::info!(
                    "damage: {} is {} ({:.0} kN of a {:.0} kN blow)",
                    change.piece,
                    change.to.name(),
                    change.felt,
                    hit.force
                );
            }
            if !changes.is_empty() {
                let each: Vec<String> = changes
                    .iter()
                    .map(|change| format!("{} {}", change.piece, change.to.name()))
                    .collect();
                self.last = (format!("{:.0} kN: {}", hit.force, each.join(", ")), 0.0);
            }
            self.told.extend(changes);
        }
        self.last.1 += seconds;
        self.mend_in_bay(play, seconds);
        self.draw(play, renderer, seconds);
    }

    /// A truck that stops in a repair bay is mended; and one that has come to rest on its
    /// side or its roof, in a scene with a bay, is stood in the nearest, mended.
    fn mend_in_bay(&mut self, play: &mut Play, seconds: f32) {
        if let (Some((at, turned)), false) = (play.car_pose(), self.bays.is_empty()) {
            let over = (turned * Vec3::Y).y < ON_ITS_SIDE;
            let still = at.distance(self.lain.0) < 0.6 * seconds.max(1.0e-3);
            self.lain = (
                at,
                if over && still {
                    self.lain.1 + seconds
                } else {
                    0.0
                },
            );
            if self.lain.1 >= LIES_SECONDS {
                let far = |bay: &(Vec3, f32, f32)| bay.0.distance_squared(at);
                let nearest = self.bays.iter().min_by(|a, b| far(a).total_cmp(&far(b)));
                if let Some(&(bay, _, yaw)) = nearest {
                    log::info!("damage: stood back in the repair bay, and mended");
                    play.place_car(bay, yaw);
                    self.repair();
                }
                self.lain.1 = 0.0;
            }
        }
        let stopped = play
            .hud()
            .is_some_and(|hud| !hud.afoot && hud.telemetry.speed.abs() < 0.5);
        let inside = play.car_pose().is_some_and(|(at, _)| {
            let near =
                |(bay, radius, _): &(Vec3, f32, f32)| (at.x - bay.x).hypot(at.z - bay.z) < *radius;
            self.bays.iter().any(near)
        });
        if !(stopped && inside) || self.damage.hurts.is_empty() {
            self.stood = 0.0;
            return;
        }
        self.stood += seconds;
        if self.stood >= BAY_SECONDS {
            log::info!("damage: mended in the repair bay");
            self.repair();
            self.stood = 0.0;
        }
    }

    /// Moves every piece to where its damage has it, lets what is loose swing and what
    /// has come off fall, and says what is wrong to the lamps and the driving.
    fn draw(&mut self, play: &mut Play, renderer: &mut Renderer, seconds: f32) {
        let car = play
            .car_pose()
            .map(|(at, turned)| Mat4::from_rotation_translation(turned, at));
        // How the truck is thrown about, in its own space: to its right, up and back.
        let (felt, speed, velocity) = play.hud().map_or((Vec3::ZERO, 0.0, Vec3::ZERO), |hud| {
            let g = hud.telemetry.g;
            (
                Vec3::new(g.y, g.z - 1.0, -g.x) * 9.81,
                hud.telemetry.speed,
                hud.velocity,
            )
        });
        // Where each piece is to be: nothing for one that is off.
        let mut poses: Vec<Option<Mat4>> = Vec::with_capacity(self.pieces.len());
        for piece in &mut self.pieces {
            let part = self.table.part(&piece.name);
            let Some(hurt) = self.damage.hurt(&piece.name) else {
                piece.swing = None;
                poses.push(Some(Mat4::IDENTITY));
                continue;
            };
            let stage = part.stage(hurt.hurt);
            if stage == Stage::Loose {
                if piece.swing.is_none() {
                    let hang = damage::hang(part, hurt, piece.bounds, piece.hinge);
                    piece.swing = hang.map(|hang| (hang, Swing::at_rest(&hang)));
                }
                if let Some((hang, swing)) = &mut piece.swing {
                    swing.advance(hang, felt, speed, seconds);
                }
            } else {
                piece.swing = None;
            }
            let swing = piece
                .swing
                .as_ref()
                .map(|(hang, swing)| (hang, swing.angle));
            poses.push(
                (stage != Stage::Off)
                    .then(|| damage::pose(&self.table, part, hurt, piece.bounds, swing)),
            );
        }
        for index in 0..self.pieces.len() {
            // What rides on another piece goes where that goes, unless it is hurt itself.
            let own = self.damage.hurt(&self.pieces[index].name).is_some();
            let from = self.pieces[index].on.filter(|_| !own).unwrap_or(index);
            let Some(pose) = poses[from] else {
                let glass = self.table.part(&self.pieces[from].name).mount == Mount::Glass;
                self.take_off(index, glass, car, velocity, play, renderer);
                continue;
            };
            if self.pieces[index].gone != Gone::No {
                self.restore(index, play, renderer);
            }
            let piece = &mut self.pieces[index];
            let by = pose * piece.shown.inverse();
            if !by.abs_diff_eq(Mat4::IDENTITY, 1.0e-6) {
                for &instance in &piece.instances {
                    play.move_within(instance, by);
                }
                piece.shown = pose;
            }
        }
        // What has come off falls, and lies.
        for lying in &mut self.lying {
            let rest = lying.fallen.from.w_axis.y;
            // On whatever of the scene is under it, from just over it: not on the truck.
            let ground = |at: Vec3| play.fixed_below(at + Vec3::Y * 0.5).unwrap_or(rest);
            if lying.fallen.advance(seconds, ground) {
                let placed = lying.fallen.placed();
                for (instance, was) in &lying.instances {
                    renderer.move_instance(*instance, placed * *was);
                }
            }
        }
        if !std::mem::take(&mut self.stale) {
            return;
        }
        // The lamps that are out; and of its head lamps, how many still light.
        self.dark.clear();
        self.lit = (0, 0);
        for piece in &self.pieces {
            let out = piece.gone != Gone::No || self.damage.is_out(&self.table, &piece.name);
            if out && self.table.part(&piece.name).does == Does::Lamp {
                self.dark.extend(&piece.instances);
            }
            if piece.name.starts_with("lamp_head") {
                self.lit = (self.lit.0 + u32::from(!out), self.lit.1 + 1);
            }
        }
        play.set_handicap(self.damage.handicap(&self.table));
    }

    /// A piece comes off the truck: its glass is gone, or it falls where the truck is.
    fn take_off(
        &mut self,
        index: usize,
        glass: bool,
        car: Option<Mat4>,
        velocity: Vec3,
        play: &mut Play,
        renderer: &mut Renderer,
    ) {
        let piece = &mut self.pieces[index];
        match (piece.gone, glass) {
            // Whatever else shows a truck's pieces again, broken glass stays gone.
            (Gone::Hidden, _) | (Gone::No, true) => {
                for &instance in &piece.instances {
                    renderer.set_instance_hidden(instance, true);
                }
                piece.gone = Gone::Hidden;
            }
            (Gone::Fallen, _) => {}
            (Gone::No, false) => {
                let Some(from) = car else { return };
                // The way the blow that took it off pushed, in the world.
                let push = self
                    .damage
                    .hurt(&piece.name)
                    .map_or(Vec3::ZERO, |hurt| hurt.push);
                let knocked = from.transform_vector3(push);
                let mut instances = Vec::new();
                for &instance in &piece.instances {
                    if let Some(was) = play.detach(instance) {
                        instances.push((instance, was));
                    }
                }
                // What cannot be taken off, as a wheel cannot, stays on.
                if instances.is_empty() {
                    return;
                }
                piece.gone = Gone::Fallen;
                piece.swing = None;
                self.lying.push(Lying {
                    piece: index,
                    fallen: Fallen::new(&piece.name, from, piece.bounds, velocity, knocked),
                    instances,
                });
            }
        }
    }
}
