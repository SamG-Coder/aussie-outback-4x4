//! Timed runs: the classes a track is run in, the bronze, silver and gold times of each, who
//! holds them, and what a run pays (docs/campaign.md, Timed runs).
//!
//! Nothing here is typed in for a track. A class is how a grade of truck drives; a track's
//! times are worked out from the track for each class when the map is entered
//! ([`Course::read`], `solstice_game::par`); the three other drivers of a class are drawn
//! from the map's name; and a purse is so much for every second of the gold time. This file
//! is the rules, which need no game to try; the shell (`shell.rs`) runs them.

use serde_json::{json, Value};
use solstice_asset::GameFile;
use solstice_game::drive::Pace;
use solstice_game::par::{Going, Profile};
use solstice_game::{SurfaceMap, Track};
use std::collections::BTreeMap;

/// A class: a grade of truck, how a good driver in one gets round a track, and how much more
/// than the first class its purses are.
pub struct Class {
    pub name: &'static str,
    pub going: Going,
    pub pays: f32,
    /// The trucks of the grade, by what they are called (`tools/blender/truck/build.py`).
    pub trucks: [&'static str; 10],
}

const fn going(cornering: f32, braking: f32, pull: f32, top_speed: f32, crawl: f32) -> Going {
    Going {
        pace: Pace {
            cornering,
            braking,
            top_speed,
            looks: Pace::TOURING.looks,
        },
        pull,
        crawl,
    }
}

/// The classes, from the paddock bashers up. To be tuned by playing: one number here moves
/// every medal of the class on every map.
pub const CLASSES: [Class; 5] = [
    Class {
        name: "Paddock bashers",
        going: going(2.6, 3.2, 1.2, 13.0, 1.0),
        pays: 1.0,
        trucks: [
            "Dargo",
            "Dargo Styleside",
            "Omeo",
            "Omeo Tray",
            "Licola",
            "Licola Hardtop",
            "Tambo",
            "Buckland",
            "Bendoc",
            "Wonnangatta",
        ],
    },
    Class {
        name: "Old tourers",
        going: going(3.0, 3.6, 1.5, 15.0, 1.1),
        pays: 1.5,
        trucks: [
            "Howitt",
            "Howitt Ute",
            "Howitt Shorty",
            "Jamieson",
            "Jamieson Troop",
            "Mitta",
            "Mitta Shorty",
            "Delatite",
            "Delatite Tray",
            "Feathertop",
        ],
    },
    Class {
        name: "Workhorses",
        going: going(3.4, 4.2, 1.9, 18.0, 1.25),
        pays: 2.5,
        trucks: [
            "Ovens",
            "Ovens Tray",
            "Ovens Extra",
            "Kiewa",
            "Buffalo",
            "Buffalo Tray",
            "Hotham",
            "Tumut",
            "Walhalla",
            "Ovens Cab-chassis",
        ],
    },
    Class {
        name: "Built tourers",
        going: going(3.9, 4.8, 2.3, 21.0, 1.4),
        pays: 4.0,
        trucks: [
            "Jamieson V8",
            "Jamieson V8 Single",
            "Jamieson Troop V8",
            "Bogong Sport",
            "Bogong Wagon",
            "Ovens Tourer",
            "Feathertop GX",
            "Howitt Y",
            "Buffalo Chase",
            "Tumut Trail",
        ],
    },
    Class {
        name: "The top end",
        going: going(4.5, 5.5, 2.8, 25.0, 1.6),
        pays: 6.0,
        trucks: [
            "Kestrel",
            "Kestrel XL",
            "Kestrel Tray",
            "Cobbler",
            "Cobbler Ute",
            "Stirling",
            "Stirling 90",
            "Feathertop Sahara",
            "Wellington",
            "Kosciuszko",
        ],
    },
];

/// A medal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Medal {
    Bronze = 1,
    Silver = 2,
    Gold = 3,
}

impl Medal {
    /// From the slowest to win to the fastest.
    pub const ALL: [Self; 3] = [Self::Bronze, Self::Silver, Self::Gold];

