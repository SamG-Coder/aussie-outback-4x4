//! Damage: what hitting things does to a truck, piece by piece.
//!
//! The game measures every blow the truck takes: where on it, which way, over what patch
//! of it, and how hard, as a force in kilonewtons (`solstice_game::impact`). This decides
//! what each blow does. A truck is a kit of named pieces (`kit.rs`), each with a box round
//! it, and each kind of piece has a line of the **parts table** ([`PARTS`]) saying how
//! strong it is. Nothing here is a count of hits: a piece's state is how hard it has been
//! hit.
//!
//! * **Which pieces feel a blow.** Those in the patch it landed on, looked at along the way
//!   it pushed, and a little beyond its edges, the further the harder the blow.
//! * **How much of it each feels.** The outermost feel all of it, and take out of it what
//!   they `soak`; what is behind feels what is left. (Pieces whose faces are within a hand
//!   of each other, the way the blow pushed, are one layer: they feel it together.) So a
//!   bull bar spares the lamps behind it, and a blow that is hard enough goes through to
//!   the chassis.
//! * **What that does to it.** A piece keeps the hardest it has been hit (and a fifth of
//!   each further blow that is hard enough to mark it): its *hurt*, in kilonewtons. Its
//!   stage is the last of the table's four thresholds its hurt has passed: [`Stage`].
//!
//! What a stage looks like is [`pose`], and what it does to the driving is
//! [`Damage::handicap`]. The state is [`Damage`]: plain data, a line a piece, which a kept
//! game and a repair shop can read and write ([`Damage::to_text`], [`Damage::mend`]).
//!
//! `docs/damage.md` has the reasoning, the numbers, and how to add a piece.

use glam::{Mat4, Quat, Vec3};
use solstice_game::drive::Handicap;
use solstice_game::Impact;

/// The parts table: how strong each kind of piece is, and the model's own numbers.
///
/// A `part` line is a kind of piece: the start of its pieces' names (the longest that fits
/// a piece is its kind; `*` is everything else), the force in kilonewtons at which it is
/// scuffed, dented, hanging loose and torn off (`-` for never), how many kilonewtons it
/// takes out of a blow before what is behind it feels the rest, how it is held on, and
/// what it does for the truck.
///
/// For scale: a two-tonne truck stopped dead by a wall is hit with 56 kN at 10 km/h, 111
/// at 20, 222 at 40 and 444 at 80; landing from a metre puts about 35 kN through each
/// front wheel, from two and a half metres 70.
pub const PARTS: &str = "\
# How far beyond the patch a blow landed on its pieces still feel it: metres, and metres
# more for each kilonewton.
set reach 0.12 0.0012
# How deep a layer of the truck is, in metres: pieces whose outer faces are within this of
# each other, the way a blow pushed, feel it together, and not one after another.
set layer 0.08
# What a piece keeps of a blow that is not the hardest it has had.
set wear 0.2
# How deep into the truck a blow's shock breaks glass: metres, and metres for each kilonewton.
set shock 0.3 0.004
# How far a dented piece is pushed in, in metres: just dented, and about to come loose.
set dent 0.03 0.11
# How far the steering pulls, of full lock, for each piece of the front running gear that
# is dented, and that is wrecked.
set pull 0.07 0.16
# The share of the engine's pull that is left with its cooling dented, loose, and gone.
set power 0.85 0.6 0.35

#    kind               scuffed dented loose  off  soaks  held    does
part acc_bullbar_plate     40    130    270   420   150   bolted  -
part acc_bullbar_tube      35    110    240   360   110   bolted  -
part acc_bullbar_nudge     30     90    190   300    60   bolted  -
part bumper                20     60    130   210    45   bolted  -
part plate                  6     20     45    80     0   bolted  -
part grille                12     35     80   130     8   bolted  cooling
part lamp                  10     30     70   110     2   bolted  lamp
part indicator              8     25     55    90     1   bolted  lamp
part acc_spots              8     25     55    90     1   bolted  lamp
part acc_lightbar          10     30     70   110     2   bolted  lamp
part nose                  25     70      -     -   100   frame   cooling
part guard                 15     50    130   240    25   bolted  -
part bonnet                12     40    110   220    15   lid     -
part door                  12     45    130   260    25   hinged  -
part tailgate              12     45    110   220    20   hinged  -
part mirror                 3      9     18    32     0   stalk   -
part aerial                 2      5     10    18     0   stalk   -
part fin                    4     12     25    45     0   stalk   -
part wipers                 4     12     25    45     0   stalk   -
part glass                 15     30     55    55     0   glass   -
part cab                   25     90      -     -   400   frame   -
part interior              60    200      -     -     0   frame   -
part liner                 40    150      -     -     0   frame   -
part chassis               60    250      -     -   250   frame   -
part tray                  20     70      -     -   150   frame   -
part tub                   20     70      -     -   150   frame   -
part filler                 8     25      -     -     0   frame   -
part acc_canopy            15     55    140   260    30   bolted  -
part acc_sportsbar         20     70    160   280    30   bolted  -
part acc_tonneau            8     25     60   110     2   bolted  -
part acc_tray              15     50    120   200    10   bolted  -
part acc_rack              15     50    120   220    20   bolted  -
part acc_snorkel            8     25     55   100     2   bolted  -
part acc_steps             12     40     90   160    15   bolted  -
part acc_sliders           30    100    220   360    60   bolted  -
part acc_flares             6     20     45    80     2   bolted  -
part acc_spare             20     70    150   250    20   bolted  -
part lip                    6     20     45    80     1   bolted  -
part flap                   3     10     22    40     0   bolted  -
part trim                   5     15     35    60     0   bolted  -
part roof                   8     30     70   120     4   bolted  -
part exhaust               10     35     80   150     5   bolted  -
part tyre                  15     40     65     -     8   wheel   running
part rim                   25     50     80     -     6   wheel   running
part susp                  30     60      -     -    15   frame   running
part axle                  45     85      -     -    30   frame   running
part *                     15     50    120   200    10   bolted  -
";

/// How far gone a piece is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    #[default]
    Sound,
    /// Marked, and knocked a little out of line.
    Scuffed,
    /// Pushed in and crumpled; a lamp is out.
    Dented,
    /// Hanging by what is left of its fixings: it swings as the truck moves.
    Loose,
    /// Torn off, and lying where it fell.
    Off,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Sound,
        Stage::Scuffed,
        Stage::Dented,
        Stage::Loose,
        Stage::Off,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Sound => "sound",
            Stage::Scuffed => "scuffed",
            Stage::Dented => "dented",
            Stage::Loose => "loose",
            Stage::Off => "off",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.name() == name)
    }

    /// How much of what it soaks a piece still soaks, this far gone.
    fn holds(self) -> f32 {
        match self {
            Stage::Sound | Stage::Scuffed => 1.0,
            Stage::Dented => 0.7,
            Stage::Loose => 0.3,
            Stage::Off => 0.0,
        }
    }
}

/// How a kind of piece is held on, which is how it comes away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    /// By bolts or clips: loose, it hangs by one end; off, it falls.
    Bolted,
    /// On a hinge the kit names (a door, a tailgate): loose, its latch has gone and it
    /// swings. A piece the kit has no hinge for is as one bolted.
    Hinged,
    /// A bonnet: hinged across its back edge, and loose it lifts in the wind.
    Lid,
    /// On a stalk (a mirror, an aerial): loose, it is folded back.
    Stalk,
    /// Glass: it is whole until it is broken, and then it is gone.
    Glass,
    /// A tyre or a rim, on its hub: it buckles, and stays on.
    Wheel,
    /// Part of the truck's frame or shell: it crumples, and never comes off.
    Frame,
}

/// What a kind of piece does for the truck, which is what is lost with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Does {
    Nothing,
    /// It lights: dented, it is out.
    Lamp,
    /// It is, or covers, the engine's cooling: the engine loses power as it goes.
    Cooling,
    /// A wheel, or what holds one: at the front, the steering pulls to its side.
    Running,
}

/// A kind of piece: a line of the parts table.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// What its pieces' names begin with.
    pub named: String,
    /// The force in kilonewtons at which it is scuffed, dented, loose, and off: without
    /// end for a stage it never reaches.
    pub at: [f32; 4],
    /// The kilonewtons it takes out of a blow before what is behind it feels the rest.
    pub soaks: f32,
    pub mount: Mount,
    pub does: Does,
}

impl Part {
    /// How far gone a piece of this kind is that has been hurt so much.
    pub fn stage(&self, hurt: f32) -> Stage {
        let passed = self.at.iter().filter(|at| hurt >= **at).count();
        Stage::ALL[passed]
    }
}

/// The parts table, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub parts: Vec<Part>,
    pub reach: [f32; 2],
    pub layer: f32,
    pub wear: f32,
    pub shock: [f32; 2],
    pub dent: [f32; 2],
    pub pull: [f32; 2],
    pub power: [f32; 3],
}

