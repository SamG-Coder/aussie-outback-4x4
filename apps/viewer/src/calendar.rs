//! The calendar: what day it is, the weather each day has, what is forecast, and when things
//! fall due (docs/campaign.md, The calendar).
//!
//! All of it is worked out from two numbers, the day of the game it is and the game's seed:
//! nothing is kept but those. This file is the rules, which need no game to try.

use serde_json::{json, Value};

/// A day's weather. In the order of the shell's `WEATHERS`, which it is one of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sky {
    Clear = 0,
    Fair = 1,
    Overcast = 2,
    Storm = 3,
}

impl Sky {
    const ALL: [Self; 4] = [Self::Clear, Self::Fair, Self::Overcast, Self::Storm];

    pub fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Fair => "fair",
            Self::Overcast => "overcast",
            Self::Storm => "storm",
        }
    }
}

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS: [(&str, u32); 12] = [
    ("January", 31),
    ("February", 28),
    ("March", 31),
    ("April", 30),
    ("May", 31),
    ("June", 30),
    ("July", 31),
    ("August", 31),
    ("September", 30),
    ("October", 31),
    ("November", 30),
    ("December", 31),
];
/// The first day of a game: Monday the fourth of May, late in the autumn.
const FIRST: (usize, u32) = (4, 4);
/// The day of the week repayments come out on, from Monday as 0: Friday.
pub const PAY_DAY: usize = 4;
/// What is repaid every week, and how much (docs/campaign.md, Debt): the mortgage on the
/// block, and the finance on a first truck.
pub const LOANS: [(&str, u64); 2] = [("The block", 1500), ("The truck", 350)];
/// The seed of a game that has none yet.
pub const NO_SEED: u64 = 1851;
/// How many days ahead are forecast, and how many beyond that are an outlook.
const FORECAST: u32 = 2;
const OUTLOOK: u32 = 6;
/// When the wet sets in: fronts come closer together from then on.
const THE_WET: u32 = 40;

/// A day of the game as a date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    /// From Monday, as 0.
    pub weekday: usize,
    /// The day of the month, and the month from January as 0.
    pub day: u32,
    pub month: usize,
}

impl Date {
    pub fn weekday_name(&self) -> &'static str {
        WEEKDAYS[self.weekday]
    }

    pub fn month_name(&self) -> &'static str {
        MONTHS[self.month].0
    }
}

/// The date of a day of the game: its first is Monday 4 May.
pub fn date(day: u32) -> Date {
    let day = day.max(1);
    let (mut month, mut of_month) = FIRST;
    of_month += day - 1;
    while of_month > MONTHS[month].1 {
        of_month -= MONTHS[month].1;
        month = (month + 1) % 12;
    }
    Date {
        weekday: ((day - 1) % 7) as usize,
        day: of_month,
        month,
    }
}

/// A number from two others, well mixed: the same every time.
fn mixed(seed: u64, which: u64) -> u64 {
    let mut mixed = seed ^ which.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// The weather of a day. Fronts come through every four to seven days, and every three to
/// five once the wet has set in: the day before one is overcast, the day it comes is a storm
/// or, a weak one, overcast, the day after a storm is fair, and the days between are clear
/// or fair.
pub fn weather(seed: u64, day: u32) -> Sky {
    let day = day.max(1);
    // Each front in turn, from the first, until one is past the day asked about.
    let mut front = 2 + (mixed(seed, 0) % 5) as u32;
    let mut count = 1_u64;
    let mut last: Option<(u32, bool)> = None;
    loop {
        // More of them are storms as the season turns.
        let strong = mixed(seed, count * 2) % 100 < 55 + u64::from(front.min(60) / 3);
        if front == day {
            return if strong { Sky::Storm } else { Sky::Overcast };
        }
        if front > day {
            if front == day + 1 {
                return Sky::Overcast;
            }
            break;
        }
        last = Some((front, strong));
        let roll = mixed(seed, count * 2 + 1);
        front += if front < THE_WET {
            4 + (roll % 4) as u32
        } else {
            3 + (roll % 3) as u32
        };
        count += 1;
    }
    if last == Some((day - 1, true)) || mixed(seed, 0x5EED + u64::from(day)) % 5 < 2 {
        Sky::Fair
    } else {
        Sky::Clear
    }
}

/// How much is known of a day's weather, looked at from another day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known {
    /// It has been, or it is today: this is what it was.
    Seen,
    /// Tomorrow or the day after: right about nine times in ten.
    Forecast,
    /// Up to six days off: right about two times in three.
    Outlook,
}

impl Known {
    fn name(self) -> &'static str {
        match self {
            Self::Seen => "seen",
            Self::Forecast => "forecast",
            Self::Outlook => "outlook",
        }
    }
}

