//! Gold: what is in the ground, and the nuggets that have come out of it.
//!
//! **In the ground.** When a map is entered, gold is put in the ground of it: some number of
//! places beside its track, a little way off into the bush, different every time
//! ([`Gold::enter`]). Nothing shows where. On foot the driver carries a detector, whose signal
//! is stronger the nearer its coil is to one ([`Gold::signal`]), and standing over one they dig.
//!
//! **A nugget** is made when it is dug up, and not before: its weight, and a number that all
//! the rest of it is drawn from. No file has a nugget in it: the game makes what is seen of
//! it from that number, its shape, what it is made of and its colours (`nugget.rs`). From
//! then on it is a thing in the world. Nothing counts nuggets or keeps them in places of
//! their own: one is in the driver's hand, or it is loose, a body that falls, rolls, lies
//! where it comes to rest and is pushed by what meets it (`Play::add_thing`). Put into the
//! truck's tray it lies in the tray, rides there, weighs on the truck's springs, and can be
//! thrown out of it by a hard enough landing. Put into the chest in the shed it lies in the
//! chest. One button does what there is to do ([`Gold::can`]): dig, pick up the nearest, or
//! put down what is held onto whatever is in front of the driver.
//!
//! **Where each is** is kept with the game ([`Lies`]): loose on a map, where it lay and
//! turned as it lay; or aboard the truck, where it lay in the truck's own space, so that it
//! goes where the truck goes, from map to map. Whoever runs the world says each frame where
//! the bodies are ([`Gold::see`]), and is asked for the bodies there should be
//! ([`Gold::sync`]).
//!
//! **Sold**, what is aboard the truck is weighed and paid for ([`Gold::sell`]).
//!
//! **The detector is a thing too** ([`Detector`]). It is in the driver's hands, or slung on
//! their back with their hands empty, or it has been put down, and lies where it was left as
//! a nugget does: on the ground, in the tray. Hands hold one thing: with the detector in them
//! gold is found and dug, and comes out of the ground onto it; to pick a nugget up the
//! detector is slung, and to find more it is taken in hand again ([`Gold::swap`]).
//!
//! A map has only so many of the nugget's model to show nuggets with ([`Gold::place`]): those
//! in sight are given them, the nearest things first.
use glam::{Quat, Vec3};
use solstice_asset::GameFile;

/// How far off the detector hears gold, and how near the driver stands to dig it: in metres.
const HEARS: f32 = 15.0;
const DIGS: f32 = 1.7;
/// How near a nugget is to be picked up, how near the edge of the truck's tray the driver
/// stands to put one in it, and how near the chest.
const REACH: f32 = 1.7;
const TRAY_REACH: f32 = 1.6;
const CHEST_REACH: f32 = 1.7;
/// How many places a map's ground is given.
const BURIED: usize = 14;
/// The chest, inside: across, deep, and long (`tools/blender/shed/build.py`).
const CHEST: Vec3 = Vec3::new(0.46, 0.42, 1.05);
/// How far from a wall of the tray or the chest a nugget is let go.
const INSET: f32 = 0.22;

/// Where a nugget is.
#[derive(Clone, Debug, PartialEq)]
pub enum Lies {
    /// In the driver's hand.
    Hand,
    /// Loose on a map: where it lies, and how it is turned. On the ground, in the chest,
    /// wherever it came to rest.
    Loose { map: String, at: Vec3, turned: Quat },
    /// Aboard the truck, in its tray: where it lies in the truck's own space.
    Aboard { at: Vec3, turned: Quat },
}

/// The number the detector's body goes by, among the nuggets': none of theirs.
pub const DETECTOR: u32 = u32::MAX - 1;
/// The box the detector falls and lies as, along its shaft, and what it weighs.
const DETECTOR_BODY: Vec3 = Vec3::new(0.3, 0.12, 1.45);
const DETECTOR_KILOS: f32 = 2.2;

/// Where the detector is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Detector {
    /// In the driver's hands: it hears gold, and the button digs.
    #[default]
    Hand,
    /// Slung on the driver's back: with them, and their hands are empty.
    Slung,
    /// Put down: loose on a map, or aboard the truck.
    Left(Lies),
}

/// How the detector is to be shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Carried {
    /// Not at all: slung, or the driver is in the truck, or it was left on another map.
    Away,
    /// In the driver's hand, its coil over the ground ahead of them.
    Held,
    /// Lying where its body is: there, turned so.
    Lying(Vec3, Quat),
}

/// The shape a nugget is, drawn from its number: no two are alike, and few are a plain lump.
/// Water-worn gold is flat more often than round, and comes as slugs, beans, flakes, hooks
/// and drawn-out, pinched and twisted things as well as knobs. The game makes each when it
/// is found, point by point (`nugget.rs`), and it falls and lies as a box of about its own
/// length, thickness and width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Form {
    /// How long, how thick and how wide it is, against the model's own: its length lies
    /// along its x, and it lies on its flat.
    pub long: f32,
    pub thick: f32,
    pub wide: f32,
    /// How much thicker one end is than the other: nothing, evenly thick; one way or the
    /// other, a teardrop.
    pub taper: f32,
    /// How much it is pinched in at its middle: a peanut.
    pub pinch: f32,
    /// How far its ends are bent round, in radians each: a bean, a hook.
    pub bend: f32,
    /// How far its ends are twisted against each other, in radians each.
    pub twist: f32,
    /// How far its ends are lifted or dropped out of its flat, in radians each: a saddle.
    pub arch: f32,
    /// How knobbed it is: how far its lobes and knobs stand out of it.
    pub knobs: f32,
}

impl Form {
    /// A number from 0 to 1 for each thing about a nugget that is its own.
    pub fn chance(shape: u32, which: u32) -> f32 {
        let mut mixed = shape.wrapping_mul(0x9E37_79B1) ^ which.wrapping_mul(0x85EB_CA6B);
        mixed ^= mixed >> 15;
        mixed = mixed.wrapping_mul(0x2C1B_3C6D);
        mixed ^= mixed >> 12;
        (mixed >> 8) as f32 / (1 << 24) as f32
    }

    /// The shape of the nugget with this number.
    pub fn of(shape: u32) -> Self {
        let chance = |which: u32| Self::chance(shape, which);
        // Most are a little longer than they are wide, and some are a great deal longer:
        // from a button to a finger of gold. Some are leaves, some all but balls; some are
        // broad, some narrow as wire.
        let long = 0.8 + 1.9 * chance(102).powi(2);
        let thick = 0.2 + 0.8 * chance(101).powf(1.4);
        let wide = ((1.45 / long).clamp(0.42, 1.2) * (0.6 + 0.8 * chance(103))).max(0.3);
        // The stranger things are rarer: each is strong in few, and many have none of it.
        let rare = |which: u32, power: i32| chance(which).powi(power);
        Self {
            long,
            thick,
            wide,
            taper: (chance(104) - 0.5) * 1.8 * rare(105, 1),
            pinch: 0.72 * rare(106, 3),
            bend: (chance(107) - 0.5) * 3.4 * rare(108, 2),
            twist: (chance(109) - 0.5) * 1.6 * rare(110, 2),
            arch: (chance(116) - 0.5) * 1.3 * rare(117, 2),
            knobs: 0.4 + 1.1 * chance(111),
        }
    }