    pub fn name(self) -> &'static str {
        match self {
            Self::Bronze => "bronze",
            Self::Silver => "silver",
            Self::Gold => "gold",
        }
    }

    fn from_level(level: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|medal| *medal as u8 == level)
    }

    /// How much slower than the gold time its time is.
    pub fn slower(self) -> f32 {
        match self {
            Self::Bronze => 1.3,
            Self::Silver => 1.12,
            Self::Gold => 1.0,
        }
    }

    /// Its time, for a gold time, in seconds: to a tenth, so that what is shown is what is
    /// to be beaten.
    pub fn time(self, gold: f32) -> f32 {
        (gold * self.slower() * 10.0).round() / 10.0
    }

    /// The best medal a time wins, against a gold time.
    pub fn won(seconds: f64, gold: f32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .rev()
            .find(|medal| seconds <= f64::from(medal.time(gold)))
    }

    /// Its purse, in dollars, for a class and a gold time: gold's is a dollar for every
    /// second of the gold time, times what the class pays; silver's half of it, and
    /// bronze's a quarter. Each to the nearest ten dollars.
    pub fn purse(self, gold: f32, class: usize) -> u64 {
        let tens = |dollars: f32| (dollars / 10.0).round().max(1.0) as u64 * 10;
        let top = tens(A_SECOND * gold * CLASSES[class.min(CLASSES.len() - 1)].pays);
        match self {
            Self::Gold => top,
            Self::Silver => tens(top as f32 / 2.0),
            Self::Bronze => tens(top as f32 / 4.0),
        }
    }
}

/// What a second of a gold time is worth to gold's purse in the first class, in dollars.
pub const A_SECOND: f32 = 1.0;
/// How much more a run pays at night, and in the rain: half as much again, each.
pub const NIGHT_BONUS: f32 = 0.5;
pub const RAIN_BONUS: f32 = 0.5;
/// What share of a medal's purse a run pays that wins nothing new.
pub const REPEAT: f32 = 0.1;

/// What a run paid, and why.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paid {
    /// Each medal won for the first time, and its purse.
    pub firsts: Vec<(Medal, u64)>,
    /// What a run that won nothing new was paid for the medal it reached.
    pub repeat: u64,
    /// What was added for the night, and for the rain.
    pub night: u64,
    pub rain: u64,
    /// All of it.
    pub dollars: u64,
}

/// What a run pays: `held` is the best medal there was before it, `won` the one it reached.
/// A medal's purse is paid in full the first time it is won, and a first win of a better
/// medal pays every medal under it not yet won; a run that wins nothing new pays a tenth of
/// its medal's purse; and the night and the rain each add half of whatever that comes to.
pub fn payout(
    gold: f32,
    class: usize,
    held: Option<Medal>,
    won: Option<Medal>,
    night: bool,
    rain: bool,
) -> Paid {
    let mut paid = Paid::default();
    let Some(won) = won else {
        return paid;
    };
    paid.firsts = Medal::ALL
        .into_iter()
        .filter(|medal| *medal <= won && held.is_none_or(|held| *medal > held))
        .map(|medal| (medal, medal.purse(gold, class)))
        .collect();
    if paid.firsts.is_empty() {
        paid.repeat = ((won.purse(gold, class) as f32 * REPEAT / 10.0).round() as u64).max(1) * 10;
    }
    let base = paid.firsts.iter().map(|(_, purse)| purse).sum::<u64>() + paid.repeat;
    let share = |bonus: f32| (base as f32 * bonus).round() as u64;
    if night {
        paid.night = share(NIGHT_BONUS);
    }
    if rain {
        paid.rain = share(RAIN_BONUS);
    }
    paid.dollars = base + paid.night + paid.rain;
    paid
}

/// How much gold so many dollars is, in grams, at what a kilogram is bought for: a purse is
/// weighed out.
pub fn grams(dollars: u64, a_kilo: f32) -> u32 {
    (dollars as f32 * 1000.0 / a_kilo.max(1.0)).round() as u32
}