/// What is said on `today` of the weather of `day`, and how far it is to be trusted; nothing
/// for a day too far off to say. A forecast that is wrong is wrong by one step, and is the
/// same forecast however often it is asked for on the same day.
pub fn forecast(seed: u64, today: u32, day: u32) -> Option<(Sky, Known)> {
    let actual = weather(seed, day);
    if day <= today {
        return Some((actual, Known::Seen));
    }
    let (known, wrong_in_100) = match day - today {
        ahead if ahead <= FORECAST => (Known::Forecast, 10),
        ahead if ahead <= OUTLOOK => (Known::Outlook, 33),
        _ => return None,
    };
    let roll = mixed(
        seed ^ u64::from(today).wrapping_mul(0xD6E8_FEB8_6659_FD93),
        u64::from(day),
    );
    if roll % 100 >= wrong_in_100 {
        return Some((actual, known));
    }
    let said = match actual {
        Sky::Clear => Sky::Fair,
        Sky::Storm => Sky::Overcast,
        other if (roll >> 32) % 2 == 0 => Sky::ALL[other as usize - 1],
        other => Sky::ALL[other as usize + 1],
    };
    Some((said, known))
}

/// Whether a day is one the bank's repayment comes out on: a Friday. How much comes out is
/// the bank's to say (`yard::Yard::friday`), not the calendar's.
pub fn pay_day(day: u32) -> bool {
    date(day).weekday == PAY_DAY
}

/// The next day anything falls due, from `today` on: today, if it is one.
pub fn next_due(today: u32) -> u32 {
    (today.max(1)..)
        .find(|day| pay_day(*day))
        .expect("there is always another Friday")
}