    /// How thick and wide it is at a place along its length, from -1 at one end to 1 at
    /// the other, against its middle.
    pub fn girth(&self, along: f32) -> f32 {
        ((1.0 + self.taper * along) * (1.0 - self.pinch * (1.0 - along * along))).max(0.22)
    }
}

/// A nugget: dug up, weighed, and somewhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Nugget {
    pub id: u32,
    pub kilos: f32,
    /// What its shape is drawn from.
    pub shape: u32,
    pub lies: Lies,
}

impl Nugget {
    /// How big it is drawn, for its weight: across, in metres. Bigger than gold is, to be
    /// seen, but small where it is light: an ounce or two is a thing to be looked for.
    pub fn size(&self) -> f32 {
        // (And no two of a weight are of a size: one is solid, one is spongy and full of holes.)
        (0.035 + 0.075 * self.kilos.cbrt()) * (0.8 + 0.5 * Form::chance(self.shape, 112))
    }

    /// The shape it is.
    pub fn form(&self) -> Form {
        Form::of(self.shape)
    }

    /// The box it falls and lies as: about as long, as thick and as wide as it is.
    pub fn body(&self) -> Vec3 {
        let (size, form) = (self.size(), self.form());
        (Vec3::new(form.long, form.thick, form.wide) * size * 0.8).max(Vec3::splat(0.025))
    }
}

/// What the button does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Dig,
    PickUp,
    IntoTray,
    IntoChest,
    FromTray,
    FromChest,
    PutDown,
    /// Pick the detector up from where it was put down.
    Take,
}

impl Use {
    /// Its name, as the pages are told it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dig => "dig",
            Self::PickUp => "pick",
            Self::IntoTray => "tray",
            Self::IntoChest => "chest",
            Self::FromTray => "untray",
            Self::FromChest => "unchest",
            Self::PutDown => "drop",
            Self::Take => "take",
        }
    }
}

/// Where the driver and the truck are, for working out what is in reach.
#[derive(Clone, Copy, Debug)]
pub struct Stands<'a> {
    pub map: &'a str,
    pub walker: Vec3,
    /// The way the driver faces: a yaw in radians (zero looks along -z).
    pub yaw: f32,
    pub truck: Vec3,
    pub turned: Quat,
    /// The truck's tray, in the truck's own space: its size inside, across, deep and long,
    /// and the middle of its floor.
    pub tray: Option<(Vec3, Vec3)>,
}

impl Stands<'_> {
    /// A place in the world, in the truck's own space; and the other way.
    fn aboard(&self, at: Vec3) -> Vec3 {
        self.turned.inverse() * (at - self.truck)
    }

    fn ashore(&self, at: Vec3) -> Vec3 {
        self.truck + self.turned * at
    }

    /// Whether a place is in the tray: within its walls, or heaped a little above them.
    fn in_tray(&self, at: Vec3) -> bool {
        let Some((size, floor)) = self.tray else {
            return false;
        };
        let from = self.aboard(at) - floor;
        from.x.abs() < size.x * 0.5 + 0.02
            && from.z.abs() < size.z * 0.5 + 0.02
            && from.y > -0.1
            && from.y < size.y + 0.5
    }

    /// Where something is let go into the tray from where the driver stands, if they stand
    /// by it: above the part of its floor nearest them, clear of its walls.
    fn over_tray(&self) -> Option<Vec3> {
        let (size, floor) = self.tray?;
        let from = self.aboard(self.walker) - floor;
        let half = Vec3::new(size.x * 0.5, 0.0, size.z * 0.5);
        let nearest = from.clamp(-half, half);
        let away = (from.x - nearest.x).hypot(from.z - nearest.z);
        // By it is alongside it or behind it: not by the cab's door, ahead of it.
        (away < TRAY_REACH && from.z > -half.z - 0.35).then(|| {
            let room = (half - Vec3::splat(INSET)).max(Vec3::ZERO);
            let over = from.clamp(-room, room);
            self.ashore(floor + Vec3::new(over.x, size.y + 0.5, over.z))
        })
    }
}

/// The chest in the shed: the middle of its floor, inside, and how it is turned about the
/// upright, in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Chest {
    at: Vec3,
    yaw: f32,
}

impl Chest {
    /// Whether a place is in it, or heaped a little above its rim.
    fn holds(&self, at: Vec3) -> bool {
        let from = Quat::from_rotation_y(self.yaw).inverse() * (at - self.at);
        from.x.abs() < CHEST.x * 0.5 + 0.03
            && from.z.abs() < CHEST.z * 0.5 + 0.03
            && from.y > -0.1
            && from.y < CHEST.y + 0.45
    }

    /// Where something is let go into it: above its floor, `along` its length from 0 to 1.
    fn over(&self, along: f32) -> Vec3 {
        let reach = (CHEST.z - 2.0 * INSET).max(0.0);
        self.at
            + Quat::from_rotation_y(self.yaw)
                * Vec3::new(0.0, CHEST.y + 0.45, (along - 0.5) * reach)
    }
}

/// One of a scene's blanks, showing a nugget: where, turned how, how big, and the number
/// the nugget is made from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub slot: usize,
    pub at: Option<(Vec3, Quat, f32)>,
    pub shape: u32,
}

/// What is asked of whoever runs the world: a nugget's body put in it, or taken out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Body {
    /// A body for this nugget: a box of this size and weight, there, turned so. If it is to
    /// `settle`, it is let go from there, and lies on whatever is under it: the ground, the
    /// floor of the tray or the chest, another nugget.
    Put {
        id: u32,
        at: Vec3,
        turned: Quat,
        size: Vec3,
        kilos: f32,
        settle: bool,
    },
    /// This nugget's body is taken out of the world.
    Take { id: u32 },
}