/// One of the other drivers of a class on a track: who they are, what they drive, the
/// colour it is (as a page writes one, `0xRRGGBB`), and the medal whose time is theirs.
#[derive(Clone, Debug, PartialEq)]
pub struct Rival {
    pub name: String,
    pub truck: &'static str,
    pub paint: u32,
    pub medal: Medal,
}

const GIVEN: [&str; 24] = [
    "Davo", "Shaz", "Macca", "Kylie", "Bluey", "Robbo", "Narelle", "Jacko", "Trace", "Deano",
    "Stretch", "Dusty", "Wozza", "Cheryl", "Tommo", "Gaz", "Bec", "Smithy", "Mick", "Jules", "Kev",
    "Donna", "Lofty", "Noelene",
];
const FAMILY: [&str; 16] = [
    "Hargreaves",
    "Pickett",
    "O'Dwyer",
    "Treloar",
    "Nankervis",
    "Baxter",
    "Mahoney",
    "Fitzpatrick",
    "Rowe",
    "Tippett",
    "Carmody",
    "Whitlock",
    "Dunstan",
    "Pascoe",
    "McAlister",
    "Yeo",
];
/// Colours a truck comes in: none of them the red the player's own starts in.
const PAINTS: [u32; 10] = [
    0xe8e2d0, 0x2f5f8a, 0xd9a21b, 0x3d6b3a, 0x8a8f96, 0x1c2a44, 0xc96a1e, 0x6b7a4a, 0x23262b,
    0x9fb8c8,
];

/// A number from a few others, well mixed: the same every time.
fn mixed(seed: u64, which: u64) -> u64 {
    let mut mixed = seed ^ which.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// The three other drivers of a class on a map, bronze's first: always the same three.
pub fn rivals(map: &str, class: usize) -> [Rival; 3] {
    let class = class.min(CLASSES.len() - 1);
    let seed = map.bytes().fold(0xCBF2_9CE4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
    }) ^ (class as u64 + 1).wrapping_mul(0x2545_F491_4F6C_DD1D);
    // Each a third of the way round the lists from the last, so that no two are alike.
    let start = |which: u64, len: usize| (mixed(seed, which) % len as u64) as usize;
    let (given, family, truck, paint) = (
        start(1, GIVEN.len()),
        start(2, FAMILY.len()),
        start(3, 10),
        start(4, PAINTS.len()),
    );
    let rival = |slot: usize, medal: Medal| Rival {
        name: format!(
            "{} {}",
            GIVEN[(given + slot * 8) % GIVEN.len()],
            FAMILY[(family + slot * 5) % FAMILY.len()]
        ),
        truck: CLASSES[class].trucks[(truck + slot * 3) % 10],
        paint: PAINTS[(paint + slot * 3) % PAINTS.len()],
        medal,
    };
    [
        rival(0, Medal::Bronze),
        rival(1, Medal::Silver),
        rival(2, Medal::Gold),
    ]
}

/// A track, as every class drives it.
pub struct Course {
    pub track: Track,
    /// How each class gets round, at its gold time.
    pub profiles: Vec<Profile>,
}

impl Course {
    /// The course a map's gameplay files make: its track (`*.track`), driven over what its
    /// map of the ground says is there (`*.surfaces`). None for a map with no track.
    pub fn read(files: &[GameFile]) -> Option<Self> {
        let named = |ending: &str| files.iter().find(|file| file.name.ends_with(ending));
        let track = Track::parse(&named(".track")?.text).ok()?;
        let ground = named(".surfaces").and_then(|file| SurfaceMap::parse(&file.text).ok());
        // As a car that drives itself is told (`Play::load`): a crawl through water and bog,
        // slowly over rock and mud.
        let caution = ground.as_ref().map(|ground| {
            move |point: glam::Vec3| match ground.at(point).name() {
                "ford" | "bog" => 2.6_f32,
                "rock" => 3.2,
                "mud" => 5.0,
                _ => f32::INFINITY,
            }
        });
        let profiles = CLASSES
            .iter()
            .map(|class| {
                Profile::of(
                    &track,
                    &class.going,
                    caution
                        .as_ref()
                        .map(|caution| caution as &dyn Fn(glam::Vec3) -> f32),
                )
            })
            .collect();
        Some(Self { track, profiles })
    }