impl Table {
    /// The game's own table ([`PARTS`]).
    pub fn built_in() -> Self {
        Self::parse(PARTS).expect("the game's own parts table reads")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut table = Table {
            parts: Vec::new(),
            reach: [0.0; 2],
            layer: 0.0,
            wear: 0.0,
            shock: [0.0; 2],
            dent: [0.0; 2],
            pull: [0.0; 2],
            power: [1.0; 3],
        };
        for (number, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("");
            let words: Vec<&str> = line.split_whitespace().collect();
            let wrong = |message: String| format!("line {}: {message}", number + 1);
            let number_of = |word: &str| -> Result<f32, String> {
                if word == "-" {
                    return Ok(f32::INFINITY);
                }
                word.parse::<f32>()
                    .ok()
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .ok_or_else(|| wrong(format!("`{word}` is not a number")))
            };
            match words[..] {
                [] => {}
                ["set", name, ref values @ ..] => {
                    let values: Vec<f32> = values
                        .iter()
                        .map(|word| number_of(word))
                        .collect::<Result<_, _>>()?;
                    let fits = match (name, &values[..]) {
                        ("reach", [a, b]) => {
                            table.reach = [*a, *b];
                            true
                        }
                        ("layer", [a]) => {
                            table.layer = *a;
                            true
                        }
                        ("wear", [a]) => {
                            table.wear = *a;
                            true
                        }
                        ("shock", [a, b]) => {
                            table.shock = [*a, *b];
                            true
                        }
                        ("dent", [a, b]) => {
                            table.dent = [*a, *b];
                            true
                        }
                        ("pull", [a, b]) => {
                            table.pull = [*a, *b];
                            true
                        }
                        ("power", [a, b, c]) => {
                            table.power = [*a, *b, *c];
                            true
                        }
                        _ => false,
                    };
                    if !fits {
                        return Err(wrong(format!(
                            "`set {name}` with {} values: reach 2, layer 1, wear 1, shock 2, dent 2, \
                             pull 2, power 3",
                            values.len()
                        )));
                    }
                }
                ["part", named, scuffed, dented, loose, off, soaks, mount, does] => {
                    let at = [
                        number_of(scuffed)?,
                        number_of(dented)?,
                        number_of(loose)?,
                        number_of(off)?,
                    ];
                    if at.windows(2).any(|pair| pair[1] < pair[0]) {
                        return Err(wrong(format!(
                            "`{named}`: each stage takes more than the last"
                        )));
                    }
                    table.parts.push(Part {
                        named: named.to_owned(),
                        at,
                        soaks: number_of(soaks)?,
                        mount: match mount {
                            "bolted" => Mount::Bolted,
                            "hinged" => Mount::Hinged,
                            "lid" => Mount::Lid,
                            "stalk" => Mount::Stalk,
                            "glass" => Mount::Glass,
                            "wheel" => Mount::Wheel,
                            "frame" => Mount::Frame,
                            _ => {
                                return Err(wrong(format!(
                                    "`{mount}`: a piece is held bolted, hinged, lid, stalk, \
                                     glass, wheel or frame"
                                )))
                            }
                        },
                        does: match does {
                            "-" => Does::Nothing,
                            "lamp" => Does::Lamp,
                            "cooling" => Does::Cooling,
                            "running" => Does::Running,
                            _ => {
                                return Err(wrong(format!(
                                    "`{does}`: a piece does -, lamp, cooling or running"
                                )))
                            }
                        },
                    });
                }
                [keyword, ..] => {
                    return Err(wrong(format!(
                        "`{keyword}` is not a line of a parts table: `set NAME VALUES`, or \
                         `part KIND SCUFFED DENTED LOOSE OFF SOAKS HELD DOES`"
                    )))
                }
            }
        }
        if !table.parts.iter().any(|part| part.named == "*") {
            return Err("a parts table needs a `part *` line, for everything else".to_owned());
        }
        Ok(table)
    }

    /// The kind a piece is: the line whose name is the longest that the piece's begins
    /// with, or the line for everything else.
    pub fn part(&self, piece: &str) -> &Part {
        let fits = |part: &&Part| piece.starts_with(part.named.as_str());
        let found = self.parts.iter().filter(fits);
        found
            .max_by_key(|part| part.named.len())
            .or_else(|| self.parts.iter().find(|part| part.named == "*"))
            .expect("a table has a line for everything else")
    }
}

/// A blow, as this reckons with it: an [`Impact`] with its force in kilonewtons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Where it landed, and the way it pushed the truck, in the truck's own space.
    pub at: Vec3,
    pub push: Vec3,
    /// The patch it landed on: the least and the greatest corners of a box.
    pub low: Vec3,
    pub high: Vec3,
    /// How hard, in kilonewtons.
    pub force: f32,
    /// The wheel it came up through, if it did.
    pub wheel: Option<usize>,
}

impl From<&Impact> for Hit {
    fn from(blow: &Impact) -> Self {
        Self {
            at: blow.at,
            push: blow.push,
            low: blow.low,
            high: blow.high,
            force: blow.force / 1000.0,
            wheel: blow.wheel,
        }
    }
}

impl Hit {
    /// A blow at a point, with no more of a patch than that: for a picture, or a test.
    pub fn at_point(at: Vec3, push: Vec3, force: f32) -> Self {
        Self {
            at,
            push: push.normalize_or(Vec3::Z),
            low: at,
            high: at,
            force,
            wheel: None,
        }
    }

    /// A blow from words, for a picture: `X,Y,Z,PX,PY,PZ,KN`, where it landed and the way
    /// it pushed, in the truck's own space (its nose is along -z), and how hard.
    pub fn parse(text: &str) -> Result<Self, String> {
        let numbers: Result<Vec<f32>, _> =
            text.split(',').map(|word| word.trim().parse()).collect();
        match numbers.as_deref() {
            Ok([x, y, z, px, py, pz, force]) => Ok(Self::at_point(
                Vec3::new(*x, *y, *z),
                Vec3::new(*px, *py, *pz),
                *force,
            )),
            _ => Err(format!("`{text}` is not a blow: X,Y,Z,PX,PY,PZ,KN")),
        }
    }
}

/// A piece on the truck, and the box round it in the truck's own space.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub name: String,
    pub low: Vec3,
    pub high: Vec3,
}

/// What one piece has taken.
#[derive(Clone, Debug, PartialEq)]
pub struct Hurt {
    pub piece: String,
    /// How hard it has been hit, in kilonewtons: what its stage is told from.
    pub hurt: f32,
    /// Where the hardest blow it has felt landed, and the way it pushed, in the truck's
    /// own space: which way it is bent.
    pub at: Vec3,
    pub push: Vec3,
}

/// A piece whose stage a blow changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub piece: String,
    pub from: Stage,
    pub to: Stage,
    /// How much of the blow it felt, in kilonewtons.
    pub felt: f32,
}

/// A truck's damage: each piece that has been hurt. A piece that is not here is sound.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Damage {
    pub hurts: Vec<Hurt>,
}

/// Which corner each wheel is, as a tyre's or a rim's name ends, and which end of the truck
/// as a suspension's or an axle's does.
const CORNERS: [&str; 4] = ["_FL", "_FR", "_RL", "_RR"];
const ENDS: [[&str; 2]; 2] = [["_F", "_front"], ["_R", "_rear"]];

/// Whether a piece of the running gear is at a wheel: that wheel's own tyre and rim, and
/// the suspension and the axle of its end of the truck.
fn at_wheel(piece: &str, wheel: usize) -> bool {
    piece.ends_with(CORNERS[wheel]) || ENDS[wheel / 2].iter().any(|end| piece.ends_with(end))
}