/// The gold of a game.
#[derive(Clone, Debug, Default)]
pub struct Gold {
    /// Where gold is in the ground of the map being played.
    buried: Vec<Vec3>,
    pub nuggets: Vec<Nugget>,
    pub detector: Detector,
    next: u32,
    rolls: u64,
    /// The map's chest, if it has one.
    chest: Option<Chest>,
    /// What each of the scene's blanks was last told to show: a nugget is made on one of
    /// them when it comes in sight (`nugget.rs`).
    shown: Vec<Option<(u32, Vec3, Quat)>>,
    /// The nuggets that are bodies in the world of the map being played.
    bodied: Vec<u32>,
    /// What has been asked of the world and not yet taken.
    asked: Vec<Body>,
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

impl Gold {
    /// The gold there was when the game was last kept: a line a nugget, `nugget ID KILOS SHAPE`
    /// then where it is: `hand`, `loose MAP X Y Z QX QY QZ QW` or `aboard X Y Z QX QY QZ QW`.
    /// A game kept before nuggets were things said `ground MAP X Y Z YAW`, `tray PLACE` or
    /// `chest`: those on the ground are where they were, and the rest are in the tray.
    pub fn read(kept: &str) -> Self {
        let mut gold = Self::default();
        for line in kept.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            // The detector: `detector hand`, `detector slung`, or where it was put down, as a
            // nugget's is said.
            if let ["detector", lies @ ..] = &words[..] {
                let numbers: Vec<f32> = lies
                    .iter()
                    .skip(1)
                    .filter_map(|word| word.parse().ok())
                    .collect();
                gold.detector = match (lies.first().copied(), &numbers[..]) {
                    (Some("slung"), []) => Detector::Slung,
                    (Some("loose"), [x, y, z, qx, qy, qz, qw]) if lies.len() == 9 => {
                        Detector::Left(Lies::Loose {
                            map: lies[1].to_owned(),
                            at: Vec3::new(*x, *y, *z),
                            turned: Quat::from_xyzw(*qx, *qy, *qz, *qw).normalize(),
                        })
                    }
                    (Some("aboard"), [x, y, z, qx, qy, qz, qw]) => Detector::Left(Lies::Aboard {
                        at: Vec3::new(*x, *y, *z),
                        turned: Quat::from_xyzw(*qx, *qy, *qz, *qw).normalize(),
                    }),
                    _ => Detector::Hand,
                };
                continue;
            }
            let ["nugget", id, kilos, shape, lies @ ..] = &words[..] else {
                continue;
            };
            let numbers: Vec<f32> = lies
                .iter()
                .skip(1)
                .filter_map(|word| word.parse().ok())
                .collect();
            let lies = match (lies.first().copied(), &numbers[..]) {
                (Some("hand"), []) => Lies::Hand,
                // The map's name is not a number: seven follow it.
                (Some("loose"), [x, y, z, qx, qy, qz, qw]) if lies.len() == 9 => Lies::Loose {
                    map: lies[1].to_owned(),
                    at: Vec3::new(*x, *y, *z),
                    turned: Quat::from_xyzw(*qx, *qy, *qz, *qw).normalize(),
                },
                (Some("aboard"), [x, y, z, qx, qy, qz, qw]) => Lies::Aboard {
                    at: Vec3::new(*x, *y, *z),
                    turned: Quat::from_xyzw(*qx, *qy, *qz, *qw).normalize(),
                },
                (Some("ground"), [x, y, z, yaw]) if lies.len() == 6 => Lies::Loose {
                    map: lies[1].to_owned(),
                    at: Vec3::new(*x, *y + 0.15, *z),
                    turned: Quat::from_rotation_y(*yaw),
                },
                (Some("tray" | "chest"), _) => {
                    // As they were laid in the tray before: in rows, on its floor.
                    let place = gold.nuggets.len() % 24;
                    let (across, deep, up) = (place % 4, place / 4 % 3, place / 12);
                    Lies::Aboard {
                        at: Vec3::new(
                            (across as f32 - 1.5) * 0.32,
                            1.1 + up as f32 * 0.3,
                            1.9 + (deep as f32 - 1.0) * 0.45,
                        ),
                        turned: Quat::IDENTITY,
                    }
                }
                _ => continue,
            };
            if let (Ok(id), Ok(kilos), Ok(shape)) = (id.parse(), kilos.parse(), shape.parse()) {
                gold.next = gold.next.max(id + 1);
                gold.nuggets.push(Nugget {
                    id,
                    kilos,
                    shape,
                    lies,
                });
            }
        }
        gold
    }

    /// Adds the gold to what is kept of the game.
    pub fn keep(&self, kept: &mut String) {
        use std::fmt::Write as _;
        let pose = |at: &Vec3, turned: &Quat| {
            format!(
                "{:.3} {:.3} {:.3} {:.4} {:.4} {:.4} {:.4}",
                at.x, at.y, at.z, turned.x, turned.y, turned.z, turned.w
            )
        };
        let _ = match &self.detector {
            Detector::Hand => writeln!(kept, "detector hand"),
            Detector::Slung => writeln!(kept, "detector slung"),
            Detector::Left(Lies::Loose { map, at, turned }) => {
                writeln!(kept, "detector loose {map} {}", pose(at, turned))
            }
            Detector::Left(Lies::Aboard { at, turned }) => {
                writeln!(kept, "detector aboard {}", pose(at, turned))
            }
            Detector::Left(Lies::Hand) => writeln!(kept, "detector hand"),
        };
        for nugget in &self.nuggets {
            let _ = write!(
                kept,
                "nugget {} {:.2} {} ",
                nugget.id, nugget.kilos, nugget.shape
            );
            let _ = match &nugget.lies {
                Lies::Hand => writeln!(kept, "hand"),
                Lies::Loose { map, at, turned } => {
                    writeln!(kept, "loose {map} {}", pose(at, turned))
                }
                Lies::Aboard { at, turned } => writeln!(kept, "aboard {}", pose(at, turned)),
            };
        }
    }

    /// A number from 0 to 1, different every time.
    fn roll(&mut self) -> f32 {
        self.rolls ^= self.rolls << 13;
        self.rolls ^= self.rolls >> 7;
        self.rolls ^= self.rolls << 17;
        (self.rolls >> 40) as f32 / (1_u64 << 24) as f32
    }

    /// A map has been entered: gold is put in its ground, beside its track, where `chance`
    /// (any number, different each time) says. `files` are the map's gameplay files: its track
    /// (`*.track`, a line a point, `p X Y Z LEFT_X LEFT_Y LEFT_Z HALF_LEFT HALF_RIGHT`), and
    /// what it has to show nuggets with and where its chest is (`*.items`: `nugget INSTANCE
    /// POSE` a line a model, and `chest X Y Z YAW`). Its world is new: no nugget has a body
    /// in it yet.
    pub fn enter(&mut self, files: &[GameFile], chance: u64) {
        self.rolls = chance | 1;
        for _ in 0..8 {
            self.roll();
        }
        self.buried.clear();
        self.bodied.clear();
        self.asked.clear();
        self.chest = None;
        let mut slots = 0;
        let mut track = Vec::new();
        for file in files {
            for line in file.text.lines() {
                let words: Vec<&str> = line.split_whitespace().collect();
                let numbers: Vec<f32> = words
                    .iter()
                    .skip(1)
                    .filter_map(|w| w.parse().ok())
                    .collect();
                match (
                    file.name.rsplit('.').next(),
                    words.first().copied(),
                    &numbers[..],
                ) {
                    (Some("track"), Some("p"), [x, y, z, lx, _, lz, left, right, ..]) => {
                        track.push((
                            Vec3::new(*x, *y, *z),
                            Vec3::new(*lx, 0.0, *lz),
                            *left,
                            *right,
                        ));
                    }
                    // What the scene was given to show nuggets with, as it was read.
                    (Some("slots"), Some("slot"), [_, _]) => slots += 1,
                    (Some("items"), Some("chest"), [x, y, z, yaw]) => {
                        self.chest = Some(Chest {
                            at: Vec3::new(*x, *y, *z),
                            yaw: *yaw,
                        });
                    }
                    _ => {}
                }
            }
        }
        self.shown = vec![None; slots];
        // Only a track has gold in its ground: a hub has a road and a shed, a town a street,
        // and gold is brought to them.
        let found_on = files.iter().any(|file| {
            file.name.ends_with(".places")
                && file.text.lines().any(|line| line.starts_with("place "))
        });
        if found_on || track.is_empty() {
            return;
        }
        for _ in 0..BURIED {
            let (at, left, half_left, half_right) =
                track[(self.roll() * track.len() as f32) as usize % track.len()];
            let (side, half) = if self.roll() < 0.5 {
                (1.0, half_left)
            } else {
                (-1.0, half_right)
            };
            let off = half + 5.0 + self.roll() * 22.0;
            self.buried.push(at + left.normalize_or_zero() * side * off);
        }
    }