/// The calendar as the pages are told of it on a day: four weeks from the Monday of the
/// week it is, a square a day, and what is due next. `weekly` is what the bank takes each
/// Friday, and `owed` what is owed it in all: nothing, where there is no debt.
pub fn told(seed: u64, today: u32, weekly: u64, owed: u64) -> Value {
    let today = today.max(1);
    let first = today - date(today).weekday as u32;
    let days: Vec<Value> = (first..first + 28)
        .map(|day| {
            let on = date(day);
            let said = forecast(seed, today, day);
            let due = if pay_day(day) && day >= today {
                weekly
            } else {
                0
            };
            json!({
                "day": day,
                "date": on.day,
                // A month is named on its first day, and on the first square.
                "month": (on.day == 1 || day == first).then(|| &on.month_name()[..3]),
                "when": match day.cmp(&today) {
                    std::cmp::Ordering::Less => "past",
                    std::cmp::Ordering::Equal => "today",
                    std::cmp::Ordering::Greater => said.map_or("later", |(_, known)| known.name()),
                },
                "sky": said.map(|(sky, _)| sky.name()),
                "due": (due > 0).then_some(due),
            })
        })
        .collect();
    let (now, last) = (date(today), date(first + 27));
    let next = next_due(today);
    let upon = date(next);
    json!({
        "day": today,
        "weekday": now.weekday_name(),
        "date": now.day,
        "month": now.month_name(),
        "months": if date(first).month == last.month {
            last.month_name().to_owned()
        } else {
            format!("{} and {}", date(first).month_name(), last.month_name())
        },
        "sky": weather(seed, today).name(),
        "days": days,
        "next": {
            "day": next,
            "in": next - today,
            "weekday": upon.weekday_name(),
            "date": upon.day,
            "month": upon.month_name(),
            "items": if weekly > 0 {
                vec![json!({ "what": "Bank repayment", "dollars": weekly })]
            } else {
                Vec::new()
            },
            "total": weekly,
            "owed": owed,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_of_the_game_is_a_date() {
        let on = |day: u32| {
            let date = date(day);
            (date.weekday_name(), date.day, date.month_name())
        };
        assert_eq!(on(1), ("Monday", 4, "May"));
        assert_eq!(on(5), ("Friday", 8, "May"));
        assert_eq!(on(28), ("Sunday", 31, "May"));
        assert_eq!(on(29), ("Monday", 1, "June"));
        // The year turns, and goes on.
        assert_eq!(on(242), ("Thursday", 31, "December"));
        assert_eq!(on(243), ("Friday", 1, "January"));
        assert_eq!(on(366), ("Tuesday", 4, "May"));
        // A game's day is never before its first.
        assert_eq!(on(0), on(1));
    }

    #[test]
    fn repayments_fall_due_on_fridays() {
        for day in 1..=60 {
            let friday = date(day).weekday_name() == "Friday";
            assert_eq!(pay_day(day), friday, "day {day}");
        }
        // What comes out is what the bank says, and nothing where nothing is owed.
        assert_eq!(told(7, 1, 1850, 100_000)["next"]["total"], 1850);
        assert_eq!(
            told(7, 1, 0, 0)["next"]["items"].as_array().map(Vec::len),
            Some(0)
        );
        // From a Monday the first Friday is four days off; on a Friday it is today; the day
        // after, it is next week's.
        assert_eq!(next_due(1), 5);
        assert_eq!(next_due(5), 5);
        assert_eq!(next_due(6), 12);
    }

    #[test]
    fn the_weather_is_the_seeds_and_comes_in_fronts() {
        let season = |seed: u64| (1..=90).map(|day| weather(seed, day)).collect::<Vec<_>>();
        // The same seed, the same season; another seed, another.
        assert_eq!(season(7), season(7));
        assert_ne!(season(7), season(8));
        for seed in [1_u64, 7, 1851, 0xDEAD_BEEF] {
            let days = season(seed);
            let count = |sky: Sky| days.iter().filter(|day| **day == sky).count();
            // There is some of everything, and more fine days than foul.
            assert!(count(Sky::Storm) >= 5, "seed {seed}: {days:?}");
            assert!(
                count(Sky::Clear) + count(Sky::Fair) > count(Sky::Storm) + count(Sky::Overcast)
            );
            assert!(count(Sky::Clear) >= 10 && count(Sky::Overcast) >= 10);
            for (index, sky) in days.iter().enumerate().skip(1) {
                // A storm is seen coming: the day before it is overcast.
                if *sky == Sky::Storm {
                    assert_eq!(
                        days[index - 1],
                        Sky::Overcast,
                        "seed {seed}, day {}",
                        index + 1
                    );
                }
            }
            // The wet has more storms than the dry before it.
            let storms = |days: &[Sky]| days.iter().filter(|day| **day == Sky::Storm).count();
            assert!(storms(&days[45..]) >= storms(&days[..45]), "seed {seed}");
        }
    }

    #[test]
    fn a_forecast_is_mostly_right_and_an_outlook_less_so() {
        let seed = 1851;
        let (mut near, mut far) = ((0, 0), (0, 0));
        for today in 1..=120 {
            // What has been is known, and so is today.
            assert_eq!(
                forecast(seed, today, today),
                Some((weather(seed, today), Known::Seen))
            );
            if today > 1 {
                assert_eq!(
                    forecast(seed, today, today - 1).map(|said| said.1),
                    Some(Known::Seen)
                );
            }
            for ahead in 1..=8 {
                let day = today + ahead;
                let said = forecast(seed, today, day);
                // The same asked again.
                assert_eq!(said, forecast(seed, today, day));
                let Some((sky, known)) = said else {
                    assert!(ahead > 6, "nothing is said only of days far off");
                    continue;
                };
                assert_eq!(
                    known,
                    if ahead <= 2 {
                        Known::Forecast
                    } else {
                        Known::Outlook
                    }
                );
                let actual = weather(seed, day);
                // Wrong, it is wrong by one step.
                assert!((sky as i32 - actual as i32).abs() <= 1);
                let tally = if ahead <= 2 { &mut near } else { &mut far };
                tally.0 += usize::from(sky == actual);
                tally.1 += 1;
            }
        }
        let share = |(right, all): (usize, usize)| right as f32 / all as f32;
        assert!(share(near) > 0.84 && share(near) < 0.97, "{}", share(near));
        assert!(share(far) > 0.55 && share(far) < 0.78, "{}", share(far));
    }

    #[test]
    fn the_pages_are_told_four_weeks_from_this_weeks_monday() {
        // Day 10 is a Wednesday: the week began on day 8.
        let told = told(1851, 10, 1850, 100_000);
        assert_eq!(
            (&told["weekday"], &told["date"], &told["month"]),
            (&json!("Wednesday"), &json!(13), &json!("May"))
        );
        let days = told["days"].as_array().expect("a list of days");
        assert_eq!(days.len(), 28);
        assert_eq!(
            (&days[0]["day"], &days[0]["when"]),
            (&json!(8), &json!("past"))
        );
        assert_eq!(days[2]["when"], "today");
        assert_eq!(
            (&days[3]["when"], &days[4]["when"]),
            (&json!("forecast"), &json!("forecast"))
        );
        assert_eq!(
            (&days[5]["when"], &days[8]["when"]),
            (&json!("outlook"), &json!("outlook"))
        );
        // Past the outlook nothing is said of the weather; the Fridays are still marked.
        assert_eq!(
            (&days[9]["when"], &days[9]["sky"]),
            (&json!("later"), &Value::Null)
        );
        assert_eq!(days[4]["due"], 1850);
        assert_eq!(days[11]["due"], 1850);
        assert_eq!(days[5]["due"], Value::Null);
        assert_eq!(told["months"], "May and June");
        assert_eq!(
            (
                &told["next"]["day"],
                &told["next"]["in"],
                &told["next"]["total"]
            ),
            (&json!(12), &json!(2), &json!(1850))
        );
        assert_eq!(told["next"]["items"][0]["what"], "Bank repayment");
        assert_eq!(told["next"]["owed"], 100_000);
        assert_eq!(told["sky"], weather(1851, 10).name());
    }
}
