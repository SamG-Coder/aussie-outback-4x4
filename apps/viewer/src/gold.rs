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
//! thrown out of it by a hard enough landing. A truck with a cab to put things in has
//! **holds** besides ([`Hold`]): its passenger's seat, its back seat, its boot, never the
//! driver's own. Each is behind a door, which the button opens and shuts; with the door
//! open a nugget is put in, and lies there shut in, where no landing throws it out. Put
//! into the chest in the shed it lies in the chest. One button does what there is to do ([`Gold::can`]): dig, pick up the nearest, or
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
/// It hears a long way, faintly: gold is a few places in kilometres of bush, and a driver
/// walking the track is to be told that there is some off to one side, and then led to it.
const HEARS: f32 = 60.0;
/// How much of gold's signal is heard with the driver's back to it: the coil is held out
/// in front, and hears best what is ahead. Turning on the spot finds which way the gold is.
const BEHIND: f32 = 0.3;
const DIGS: f32 = 1.7;
/// The signal there is at the edge of where gold can be dug: at this or more, the driver
/// is over it, and the detector says so with one unbroken tone.
pub const OVER: f32 = 0.88;
/// How near a nugget is to be picked up, how near the edge of the truck's tray the driver
/// stands to put one in it, and how near the chest.
const REACH: f32 = 1.7;
const TRAY_REACH: f32 = 1.6;
/// How near a doorway of the truck the driver stands to open its door, and to reach in.
const DOOR_REACH: f32 = 1.15;
const CHEST_REACH: f32 = 1.7;
/// How many days a map's gold lies as it was scattered, once any of it has been dug: after
/// more than these it is scattered again, the next time the map is come into. Ground that
/// has not been touched is left as it is.
pub const RESCATTER_DAYS: u32 = 3;
/// What is dug up: each kind of find, the least and the most it weighs in kilograms, and how
/// often it is found in ordinary ground, in parts of a thousand. Mostly it is a little; a
/// kilogram is a good week; and the last is found once in a long game.
///
/// At the buyer's price (`shell::PRICE`) a find in ordinary ground is worth about $270 on
/// average, so half a dozen dug in a day's outing is a week's repayment and a little over.
/// Richer ground has more of the big ones, poorer ground fewer (`chances`).
pub const FINDS: [(&str, f32, f32, f32); 7] = [
    ("a speck", 0.010, 0.030, 400.0),
    ("a small nugget", 0.030, 0.080, 300.0),
    ("a fair nugget", 0.080, 0.200, 170.0),
    ("a good nugget", 0.200, 0.500, 85.0),
    ("a big nugget", 0.500, 1.500, 30.0),
    ("a great nugget", 1.500, 4.000, 6.0),
    ("the find of a lifetime", 4.000, 10.000, 1.0),
];

/// How often each kind of find is found in ground this rich, as shares that add to 1. Rich
/// is 1 for ordinary ground; above it the bigger a find is the more often it is found, and
/// below it the less.
pub fn chances(rich: f32) -> [f32; 7] {
    let mut shares = [0.0; 7];
    for (kind, (_, _, _, often)) in FINDS.iter().enumerate() {
        shares[kind] = often * rich.max(0.05).powf(kind as f32 * 0.5);
    }
    let all: f32 = shares.iter().sum();
    shares.map(|share| share / all)
}

/// What a find weighs, in kilograms: `kind` and `size` are each a number from 0 to 1, the
/// one saying which kind of find it is in ground this rich and the other how big a one.
pub fn find(kind: f32, size: f32, rich: f32) -> (usize, f32) {
    let mut left = kind.clamp(0.0, 0.999_999);
    let shares = chances(rich);
    let mut which = FINDS.len() - 1;
    for (index, share) in shares.iter().enumerate() {
        if left < *share {
            which = index;
            break;
        }
        left -= share;
    }
    let (_, least, most, _) = FINDS[which];
    // More of each kind are nearer its least than its most.
    (
        which,
        least * (most / least).powf(size.clamp(0.0, 1.0).powf(1.5)),
    )
}