    /// Puts gold in the ground at a place: for a trial, which cannot go looking for it.
    pub fn bury(&mut self, at: Vec3) {
        self.buried.push(at);
    }

    /// How strong the detector's signal is where the driver stands, from 0, nothing, to 1,
    /// right over gold: what is in the ground, and a nugget lying loose on it, as one that
    /// has been thrown out of the tray is.
    pub fn signal(&self, stands: &Stands) -> f32 {
        let lying = self.nuggets.iter().filter_map(|nugget| match &nugget.lies {
            Lies::Loose { map, at, .. }
                if map == stands.map && !self.chest.is_some_and(|chest| chest.holds(*at)) =>
            {
                Some(*at)
            }
            _ => None,
        });
        let nearest = self
            .buried
            .iter()
            .copied()
            .chain(lying)
            .map(|at| flat(at, stands.walker))
            .fold(f32::MAX, f32::min);
        (1.0 - nearest / HEARS).clamp(0.0, 1.0)
    }

    fn held(&self) -> Option<usize> {
        self.nuggets
            .iter()
            .position(|nugget| nugget.lies == Lies::Hand)
    }

    /// The nugget in the driver's hand.
    pub fn in_hand(&self) -> Option<&Nugget> {
        self.held().map(|index| &self.nuggets[index])
    }

    fn count(&self, of: impl Fn(&Nugget) -> bool) -> (usize, f32) {
        self.nuggets
            .iter()
            .filter(|nugget| of(nugget))
            .fold((0, 0.0), |(count, kilos), nugget| {
                (count + 1, kilos + nugget.kilos)
            })
    }

    /// How many nuggets are aboard the truck, in its tray, and what they weigh.
    pub fn tray(&self) -> (usize, f32) {
        self.count(|nugget| matches!(nugget.lies, Lies::Aboard { .. }))
    }

    /// How many nuggets are in the chest of the map being played, and what they weigh.
    pub fn in_chest(&self, map: &str) -> (usize, f32) {
        let Some(chest) = self.chest else {
            return (0, 0.0);
        };
        self.count(|nugget| matches!(&nugget.lies, Lies::Loose { map: on, at, .. } if on == map && chest.holds(*at)))
    }

    /// Where a nugget is in the world of the map being played, if it is in it and loose.
    fn pose(nugget: &Nugget, stands: &Stands) -> Option<(Vec3, Quat)> {
        Self::lying(&nugget.lies, stands)
    }

    /// Where something that lies somewhere is in the world of the map being played.
    fn lying(lies: &Lies, stands: &Stands) -> Option<(Vec3, Quat)> {
        match lies {
            Lies::Loose { map, at, turned } if map == stands.map => Some((*at, *turned)),
            Lies::Aboard { at, turned } => Some((stands.ashore(*at), stands.turned * *turned)),
            _ => None,
        }
    }

    /// The nugget nearest the driver that they can reach: on the ground at their feet, in
    /// the tray beside them, in the chest.
    fn in_reach(&self, stands: &Stands) -> Option<usize> {
        self.nuggets
            .iter()
            .enumerate()
            .filter_map(|(index, nugget)| {
                let (at, _) = Self::pose(nugget, stands)?;
                let up = at.y - stands.walker.y;
                // Whatever is in the tray is in reach of somebody standing by the tray, and
                // whatever is in the chest of somebody standing by the chest: all of it.
                let aboard =
                    matches!(nugget.lies, Lies::Aboard { .. }) && stands.over_tray().is_some();
                let chested = self.by_chest(stands).is_some_and(|chest| chest.holds(at));
                ((flat(at, stands.walker) < REACH || aboard || chested)
                    && (-0.6..2.1).contains(&up))
                .then_some((index, at))
            })
            .min_by(|a, b| {
                a.1.distance(stands.walker)
                    .total_cmp(&b.1.distance(stands.walker))
            })
            .map(|(index, _)| index)
    }

    fn by_chest(&self, stands: &Stands) -> Option<Chest> {
        self.chest
            .filter(|chest| flat(chest.at, stands.walker) < CHEST_REACH)
    }

    /// How far off the detector lies, if it has been put down within reach of the driver.
    fn detector_near(&self, stands: &Stands) -> Option<f32> {
        let Detector::Left(lies) = &self.detector else {
            return None;
        };
        let (at, _) = Self::lying(lies, stands)?;
        let up = at.y - stands.walker.y;
        let aboard = matches!(lies, Lies::Aboard { .. }) && stands.over_tray().is_some();
        ((flat(at, stands.walker) < REACH + 0.5 || aboard) && (-0.6..2.1).contains(&up))
            .then(|| at.distance(stands.walker))
    }

    /// What the button would do where the driver stands, on foot. Their hands hold one
    /// thing. A nugget in them is put down. The detector in them digs where it is over gold,
    /// and is slung to pick a nugget up. Empty, they pick up what is nearest: a nugget, or
    /// the detector from where it was left.
    pub fn can(&self, stands: &Stands) -> Option<Use> {
        if self.held().is_some() {
            return Some(if stands.over_tray().is_some() {
                Use::IntoTray
            } else if self.by_chest(stands).is_some() {
                Use::IntoChest
            } else {
                Use::PutDown
            });
        }
        let detecting = self.detector == Detector::Hand;
        if detecting && self.buried.iter().any(|at| flat(*at, stands.walker) < DIGS) {
            return Some(Use::Dig);
        }
        let nugget = self.in_reach(stands).map(|index| {
            let nugget = &self.nuggets[index];
            let away =
                Self::pose(nugget, stands).map_or(f32::MAX, |(at, _)| at.distance(stands.walker));
            (index, away)
        });
        match (nugget, self.detector_near(stands)) {
            (Some((_, away)), Some(detector)) if detector < away => Some(Use::Take),
            (None, Some(_)) => Some(Use::Take),
            (Some((index, _)), _) => Some(match &self.nuggets[index].lies {
                Lies::Aboard { .. } => Use::FromTray,
                Lies::Loose { at, .. } if self.chest.is_some_and(|chest| chest.holds(*at)) => {
                    Use::FromChest
                }
                _ => Use::PickUp,
            }),
            (None, None) => None,
        }
    }

    /// Slings the detector, or takes it in hand again: whether that could be done. Not
    /// with a nugget in the hands, nor when the detector has been put down somewhere.
    pub fn swap(&mut self) -> bool {
        if self.held().is_some() {
            return false;
        }
        self.detector = match self.detector {
            Detector::Hand => Detector::Slung,
            Detector::Slung => Detector::Hand,
            Detector::Left(_) => return false,
        };
        true
    }