    /// Each class's gold time, in seconds, to a tenth.
    pub fn golds(&self) -> [f32; 5] {
        let mut golds = [0.0; 5];
        for (gold, profile) in golds.iter_mut().zip(&self.profiles) {
            *gold = (profile.seconds() * 10.0).round() / 10.0;
        }
        golds
    }

    /// How a run is going, `seconds` into it and `along` metres round: how far behind each
    /// medal's time it is here (ahead, below zero), bronze's first; the best medal it is
    /// still inside; and where each of the other drivers would be by now.
    pub fn pace(&self, class: usize, seconds: f64, along: f32) -> Value {
        let profile = &self.profiles[class.min(self.profiles.len() - 1)];
        let here = f64::from(profile.time_at(along));
        let gaps: Vec<f64> = Medal::ALL
            .iter()
            .map(|medal| ((seconds - here * f64::from(medal.slower())) * 10.0).round() / 10.0)
            .collect();
        let on = Medal::ALL
            .iter()
            .zip(&gaps)
            .rev()
            .find(|(_, gap)| **gap <= 0.0)
            .map(|(medal, _)| medal.name());
        let ghosts: Vec<Value> = Medal::ALL
            .iter()
            .map(|medal| {
                let at = profile.along_at(seconds as f32 / medal.slower());
                let (place, _, _) = self.track.place(at);
                json!({ "medal": medal.name(), "x": place.x.round(), "z": place.z.round(),
                    "done": at >= profile.length() })
            })
            .collect();
        json!({ "gaps": gaps, "on": on, "ghosts": ghosts })
    }
}

/// What has been won on one class of one map: the best medal, and the best time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    pub medal: Option<Medal>,
    pub best: Option<f64>,
}

/// What has been won, on every class of every map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Records(BTreeMap<(String, usize), Record>);

impl Records {
    /// From what was kept of a game: `run MAP CLASS MEDAL SECONDS` a line, the class from 1,
    /// the medal from 0, none, to 3, gold, and the seconds `-` if no run has counted.
    pub fn read(kept: &str) -> Self {
        let mut records = Self::default();
        for line in kept.lines() {
            if let ["run", map, class, medal, best] =
                line.split_whitespace().collect::<Vec<_>>()[..]
            {
                let (Ok(class), Ok(medal)) = (class.parse::<usize>(), medal.parse::<u8>()) else {
                    continue;
                };
                if (1..=CLASSES.len()).contains(&class) {
                    records.0.insert(
                        (map.to_owned(), class - 1),
                        Record {
                            medal: Medal::from_level(medal),
                            best: best.parse().ok(),
                        },
                    );
                }
            }
        }
        records
    }

    /// Adds them to what is kept of the game.
    pub fn keep(&self, kept: &mut String) {
        for ((map, class), record) in &self.0 {
            let best = record
                .best
                .map_or("-".to_owned(), |best| format!("{best:.3}"));
            *kept += &format!(
                "run {map} {} {} {best}\n",
                class + 1,
                record.medal.map_or(0, |medal| medal as u8)
            );
        }
    }

    pub fn of(&self, map: &str, class: usize) -> Record {
        self.0
            .get(&(map.to_owned(), class))
            .copied()
            .unwrap_or_default()
    }

    /// Takes in a run that counted: its time, and the medal it won. Says what there was
    /// before it.
    pub fn take(&mut self, map: &str, class: usize, seconds: f64, won: Option<Medal>) -> Record {
        let record = self.0.entry((map.to_owned(), class)).or_default();
        let before = *record;
        record.medal = record.medal.max(won);
        if record.best.is_none_or(|best| seconds < best) {
            record.best = Some(seconds);
        }
        before
    }
}