/// A map's ground, as it was last scattered: on which day, from what number, and which of
/// its places have been dug since (a bit each).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub day: u32,
    pub seed: u64,
    pub dug: u32,
}

/// How many places a map's ground is given.
const BURIED: usize = 24;
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

/// A door of the truck that something can be reached through: the piece of the truck that
/// swings (`kit.rs`), and whether it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct Door {
    pub piece: String,
    pub open: bool,
}

/// A place in the truck that gold can be put, shut in: a seat, a boot. Its size, across,
/// high and long, and the middle of its floor, in the truck's own space; and the ways into
/// it, each a door (its number among the truck's doors) and where its doorway is.
#[derive(Clone, Debug, PartialEq)]
pub struct Hold {
    pub name: String,
    pub size: Vec3,
    pub floor: Vec3,
    pub ways: Vec<(usize, Vec3)>,
}

/// What the button does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Dig,
    PickUp,
    /// Open the door the driver stands at, or shut it.
    OpenDoor,
    ShutDoor,
    /// Put what is held into the hold whose open door the driver stands at; or take the
    /// nearest nugget out of it.
    IntoHold,
    FromHold,
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
            Self::OpenDoor => "open",
            Self::ShutDoor => "shut",
            Self::IntoHold => "stow",
            Self::FromHold => "unstow",
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
    /// The truck's holds, and its doors, if it has a cab to put things in.
    pub holds: &'a [Hold],
    pub doors: &'a [Door],
}