    /// Puts down what is in the driver's hands: a nugget, as the button does, or the
    /// detector, which then lies there, a thing like any other, until it is picked up.
    pub fn put_down(&mut self, stands: &Stands) -> Option<Use> {
        if self.held().is_some() {
            return self.act(stands);
        }
        if self.detector != Detector::Hand {
            return None;
        }
        let ahead = Vec3::new(-stands.yaw.sin(), 0.0, -stands.yaw.cos());
        // Into the tray, lying along it, if the driver stands by it; else on the ground.
        let (over, turned, aboard) = match stands.over_tray() {
            Some(_) => {
                let (size, floor) = stands.tray.expect("it has a tray to stand by");
                (
                    stands.ashore(floor + Vec3::Y * (size.y + 0.4)),
                    stands.turned,
                    true,
                )
            }
            None => (
                stands.walker + ahead * 0.9 + Vec3::Y * 1.2,
                Quat::from_rotation_y(stands.yaw),
                false,
            ),
        };
        self.detector = Detector::Left(if aboard {
            Lies::Aboard {
                at: stands.aboard(over),
                turned: stands.turned.inverse() * turned,
            }
        } else {
            Lies::Loose {
                map: stands.map.to_owned(),
                at: over,
                turned,
            }
        });
        self.asked.push(Body::Put {
            id: DETECTOR,
            at: over,
            turned,
            size: DETECTOR_BODY,
            kilos: DETECTOR_KILOS,
            settle: true,
        });
        self.bodied.push(DETECTOR);
        Some(Use::PutDown)
    }

    /// How the detector is to be shown now.
    pub fn carried(&self, stands: &Stands, afoot: bool) -> Carried {
        match &self.detector {
            Detector::Hand if afoot => Carried::Held,
            Detector::Left(lies) => match Self::lying(lies, stands) {
                Some((at, turned)) => Carried::Lying(at, turned),
                None => Carried::Away,
            },
            _ => Carried::Away,
        }
    }

    /// Lets go of the nugget in the hand over a place: it is a body there, and falls onto
    /// whatever is under it.
    fn let_go(&mut self, stands: &Stands, over: Vec3, aboard: bool) {
        let Some(index) = self.held() else { return };
        let nugget = &mut self.nuggets[index];
        let turned = Quat::from_rotation_y(stands.yaw + nugget.shape as f32 * 0.37);
        // Where it will lie is seen once it has landed: until then, where it was let go.
        nugget.lies = if aboard {
            Lies::Aboard {
                at: stands.aboard(over),
                turned: stands.turned.inverse() * turned,
            }
        } else {
            Lies::Loose {
                map: stands.map.to_owned(),
                at: over,
                turned,
            }
        };
        self.asked.push(Body::Put {
            id: nugget.id,
            at: over,
            turned,
            size: nugget.body(),
            kilos: nugget.kilos,
            settle: true,
        });
        self.bodied.push(nugget.id);
    }

    /// Does what the button does where the driver stands, and says what that was.
    pub fn act(&mut self, stands: &Stands) -> Option<Use> {
        let does = self.can(stands)?;
        match does {
            Use::Dig => {
                // The nugget is made now: what it weighs, mostly little and now and then a
                // great deal, and what its shape is drawn from. The detector is in the
                // driver's hands: the nugget comes out of the ground onto it, at their feet.
                self.buried.retain(|at| flat(*at, stands.walker) >= DIGS);
                // An ounce or two, often; half a kilogram, as often; and now and then one
                // to be talked of for years.
                let luck = self.roll();
                let kilos = 0.03 + 0.5 * luck + 14.5 * luck.powi(5);
                let shape = (self.roll() * 16_777_216.0) as u32;
                self.nuggets.push(Nugget {
                    id: self.next,
                    kilos,
                    shape,
                    lies: Lies::Hand,
                });
                self.next += 1;
                let ahead = Vec3::new(-stands.yaw.sin(), 0.0, -stands.yaw.cos());
                // Where the coil is: a stride and a half ahead, where it can be seen.
                self.let_go(stands, stands.walker + ahead * 1.35 + Vec3::Y * 1.0, false);
            }
            Use::PickUp | Use::FromTray | Use::FromChest => {
                let index = self.in_reach(stands)?;
                let id = self.nuggets[index].id;
                self.nuggets[index].lies = Lies::Hand;
                self.bodied.retain(|bodied| *bodied != id);
                self.asked.push(Body::Take { id });
                // Both hands, for a nugget: the detector is slung.
                if self.detector == Detector::Hand {
                    self.detector = Detector::Slung;
                }
            }
            Use::Take => {
                self.detector = Detector::Hand;
                self.bodied.retain(|bodied| *bodied != DETECTOR);
                self.asked.push(Body::Take { id: DETECTOR });
            }
            Use::IntoTray => {
                let over = stands.over_tray()?;
                self.let_go(stands, over, true);
            }
            Use::IntoChest => {
                let along = self.roll();
                let over = self.by_chest(stands)?.over(along);
                self.let_go(stands, over, false);
            }
            Use::PutDown => {
                let ahead = Vec3::new(-stands.yaw.sin(), 0.0, -stands.yaw.cos());
                self.let_go(stands, stands.walker + ahead * 0.8 + Vec3::Y * 1.4, false);
            }
        }
        Some(does)
    }

    /// The driver has got into the truck with a nugget in their hand: it goes in the tray,
    /// or, if the truck has none, on the ground beside it.
    pub fn board(&mut self, stands: &Stands) {
        if self.held().is_none() {
            return;
        }
        match stands.tray {
            Some((size, floor)) => {
                let (across, along) = (self.roll() - 0.5, self.roll() - 0.5);
                let room =
                    Vec3::new(size.x - 2.0 * INSET, 0.0, size.z - 2.0 * INSET).max(Vec3::ZERO);
                let over = floor + Vec3::new(across * room.x, size.y + 0.5, along * room.z);
                self.let_go(stands, stands.ashore(over), true);
            }
            None => self.let_go(stands, stands.walker + Vec3::Y * 1.0, false),
        }
    }

    /// Whoever runs the world says where the nuggets' bodies are: each is where it has got
    /// to, aboard the truck if it lies in its tray and loose on the map if it does not.
    pub fn see(&mut self, stands: &Stands, seen: &[(u32, Vec3, Quat)]) {
        for &(id, at, turned) in seen {
            if !self.bodied.contains(&id) || !at.is_finite() {
                continue;
            }
            let lies = if id == DETECTOR {
                match &mut self.detector {
                    Detector::Left(lies) => lies,
                    _ => continue,
                }
            } else {
                match self.nuggets.iter_mut().find(|nugget| nugget.id == id) {
                    Some(nugget) => &mut nugget.lies,
                    None => continue,
                }
            };
            *lies = if stands.in_tray(at) {
                Lies::Aboard {
                    at: stands.aboard(at),
                    turned: stands.turned.inverse() * turned,
                }
            } else {
                Lies::Loose {
                    map: stands.map.to_owned(),
                    at,
                    turned,
                }
            };
        }
    }

    /// The truck has been put somewhere else, all at once: what is aboard it goes with it.
    pub fn reseat(&mut self) {
        for nugget in &self.nuggets {
            if matches!(nugget.lies, Lies::Aboard { .. }) && self.bodied.contains(&nugget.id) {
                self.asked.push(Body::Take { id: nugget.id });
            }
        }
        let mut aboard: Vec<u32> = self
            .nuggets
            .iter()
            .filter(|nugget| matches!(nugget.lies, Lies::Aboard { .. }))
            .map(|nugget| nugget.id)
            .collect();
        if matches!(self.detector, Detector::Left(Lies::Aboard { .. })) {
            if self.bodied.contains(&DETECTOR) {
                self.asked.push(Body::Take { id: DETECTOR });
            }
            aboard.push(DETECTOR);
        }
        self.bodied.retain(|id| !aboard.contains(id));
    }