impl Damage {
    /// A truck's damage from its text: a line a piece, `PIECE HURT X Y Z PX PY PZ`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut damage = Self::default();
        for (number, line) in text.lines().enumerate() {
            let mut words = line.split_whitespace();
            let Some(piece) = words.next() else { continue };
            let numbers: Result<Vec<f32>, _> = words.map(str::parse::<f32>).collect();
            match numbers.as_deref() {
                Ok([hurt, x, y, z, px, py, pz]) if hurt.is_finite() && *hurt >= 0.0 => {
                    damage.hurts.push(Hurt {
                        piece: piece.to_owned(),
                        hurt: *hurt,
                        at: Vec3::new(*x, *y, *z),
                        push: Vec3::new(*px, *py, *pz).normalize_or(Vec3::Z),
                    });
                }
                _ => {
                    return Err(format!(
                        "line {}: expected PIECE HURT X Y Z PX PY PZ",
                        number + 1
                    ))
                }
            }
        }
        Ok(damage)
    }

    /// Its text, which [`Damage::parse`] reads back.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for hurt in &self.hurts {
            let (at, push) = (hurt.at, hurt.push);
            text.push_str(&format!(
                "{} {:.1} {:.2} {:.2} {:.2} {:.3} {:.3} {:.3}\n",
                hurt.piece, hurt.hurt, at.x, at.y, at.z, push.x, push.y, push.z
            ));
        }
        text
    }

    /// Damage as words say it, for a picture: `bumper_front=dented,door_FL=loose`, each
    /// piece hurt just enough to be so, pushed from wherever is nearest outside the truck.
    /// A piece the truck has not is an error; a stage its kind never reaches is too.
    pub fn staged(table: &Table, placed: &[Placed], text: &str) -> Result<Self, String> {
        let mut damage = Self::default();
        for word in text
            .split(',')
            .map(str::trim)
            .filter(|word| !word.is_empty())
        {
            let (piece, stage) = word
                .split_once('=')
                .ok_or_else(|| format!("`{word}`: expected PIECE=STAGE"))?;
            let stage = Stage::named(stage).ok_or_else(|| {
                format!("`{stage}`: a piece is sound, scuffed, dented, loose or off")
            })?;
            let found = placed.iter().find(|placed| placed.name == piece);
            let found = found.ok_or_else(|| format!("the truck has no `{piece}` on it"))?;
            let Some(passed) = (stage as usize).checked_sub(1) else {
                continue;
            };
            let hurt = table.part(piece).at[passed];
            if !hurt.is_finite() {
                return Err(format!("`{piece}` is never {}", stage.name()));
            }
            // Pushed in from outside: along whichever way its middle is furthest out.
            let middle = (found.low + found.high) * 0.5 - Vec3::Y * 1.0;
            let out = middle.abs().max_element();
            let push = Vec3::select(
                middle.abs().cmpeq(Vec3::splat(out)),
                -middle.signum(),
                Vec3::ZERO,
            );
            damage.hurts.push(Hurt {
                piece: piece.to_owned(),
                hurt: hurt * 1.02,
                at: (found.low + found.high) * 0.5,
                push: push.normalize_or(Vec3::Z),
            });
        }
        Ok(damage)
    }

    pub fn hurt(&self, piece: &str) -> Option<&Hurt> {
        self.hurts.iter().find(|hurt| hurt.piece == piece)
    }

    /// How far gone a piece is.
    pub fn stage(&self, table: &Table, piece: &str) -> Stage {
        self.hurt(piece)
            .map_or(Stage::Sound, |hurt| table.part(piece).stage(hurt.hurt))
    }

    /// Every piece that is not sound, with how far gone it is: what a repair shop lists.
    pub fn list(&self, table: &Table) -> Vec<(&str, Stage)> {
        let each = self.hurts.iter();
        each.map(|hurt| {
            (
                hurt.piece.as_str(),
                table.part(&hurt.piece).stage(hurt.hurt),
            )
        })
        .filter(|(_, stage)| *stage != Stage::Sound)
        .collect()
    }

    /// Mends a piece: it is sound again. Whether it was hurt.
    pub fn mend(&mut self, piece: &str) -> bool {
        let before = self.hurts.len();
        self.hurts.retain(|hurt| hurt.piece != piece);
        self.hurts.len() != before
    }

    /// Mends every piece.
    pub fn mend_all(&mut self) {
        self.hurts.clear();
    }

    /// How much of a blow each piece on the truck feels, in kilonewtons: each that feels
    /// any, by its place among `placed`.
    pub fn felt(&self, table: &Table, placed: &[Placed], hit: &Hit) -> Vec<(usize, f32)> {
        let stage = |piece: &Placed| self.stage(table, &piece.name);
        // Each piece in the way: how near the patch it is, from 0 to 1, and how deep into
        // the truck its nearest face is, the way the blow pushed.
        let mut reached: Vec<(usize, f32, f32)> = Vec::new();
        if let Some(wheel) = hit.wheel {
            // Up through a wheel: its tyre, then its rim, then what holds it, in the order
            // the table has them.
            for (index, piece) in placed.iter().enumerate() {
                let part = table.part(&piece.name);
                if part.does == Does::Running && at_wheel(&piece.name, wheel) {
                    let order = table.parts.iter().position(|each| each == part);
                    reached.push((index, 1.0, order.unwrap_or(0) as f32));
                }
            }
        } else {
            let push = hit.push;
            let margin = table.reach[0] + table.reach[1] * hit.force;
            // How deep into the truck, by whichever of its own axes the blow was most
            // along: a truck is made in layers front to back, side to side and top to
            // bottom, and a blow a little off square is still a blow to the same layer.
            let way = dominant(push);
            let least = |low: Vec3, high: Vec3| (low * way).min(high * way).element_sum();
            let face = least(hit.low, hit.high);
            for (index, piece) in placed.iter().enumerate() {
                // How far from the patch it is, across the way the blow pushed.
                let apart = (piece.low - hit.high)
                    .max(hit.low - piece.high)
                    .max(Vec3::ZERO);
                let gap = (apart * apart * (Vec3::ONE - push * push))
                    .element_sum()
                    .max(0.0)
                    .sqrt();
                if gap < margin {
                    let depth = least(piece.low, piece.high) - face;
                    reached.push((index, 1.0 - gap / margin, depth));
                }
            }
        }
        reached.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        let mut felt = Vec::new();
        let mut left = hit.force;
        // A layer at a time: every piece of it feels what the layers before it left, and
        // what they soak between them is taken out for the next.
        let mut layer = 0;
        while layer < reached.len() {
            let within = |each: &&(usize, f32, f32)| each.2 <= reached[layer].2 + table.layer;
            let pieces = reached[layer..].iter().take_while(within).count().max(1);
            let mut soaked = 0.0;
            for &(index, near, depth) in &reached[layer..layer + pieces] {
                let piece = &placed[index];
                let (part, stage) = (table.part(&piece.name), stage(piece));
                if stage == Stage::Off {
                    continue;
                }
                let feels = if part.mount == Mount::Glass {
                    // Glass is not in the way of anything: it is shaken, as deep as the
                    // shock of the blow goes.
                    let deep = table.shock[0] + table.shock[1] * hit.force;
                    hit.force * near * (1.0 - depth.max(0.0) / deep).max(0.0)
                } else {
                    let feels = left * near;
                    soaked += feels.min(part.soaks * near * stage.holds());
                    feels
                };
                if feels > 0.0 {
                    felt.push((index, feels));
                }
            }
            left = (left - soaked).max(0.0);
            layer += pieces;
        }
        felt
    }

    /// The truck takes a blow. Gives each piece whose stage it changed.
    pub fn strike(&mut self, table: &Table, placed: &[Placed], hit: &Hit) -> Vec<Change> {
        let mut changes = Vec::new();
        for (index, felt) in self.felt(table, placed, hit) {
            let piece = &placed[index].name;
            let part = table.part(piece);
            // What does not so much as mark it is nothing to it, however often.
            if felt < part.at[0] {
                continue;
            }
            let at = self.hurts.iter().position(|hurt| hurt.piece == *piece);
            let at = at.unwrap_or_else(|| {
                self.hurts.push(Hurt {
                    piece: piece.clone(),
                    hurt: 0.0,
                    at: hit.at,
                    push: hit.push,
                });
                self.hurts.len() - 1
            });
            let hurt = &mut self.hurts[at];
            let from = part.stage(hurt.hurt);
            // It is bent the way of the hardest blow it has had.
            if felt >= hurt.hurt {
                (hurt.at, hurt.push) = (hit.at, hit.push);
            }
            hurt.hurt = hurt.hurt.max(felt) + table.wear * hurt.hurt.min(felt);
            let to = part.stage(hurt.hurt);
            if to != from {
                changes.push(Change {
                    piece: piece.clone(),
                    from,
                    to,
                    felt,
                });
            }
        }
        changes
    }

    /// What the damage does to the driving: a wrecked front wheel, or what holds it, pulls
    /// the steering to its side; and an engine that has lost its cooling loses power.
    pub fn handicap(&self, table: &Table) -> Handicap {
        let mut handicap = Handicap::default();
        for hurt in &self.hurts {
            let part = table.part(&hurt.piece);
            let stage = part.stage(hurt.hurt);
            match (part.does, stage) {
                (Does::Running, Stage::Dented | Stage::Loose | Stage::Off)
                    if at_wheel(&hurt.piece, 0) || at_wheel(&hurt.piece, 1) =>
                {
                    // To the side of the wheel it is, or of the blow that bent it.
                    let side = if hurt.piece.ends_with(CORNERS[0]) {
                        -1.0
                    } else if hurt.piece.ends_with(CORNERS[1]) || hurt.at.x >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let much = if stage == Stage::Dented { 0 } else { 1 };
                    handicap.pull += side * table.pull[much];
                }
                (Does::Cooling, Stage::Dented | Stage::Loose | Stage::Off) => {
                    let left = table.power[stage as usize - Stage::Dented as usize];
                    handicap.power = handicap.power.min(left);
                }
                _ => {}
            }
        }
        handicap.pull = handicap.pull.clamp(-0.45, 0.45);
        handicap
    }

    /// Whether a lamp is out: it is one, and is dented or worse.
    pub fn is_out(&self, table: &Table, piece: &str) -> bool {
        table.part(piece).does == Does::Lamp && self.stage(table, piece) >= Stage::Dented
    }
}

/// A number from -1 to 1 that is a piece's own: so that no two hang quite alike.
fn own(piece: &str, salt: u32) -> f32 {
    let mut hash = 0x811C_9DC5_u32 ^ salt.wrapping_mul(0x9E37_79B9);
    for byte in piece.bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
    }
    hash ^= hash >> 15;
    (hash & 0xffff) as f32 / 32767.5 - 1.0
}

/// Turned about a line through a point.
fn about(point: Vec3, axis: Vec3, angle: f32) -> Mat4 {
    Mat4::from_translation(point)
        * Mat4::from_axis_angle(axis, angle)
        * Mat4::from_translation(-point)
}

/// Which of the truck's own axes a way is most along, as a way one long.
fn dominant(way: Vec3) -> Vec3 {
    let size = way.abs();
    if size.x >= size.y && size.x >= size.z {
        Vec3::X * way.x.signum()
    } else if size.y >= size.z {
        Vec3::Y * way.y.signum()
    } else {
        Vec3::Z * way.z.signum()
    }
}