impl Stands<'_> {
    /// The hold a place is in, if it is in one.
    fn in_hold(&self, at: Vec3) -> Option<usize> {
        let at = self.aboard(at);
        self.holds.iter().position(|hold| {
            let from = at - hold.floor;
            from.x.abs() < hold.size.x * 0.5 + 0.04
                && from.z.abs() < hold.size.z * 0.5 + 0.04
                && from.y > -0.1
                && from.y < hold.size.y + 0.1
        })
    }

    /// The doorways the driver stands at: each the hold it is a way into, its door, and how
    /// far off it is.
    fn doorways(&self) -> impl Iterator<Item = (usize, usize, f32)> + '_ {
        let from = self.aboard(self.walker);
        self.holds
            .iter()
            .enumerate()
            .flat_map(move |(hold, held)| {
                held.ways
                    .iter()
                    .map(move |(door, at)| (hold, *door, flat(*at, from)))
            })
            .filter(|(_, door, away)| *away < DOOR_REACH && *door < self.doors.len())
    }

    /// The doorway nearest the driver that they stand at: the hold it is a way into, and
    /// its door's number.
    pub fn doorway(&self) -> Option<(usize, usize)> {
        self.doorways()
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(hold, door, _)| (hold, door))
    }

    /// Whether the driver can reach into a hold: they stand at a doorway of it that is open.
    fn reaches(&self, hold: usize) -> bool {
        self.doorways()
            .any(|(into, door, _)| into == hold && self.doors[door].open)
    }

    /// The hold the driver can put something into: that of the nearest open doorway they
    /// stand at.
    fn open_hold(&self) -> Option<usize> {
        self.doorways()
            .filter(|(_, door, _)| self.doors[*door].open)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(hold, _, _)| hold)
    }

    /// Where something is let go into a hold: a little above its floor, `across` and `along`
    /// it from 0 to 1, clear of its walls.
    fn over_hold(&self, hold: usize, across: f32, along: f32) -> Vec3 {
        let hold = &self.holds[hold];
        let room =
            (Vec3::new(hold.size.x, 0.0, hold.size.z) * 0.5 - Vec3::splat(0.14)).max(Vec3::ZERO);
        let up = (hold.size.y * 0.6).min(0.25);
        self.ashore(
            hold.floor
                + Vec3::new(
                    (across * 2.0 - 1.0) * room.x,
                    up,
                    (along * 2.0 - 1.0) * room.z,
                ),
        )
    }

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
    /// Where gold is in the ground of the map being played, and which of the map's places
    /// each is (255 for one that was put there by hand).
    buried: Vec<Vec3>,
    places: Vec<u8>,
    /// Every map's ground that has been come into, as it was last scattered.
    grounds: std::collections::BTreeMap<String, Ground>,
    /// The map being played, if gold is kept for it; and how rich its ground is.
    here: Option<String>,
    rich: f32,
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
            // A map's ground, as it was last scattered: `scatter MAP DAY SEED DUG`.
            if let ["scatter", map, day, seed, dug] = &words[..] {
                if let (Ok(day), Ok(seed), Ok(dug)) = (day.parse(), seed.parse(), dug.parse()) {
                    gold.grounds
                        .insert((*map).to_owned(), Ground { day, seed, dug });
                }
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
        for (map, ground) in &self.grounds {
            let _ = writeln!(
                kept,
                "scatter {map} {} {} {}",
                ground.day, ground.seed, ground.dug
            );
        }
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
                "nugget {} {:.3} {} ",
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
    pub fn enter(
        &mut self,
        files: &[GameFile],
        map: Option<&str>,
        day: u32,
        chance: u64,
        rich: f32,
    ) {
        self.buried.clear();
        self.places.clear();
        self.here = map.map(str::to_owned);
        self.rich = rich;
        self.bodied.clear();
        self.asked.clear();
        self.chest = None;
        let mut slots = 0;
        let mut track = Vec::new();
        // What gold is not put close by: a building, a yard, a ring to stop in.
        let mut kept_off: Vec<(Vec3, f32)> = Vec::new();
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
                    (Some("places"), Some("place"), [x, y, z, ..]) => {
                        let what = words.get(1).copied().unwrap_or("");
                        let built = ["door:", "stand:"]
                            .iter()
                            .any(|kind| what.starts_with(kind));
                        let clear = if built || what == "buyer" {
                            45.0
                        } else if what == "shed" {
                            22.0
                        } else {
                            11.0
                        };
                        kept_off.push((Vec3::new(*x, *y, *z), clear));
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
        // A map with no track to put it beside, or one that is not kept for (a showroom),
        // has none.
        let Some(map) = map.filter(|_| !track.is_empty()) else {
            self.rolls = chance | 1;
            return;
        };
        // The map's ground is as it was last scattered, with what has been dug still dug:
        // unless it has not been come into before, or some of it has been dug and more
        // than a few days have gone by, when it is scattered again.
        let stale =
            |ground: &Ground| ground.dug != 0 && day.saturating_sub(ground.day) > RESCATTER_DAYS;
        let ground = match self
            .grounds
            .get(map)
            .copied()
            .filter(|ground| !stale(ground))
        {
            Some(ground) => ground,
            None => {
                let named = map.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
                });
                let ground = Ground {
                    day,
                    seed: chance ^ named ^ u64::from(day).wrapping_mul(0x2545_F491_4F6C_DD1D),
                    dug: 0,
                };
                self.grounds.insert(map.to_owned(), ground);
                ground
            }
        };
        // Its places, from its own number: the same every time, until it is scattered again.
        self.rolls = ground.seed | 1;
        for _ in 0..8 {
            self.roll();
        }
        for place in 0..BURIED {
            // Beside the track, on either side, clear of what is built: a few tries at it.
            let mut at = Vec3::ZERO;
            for _ in 0..12 {
                let (on, left, half_left, half_right) =
                    track[(self.roll() * track.len() as f32) as usize % track.len()];
                let (side, half) = if self.roll() < 0.5 {
                    (1.0, half_left)
                } else {
                    (-1.0, half_right)
                };
                let off = half + 5.0 + self.roll() * 22.0;
                at = on + left.normalize_or_zero() * side * off;
                if kept_off
                    .iter()
                    .all(|(from, clear)| flat(*from, at) > *clear)
                {
                    break;
                }
            }
            if ground.dug & (1 << place) == 0 {
                self.buried.push(at);
                self.places.push(place as u8);
            }
        }
        // What is dug up is different every time the map is come into.
        self.rolls = (chance ^ u64::from(ground.dug).wrapping_mul(0x9E37_79B9_7F4A_7C15)) | 1;
        for _ in 0..8 {
            self.roll();
        }
    }

    /// The map's ground as it was last scattered, if it has been come into.
    pub fn ground(&self, map: &str) -> Option<Ground> {
        self.grounds.get(map).copied()
    }

    /// Whether there is any gold in the ground of the map: there is none in a town's.
    pub fn any_buried(&self) -> bool {
        !self.buried.is_empty()
    }

    /// Puts gold in the ground at a place: for a trial, which cannot go looking for it.
    pub fn bury(&mut self, at: Vec3) {
        self.buried.push(at);
        self.places.push(u8::MAX);
    }

    /// How strong the detector's signal is where the driver stands, from 0, nothing, to 1,
    /// right over gold: what is in the ground, and a nugget lying loose on it, as one that
    /// has been thrown out of the tray is.
    pub fn signal(&self, stands: &Stands) -> f32 {
        let nearest = self.nearest(stands).map_or(f32::MAX, |(away, _)| away);
        // From nothing at the edge of its hearing, evenly up to `OVER` at the edge of where
        // the gold can be dug, and from there to 1 right on top of it.
        if nearest <= DIGS {
            OVER + (1.0 - OVER) * (1.0 - nearest / DIGS)
        } else {
            OVER * (1.0 - (nearest - DIGS) / (HEARS - DIGS)).clamp(0.0, 1.0)
        }
    }

    /// The gold nearest the driver that a detector would hear: how far off it is over the
    /// ground, and where it is.
    fn nearest(&self, stands: &Stands) -> Option<(f32, Vec3)> {
        let lying = self.nuggets.iter().filter_map(|nugget| match &nugget.lies {
            Lies::Loose { map, at, .. }
                if map == stands.map && !self.chest.is_some_and(|chest| chest.holds(*at)) =>
            {
                Some(*at)
            }
            _ => None,
        });
        let each = self.buried.iter().copied().chain(lying);
        each.map(|at| (flat(at, stands.walker), at))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// What the detector in the hand of a driver facing `yaw` hears: the signal, from 0 to
    /// 1, weaker with their back to the gold than facing it, until they are over it; and
    /// how far off the gold is, in metres, if it is heard at all.
    pub fn heard(&self, stands: &Stands, yaw: f32) -> (f32, Option<f32>) {
        let signal = self.signal(stands);
        let Some((away, at)) = self.nearest(stands).filter(|_| signal > 0.0) else {
            return (0.0, None);
        };
        if signal >= OVER {
            return (signal, Some(away));
        }
        let ahead = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let towards = Vec3::new(at.x - stands.walker.x, 0.0, at.z - stands.walker.z);
        let facing = ahead.dot(towards.normalize_or_zero()) * 0.5 + 0.5;
        (signal * (BEHIND + (1.0 - BEHIND) * facing), Some(away))
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

    /// Whether anything is aboard the truck: a nugget in its tray or behind one of its
    /// doors, or the detector, left there. What is aboard belongs with that truck.
    pub fn aboard(&self) -> bool {
        self.tray().0 > 0 || matches!(self.detector, Detector::Left(Lies::Aboard { .. }))
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
                // What is in a hold is behind its door: in reach of somebody standing at
                // the doorway with the door open, and of nobody else.
                let aboard = matches!(nugget.lies, Lies::Aboard { .. });
                let near = match stands.in_hold(at).filter(|_| aboard) {
                    Some(hold) => stands.reaches(hold),
                    None => {
                        flat(at, stands.walker) < REACH
                            || (aboard && stands.over_tray().is_some())
                            || self.by_chest(stands).is_some_and(|chest| chest.holds(at))
                    }
                };
                (near && (-0.6..2.1).contains(&up)).then_some((index, at))
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
            // At a doorway of the truck: in through it, and if its door is shut, that first.
            return Some(if stands.open_hold().is_some() {
                Use::IntoHold
            } else if stands.doorway().is_some() {
                Use::OpenDoor
            } else if stands.over_tray().is_some() {
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
                Lies::Aboard { at, .. } if stands.in_hold(stands.ashore(*at)).is_some() => {
                    Use::FromHold
                }
                Lies::Aboard { .. } => Use::FromTray,
                Lies::Loose { at, .. } if self.chest.is_some_and(|chest| chest.holds(*at)) => {
                    Use::FromChest
                }
                _ => Use::PickUp,
            }),
            // Nothing to pick up: an open door is shut, and a shut one with gold behind it
            // is opened.
            (None, None) => stands.doorway().and_then(|(hold, door)| {
                if stands.doors[door].open {
                    Some(Use::ShutDoor)
                } else {
                    self.stowed(stands, hold).then_some(Use::OpenDoor)
                }
            }),
        }
    }

    /// Whether there is gold in a hold of the truck.
    fn stowed(&self, stands: &Stands, hold: usize) -> bool {
        self.nuggets.iter().any(|nugget| match &nugget.lies {
            Lies::Aboard { at, .. } => stands.in_hold(stands.ashore(*at)) == Some(hold),
            _ => false,
        })
    }

    /// How many nuggets are in each hold of the truck, by the hold's name.
    pub fn in_holds(&self, stands: &Stands) -> Vec<(String, usize)> {
        let count = |hold: usize| {
            let inside = |nugget: &&Nugget| match &nugget.lies {
                Lies::Aboard { at, .. } => stands.in_hold(stands.ashore(*at)) == Some(hold),
                _ => false,
            };
            self.nuggets.iter().filter(inside).count()
        };
        let holds = stands.holds.iter().enumerate();
        holds
            .map(|(index, hold)| (hold.name.clone(), count(index)))
            .collect()
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
                // The place is dug, and stays dug for as long as the map's ground is as it
                // was scattered.
                let mut dug = 0_u32;
                for index in (0..self.buried.len()).rev() {
                    if flat(self.buried[index], stands.walker) < DIGS {
                        self.buried.remove(index);
                        let place = self.places.remove(index);
                        if u32::from(place) < u32::BITS {
                            dug |= 1 << place;
                        }
                    }
                }
                let here = self.here.clone();
                if let Some(ground) = here.and_then(|map| self.grounds.get_mut(&map)) {
                    ground.dug |= dug;
                }
                // What it is: mostly a little, and now and then one to be talked of for
                // years (`FINDS`), the more often the richer the ground is.
                let (kind, size) = (self.roll(), self.roll());
                let (which, kilos) = find(kind, size, self.rich);
                log::info!("gold: dug {}, {:.0} g", FINDS[which].0, kilos * 1000.0);
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
            // The door is the truck's: whoever shows the truck swings it (`Shell`).
            Use::OpenDoor | Use::ShutDoor => {}
            Use::IntoHold => {
                let hold = stands.open_hold()?;
                let (across, along) = (self.roll(), self.roll());
                let over = stands.over_hold(hold, across, along);
                self.let_go(stands, over, true);
            }
            Use::PickUp | Use::FromTray | Use::FromChest | Use::FromHold => {
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
            // No tray: onto the seat beside them, if the truck has one to put things on.
            None if !stands.holds.is_empty() => {
                let (across, along) = (self.roll(), self.roll());
                let over = stands.over_hold(0, across, along);
                self.let_go(stands, over, true);
            }
            None => self.let_go(stands, stands.walker + Vec3::Y * 1.0, false),
        }
    }

    /// A purse paid in gold: a nugget of this weight is put in the truck's tray, whose size
    /// inside and the middle of whose floor these are, in the truck's own space. It is a
    /// nugget like any other from then on.
    pub fn award(&mut self, kilos: f32, tray: (Vec3, Vec3)) {
        let shape = (self.roll() * 16_777_216.0) as u32;
        let (across, along) = (self.roll() - 0.5, self.roll() - 0.5);
        let (size, floor) = tray;
        let room = Vec3::new(size.x - 2.0 * INSET, 0.0, size.z - 2.0 * INSET).max(Vec3::ZERO);
        self.nuggets.push(Nugget {
            id: self.next,
            kilos,
            shape,
            lies: Lies::Aboard {
                at: floor + Vec3::new(across * room.x, 0.3, along * room.z),
                turned: Quat::IDENTITY,
            },
        });
        self.next += 1;
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
            *lies = if stands.in_tray(at) || stands.in_hold(at).is_some() {
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
            holds: &[],
            doors: &[],
        }
    }

    /// A truck with a seat to put things on, behind its passenger's door; and a driver
    /// standing somewhere beside it, the door open or shut.
    fn seat() -> Vec<Hold> {
        vec![Hold {
            name: "passenger_seat".to_owned(),
            size: Vec3::new(0.6, 0.42, 0.45),
            floor: Vec3::new(-0.45, 0.85, -0.2),
            ways: vec![(0, Vec3::new(-0.9, 1.05, -0.2))],
        }]
    }

    #[test]
    fn gold_goes_on_a_seat_through_an_open_door_and_stays_behind_a_shut_one() {
        let holds = seat();
        let door = |open: bool| {
            vec![Door {
                piece: "door_FL".to_owned(),
                open,
            }]
        };
        let (shut, open) = (door(false), door(true));
        let truck = Vec3::new(500.0, 5.0, 500.0);
        // Standing at the passenger's door, and standing by the tray, far from it.
        let at_door = truck + Vec3::new(-1.6, 0.0, -0.2);
        let with = |walker: Vec3, doors: &'static [Door], holds: &'static [Hold]| Stands {
            holds,
            doors,
            ..stands(walker)
        };
        let holds: &'static [Hold] = Vec::leak(holds);
        let (shut, open): (&'static [Door], &'static [Door]) = (Vec::leak(shut), Vec::leak(open));
        let mut gold = Gold::default();
        let mut bodies = Vec::new();
        gold.nuggets.push(Nugget {
            id: 7,
            kilos: 0.4,
            shape: 99,
            lies: Lies::Hand,
        });
        // A nugget in the hand at a shut door: the door is opened first. Nothing is let go.
        assert_eq!(gold.can(&with(at_door, shut, holds)), Some(Use::OpenDoor));
        assert_eq!(gold.act(&with(at_door, shut, holds)), Some(Use::OpenDoor));
        assert!(gold.in_hand().is_some());
        // Open, it goes in: onto the seat, aboard the truck, and is counted there.
        let stood = with(at_door, open, holds);
        assert_eq!(gold.act(&stood), Some(Use::IntoHold));
        // (It comes to rest a little above the seat, which is 0.85 up in a truck 5 up.)
        world(&mut bodies, gold.sync(&stood), 5.0 + 0.85 + 0.05);
        gold.see(&stood, &bodies);
        let Lies::Aboard { at, .. } = gold.nuggets[0].lies else {
            panic!("it is not aboard: {:?}", gold.nuggets[0].lies);
        };
        assert!(
            at.x < -0.15 && at.x > -0.75 && (at.y - 0.9).abs() < 0.3,
            "{at}"
        );
        assert_eq!(gold.in_holds(&stood), [("passenger_seat".to_owned(), 1)]);
        assert_eq!(gold.tray().0, 1);
        // With empty hands at the open door it is taken out again; or, nothing else to do,
        // the door is shut.
        assert_eq!(gold.can(&stood), Some(Use::FromHold));
        // Shut, it is not in reach, though it is an arm's length away: the door is opened.
        let closed = with(at_door, shut, holds);
        assert_eq!(gold.can(&closed), Some(Use::OpenDoor));
        // And from the other side of the truck there is nothing to do at all.
        let far_side = with(truck + Vec3::new(1.7, 0.0, -0.2), open, holds);
        assert_eq!(gold.can(&far_side), None);
        // Taken out, the hand has it and the seat is empty: the open door is shut.
        assert_eq!(gold.act(&stood), Some(Use::FromHold));
        assert!(gold.in_hand().is_some());
        gold.nuggets[0].lies = Lies::Loose {
            map: "elsewhere".to_owned(),
            at: Vec3::ZERO,
            turned: Quat::IDENTITY,
        };
        assert_eq!(gold.can(&stood), Some(Use::ShutDoor));
        // A shut door with nothing behind it is left alone: the driver may just be getting in.
        assert_eq!(gold.can(&closed), None);
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
    fn what_is_dug_up_is_mostly_a_little_and_worth_a_days_work_between_them() {
        // Every find is within its kind's weights, and the kinds are found as often as
        // the table says: over a great many, in ordinary ground.
        let mut found = [0_u32; 7];
        let (mut kilos, count) = (0.0_f64, 200_000_u32);
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut roll = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1_u64 << 24) as f32
        };
        for _ in 0..count {
            let (kind, weight) = find(roll(), roll(), 1.0);
            assert!(weight >= FINDS[kind].1 && weight <= FINDS[kind].2);
            found[kind] += 1;
            kilos += f64::from(weight);
        }
        let share = |kind: usize| f64::from(found[kind]) / f64::from(count);
        assert!(share(0) > 0.38 && share(0) < 0.43, "{}", share(0));
        assert!(share(4) > 0.025 && share(4) < 0.036, "{}", share(4));
        assert!(share(6) > 0.0003 && share(6) < 0.002, "{}", share(6));
        // On average a find is worth about $300 at the buyer's: between $230 and $380.
        let worth = kilos / f64::from(count) * f64::from(crate::shell::PRICE);
        assert!(
            worth > 230.0 && worth < 380.0,
            "a find is worth ${worth:.0} on average"
        );
        // Poor ground has fewer of the big ones, and rich ground more.
        let big = |rich: f32| chances(rich)[4..].iter().sum::<f32>();
        assert!(big(0.5) < big(1.0) * 0.5 && big(1.4) > big(1.0) * 1.5);
        println!("a find is worth ${worth:.0} on average in ordinary ground");
        let shares: f32 = chances(1.4).iter().sum();
        assert!((shares - 1.0).abs() < 1.0e-4);
    }

    #[test]
    fn gold_is_put_in_the_ground_when_a_map_is_entered_and_is_not_where_it_was() {
        let mut gold = Gold::default();
        gold.enter(&files(), Some("wombat"), 1, 7, 1.0);
        assert_eq!(gold.buried.len(), BURIED);
        // Off the track, on either side, and within the map's length.
        assert!(gold
            .buried
            .iter()
            .all(|at| at.z.abs() > 7.9 && at.z.abs() < 31.0));
        // Come into again, the next day or the day after, it is where it was: the map's
        // ground is kept as it was scattered.
        let first = gold.buried.clone();
        gold.enter(&files(), Some("wombat"), 2, 8, 1.0);
        assert_eq!(gold.buried, first);
        // Another map has its own.
        gold.enter(&files(), Some("granite"), 2, 8, 1.0);
        assert_ne!(gold.buried, first);
        // One dug, it stays dug; and the rest are where they were, for a few days.
        gold.enter(&files(), Some("wombat"), 2, 8, 1.0);
        let over = gold.buried[0];
        assert_eq!(gold.act(&stands(over)), Some(Use::Dig));
        gold.enter(&files(), Some("wombat"), 1 + RESCATTER_DAYS, 21, 1.0);
        assert_eq!(gold.buried.len(), BURIED - 1);
        assert!(gold
            .buried
            .iter()
            .all(|at| first.contains(at) && *at != over));
        // It is kept with the game, and read back.
        let mut kept = String::new();
        gold.keep(&mut kept);
        let mut read = Gold::read(&kept);
        assert_eq!(read.ground("wombat"), gold.ground("wombat"));
        read.enter(&files(), Some("wombat"), 1 + RESCATTER_DAYS, 99, 1.0);
        assert_eq!(read.buried, gold.buried);
        // More than a few days on, with one dug, it is scattered again: all of it, anew.
        gold.enter(&files(), Some("wombat"), 2 + RESCATTER_DAYS, 22, 1.0);
        assert_eq!(gold.buried.len(), BURIED);
        assert_ne!(gold.buried, first);
        // Ground nobody has dug in is left as it is, however long it has been.
        let untouched = gold.buried.clone();
        gold.enter(&files(), Some("wombat"), 60, 23, 1.0);
        assert_eq!(gold.buried, untouched);
        // A showroom, which is no map of the game's, has none and keeps none.
        gold.enter(&files(), None, 60, 24, 1.0);
        assert!(gold.buried.is_empty() && gold.ground("showroom").is_none());
        // A hub, which has a chest and places to stop at, has gold by its road as well.
        gold.enter(&hub(), Some("crossroads"), 1, 9, 1.0);
        assert_eq!(gold.buried.len(), BURIED);
        assert!(gold.any_buried());
        // A town has it too, though none where its buildings and its yard are.
        let mut town = files();
        town.push(GameFile {
            name: "outback.places".to_owned(),
            text: "place crossroads 100 5 0 7\nplace buyer 40 5 12 5.5\n".to_owned(),
        });
        gold.enter(&town, Some("town"), 1, 9, 0.4);
        assert_eq!(gold.buried.len(), BURIED);
    }

    #[test]
    fn a_nugget_is_a_thing_from_the_ground_to_the_hand_to_the_tray_to_the_chest_and_is_sold() {
        let mut gold = Gold::default();
        gold.enter(&files(), Some("wombat"), 1, 11, 1.0);
        let mut bodies: Vec<(u32, Vec3, Quat)> = Vec::new();
        let over = gold.buried[0];
        // It is heard from a long way off, faintly, and not from further; and the signal
        // says that the driver is over it just where it can be dug.
        // (Where there is no other gold about to be heard instead.)
        let mut alone = Gold::default();
        alone.bury(over);
        let off = |metres: f32| alone.signal(&stands(over + Vec3::X * metres));
        assert!(off(55.0) > 0.0 && off(55.0) < 0.15 && off(65.0) == 0.0);
        assert!(off(20.0) > off(30.0) && off(8.0) > off(20.0));
        assert!(off(DIGS + 0.3) < OVER && off(DIGS - 0.3) > OVER && off(0.0) > 0.99);
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
        assert!(gold.nuggets[0].kilos >= 0.01 && gold.in_hand().is_none());
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
        again.enter(&hub(), Some("crossroads"), 1, 3, 1.0);
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
        // In the chest it is not what the detector is listening for. (With what is in
        // the hub's own ground out of the way.)
        again.buried.clear();
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
        gold.enter(&files(), Some("wombat"), 1, 5, 1.0);
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
        again.enter(&files(), Some("wombat"), 1, 6, 1.0);
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
        gold.enter(&files(), Some("wombat"), 1, 3, 1.0);
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