    /// What is to be asked of the world now: what the button did, and a body for every
    /// nugget that is in this map's world, loose or aboard the truck, and has none yet.
    pub fn sync(&mut self, stands: &Stands) -> Vec<Body> {
        let mut asked = std::mem::take(&mut self.asked);
        for nugget in &self.nuggets {
            if self.bodied.contains(&nugget.id) {
                continue;
            }
            if let Some((at, turned)) = Self::pose(nugget, stands) {
                self.bodied.push(nugget.id);
                asked.push(Body::Put {
                    id: nugget.id,
                    at,
                    turned,
                    size: nugget.body(),
                    kilos: nugget.kilos,
                    settle: false,
                });
            }
        }
        if let Detector::Left(lies) = &self.detector {
            if !self.bodied.contains(&DETECTOR) {
                if let Some((at, turned)) = Self::lying(lies, stands) {
                    self.bodied.push(DETECTOR);
                    asked.push(Body::Put {
                        id: DETECTOR,
                        at,
                        turned,
                        size: DETECTOR_BODY,
                        kilos: DETECTOR_KILOS,
                        settle: false,
                    });
                }
            }
        }
        asked
    }

    /// Sells what is aboard the truck: how many nuggets, and what they weighed. They are
    /// gone from the tray.
    pub fn sell(&mut self) -> (usize, f32) {
        let sold = self.tray();
        for nugget in &self.nuggets {
            if matches!(nugget.lies, Lies::Aboard { .. }) && self.bodied.contains(&nugget.id) {
                self.asked.push(Body::Take { id: nugget.id });
            }
        }
        self.nuggets
            .retain(|nugget| !matches!(nugget.lies, Lies::Aboard { .. }));
        let left: Vec<u32> = self.nuggets.iter().map(|nugget| nugget.id).collect();
        self.bodied
            .retain(|id| *id == DETECTOR || left.contains(id));
        sold
    }