/// How a piece that swings is hung: about what line, and between what angles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hang {
    pub pivot: Vec3,
    pub axis: Vec3,
    /// The angle it hangs at with the truck at rest, and the furthest it swings, each way
    /// of that, in radians.
    pub rest: f32,
    pub low: f32,
    pub high: f32,
    /// How the truck's own acceleration (to its right, up, back, in metres a second
    /// squared) swings it, in radians a second squared for each: and how fast going lifts
    /// it, in radians for each metre a second.
    pub drive: Vec3,
    pub lift: f32,
}

/// How a loose piece hangs: nothing for one that does not swing. `hinge` is the kit's
/// hinge for the piece, if it has one: where it is, whether it is upright, and how far it
/// opens, in degrees.
pub fn hang(
    part: &Part,
    hurt: &Hurt,
    (low, high): (Vec3, Vec3),
    hinge: Option<(Vec3, bool, f32)>,
) -> Option<Hang> {
    let (middle, half) = ((low + high) * 0.5, (high - low) * 0.5);
    match (part.mount, hinge) {
        (Mount::Hinged, Some((at, upright, opens))) => {
            let opens = opens.to_radians();
            let (near, far) = (0.06 * opens, 0.5 * opens);
            Some(Hang {
                pivot: at,
                axis: if upright { Vec3::Y } else { Vec3::X },
                rest: 0.16 * opens,
                low: near.min(far),
                high: near.max(far),
                // A door is thrown open by the truck turning away from it and by its
                // slowing; a tailgate by its pulling away.
                drive: if upright {
                    Vec3::new(
                        -0.9 * opens.signum() * at.x.signum(),
                        0.0,
                        0.25 * opens.signum(),
                    ) * opens.signum()
                } else {
                    Vec3::new(0.0, 0.3, -0.8) * opens.signum()
                },
                lift: 0.0,
            })
        }
        (Mount::Lid, _) => Some(Hang {
            // Across its back edge: its front lifts.
            pivot: Vec3::new(middle.x, high.y, high.z),
            axis: Vec3::X,
            rest: 0.05,
            low: 0.02,
            high: 0.95,
            drive: Vec3::new(0.0, 0.5, 0.0),
            lift: 0.03,
        }),
        (Mount::Bolted | Mount::Hinged, _) => {
            // By the end the blow did not land on: the other end droops. For something
            // that stands taller than it is long, its top leans out.
            let along = if half.x >= half.z { Vec3::X } else { Vec3::Z };
            if half.y > half.x.max(half.z) {
                let out = dominant(Vec3::new(middle.x, 0.0, middle.z));
                return Some(Hang {
                    pivot: middle - Vec3::Y * half.y,
                    axis: Vec3::Y.cross(out).normalize_or(Vec3::X),
                    rest: 0.2,
                    low: 0.08,
                    high: 0.36,
                    drive: Vec3::new(0.25, 0.4, 0.25) * 0.5,
                    lift: 0.0,
                });
            }
            let struck = (hurt.at - middle).dot(along);
            let end = if struck.abs() > 0.02 {
                struck.signum()
            } else {
                own(&hurt.piece, 1).signum()
            };
            let reach = along * end;
            // No more than lets its low end down to the ground, about.
            let droop = (0.22_f32).min((low.y - 0.12).max(0.02) / (half.dot(along) * 2.0).max(0.2));
            Some(Hang {
                pivot: middle - reach * half.dot(along) + Vec3::Y * half.y,
                axis: Vec3::Y.cross(reach),
                rest: droop,
                low: droop * 0.5,
                high: droop * 1.5,
                drive: Vec3::new(0.0, 0.5, 0.0),
                lift: 0.0,
            })
        }
        (Mount::Stalk, _) => {
            if middle.x.abs() < 0.3 {
                // On the middle of the truck: it is bent over backwards.
                return Some(Hang {
                    pivot: middle - Vec3::Y * half.y,
                    axis: Vec3::X,
                    rest: 0.5,
                    low: 0.4,
                    high: 0.6,
                    drive: Vec3::new(0.0, 0.1, 0.1),
                    lift: 0.0,
                });
            }
            // Out on one side: folded back along the truck, about its inner end.
            let side = middle.x.signum();
            Some(Hang {
                pivot: Vec3::new(middle.x - side * half.x, middle.y, middle.z),
                axis: Vec3::Y,
                rest: -side * 1.0,
                low: -side * 1.0 - 0.12,
                high: -side * 1.0 + 0.12,
                drive: Vec3::new(0.0, 0.0, 0.3 * side),
                lift: 0.0,
            })
        }
        (Mount::Glass | Mount::Wheel | Mount::Frame, _) => None,
    }
}

/// How far a loose piece has swung, and how fast it is swinging.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Swing {
    pub angle: f32,
    pub rate: f32,
}

impl Swing {
    /// At rest where it hangs.
    pub fn at_rest(hang: &Hang) -> Self {
        Self {
            angle: hang.rest,
            rate: 0.0,
        }
    }

    /// Time passes. `felt` is how the truck is accelerating in its own space (to its
    /// right, up and back, in metres a second squared), and `speed` how fast it is going.
    pub fn advance(&mut self, hang: &Hang, felt: Vec3, speed: f32, seconds: f32) {
        // What is left of its fixings pulls it back to where it hangs, weakly, and rubs.
        const PULL: f32 = 30.0;
        const RUB: f32 = 3.2;
        let rest = (hang.rest + hang.lift * speed.abs()).clamp(hang.low, hang.high);
        let mut left = seconds.clamp(0.0, 0.1);
        while left > 0.0 {
            let dt = left.min(1.0 / 120.0);
            left -= dt;
            let pushed = hang.drive.dot(felt).clamp(-60.0, 60.0);
            self.rate += (PULL * (rest - self.angle) + pushed - RUB * self.rate) * dt;
            self.angle += self.rate * dt;
            // It comes up against its stops, and does not bounce far off them.
            if self.angle < hang.low || self.angle > hang.high {
                self.angle = self.angle.clamp(hang.low, hang.high);
                self.rate *= -0.3;
            }
        }
    }
}

/// How a hurt piece is moved from where it is when sound, in the truck's own space (or,
/// for a tyre or a rim, in its wheel's, about its hub). `swing` is how far it has swung, if
/// it hangs loose. A piece that is off is not posed: it is no longer on the truck.
pub fn pose(
    table: &Table,
    part: &Part,
    hurt: &Hurt,
    bounds: (Vec3, Vec3),
    swing: Option<(&Hang, f32)>,
) -> Mat4 {
    let stage = part.stage(hurt.hurt);
    let (low, high) = bounds;
    let (middle, half) = ((low + high) * 0.5, (high - low) * 0.5);
    let push = hurt.push.normalize_or(Vec3::Z);
    match (stage, part.mount) {
        (Stage::Sound | Stage::Off, _) => Mat4::IDENTITY,
        // Glass shows nothing until it goes.
        (_, Mount::Glass) => Mat4::IDENTITY,
        (Stage::Scuffed, Mount::Frame | Mount::Wheel) => Mat4::IDENTITY,
        // Knocked a little out of line.
        (Stage::Scuffed, _) => {
            let axis = Vec3::new(
                own(&hurt.piece, 2),
                own(&hurt.piece, 3),
                own(&hurt.piece, 4),
            );
            Mat4::from_translation(push * 0.006)
                * about(
                    middle,
                    axis.normalize_or(Vec3::Y),
                    0.012 * own(&hurt.piece, 5).signum(),
                )
        }
        // Buckled: it is tipped on its hub, and wobbles as it turns.
        (_, Mount::Wheel) => {
            let tipped: f32 = if stage == Stage::Dented { 0.045 } else { 0.11 };
            Mat4::from_axis_angle(Vec3::Z, tipped * own(&hurt.piece, 6).signum())
        }
        _ => {
            // How far through being dented it is: all the way, once it is loose.
            let through = if stage > Stage::Dented {
                1.0
            } else {
                let to = if part.at[2].is_finite() {
                    part.at[2]
                } else {
                    part.at[1] * 2.5
                };
                ((hurt.hurt - part.at[1]) / (to - part.at[1]).max(1.0)).clamp(0.0, 1.0)
            };
            let depth = table.dent[0] + (table.dent[1] - table.dent[0]) * through;
            // Crumpled: shortened the way it was pushed, its far side staying where it is.
            let way = dominant(push);
            let long = (half * 2.0).dot(way.abs()).max(0.05);
            let squash = (1.0 - depth / long).max(if part.mount == Mount::Frame {
                0.9
            } else {
                0.72
            });
            let far = middle + way * half.dot(way.abs());
            let crumple = Mat4::from_translation(far)
                * Mat4::from_scale(Vec3::ONE - way.abs() * (1.0 - squash))
                * Mat4::from_translation(-far);
            if part.mount == Mount::Frame {
                return crumple;
            }
            // Pushed in, and tipped: the side the blow landed on goes in furthest.
            let lever = (hurt.at - middle) - push * (hurt.at - middle).dot(push);
            let axis = lever.cross(push).normalize_or(Vec3::ZERO);
            let tipped = if axis == Vec3::ZERO {
                Mat4::IDENTITY
            } else {
                about(middle, axis, 0.04 + 0.1 * through)
            };
            let dented = Mat4::from_translation(push * depth * 0.6) * tipped * crumple;
            match swing {
                Some((hang, angle)) if stage == Stage::Loose => {
                    about(hang.pivot, hang.axis, angle) * dented
                }
                _ => dented,
            }
        }
    }
}