/// How a timed run ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub class: usize,
    pub seconds: f64,
    /// How many times the truck was put back on the track, and what of `seconds` that is.
    pub resets: u32,
    pub penalty: f64,
    /// Whether it counted.
    pub valid: bool,
    pub won: Option<Medal>,
    /// What there was before it.
    pub before: Record,
    pub paid: Paid,
    /// The purse as it was paid: so many grams of gold in the tray, or, with no tray, money.
    pub grams: Option<u32>,
    pub gold: f32,
    pub night: bool,
    pub rain: bool,
}

impl Outcome {
    /// As the pages are told of it.
    pub fn told(&self) -> Value {
        json!({
            "class": self.class,
            "time": self.seconds,
            "resets": self.resets,
            "penalty": self.penalty,
            "valid": self.valid,
            "medal": self.won.map(Medal::name),
            "held": self.before.medal.map(Medal::name),
            "best": self.before.best,
            "record": self.valid && self.before.best.is_none_or(|best| self.seconds < best),
            "medals": Medal::ALL.map(|medal| medal.time(self.gold)),
            "firsts": self.paid.firsts.iter()
                .map(|(medal, purse)| json!({ "medal": medal.name(), "purse": purse }))
                .collect::<Vec<_>>(),
            "repeat": self.paid.repeat,
            "night": self.night.then_some(self.paid.night),
            "rain": self.rain.then_some(self.paid.rain),
            "paid": self.paid.dollars,
            "grams": self.grams,
        })
    }
}