    /// Which of the scene's blanks is to show which nugget, and where, for the nuggets in
    /// sight: only those that have changed since this was last called.
    pub fn place(&mut self, stands: &Stands, afoot: bool) -> Vec<Shown> {
        // Each nugget in sight, and where: in the hand, or where its body is.
        let mut wanted: Vec<(u32, Vec3, Quat, f32, u32)> = Vec::new();
        for nugget in &self.nuggets {
            let at = match &nugget.lies {
                Lies::Hand if afoot => {
                    let facing = Quat::from_rotation_y(stands.yaw);
                    let spin = Quat::from_rotation_y(nugget.shape as f32 * 0.37);
                    Some((
                        stands.walker + facing * Vec3::new(-0.2, 1.08, -0.55),
                        facing * spin,
                    ))
                }
                Lies::Hand => None,
                _ => Self::pose(nugget, stands),
            };
            if let Some((at, turned)) = at {
                wanted.push((nugget.id, at, turned, nugget.size(), nugget.shape));
            }
        }
        // More than there are blanks to show them with: the nearest are shown.
        wanted.sort_by(|a, b| flat(a.1, stands.walker).total_cmp(&flat(b.1, stands.walker)));
        wanted.truncate(self.shown.len());
        let mut changed = Vec::new();
        // A model keeps the nugget it has while that is still in sight.
        let mut taken = vec![false; wanted.len()];
        let mut free = Vec::new();
        for slot in 0..self.shown.len() {
            let keeps = self.shown[slot].and_then(|(id, ..)| wanted.iter().position(|w| w.0 == id));
            match keeps {
                Some(index) => {
                    taken[index] = true;
                    let (id, at, turned, size, shape) = wanted[index];
                    if self.shown[slot] != Some((id, at, turned)) {
                        self.shown[slot] = Some((id, at, turned));
                        changed.push(Shown {
                            slot,
                            at: Some((at, turned, size)),
                            shape,
                        });
                    }
                }
                None => {
                    if self.shown[slot].take().is_some() {
                        changed.push(Shown {
                            slot,
                            at: None,
                            shape: 0,
                        });
                    }
                    free.push(slot);
                }
            }
        }
        for (index, (id, at, turned, size, shape)) in wanted.into_iter().enumerate() {
            if taken[index] {
                continue;
            }
            // Any blank that shows nothing: there is one, as no more are wanted than there are.
            let Some(slot) = free.pop() else {
                continue;
            };
            self.shown[slot] = Some((id, at, turned));
            changed.retain(|shown| shown.slot != slot);
            changed.push(Shown {
                slot,
                at: Some((at, turned, size)),
                shape,
            });
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<GameFile> {
        let track: String = (0..40)
            .map(|i| format!("p {} 5 0 0 0 1 3 3\n", i * 10))
            .collect();
        vec![
            GameFile {
                name: "outback.track".to_owned(),
                text: track,
            },
            GameFile {
                name: crate::nugget::FILE.to_owned(),
                text: "slot 900 0\nwrite 1 0 0 9\nslot 901 3\nslot 902 6\nslot 903 9\n".to_owned(),
            },
        ]
    }

    /// A hub: the same, with a chest, and a place to stop at.
    fn hub() -> Vec<GameFile> {
        let mut hub = files();
        hub.push(GameFile {
            name: "outback.items".to_owned(),
            text: "chest 0 5.1 0 0\n".to_owned(),
        });
        hub.push(GameFile {
            name: "outback.places".to_owned(),
            text: "place shed 3 5 3 2.6\n".to_owned(),
        });
        hub
    }

    const TRAY: (Vec3, Vec3) = (Vec3::new(1.55, 0.545, 1.7), Vec3::new(0.0, 0.905, 1.9));

    fn stands(walker: Vec3) -> Stands<'static> {
        Stands {
            map: "wombat",
            walker,
            yaw: 0.0,
            truck: Vec3::new(500.0, 5.0, 500.0),
            turned: Quat::IDENTITY,
            tray: Some(TRAY),
        }
    }

    /// What the world would do with what was asked of it: each body put where it was asked
    /// for, one that is to settle lying `rests` up from the floor it was let go over.
    fn world(bodies: &mut Vec<(u32, Vec3, Quat)>, asked: Vec<Body>, rests: f32) {
        for body in asked {
            match body {
                Body::Put {
                    id,
                    at,
                    turned,
                    settle,
                    ..
                } => {
                    let at = if settle {
                        Vec3::new(at.x, rests, at.z)
                    } else {
                        at
                    };
                    bodies.retain(|(other, ..)| *other != id);
                    bodies.push((id, at, turned));
                }
                Body::Take { id } => bodies.retain(|(other, ..)| *other != id),
            }
        }
    }

    #[test]
    fn gold_is_put_in_the_ground_when_a_map_is_entered_and_is_not_where_it_was() {
        let mut gold = Gold::default();
        gold.enter(&files(), 7);
        assert_eq!(gold.buried.len(), BURIED);
        // Off the track, on either side, and within the map's length.
        assert!(gold
            .buried
            .iter()
            .all(|at| at.z.abs() > 7.9 && at.z.abs() < 31.0));
        let first = gold.buried.clone();
        gold.enter(&files(), 8);
        assert_ne!(gold.buried, first);
        // A hub, which has a chest and places to stop at, and no gold in its ground.
        gold.enter(&hub(), 9);
        assert!(gold.buried.is_empty());
    }

    #[test]
    fn a_nugget_is_a_thing_from_the_ground_to_the_hand_to_the_tray_to_the_chest_and_is_sold() {
        let mut gold = Gold::default();
        gold.enter(&files(), 11);
        let mut bodies: Vec<(u32, Vec3, Quat)> = Vec::new();
        let over = gold.buried[0];
        // The signal rises as the driver nears it, and over it there is something to dig.
        assert!(
            gold.signal(&stands(over + Vec3::X * 6.0)) < gold.signal(&stands(over + Vec3::X * 1.0))
        );
        assert_eq!(gold.can(&stands(over + Vec3::X * 6.0)), None);
        // Without the detector in hand there is nothing to dig by.
        assert!(gold.swap() && gold.detector == Detector::Slung);
        assert_eq!(gold.can(&stands(over)), None);
        assert!(gold.swap() && gold.detector == Detector::Hand);
        assert_eq!(gold.act(&stands(over)), Some(Use::Dig));
        // It is made now, with a weight and a shape. The driver's hands hold the detector:
        // it comes out of the ground onto it, a body, which falls and lies there.
        assert_eq!(gold.nuggets.len(), 1);
        assert!(gold.nuggets[0].kilos >= 0.03 && gold.in_hand().is_none());
        let asked = gold.sync(&stands(over));
        assert!(matches!(asked[..], [Body::Put { settle: true, .. }]));
        world(&mut bodies, asked, 5.07);
        gold.see(&stands(over), &bodies);
        let Lies::Loose { map, at, .. } = &gold.nuggets[0].lies else {
            panic!("it is loose")
        };
        assert!(map == "wombat" && (at.y - 5.07).abs() < 1e-3 && flat(*at, over) < 1.6);
        // Lying there it is heard, as gold in the ground is; and it is shown where it lies.
        assert!(gold.signal(&stands(*at)) > 0.99);
        let shown = gold.place(&stands(over), true);
        assert!(matches!(shown[..], [Shown { at: Some(_), .. }]));
        // Picked up, its body is gone from the world, and the detector is slung to do it.
        assert_eq!(gold.act(&stands(over)), Some(Use::PickUp));
        world(&mut bodies, gold.sync(&stands(over)), 0.0);
        assert!(bodies.is_empty() && gold.in_hand().is_some());
        assert_eq!(gold.detector, Detector::Slung);
        // With a nugget in the hands the detector stays where it is.
        assert!(!gold.swap());

        // Beside the truck's tray it is let go into it, lies on its floor, and is aboard.
        let truck = Stands {
            truck: over + Vec3::Z * 2.0,
            ..stands(over)
        };
        let beside = Stands {
            walker: truck.ashore(TRAY.1 + Vec3::new(1.3, -0.9, 0.2)),
            ..truck
        };
        assert_eq!(gold.act(&beside), Some(Use::IntoTray));
        world(
            &mut bodies,
            gold.sync(&beside),
            beside.truck.y + TRAY.1.y + 0.08,
        );
        gold.see(&beside, &bodies);
        assert!(matches!(gold.nuggets[0].lies, Lies::Aboard { .. }));
        assert_eq!(gold.tray().0, 1);
        // The truck drives off: the world carries the body, and it is still aboard, where
        // it lay in the truck.
        let Lies::Aboard { at: lay, .. } = gold.nuggets[0].lies.clone() else {
            panic!()
        };
        let driven = Stands {
            truck: beside.truck + Vec3::X * 300.0,
            turned: Quat::from_rotation_y(1.0),
            ..beside
        };
        bodies[0].1 = driven.ashore(lay);
        gold.see(&driven, &bodies);
        let Lies::Aboard { at, .. } = gold.nuggets[0].lies.clone() else {
            panic!("still aboard")
        };
        assert!(at.distance(lay) < 1e-3);
        // Thrown out, it is loose where it landed.
        bodies[0].1 = driven.truck + Vec3::new(4.0, 0.1, 0.0);
        gold.see(&driven, &bodies);
        assert!(matches!(gold.nuggets[0].lies, Lies::Loose { .. }));
        bodies[0].1 = driven.ashore(lay);
        gold.see(&driven, &bodies);

        // Kept, and read back, it is the same nugget in the same place in the truck.
        let mut kept = String::new();
        gold.keep(&mut kept);
        let mut again = Gold::read(&kept);
        assert_eq!(again.nuggets.len(), 1);
        assert!(
            matches!(&again.nuggets[0].lies, Lies::Aboard { at, .. } if at.distance(lay) < 0.01)
        );
        // And in a map's new world it is given a body where the truck is now.
        again.enter(&hub(), 3);
        let at_hub = Stands {
            map: "crossroads",
            truck: Vec3::new(40.0, 5.0, 3.0),
            ..stands(Vec3::new(40.0, 5.0, 3.0))
        };
        let asked = again.sync(&at_hub);
        assert!(
            matches!(asked[..], [Body::Put { settle: false, at, .. }] if at.distance(at_hub.ashore(lay)) < 0.01)
        );
        let mut bodies = Vec::new();
        world(&mut bodies, asked, 0.0);
        // The truck put somewhere else all at once takes what is aboard it along.
        again.reseat();
        let moved = Stands {
            truck: Vec3::new(3.0, 5.0, 3.0),
            ..at_hub
        };
        let asked = again.sync(&moved);
        assert!(matches!(asked[..], [Body::Take { .. }, Body::Put { .. }]));
        world(&mut bodies, asked, 0.0);
        assert!(bodies[0].1.distance(moved.ashore(lay)) < 0.01);

        // At the hub: out of the tray, and into the chest, where it lies and is counted.
        let by_tray = Stands {
            walker: moved.ashore(TRAY.1 + Vec3::new(1.3, -0.9, 0.0)),
            ..moved
        };
        assert_eq!(again.act(&by_tray), Some(Use::FromTray));
        world(&mut bodies, again.sync(&by_tray), 0.0);
        assert_eq!(again.tray().0, 0);
        let by_chest = Stands {
            walker: Vec3::new(0.9, 5.0, 0.0),
            truck: Vec3::new(60.0, 5.0, 0.0),
            ..moved
        };
        assert_eq!(again.act(&by_chest), Some(Use::IntoChest));
        world(&mut bodies, again.sync(&by_chest), 5.2);
        again.see(&by_chest, &bodies);
        assert_eq!(again.in_chest("crossroads").0, 1);
        assert_eq!(again.can(&by_chest), Some(Use::FromChest));
        // In the chest it is not what the detector is listening for.
        assert!(again.signal(&by_chest) < 0.01);

        // Back in the tray, it is sold: weighed, and gone.
        assert_eq!(again.act(&by_chest), Some(Use::FromChest));
        world(&mut bodies, again.sync(&by_chest), 0.0);
        again.board(&moved);
        world(
            &mut bodies,
            again.sync(&moved),
            moved.truck.y + TRAY.1.y + 0.08,
        );
        again.see(&moved, &bodies);
        let kilos = again.tray().1;
        assert_eq!(again.sell(), (1, kilos));
        world(&mut bodies, again.sync(&moved), 0.0);
        assert!(bodies.is_empty() && again.nuggets.is_empty());
    }

    #[test]
    fn the_detector_is_put_down_lies_there_is_kept_and_is_picked_up_again() {
        let mut gold = Gold::default();
        gold.enter(&files(), 5);
        let here = stands(Vec3::new(20.0, 5.0, 60.0));
        let mut bodies: Vec<(u32, Vec3, Quat)> = Vec::new();
        // In hand it is shown held; in the truck, not at all.
        assert_eq!(gold.carried(&here, true), Carried::Held);
        assert_eq!(gold.carried(&here, false), Carried::Away);
        // Put down, it is a body, and lies where it comes to rest.
        assert_eq!(gold.put_down(&here), Some(Use::PutDown));
        let asked = gold.sync(&here);
        assert!(matches!(
            asked[..],
            [Body::Put {
                id: DETECTOR,
                settle: true,
                ..
            }]
        ));
        world(&mut bodies, asked, 5.06);
        gold.see(&here, &bodies);
        let Carried::Lying(at, _) = gold.carried(&here, true) else {
            panic!("it lies there")
        };
        assert!((at.y - 5.06).abs() < 1e-3 && flat(at, here.walker) < 1.2);
        // It cannot be slung or taken in hand from over there, and hears nothing.
        assert!(!gold.swap());
        // Kept, and read back, it is where it was left; on another map it is not seen.
        let mut kept = String::new();
        gold.keep(&mut kept);
        let mut again = Gold::read(&kept);
        assert!(
            matches!(&again.detector, Detector::Left(Lies::Loose { map, .. }) if map == "wombat")
        );
        assert_eq!(
            again.carried(
                &Stands {
                    map: "granite",
                    ..here
                },
                true
            ),
            Carried::Away
        );
        // Back on its map it has a body again, and the button picks it up.
        again.enter(&files(), 6);
        let mut bodies = Vec::new();
        world(&mut bodies, again.sync(&here), 0.0);
        assert_eq!(bodies.len(), 1);
        assert_eq!(again.can(&here), Some(Use::Take));
        assert_eq!(again.act(&here), Some(Use::Take));
        world(&mut bodies, again.sync(&here), 0.0);
        assert!(bodies.is_empty() && again.detector == Detector::Hand);
    }

    #[test]
    fn the_nearest_nuggets_are_shown_and_each_blank_keeps_the_one_it_has() {
        let nugget = |id: u32| Nugget {
            id,
            kilos: 1.0,
            shape: id.wrapping_mul(2_654_435_761),
            lies: Lies::Loose {
                map: "wombat".to_owned(),
                at: Vec3::new(id as f32 * 0.3, 5.0, 0.0),
                turned: Quat::IDENTITY,
            },
        };
        // Four blanks, and six nuggets in sight: the four nearest are shown, each on a
        // blank of its own, made from its own number.
        let mut gold = Gold::default();
        gold.enter(&files(), 3);
        gold.nuggets = (0..6).map(nugget).collect();
        let here = stands(Vec3::new(0.0, 5.0, 0.0));
        let shown = gold.place(&here, true);
        assert_eq!(shown.len(), 4);
        let mut slots: Vec<usize> = shown.iter().map(|shown| shown.slot).collect();
        slots.sort_unstable();
        assert_eq!(slots, [0, 1, 2, 3]);
        let mut shapes: Vec<u32> = shown.iter().map(|shown| shown.shape).collect();
        shapes.sort_unstable();
        let mut nearest: Vec<u32> = (0..4).map(|id| nugget(id).shape).collect();
        nearest.sort_unstable();
        assert_eq!(shapes, nearest);
        // Asked again, nothing has changed: each blank keeps the nugget it has.
        assert!(gold.place(&here, true).is_empty());
    }

    #[test]
    fn nuggets_are_many_shapes_and_sizes_and_each_is_its_own_every_time() {
        let forms: Vec<Form> = (0..3000_u32)
            .map(|n| Form::of(n.wrapping_mul(2_654_435_761).rotate_left(11)))
            .collect();
        // The same number is the same shape.
        assert_eq!(Form::of(77_123), Form::of(77_123));
        assert_ne!(Form::of(77_123), Form::of(77_124));
        // Most are flat, a little longer than wide; some are long slugs, some thin flakes,
        // some hooked, some pinched, some drawn out at one end: and most are none of these.
        let share = |of: &dyn Fn(&Form) -> bool| {
            forms.iter().filter(|form| of(form)).count() as f32 / forms.len() as f32
        };
        assert!(share(&|form| form.thick < form.wide) > 0.6);
        let slugs = share(&|form| form.long > 1.7);
        let flakes = share(&|form| form.thick < 0.35);
        let hooks = share(&|form| form.bend.abs() > 0.5);
        let peanuts = share(&|form| form.pinch > 0.3);
        let drops = share(&|form| form.taper.abs() > 0.4);
        for (what, some) in [
            ("slugs", slugs),
            ("flakes", flakes),
            ("hooks", hooks),
            ("peanuts", peanuts),
            ("drops", drops),
        ] {
            assert!(some > 0.04 && some < 0.34, "{what}: {some}");
        }
        // Nothing is thinner than it can be drawn, or pinched through.
        assert!(forms.iter().all(|form| form.thick > 0.19
            && form.wide > 0.29
            && form.girth(0.0) > 0.2
            && form.girth(1.0) > 0.2
            && form.girth(-1.0) > 0.2));
        // And they come small as well as big: an ounce is a finger's width, the heaviest a
        // double handful, each lying as a box of its own shape.
        let nugget = |kilos: f32, shape: u32| Nugget {
            id: 0,
            kilos,
            shape,
            lies: Lies::Hand,
        };
        assert!(nugget(0.03, 5).size() < 0.08 && nugget(15.0, 5).size() > 0.17);
        let sizes: Vec<f32> = (0..200).map(|shape| nugget(1.0, shape).size()).collect();
        let (least, most) = sizes
            .iter()
            .fold((f32::MAX, 0.0_f32), |(least, most), &size| {
                (least.min(size), most.max(size))
            });
        assert!(most > least * 1.4, "{least} {most}");
        let slug = (0..9000_u32)
            .find(|&shape| Form::of(shape).long > 2.0)
            .expect("there are slugs");
        let body = nugget(1.0, slug).body();
        assert!(body.x > body.z * 1.5 && body.y < body.x);
    }

    #[test]
    fn a_game_kept_before_nuggets_were_things_still_has_its_gold() {
        let gold = Gold::read("nugget 3 2.50 77 tray 1\nnugget 4 1.00 5 chest\nnugget 5 0.60 9 ground wombat 10 5 20 0.5\nnugget 6 0.90 1 hand\n");
        assert_eq!(gold.nuggets.len(), 4);
        assert_eq!(gold.tray().0, 2);
        assert!(matches!(&gold.nuggets[2].lies, Lies::Loose { map, .. } if map == "wombat"));
        assert_eq!(gold.in_hand().map(|nugget| nugget.id), Some(6));
        assert_eq!(gold.next, 7);
    }
}