/// A piece that has come off, on its way to the ground and lying on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fallen {
    pub piece: String,
    /// Where the truck was when it came off: the piece's instances are drawn from here.
    pub from: Mat4,
    /// The box round it, in the truck's own space as it then was.
    pub bounds: (Vec3, Vec3),
    /// How far it has moved since, and how fast it is moving, in the world.
    pub moved: Vec3,
    pub velocity: Vec3,
    /// How far it has tumbled, of the way to how it will lie, and whether it lies.
    pub tumbled: f32,
    pub lies: bool,
}

/// How long a piece takes to tumble over to how it will lie, in seconds.
const TUMBLES: f32 = 0.7;

impl Fallen {
    /// A piece comes off a truck that is at `from`, going at `velocity`, knocked off it
    /// the way `knocked` goes, in the world.
    pub fn new(
        piece: &str,
        from: Mat4,
        bounds: (Vec3, Vec3),
        velocity: Vec3,
        knocked: Vec3,
    ) -> Self {
        Self {
            piece: piece.to_owned(),
            from,
            bounds,
            moved: Vec3::ZERO,
            // It keeps some of the truck's going, and is knocked the way the blow went:
            // back off whatever it was torn off against.
            velocity: velocity * 0.45 + Vec3::Y * 0.8 + knocked.normalize_or_zero() * 1.2,
            tumbled: 0.0,
            lies: false,
        }
    }