/// A class of a map as the pages are told of it, at a trailhead and at the line: its medal
/// times if they are known, each one's purse, and what has been won.
pub fn told(map: &str, class: usize, gold: Option<f32>, record: Record) -> Value {
    json!({
        "class": class,
        "name": CLASSES[class.min(CLASSES.len() - 1)].name,
        "medals": gold.map(|gold| Medal::ALL.map(|medal| medal.time(gold))),
        "purses": gold.map(|gold| Medal::ALL.map(|medal| medal.purse(gold, class))),
        "held": record.medal.map(Medal::name),
        "best": record.best,
        "rivals": rivals(map, class).iter().map(|rival| json!({
            "name": rival.name,
            "truck": rival.truck,
            "paint": format!("#{:06x}", rival.paint),
            "medal": rival.medal.name(),
            "time": gold.map(|gold| rival.medal.time(gold)),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn a_time_wins_the_best_medal_it_is_inside() {
        let gold = 300.0;
        assert_eq!(Medal::Gold.time(gold), 300.0);
        assert_eq!(Medal::Silver.time(gold), 336.0);
        assert_eq!(Medal::Bronze.time(gold), 390.0);
        assert_eq!(Medal::won(299.9, gold), Some(Medal::Gold));
        assert_eq!(Medal::won(300.0, gold), Some(Medal::Gold));
        assert_eq!(Medal::won(300.05, gold), Some(Medal::Silver));
        assert_eq!(Medal::won(336.0, gold), Some(Medal::Silver));
        assert_eq!(Medal::won(389.0, gold), Some(Medal::Bronze));
        assert_eq!(Medal::won(390.1, gold), None);
    }

    #[test]
    fn a_purse_is_from_the_gold_time_and_the_class() {
        // Twelve minutes in the first class: $720, half of it, and a quarter.
        assert_eq!(Medal::Gold.purse(720.0, 0), 720);
        assert_eq!(Medal::Silver.purse(720.0, 0), 360);
        assert_eq!(Medal::Bronze.purse(720.0, 0), 180);
        // A better class pays more for the same time, and a longer track more in any class.
        assert_eq!(Medal::Gold.purse(720.0, 4), 4320);
        assert!(Medal::Gold.purse(400.0, 2) > Medal::Gold.purse(360.0, 2));
        for class in 0..CLASSES.len() {
            assert!(Medal::Gold.purse(250.0, class) > Medal::Silver.purse(250.0, class));
            assert!(Medal::Silver.purse(250.0, class) > Medal::Bronze.purse(250.0, class));
        }
    }

    #[test]
    fn a_purse_is_paid_in_full_once() {
        let pay = |held, won| payout(720.0, 0, held, won, false, false);
        // No medal, nothing.
        assert_eq!(pay(None, None), Paid::default());
        // A first bronze: its purse.
        let first = pay(None, Some(Medal::Bronze));
        assert_eq!(first.firsts, vec![(Medal::Bronze, 180)]);
        assert_eq!((first.repeat, first.dollars), (0, 180));
        // Bronze again: a tenth of it.
        let again = pay(Some(Medal::Bronze), Some(Medal::Bronze));
        assert!(again.firsts.is_empty());
        assert_eq!((again.repeat, again.dollars), (20, 20));
        // Then gold: silver's and gold's, and not bronze's again.
        let gold = pay(Some(Medal::Bronze), Some(Medal::Gold));
        assert_eq!(gold.firsts, vec![(Medal::Silver, 360), (Medal::Gold, 720)]);
        assert_eq!(gold.dollars, 1080);
        // A first gold with nothing won yet pays all three.
        assert_eq!(pay(None, Some(Medal::Gold)).dollars, 180 + 360 + 720);
        // With gold held, a worse run pays a tenth of what it reached, and no first win.
        let worse = pay(Some(Medal::Gold), Some(Medal::Silver));
        assert_eq!((worse.firsts.len(), worse.dollars), (0, 40));
        // Everything there is to win on a class of a track is won once.
        let all = pay(None, Some(Medal::Bronze)).dollars
            + pay(Some(Medal::Bronze), Some(Medal::Silver)).dollars
            + pay(Some(Medal::Silver), Some(Medal::Gold)).dollars;
        assert_eq!(all, pay(None, Some(Medal::Gold)).dollars);
    }

    #[test]
    fn the_night_and_the_rain_each_add_half() {
        let pay = |night, rain| payout(720.0, 0, None, Some(Medal::Silver), night, rain);
        let dry = pay(false, false);
        assert_eq!(dry.dollars, 540);
        assert_eq!((dry.night, dry.rain), (0, 0));
        assert_eq!(pay(true, false).dollars, 810);
        assert_eq!(pay(false, true).dollars, 810);
        let both = pay(true, true);
        assert_eq!((both.night, both.rain, both.dollars), (270, 270, 1080));
        // On a repeat as on a first win.
        let repeat = payout(720.0, 0, Some(Medal::Gold), Some(Medal::Gold), true, true);
        assert_eq!((repeat.repeat, repeat.dollars), (70, 140));
    }

    #[test]
    fn a_purse_is_weighed_out_in_gold() {
        assert_eq!(grams(720, 2400.0), 300);
        assert_eq!(grams(180, 2400.0), 75);
        assert_eq!(grams(0, 2400.0), 0);
    }

    #[test]
    fn the_others_are_always_the_same_three_and_no_two_alike() {
        let wombat = rivals("wombat", 2);
        assert_eq!(wombat, rivals("wombat", 2));
        assert_eq!(
            wombat.clone().map(|rival| rival.medal),
            [Medal::Bronze, Medal::Silver, Medal::Gold]
        );
        for (a, b) in [(0, 1), (0, 2), (1, 2)] {
            assert_ne!(wombat[a].name, wombat[b].name);
            assert_ne!(wombat[a].truck, wombat[b].truck);
            assert_ne!(wombat[a].paint, wombat[b].paint);
        }
        // Another map, or another class, has others; and each drives a truck of the class.
        assert_ne!(wombat, rivals("granite", 2));
        assert_ne!(wombat[2].name, rivals("wombat", 3)[2].name);
        for class in 0..CLASSES.len() {
            for rival in rivals("blackwood", class) {
                assert!(CLASSES[class].trucks.contains(&rival.truck));
            }
        }
    }

    #[test]
    fn what_is_won_is_kept_and_only_gets_better() {
        let mut records = Records::default();
        assert_eq!(records.of("wombat", 0), Record::default());
        let before = records.take("wombat", 0, 380.0, Some(Medal::Bronze));
        assert_eq!(before, Record::default());
        // A slower run with no medal changes nothing.
        let before = records.take("wombat", 0, 500.0, None);
        assert_eq!(before.medal, Some(Medal::Bronze));
        assert_eq!(
            records.of("wombat", 0),
            Record {
                medal: Some(Medal::Bronze),
                best: Some(380.0)
            }
        );
        records.take("wombat", 0, 299.5, Some(Medal::Gold));
        records.take("wombat", 0, 330.0, Some(Medal::Silver));
        records.take("granite", 4, 410.25, None);
        let mut kept = String::new();
        records.keep(&mut kept);
        assert_eq!(kept, "run granite 5 0 410.250\nrun wombat 1 3 299.500\n");
        assert_eq!(Records::read(&kept), records);
        // Lines that are not runs, or not of a class there is, are passed over.
        assert_eq!(
            Records::read("best wombat 301.5\nrun wombat 9 3 100\nrun wombat one 3 100\n"),
            Records::default()
        );
    }

    /// A loop of track in the game's own words: a square with rounded corners, 800 m round.
    pub(crate) fn square() -> Vec<GameFile> {
        let mut text = String::from("name Square\n");
        let (side, corner, step) = (160.0_f32, 40.0_f32, 4.0_f32);
        let mut points: Vec<(f32, f32, f32, f32)> = Vec::new();
        // Each side, then the quarter turn at its end: heading turns from -Z to +X, +Z, -X.
        let mut heading = 0.0_f32;
        let (mut x, mut z) = (0.0_f32, 0.0_f32);
        for _ in 0..4 {
            let mut gone = 0.0;
            while gone < side {
                points.push((x, z, heading.sin(), -heading.cos()));
                x += heading.sin() * step;
                z -= heading.cos() * step;
                gone += step;
            }
            let turns = (std::f32::consts::FRAC_PI_2 * corner / step) as usize;
            for _ in 0..turns {
                points.push((x, z, heading.sin(), -heading.cos()));
                heading += std::f32::consts::FRAC_PI_2 / turns as f32;
                x += heading.sin() * step;
                z -= heading.cos() * step;
            }
        }
        for (x, z, fx, fz) in points {
            // The way to the left of a heading (fx, fz), on flat ground.
            text += &format!("p {x:.3} 5 {z:.3} {:.4} 0 {:.4} 2 2\n", fz, -fx);
        }
        vec![GameFile {
            name: "outback.track".to_owned(),
            text,
        }]
    }

    #[test]
    fn a_track_has_times_for_every_class_and_a_better_class_is_quicker() {
        let course = Course::read(&square()).expect("a track is a course");
        let golds = course.golds();
        assert!(golds.windows(2).all(|pair| pair[1] < pair[0]), "{golds:?}");
        // The first class, at 47 km/h at most, takes more than a minute to go 800 m.
        assert!(golds[0] > 61.0 && golds[0] < 120.0, "{golds:?}");
        // A map with no track has none.
        assert!(Course::read(&[]).is_none());
    }

    #[test]
    fn a_run_is_told_how_it_stands_against_each_medal() {
        let course = Course::read(&square()).expect("a track is a course");
        let gold = f64::from(course.profiles[1].seconds());
        // Half-way round in half the gold time, near enough: ahead of all three or level.
        let half = course.profiles[1].length() * 0.5;
        let at_half = f64::from(course.profiles[1].time_at(half));
        let told = course.pace(1, at_half - 1.0, half);
        assert_eq!(told["on"], "gold");
        assert_eq!(told["gaps"][2], -1.0);
        assert!(told["gaps"][0].as_f64().unwrap() < told["gaps"][1].as_f64().unwrap());
        // Ten per cent behind gold's time there: inside silver's.
        let told = course.pace(1, at_half * 1.1, half);
        assert_eq!(told["on"], "silver");
        assert!(told["gaps"][2].as_f64().unwrap() > 0.0);
        // A long way behind: inside none.
        assert_eq!(course.pace(1, at_half * 1.5, half)["on"], Value::Null);
        // The others are where they would be: gold's home when its time is up, bronze's not.
        let ghosts = &course.pace(1, gold + 0.5, 0.0)["ghosts"];
        assert_eq!(ghosts[2]["done"], true);
        assert_eq!(ghosts[0]["done"], false);
    }
}