    /// Where its instances are drawn from now: what the truck's place was, moved and
    /// tumbled with the piece.
    pub fn placed(&self) -> Mat4 {
        let (low, high) = self.bounds;
        let (middle, half) = ((low + high) * 0.5, (high - low) * 0.5);
        // It ends up lying on its biggest face: its thinnest way up, a little askew.
        let thin = if half.y <= half.x && half.y <= half.z {
            Quat::IDENTITY
        } else if half.x <= half.z {
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2 * own(&self.piece, 7).signum())
        } else {
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 * own(&self.piece, 7).signum())
        };
        let askew = Quat::from_rotation_y(own(&self.piece, 8) * 0.9);
        let turned = Quat::IDENTITY.slerp(askew * thin, self.tumbled);
        Mat4::from_translation(self.moved)
            * self.from
            * Mat4::from_translation(middle)
            * Mat4::from_quat(turned)
            * Mat4::from_translation(-middle)
    }

    /// The lowest of it, in the world.
    fn lowest(&self) -> f32 {
        let (low, high) = self.bounds;
        let placed = self.placed();
        let corner = |index: u32| {
            let pick = |bit: u32, low: f32, high: f32| if index & bit == 0 { low } else { high };
            placed
                .transform_point3(Vec3::new(
                    pick(1, low.x, high.x),
                    pick(2, low.y, high.y),
                    pick(4, low.z, high.z),
                ))
                .y
        };
        (0..8).map(corner).fold(f32::MAX, f32::min)
    }

    /// Time passes: it falls, tumbling, until it lies on the ground, whose height under a
    /// place `ground` gives. Whether it moved.
    pub fn advance(&mut self, seconds: f32, ground: impl Fn(Vec3) -> f32) -> bool {
        if self.lies {
            return false;
        }
        let seconds = seconds.clamp(0.0, 0.1);
        self.tumbled = (self.tumbled + seconds / TUMBLES).min(1.0);
        self.velocity.y -= 9.81 * seconds;
        self.moved += self.velocity * seconds;
        let (low, high) = self.bounds;
        let middle = self.placed().transform_point3((low + high) * 0.5);
        let under = ground(middle) + 0.01;
        let lowest = self.lowest();
        if lowest <= under {
            // On the ground: it slides to a stop, and lies once it has tumbled over.
            self.moved.y += under - lowest;
            self.velocity.y = 0.0;
            let slowed = (1.0 - 6.0 * seconds).max(0.0);
            self.velocity = Vec3::new(self.velocity.x * slowed, 0.0, self.velocity.z * slowed);
            self.lies = self.tumbled >= 1.0 && self.velocity.length() < 0.2;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A truck's pieces and their boxes: a Howitt's, as its own model measures
    /// (`tools/blender/truck/build.py`), with its nose along -z.
    fn truck(more: &[&str]) -> Vec<Placed> {
        let boxes: [(&str, [f32; 6]); 30] = [
            ("nose", [-0.919, 0.46, -2.365, 0.919, 1.138, -0.925]),
            ("bonnet", [-0.819, 1.033, -2.275, 0.819, 1.152, -0.955]),
            ("grille", [-0.466, 0.715, -2.405, 0.466, 0.885, -2.365]),
            (
                "lamp_head_L",
                [-0.841, 0.745, -2.392, -0.491, 0.865, -2.367],
            ),
            ("lamp_head_R", [0.491, 0.745, -2.392, 0.841, 0.865, -2.367]),
            ("bumper_front", [-0.93, 0.341, -2.49, 0.93, 0.62, -2.345]),
            ("bumper_rear", [-0.9, 0.38, 2.61, 0.9, 0.58, 2.769]),
            ("cab", [-0.92, 0.46, -0.925, 0.92, 1.895, 2.585]),
            (
                "glass_screen",
                [-0.845, 1.144, -0.919, 0.845, 1.738, -0.608],
            ),
            (
                "glass_quarter_R",
                [0.804, 1.117, 1.022, 0.923, 1.763, 2.459],
            ),
            ("door_FL", [-0.946, 0.51, -0.885, -0.745, 1.79, 0.086]),
            ("door_FR", [0.745, 0.51, -0.885, 0.946, 1.79, 0.086]),
            ("door_RR", [0.745, 0.51, 0.11, 0.946, 1.79, 0.97]),
            ("door_back", [-0.791, 0.6, 2.357, 0.791, 1.745, 2.604]),
            ("lamp_tail_L", [-0.877, 0.81, 2.577, -0.798, 1.745, 2.603]),
            ("mirror_L", [-1.108, 1.18, -0.913, -0.898, 1.31, -0.851]),
            ("mirror_R", [0.898, 1.18, -0.913, 1.108, 1.31, -0.851]),
            ("chassis", [-0.435, 0.18, -2.245, 0.435, 0.67, 2.545]),
            ("axle_front", [-0.645, 0.25, -1.635, 0.645, 0.569, -0.58]),
            ("axle_rear", [-0.645, 0.25, 0.581, 0.645, 0.569, 1.635]),
            (
                "susp_stock_F",
                [-0.522, 0.335, -2.105, 0.522, 0.816, -0.865],
            ),
            ("susp_stock_R", [-0.522, 0.335, 0.865, 0.522, 0.816, 2.105]),
            ("tyre_at_FL", [-0.923, 0.0, -1.885, -0.637, 0.8, -1.085]),
            ("tyre_at_FR", [0.637, 0.0, -1.885, 0.923, 0.8, -1.085]),
            ("tyre_at_RL", [-0.923, 0.0, 1.085, -0.637, 0.8, 1.885]),
            ("tyre_at_RR", [0.637, 0.0, 1.085, 0.923, 0.8, 1.885]),
            ("rim_mesh_FL", [-0.86, 0.188, -1.697, -0.704, 0.612, -1.273]),
            ("rim_mesh_FR", [0.704, 0.188, -1.697, 0.86, 0.612, -1.273]),
            ("roof_rails", [-0.72, 1.878, -0.31, 0.72, 1.941, 2.225]),
            (
                "acc_bullbar_tube",
                [-0.921, 0.42, -2.573, 0.921, 1.085, -2.259],
            ),
        ];
        let wanted = |name: &str| name != "acc_bullbar_tube" || more.contains(&name);
        boxes
            .iter()
            .filter(|(name, _)| wanted(name))
            .map(|(name, b)| Placed {
                name: (*name).to_owned(),
                low: Vec3::new(b[0], b[1], b[2]),
                high: Vec3::new(b[3], b[4], b[5]),
            })
            .collect()
    }

    /// Driven into a wall at so many kilometres an hour: two tonnes stopped dead, the
    /// whole of its nose against the wall, as the game measures it (its collision box's
    /// front, which is a little behind its bumper).
    fn wall(kmh: f32) -> Hit {
        Hit {
            at: Vec3::new(0.0, 0.92, -2.315),
            push: Vec3::Z,
            low: Vec3::new(-0.92, 0.56, -2.315),
            high: Vec3::new(0.92, 1.28, -2.315),
            force: 2010.0 * kmh / 3.6 / 0.1 / 1000.0,
            wheel: None,
        }
    }

    /// A post met with the right-hand end of the nose.
    fn post(kmh: f32) -> Hit {
        Hit {
            at: Vec3::new(0.76, 0.92, -2.315),
            low: Vec3::new(0.6, 0.56, -2.315),
            high: Vec3::new(0.92, 1.28, -2.315),
            ..wall(kmh)
        }
    }

    fn stages(damage: &Damage, table: &Table, pieces: &[&str]) -> Vec<Stage> {
        pieces
            .iter()
            .map(|piece| damage.stage(table, piece))
            .collect()
    }

    #[test]
    fn the_games_table_reads_and_names_every_kind_of_piece_a_truck_has() {
        let table = Table::built_in();
        // The longest name that fits is the kind.
        assert_eq!(table.part("acc_bullbar_plate").named, "acc_bullbar_plate");
        assert_eq!(table.part("lamp_head_L").named, "lamp");
        assert_eq!(table.part("door_back").named, "door");
        assert_eq!(table.part("glass_quarter_L").mount, Mount::Glass);
        assert_eq!(table.part("tyre_mud_RR").mount, Mount::Wheel);
        assert_eq!(table.part("rim_beadlock_FL").does, Does::Running);
        assert_eq!(table.part("susp_lift_F").mount, Mount::Frame);
        assert_eq!(table.part("roof_rails").named, "roof");
        // Anything it has no line for is as an ordinary bolted panel.
        assert_eq!(table.part("towbar").named, "*");
        // A stage takes more than the one before it, and what never happens never does.
        for part in &table.parts {
            assert!(
                part.at.windows(2).all(|pair| pair[0] <= pair[1]),
                "{}",
                part.named
            );
        }
        assert_eq!(table.part("cab").stage(10_000.0), Stage::Dented);
        assert_eq!(table.part("tyre_at_FL").stage(10_000.0), Stage::Loose);
        let bumper = table.part("bumper_front");
        let told: Vec<Stage> = [0.0, 19.9, 20.0, 60.0, 130.0, 210.0]
            .iter()
            .map(|hurt| bumper.stage(*hurt))
            .collect();
        assert_eq!(
            told,
            [
                Stage::Sound,
                Stage::Sound,
                Stage::Scuffed,
                Stage::Dented,
                Stage::Loose,
                Stage::Off
            ]
        );
        // Mistakes in a table are said, with their line.
        assert!(Table::parse("part * 1 2 3 4 0 bolted -\n").is_ok());
        assert!(Table::parse("part cab 1 2 3 4 0 bolted -\n")
            .unwrap_err()
            .contains("part *"));
        let wrong = Table::parse("part * 1 2 3 4 0 glued -\n").unwrap_err();
        assert!(
            wrong.contains("line 1") && wrong.contains("glued"),
            "{wrong}"
        );
        assert!(Table::parse("part * 5 2 3 4 0 bolted -\n").is_err());
        assert!(Table::parse("set wear 1 2\npart * 1 2 3 4 0 bolted -\n").is_err());
    }

    #[test]
    fn a_tap_does_nothing_and_force_decides_the_stage_not_the_count() {
        let table = Table::built_in();
        let truck = truck(&[]);
        // Walking pace into the wall, a hundred times: not a mark on it.
        let mut damage = Damage::default();
        for _ in 0..100 {
            assert!(damage.strike(&table, &truck, &wall(3.0)).is_empty());
        }
        assert_eq!(damage, Damage::default());
        // Once at each speed: the faster, the further gone its bumper is.
        let bumper = |kmh: f32| {
            let mut damage = Damage::default();
            damage.strike(&table, &truck, &wall(kmh));
            damage.stage(&table, "bumper_front")
        };
        assert_eq!(bumper(5.0), Stage::Scuffed);
        assert_eq!(bumper(12.0), Stage::Dented);
        assert_eq!(bumper(25.0), Stage::Loose);
        assert_eq!(bumper(40.0), Stage::Off);
        // And what is hit is the front: nothing behind the cab feels a wall at any speed.
        let mut damage = Damage::default();
        let changes = damage.strike(&table, &truck, &wall(80.0));
        assert!(changes.iter().any(|change| change.piece == "nose"));
        let behind = [
            "door_back",
            "bumper_rear",
            "lamp_tail_L",
            "axle_rear",
            "tyre_at_RR",
        ];
        assert_eq!(stages(&damage, &table, &behind), [Stage::Sound; 5]);
    }

    #[test]
    fn what_is_in_front_takes_the_blow_first_and_spares_what_is_behind() {
        let table = Table::built_in();
        let lamps = ["lamp_head_L", "lamp_head_R"];
        // Twenty kilometres an hour into a wall, as it is sold: the bumper is dented, and
        // what gets past it puts the lamps out.
        let mut bare = Damage::default();
        bare.strike(&table, &truck(&[]), &wall(20.0));
        assert_eq!(bare.stage(&table, "bumper_front"), Stage::Dented);
        assert!(
            lamps.iter().all(|lamp| bare.is_out(&table, lamp)),
            "{bare:?}"
        );
        // With a bull bar on it the bar takes it, and the lamps still light.
        let barred = truck(&["acc_bullbar_tube"]);
        let mut damage = Damage::default();
        damage.strike(&table, &barred, &wall(20.0));
        assert!(damage.stage(&table, "acc_bullbar_tube") >= Stage::Scuffed);
        assert!(
            lamps.iter().all(|lamp| !damage.is_out(&table, lamp)),
            "{damage:?}"
        );
        assert!(damage.stage(&table, "bumper_front") < Stage::Dented);
        // Each piece feels no more than the one in front of it left, in order of depth.
        let felt = damage_felt(&table, &barred, &wall(40.0));
        let of = |name: &str| {
            felt.iter()
                .find(|(piece, _)| piece == name)
                .map_or(0.0, |f| f.1)
        };
        assert!((of("acc_bullbar_tube") - wall(40.0).force).abs() < 0.01);
        assert!(of("bumper_front") < of("acc_bullbar_tube") - 100.0);
        assert!(of("nose") < of("bumper_front") && of("chassis") < of("nose"));
        assert_eq!(of("cab"), 0.0);
        // A bar that is already hanging off soaks up less of the next blow.
        let mut wrecked = Damage::default();
        wrecked.strike(&table, &barred, &wall(45.0));
        assert_eq!(wrecked.stage(&table, "acc_bullbar_tube"), Stage::Loose);
        let after = wrecked.felt(&table, &barred, &wall(20.0));
        let sound = Damage::default().felt(&table, &barred, &wall(20.0));
        let bumper = barred
            .iter()
            .position(|piece| piece.name == "bumper_front")
            .unwrap();
        let on = |felt: &[(usize, f32)]| felt.iter().find(|f| f.0 == bumper).map_or(0.0, |f| f.1);
        assert!(on(&after) > on(&sound) + 50.0);
    }

    fn damage_felt(table: &Table, placed: &[Placed], hit: &Hit) -> Vec<(String, f32)> {
        let felt = Damage::default().felt(table, placed, hit);
        felt.into_iter()
            .map(|(index, felt)| (placed[index].name.clone(), felt))
            .collect()
    }

    #[test]
    fn a_blow_a_little_off_square_is_felt_the_same_on_both_sides() {
        let table = Table::built_in();
        let truck = truck(&[]);
        // Into the wall at thirty, pushed back and a little to one side, as a truck that
        // is not quite straight is: its two head lamps and their like fare alike.
        for sideways in [-0.08, 0.0, 0.08] {
            let hit = Hit {
                push: Vec3::new(sideways, 0.0, 1.0).normalize(),
                ..wall(30.0)
            };
            let felt = damage_felt(&table, &truck, &hit);
            let of = |name: &str| felt.iter().find(|f| f.0 == name).map_or(0.0, |f| f.1);
            assert!(of("lamp_head_L") > 100.0, "{felt:?}");
            assert!(
                (of("lamp_head_L") - of("lamp_head_R")).abs() < 2.0,
                "{felt:?}"
            );
            // And the layer they are in feels what the bumper in front of them left.
            assert!((of("grille") - of("lamp_head_L")).abs() < 2.0, "{felt:?}");
            assert!(of("bumper_front") > of("grille") + 30.0);
        }
    }

    #[test]
    fn a_blow_lands_where_it_landed_and_the_way_it_pushed() {
        let table = Table::built_in();
        let truck = truck(&[]);
        // A post met with the right of the nose at thirty: the right lamp is gone and the
        // left is untouched, though the same speed into a wall takes both.
        let mut damage = Damage::default();
        damage.strike(&table, &truck, &post(30.0));
        assert!(
            damage.stage(&table, "lamp_head_R") >= Stage::Loose,
            "{damage:?}"
        );
        assert_eq!(damage.stage(&table, "lamp_head_L"), Stage::Sound);
        assert_eq!(damage.stage(&table, "mirror_L"), Stage::Sound);
        let mut square = Damage::default();
        square.strike(&table, &truck, &wall(30.0));
        assert!(square.stage(&table, "lamp_head_L") >= Stage::Loose);
        // Its bumper is bent the way it was pushed, from where it was hit.
        let bumper = damage.hurt("bumper_front").unwrap();
        assert_eq!((bumper.at, bumper.push), (post(30.0).at, Vec3::Z));

        // The same force into its right side, at the front door: the mirror that stands
        // out goes first, then the door; its nose and its other side feel nothing.
        let side = Hit {
            at: Vec3::new(0.92, 1.0, -0.4),
            push: Vec3::NEG_X,
            low: Vec3::new(0.92, 0.56, -0.7),
            high: Vec3::new(0.92, 1.86, -0.3),
            force: 110.0,
            wheel: None,
        };
        let mut damage = Damage::default();
        damage.strike(&table, &truck, &side);
        assert_eq!(damage.stage(&table, "mirror_R"), Stage::Off);
        assert_eq!(damage.stage(&table, "door_FR"), Stage::Dented);
        let spared = [
            "door_FL",
            "mirror_L",
            "bumper_front",
            "lamp_head_R",
            "door_back",
        ];
        assert_eq!(stages(&damage, &table, &spared), [Stage::Sound; 5]);
        // The door behind it is clear of the patch, and of the little way beyond it that
        // a blow this hard reaches.
        assert_eq!(damage.stage(&table, "door_RR"), Stage::Sound);

        // Rolled onto its roof: the rails, then the cab; and the glass is shaken out of
        // it by a hard landing, and not by a soft one.
        let roof = |force: f32| Hit {
            at: Vec3::new(0.0, 1.86, 0.5),
            push: Vec3::NEG_Y,
            low: Vec3::new(-0.84, 1.86, -0.9),
            high: Vec3::new(0.84, 1.86, 2.5),
            force,
            wheel: None,
        };
        let mut damage = Damage::default();
        damage.strike(&table, &truck, &roof(45.0));
        assert_eq!(damage.stage(&table, "roof_rails"), Stage::Dented);
        assert_eq!(damage.stage(&table, "cab"), Stage::Scuffed);
        assert_eq!(damage.stage(&table, "glass_screen"), Stage::Dented);
        damage.strike(&table, &truck, &roof(120.0));
        assert_eq!(damage.stage(&table, "cab"), Stage::Dented);
        assert_eq!(damage.stage(&table, "glass_screen"), Stage::Off);
        assert_eq!(
            stages(&damage, &table, &["bumper_front", "tyre_at_FL"]),
            [Stage::Sound; 2]
        );
    }

    #[test]
    fn blows_add_up_but_only_those_hard_enough_to_mark_it() {
        let table = Table::built_in();
        let truck = truck(&[]);
        let mut damage = Damage::default();
        // Fifteen kilometres an hour dents the bumper: 84 kN, and it comes loose at 130.
        damage.strike(&table, &truck, &wall(15.0));
        assert_eq!(damage.stage(&table, "bumper_front"), Stage::Dented);
        let first = damage.hurt("bumper_front").unwrap().hurt;
        assert!((first - wall(15.0).force).abs() < 0.01);
        // Each more like it adds a fifth of itself: the fourth has it hanging.
        damage.strike(&table, &truck, &wall(15.0));
        let second = damage.hurt("bumper_front").unwrap().hurt;
        assert!((second - first * 1.2).abs() < 0.01);
        damage.strike(&table, &truck, &wall(15.0));
        assert_eq!(damage.stage(&table, "bumper_front"), Stage::Dented);
        let changes = damage.strike(&table, &truck, &wall(15.0));
        let bumper = changes
            .iter()
            .find(|change| change.piece == "bumper_front")
            .unwrap();
        assert_eq!((bumper.from, bumper.to), (Stage::Dented, Stage::Loose));
        // A harder one counts whole, and what it had before as the fifth.
        let mut damage = Damage::default();
        damage.strike(&table, &truck, &wall(6.0));
        let light = damage.hurt("bumper_front").unwrap().hurt;
        damage.strike(&table, &truck, &wall(15.0));
        let both = damage.hurt("bumper_front").unwrap().hurt;
        assert!((both - (wall(15.0).force + 0.2 * light)).abs() < 0.01);
        // Taps that would not mark it add nothing to it, a thousand of them.
        for _ in 0..1000 {
            damage.strike(&table, &truck, &wall(3.0));
        }
        assert_eq!(damage.hurt("bumper_front").unwrap().hurt, both);
    }

    #[test]
    fn a_landing_goes_up_through_its_wheel_and_wrecked_running_gear_pulls() {
        let table = Table::built_in();
        let truck = truck(&[]);
        let landing = |wheel: usize, force: f32| Hit {
            at: Vec3::new(if wheel % 2 == 0 { -0.78 } else { 0.78 }, 0.0, -1.485),
            push: Vec3::Y,
            low: Vec3::ZERO,
            high: Vec3::ZERO,
            force,
            wheel: Some(wheel),
        };
        // A drop of a metre on the front left: its tyre is marked, and nothing else.
        let mut damage = Damage::default();
        damage.strike(&table, &truck, &landing(0, 34.0));
        assert_eq!(damage.stage(&table, "tyre_at_FL"), Stage::Scuffed);
        assert_eq!(damage.hurts.len(), 2, "its tyre and its rim: {damage:?}");
        assert_eq!(damage.handicap(&table), Handicap::default());
        // Off a ledge: the tyre is wrecked, the rim buckled, the springs bent; and it is
        // that corner's, not the body over it nor the other wheels'.
        damage.strike(&table, &truck, &landing(0, 90.0));
        assert_eq!(damage.stage(&table, "tyre_at_FL"), Stage::Loose);
        assert_eq!(damage.stage(&table, "rim_mesh_FL"), Stage::Loose);
        assert_eq!(damage.stage(&table, "susp_stock_F"), Stage::Dented);
        let spared = [
            "tyre_at_FR",
            "tyre_at_RL",
            "rim_mesh_FR",
            "susp_stock_R",
            "nose",
            "cab",
        ];
        assert_eq!(stages(&damage, &table, &spared), [Stage::Sound; 6]);
        // It pulls to the left: the wrecked tyre and rim, and the bent springs.
        let handicap = damage.handicap(&table);
        let pull = -(2.0 * table.pull[1] + table.pull[0]);
        assert!((handicap.pull - pull).abs() < 1.0e-5, "{handicap:?}");
        assert_eq!(handicap.power, 1.0);
        // The same at the back steers nothing.
        let mut rear = Damage::default();
        rear.strike(&table, &truck, &landing(3, 90.0));
        assert_eq!(rear.stage(&table, "tyre_at_RR"), Stage::Loose);
        assert_eq!(rear.handicap(&table), Handicap::default());
        // Mended, it runs straight again.
        assert!(damage.mend("tyre_at_FL") && damage.mend("rim_mesh_FL"));
        assert!(damage.mend("susp_stock_F") && !damage.mend("susp_stock_F"));
        assert_eq!(damage.handicap(&table), Handicap::default());
    }

    #[test]
    fn a_smashed_nose_costs_power_and_lamps_go_out() {
        let table = Table::built_in();
        let truck = truck(&[]);
        let power = |kmh: f32| {
            let mut damage = Damage::default();
            damage.strike(&table, &truck, &wall(kmh));
            (damage.handicap(&table), damage)
        };
        assert_eq!(power(10.0).0, Handicap::default());
        // The grille dented, then hanging, then gone: and the radiator with it.
        let (dented, damage) = power(20.0);
        assert_eq!(damage.stage(&table, "grille"), Stage::Dented);
        assert_eq!(dented.power, table.power[0]);
        assert_eq!(power(27.0).0.power, table.power[1]);
        let (gone, damage) = power(45.0);
        assert_eq!(damage.stage(&table, "grille"), Stage::Off);
        assert_eq!((gone.power, gone.pull), (table.power[2], 0.0));
        // Its head lamps are out; its tail lamps are not.
        assert!(damage.is_out(&table, "lamp_head_L") && !damage.is_out(&table, "lamp_tail_L"));
        // A bumper is not a lamp, however wrecked.
        assert!(!damage.is_out(&table, "bumper_front"));
    }

    #[test]
    fn damage_is_plain_text_and_reads_back() {
        let table = Table::built_in();
        let truck = truck(&[]);
        let mut damage = Damage::default();
        damage.strike(&table, &truck, &post(35.0));
        let text = damage.to_text();
        assert!(
            text.lines().count() >= 4 && text.contains("bumper_front "),
            "{text}"
        );
        let read = Damage::parse(&text).unwrap();
        assert_eq!(read.list(&table), damage.list(&table));
        assert_eq!(read.to_text(), text);
        assert_eq!(Damage::parse("").unwrap(), Damage::default());
        assert!(Damage::parse("bumper_front 12 0 0")
            .unwrap_err()
            .contains("line 1"));
        // What a shop would list, and mend.
        let listed = damage.list(&table);
        assert!(listed.contains(&("lamp_head_R", Stage::Off)), "{listed:?}");
        damage.mend_all();
        assert!(damage.list(&table).is_empty());

        // And as words say it, for a picture: each piece just far enough gone to be so.
        let staged = Damage::staged(
            &table,
            &truck,
            "bumper_front=loose, door_FR=dented,mirror_L=off",
        )
        .unwrap();
        assert_eq!(
            stages(&staged, &table, &["bumper_front", "door_FR", "mirror_L"]),
            [Stage::Loose, Stage::Dented, Stage::Off]
        );
        // Pushed in from outside: the bumper back, the right door to the left.
        assert_eq!(staged.hurt("bumper_front").unwrap().push, Vec3::Z);
        assert_eq!(staged.hurt("door_FR").unwrap().push, Vec3::NEG_X);
        assert!(Damage::staged(&table, &truck, "towbar=dented").is_err());
        assert!(Damage::staged(&table, &truck, "cab=off")
            .unwrap_err()
            .contains("never"));
        assert!(Damage::staged(&table, &truck, "cab=bent").is_err());
        let hit = Hit::parse("0.8,0.9,-2.3, 0,0,1, 140").unwrap();
        assert_eq!((hit.at.x, hit.push, hit.force), (0.8, Vec3::Z, 140.0));
        assert!(Hit::parse("1,2,3").is_err());
    }

    #[test]
    fn a_hurt_piece_is_pushed_in_the_way_it_was_hit_and_hangs_when_loose() {
        let table = Table::built_in();
        let truck = truck(&[]);
        let bounds = |name: &str| {
            let piece = truck.iter().find(|piece| piece.name == name).unwrap();
            (piece.low, piece.high)
        };
        let middle = |name: &str| (bounds(name).0 + bounds(name).1) * 0.5;
        let hurt = |name: &str, stage: Stage, at: Vec3, push: Vec3| Hurt {
            piece: name.to_owned(),
            hurt: table.part(name).at[stage as usize - 1] * 1.01,
            at,
            push,
        };
        let posed = |hurt: &Hurt| {
            let part = table.part(&hurt.piece);
            let bounds = bounds(&hurt.piece);
            let hang = hang(part, hurt, bounds, None);
            let swing = hang.as_ref().map(|hang| (hang, hang.rest));
            pose(&table, part, hurt, bounds, swing)
        };
        let right_end = Vec3::new(0.8, 0.5, -2.49);

        // Scuffed, it has hardly moved; dented, it has gone back, and further the harder.
        let name = "bumper_front";
        let scuffed = posed(&hurt(name, Stage::Scuffed, right_end, Vec3::Z));
        assert!(
            scuffed
                .transform_point3(middle(name))
                .distance(middle(name))
                < 0.02
        );
        let dented = posed(&hurt(name, Stage::Dented, right_end, Vec3::Z));
        let moved = dented.transform_point3(middle(name)) - middle(name);
        assert!(moved.z > 0.015 && moved.z < 0.12, "{moved}");
        let mut harder = hurt(name, Stage::Dented, right_end, Vec3::Z);
        harder.hurt = 125.0;
        let further = posed(&harder).transform_point3(middle(name)) - middle(name);
        assert!(further.z > moved.z + 0.02, "{further} after {moved}");
        // The end that was hit has gone in further than the other.
        let ends = [Vec3::new(0.93, 0.48, -2.49), Vec3::new(-0.93, 0.48, -2.49)];
        let gone: Vec<f32> = ends
            .iter()
            .map(|end| (posed(&harder).transform_point3(*end) - *end).z)
            .collect();
        assert!(gone[0] > gone[1] + 0.03, "{gone:?}");

        // Loose, the end that was hit hangs down, and the other is still up by its bolts.
        let loose = posed(&hurt(name, Stage::Loose, right_end, Vec3::Z));
        let drop: Vec<f32> = ends
            .iter()
            .map(|end| (loose.transform_point3(*end) - *end).y)
            .collect();
        assert!(drop[0] < -0.1 && drop[1] > -0.05, "{drop:?}");
        // And not through the ground.
        assert!(loose.transform_point3(Vec3::new(0.93, 0.341, -2.4)).y > 0.05);

        // A mirror is folded back along the truck; the cab crumples and does not move off.
        let folded = posed(&hurt("mirror_R", Stage::Loose, middle("mirror_R"), Vec3::Z));
        let tip = Vec3::new(1.108, 1.25, -0.88);
        let now = folded.transform_point3(tip);
        assert!(now.z > tip.z + 0.1 && now.x < tip.x - 0.05, "{now}");
        let roof = hurt("cab", Stage::Dented, Vec3::new(0.0, 1.9, 0.5), Vec3::NEG_Y);
        let crushed = posed(&roof);
        let top = crushed.transform_point3(Vec3::new(0.0, 1.895, 0.5));
        let sill = crushed.transform_point3(Vec3::new(0.0, 0.46, 0.5));
        assert!(top.y < 1.895 - 0.02 && sill.distance(Vec3::new(0.0, 0.46, 0.5)) < 1.0e-4);
        // A buckled wheel is tipped on its hub, where it is, and glass shows nothing.
        let rim = hurt("rim_mesh_FL", Stage::Loose, Vec3::ZERO, Vec3::Y);
        let buckled = posed(&rim);
        assert!(buckled.transform_point3(Vec3::ZERO).length() < 1.0e-6);
        assert!(buckled.transform_point3(Vec3::Y * 0.2).x.abs() > 0.015);
        let glass = hurt("glass_screen", Stage::Dented, Vec3::ZERO, Vec3::Z);
        assert_eq!(posed(&glass), Mat4::IDENTITY);
    }

    #[test]
    fn a_door_with_no_latch_swings_on_its_hinge_as_the_truck_is_thrown_about() {
        let table = Table::built_in();
        let part = table.part("door_FR");
        let bounds = (
            Vec3::new(0.745, 0.51, -0.885),
            Vec3::new(0.946, 1.79, 0.086),
        );
        let hurt = Hurt {
            piece: "door_FR".to_owned(),
            hurt: 140.0,
            at: Vec3::new(0.95, 1.0, -0.4),
            push: Vec3::NEG_X,
        };
        // The kit's hinge for it: at its front edge, upright, opening seventy degrees.
        let hinge = (Vec3::new(0.92, 0.0, -0.885), true, 70.0);
        let hang = hang(part, &hurt, bounds, Some(hinge)).unwrap();
        assert_eq!((hang.pivot, hang.axis), (hinge.0, Vec3::Y));
        let mut swing = Swing::at_rest(&hang);
        // Standing, it hangs ajar and stays there.
        for _ in 0..120 {
            swing.advance(&hang, Vec3::ZERO, 0.0, 1.0 / 60.0);
        }
        assert!((swing.angle - hang.rest).abs() < 1.0e-3 && swing.angle > 0.1);
        // The truck turns hard to its left: the right door is thrown out, to its stop at most.
        for _ in 0..40 {
            swing.advance(&hang, Vec3::new(-7.0, 0.0, 0.0), 12.0, 1.0 / 60.0);
        }
        let thrown = swing.angle;
        assert!(thrown > hang.rest + 0.2 && thrown <= hang.high, "{thrown}");
        // Turning the other way it is thrown shut, all but: its latch has gone.
        for _ in 0..60 {
            swing.advance(&hang, Vec3::new(7.0, 0.0, 0.0), 12.0, 1.0 / 60.0);
        }
        assert!(swing.angle < hang.rest && swing.angle >= hang.low);
        // The door's far edge is out from the truck when it is open, about its hinge.
        let open = pose(&table, part, &hurt, bounds, Some((&hang, thrown)));
        let edge = Vec3::new(0.946, 1.0, 0.086);
        assert!(open.transform_point3(edge).x > edge.x + 0.2);
        // A bonnet with no catch lifts as the truck gathers speed.
        let bonnet = (
            Vec3::new(-0.819, 1.033, -2.275),
            Vec3::new(0.819, 1.152, -0.955),
        );
        let lid = Hurt {
            piece: "bonnet".to_owned(),
            hurt: 120.0,
            at: Vec3::new(0.0, 1.1, -2.2),
            push: Vec3::Z,
        };
        let hang = super::hang(table.part("bonnet"), &lid, bonnet, None).unwrap();
        let mut swing = Swing::at_rest(&hang);
        for _ in 0..240 {
            swing.advance(&hang, Vec3::ZERO, 25.0, 1.0 / 60.0);
        }
        assert!(swing.angle > 0.5, "{}", swing.angle);
        let lifted = pose(
            &table,
            table.part("bonnet"),
            &lid,
            bonnet,
            Some((&hang, swing.angle)),
        );
        let front = Vec3::new(0.0, 1.1, -2.275);
        assert!(lifted.transform_point3(front).y > front.y + 0.4);
    }

    #[test]
    fn what_is_torn_off_falls_and_lies_where_it_came_off() {
        // A bumper comes off a truck doing ten metres a second along -z, on flat ground.
        let bounds = (
            Vec3::new(-0.93, 0.341, -2.49),
            Vec3::new(0.93, 0.62, -2.345),
        );
        let from = Mat4::from_translation(Vec3::new(40.0, 0.0, -100.0));
        let mut fallen = Fallen::new("bumper_front", from, bounds, Vec3::NEG_Z * 10.0, Vec3::Z);
        // As it comes off it is where it was on the truck.
        let middle = (bounds.0 + bounds.1) * 0.5;
        let was = from.transform_point3(middle);
        assert!(fallen.placed().transform_point3(middle).distance(was) < 1.0e-4);
        let mut frames = 0;
        while fallen.advance(1.0 / 60.0, |_| 0.0) {
            frames += 1;
            assert!(frames < 600, "it comes to rest");
            assert!(fallen.lowest() > -0.01, "and never goes under the ground");
        }
        assert!(fallen.lies && frames > 20);
        // It lies on the ground a little way on from where it came off, not where the
        // truck has gone to since.
        let lies = fallen.placed().transform_point3(middle);
        assert!(lies.y < 0.35 && fallen.lowest() < 0.03, "{lies}");
        assert!(
            lies.z < was.z - 0.5 && lies.z > was.z - 8.0,
            "{lies} from {was}"
        );
        // And there it stays.
        let placed = fallen.placed();
        assert!(!fallen.advance(1.0, |_| 0.0));
        assert_eq!(fallen.placed(), placed);
    }
}
