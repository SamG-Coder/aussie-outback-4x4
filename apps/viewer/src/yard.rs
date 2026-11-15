//! Money and trucks: what is owed, the trucks that are owned, and where they are bought,
//! sold and worked on.
//!
//! A story begins in debt ([`BLOCK`]), and with the choice of a first truck from three cheap
//! ones ([`starters`]), which goes on the debt too. From then on:
//!
//! * **The bank** lends, up to what the borrower's credit allows ([`Yard::limit`]), and is
//!   paid back. Every Friday a repayment comes out ([`Yard::friday`]): paid, credit rises;
//!   short, what was not paid goes back on the debt with a penalty, and credit falls.
//! * **The used yard** has a few trucks for sale each day, different every day ([`used`]):
//!   mostly ordinary, now and then a good one going cheap. One bought is owned.
//! * **Trucks owned** are kept, each with its colour, what is fitted to it, the parts
//!   bought for it, how far it has been driven and when its oil was last changed. One of
//!   them is the one driven; another can be taken out instead, and any but the last sold.
//! * **The auto shop** sells parts for the truck that is driven (a part bought is owned:
//!   it can be taken off and put back for nothing), resprays it, and changes its oil.
//!
//! Nothing here knows of the game's screens or its world but the list of its trucks: it is
//! the rules, and what is kept of them.

use crate::kit::{Build, Kit};
use crate::shell::Truck;

/// What is owed on the block and its shed when a story begins, in dollars.
pub const BLOCK: u64 = 100_000;
/// What a first truck costs, about: the three nearest it are offered.
const FIRST_TRUCK: u32 = 10_000;
/// The repayment that comes out every Friday, and what is added to a part of it that is
/// not paid.
pub const REPAYMENT: u64 = 1_850;
const SHORT_PENALTY: f32 = 0.05;
/// What the bank will lend beyond what a story begins owing, at no credit and at full.
const LIMIT: (u64, u64) = (115_000, 320_000);
/// How many trucks the used yard has on a day.
const ON_THE_LOT: usize = 6;
/// How far a truck goes between oil changes, in kilometres; what one costs; a respray.
pub const OIL_EVERY: f32 = 150.0;
/// What an engine overdue for its oil loses of its pull for each kilometre overdue, and the
/// most it loses.
pub const OIL_WEAR: (f32, f32) = (0.004, 0.2);
pub const OIL_CHANGE: u64 = 180;
pub const RESPRAY: u64 = 1_200;
/// The colours a truck is resprayed in, and that a used one may be: as a page writes them.
pub const COLOURS: [(&str, u32); 12] = [
    ("White", 0xe8e6de),
    ("Cream", 0xd9cfa8),
    ("Sand", 0xb89a6a),
    ("Rust red", 0x8a2a1a),
    ("Fire red", 0xb01818),
    ("Mustard", 0xc79a1e),
    ("Bush green", 0x3f5a3a),
    ("Sky blue", 0x5f8fb8),
    ("Navy", 0x1c2f52),
    ("Silver", 0x9a9c9e),
    ("Charcoal", 0x2e3033),
    ("Black", 0x101112),
];

/// A truck that is owned.
#[derive(Clone, Debug, PartialEq)]
pub struct Owned {
    /// Its own number, which it keeps: two of the same truck are told apart by it.
    pub id: u32,
    /// Which of the game's trucks it is, by name.
    pub truck: String,
    /// Its colour, as a page writes one, if it is not the colour it was made in.
    pub paint: Option<u32>,
    /// What is fitted to it; and the parts bought for it, each `group.choice`.
    pub fitted: Build,
    pub parts: Vec<String>,
    /// How far it has been driven, in kilometres, and how far it had been at its last oil.
    pub km: f32,
    pub oiled: f32,
    /// How good a one it is, from 0, rough, to 1, as good as they come: what it is worth.
    pub roll: f32,
    /// What is on it, each from 0 to 1: dust, mud, and rust. It goes where the truck goes.
    pub dirt: [f32; 3],
}

impl Owned {
    /// How far it is past its oil change, in kilometres: above nothing, it is overdue.
    pub fn oil_overdue(&self) -> f32 {
        self.km - self.oiled - OIL_EVERY
    }

    /// The share of its engine's pull it still has: all of it, until its oil is overdue.
    pub fn power(&self) -> f32 {
        1.0 - (self.oil_overdue().max(0.0) * OIL_WEAR.0).min(OIL_WEAR.1)
    }

    /// Its build as the game is told to put it together: every group that is fitted, and
    /// `none` for what it was sold with and has had taken off.
    pub fn build_text(&self, kit: Option<&Kit>) -> String {
        // One that has not been seen with its kit yet is as it was sold.
        if self.fitted.0.is_empty() {
            return "stock".to_owned();
        }
        let mut words: Vec<String> = self
            .fitted
            .0
            .iter()
            .map(|(g, c)| format!("{g}={c}"))
            .collect();
        for (group, _) in kit.map(|kit| &kit.stock.0[..]).unwrap_or_default() {
            if self.fitted.fitted(group).is_none() {
                words.push(format!("{group}=none"));
            }
        }
        if words.is_empty() {
            "stock".to_owned()
        } else {
            words.join(",")
        }
    }
}

/// A truck for sale at the used yard.
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    pub truck: String,
    pub km: f32,
    pub roll: f32,
    pub paint: u32,
    pub price: u64,
    /// A good one going for less than it is worth.
    pub bargain: bool,
}

/// What is kept of money and trucks.
#[derive(Clone, Debug, PartialEq)]
pub struct Yard {
    /// What is owed to the bank, and the borrower's credit, from 0 to 100.
    pub debt: u64,
    pub credit: u32,
    pub trucks: Vec<Owned>,
    /// Which of them is driven: its place among them.
    pub driving: usize,
    next: u32,
    /// The used trucks bought today, by their places on the lot, and which day that was.
    bought: (u32, Vec<usize>),
}

impl Default for Yard {
    fn default() -> Self {
        Self {
            debt: 0,
            credit: 20,
            trucks: Vec::new(),
            driving: 0,
            next: 1,
            bought: (0, Vec::new()),
        }
    }
}

/// A number from a seed and some others, that is always the same for them: and one from 0
/// to 1 of it.
fn mixed(seed: u64, with: &[u64]) -> u64 {
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    for value in with {
        state = (state ^ value).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        state ^= state >> 31;
        state = state.wrapping_mul(0x94D0_49BB_1331_11EB);
        state ^= state >> 29;
    }
    state
}

fn chance(seed: u64, with: &[u64]) -> f32 {
    (mixed(seed, with) >> 40) as f32 / (1u64 << 24) as f32
}

/// How many years of ordinary driving a truck of a year has had: its first twenty count
/// whole, and those after them a quarter each, for an old truck is not driven as a new one.
fn driven_years(year: u32) -> f32 {
    let age = 2026u32.saturating_sub(year).max(1) as f32;
    age.min(20.0) + (age - 20.0).max(0.0) * 0.25
}

/// The three trucks a story begins with the choice of: the cheapest grade's, nearest what a
/// first truck costs.
pub fn starters(trucks: &[Truck]) -> Vec<&Truck> {
    let mut cheap: Vec<&Truck> = trucks.iter().filter(|truck| truck.grade == 1).collect();
    cheap.sort_by_key(|truck| (truck.price.abs_diff(FIRST_TRUCK), truck.price));
    cheap.truncate(3);
    cheap.sort_by_key(|truck| truck.price);
    cheap
}

/// What a truck is worth: one of that name, that good, that far driven.
pub fn worth(truck: &Truck, roll: f32, km: f32) -> u64 {
    // From seven tenths of its price, rough, to a quarter over it, as good as they come;
    // and less for every kilometre, down to half at four hundred thousand.
    let condition = 0.7 + 0.55 * roll.clamp(0.0, 1.0);
    let driven = 1.0 - 0.5 * (km / 400_000.0).clamp(0.0, 1.0);
    (truck.price as f32 * condition * driven / 50.0).round() as u64 * 50
}

/// The trucks the used yard has on a day: different every day, and the same all that day.
/// Cheap trucks are common and dear ones rare; most are ordinary; and now and then there
/// is a good one going cheap.
pub fn used(seed: u64, day: u32, trucks: &[Truck]) -> Vec<Listing> {
    if trucks.is_empty() {
        return Vec::new();
    }
    (0..ON_THE_LOT as u64)
        .map(|place| {
            let roll_of = |what: u64| chance(seed, &[u64::from(day), place, what]);
            // The grade first, low ones oftener, and then one of that grade.
            let grade = match roll_of(1) {
                r if r < 0.42 => 1,
                r if r < 0.70 => 2,
                r if r < 0.88 => 3,
                r if r < 0.97 => 4,
                _ => 5,
            };
            let of_grade: Vec<&Truck> = trucks.iter().filter(|t| t.grade == grade).collect();
            let pool = if of_grade.is_empty() {
                trucks.iter().collect()
            } else {
                of_grade
            };
            let truck = pool[(roll_of(2) * pool.len() as f32) as usize % pool.len()];
            // Most are middling: two rolls averaged. A bargain is a good one, a third off.
            let bargain = roll_of(3) < 0.08;
            let roll = if bargain {
                0.8 + 0.2 * roll_of(4)
            } else {
                (roll_of(4) + roll_of(5)) * 0.5
            };
            // (To the thousandth, as it is kept.)
            let roll = (roll * 1000.0).round() / 1000.0;
            // An old truck has gone further, and a rough one further than a good one.
            let years = driven_years(truck.year);
            let km = (years * (7_000.0 + 11_000.0 * (1.0 - roll) + 5_000.0 * roll_of(6))).round();
            let (_, paint) = COLOURS[(roll_of(7) * COLOURS.len() as f32) as usize % COLOURS.len()];
            let asked = worth(truck, roll, km) as f32 * (1.0 + 0.1 * (roll_of(8) - 0.5));
            let asked = asked * if bargain { 0.67 } else { 1.0 };
            Listing {
                truck: truck.name.clone(),
                km,
                roll,
                paint,
                price: ((asked / 50.0).round() as u64 * 50).max(500),
                bargain,
            }
        })
        .collect()
}

/// What a part costs for a truck of a grade: a group of its kit, and the choice of it.
/// Nothing, for what is not sold: the lips every truck has round its arches, its own
/// springs.
pub fn part_price(group: &str, choice: &str, grade: u32) -> u64 {
    let base = match (group, choice) {
        ("bullbar", "nudge") => 600,
        ("bullbar", "tube") => 1_400,
        ("bullbar", "plate") => 2_600,
        ("spots", _) => 450,
        ("snorkel", _) => 550,
        ("rack", _) => 900,
        ("lightbar", _) => 350,
        ("arches", "bolted") => 500,
        ("arches", "moulded") => 900,
        ("steps", "steps") => 400,
        ("steps", "sliders") => 1_100,
        ("suspension", "heavy") => 1_500,
        ("suspension", "lift") => 2_800,
        ("suspension", "long") => 5_200,
        ("tyres", "road") => 800,
        ("tyres", "at") => 1_200,
        ("tyres", "mud") => 1_600,
        ("rims", "slot8" | "sunraysia") => 600,
        ("rims", "spoke5" | "spoke6") => 1_100,
        ("rims", "mesh") => 1_400,
        ("rims", "beadlock") => 1_800,
        ("tub", "sportsbar") => 700,
        ("tub", "tonneau") => 600,
        ("tub", "canopy") => 2_400,
        ("tray", "timber") => 500,
        ("tray", "boxes") => 1_200,
        ("spare", _) => 350,
        _ => return 0,
    };
    // A big truck's parts are dearer than a small one's.
    (base as f32 * (0.8 + 0.2 * grade.clamp(1, 5) as f32) / 10.0).round() as u64 * 10
}

/// Why something could not be done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum No {
    /// There is not the money for it, nor the credit to borrow it.
    Money,
    /// The bank will lend no more.
    Credit,
    /// There is no such truck, part or place on the lot.
    Nothing,
    /// The last truck is not sold.
    LastTruck,
}

impl No {
    pub fn said(self) -> &'static str {
        match self {
            Self::Money => "You do not have the money for that.",
            Self::Credit => "The bank will not lend you that much.",
            Self::Nothing => "There is nothing there.",
            Self::LastTruck => "You need a truck to get about in: this is your only one.",
        }
    }
}

impl Yard {
    /// Whether a story has begun: a first truck has been chosen.
    pub fn begun(&self) -> bool {
        !self.trucks.is_empty()
    }

    /// The truck that is driven.
    pub fn driven(&self) -> Option<&Owned> {
        self.trucks.get(self.driving)
    }

    fn own(&mut self, truck: &str, km: f32, roll: f32, paint: Option<u32>) -> usize {
        self.trucks.push(Owned {
            id: self.next,
            truck: truck.to_owned(),
            paint,
            fitted: Build::default(),
            parts: Vec::new(),
            km,
            oiled: km,
            roll,
            dirt: [0.0; 3],
        });
        self.next += 1;
        self.trucks.len() - 1
    }

    /// A story begins: the block is owed for, and so is the first truck, one of the three
    /// offered. It is old, and has been driven as far as an old truck has.
    pub fn begin(&mut self, truck: &Truck) {
        *self = Self::default();
        self.debt = BLOCK + u64::from(truck.price);
        let km = (driven_years(truck.year) * 13_000.0).round();
        self.driving = self.own(&truck.name, km, 0.45, None);
    }

    /// The most the bank will have this borrower owe.
    pub fn limit(&self) -> u64 {
        LIMIT.0 + (LIMIT.1 - LIMIT.0) * u64::from(self.credit.min(100)) / 100
    }

    /// Borrows: the money is in hand, and owed.
    pub fn borrow(&mut self, money: &mut u64, dollars: u64) -> Result<(), No> {
        if self.debt + dollars > self.limit() {
            return Err(No::Credit);
        }
        self.debt += dollars;
        *money += dollars;
        Ok(())
    }

    /// Pays the bank back, as much of `dollars` as is owed and is in hand: what was paid.
    /// Paying the whole of it off is worth a good deal of credit.
    pub fn repay(&mut self, money: &mut u64, dollars: u64) -> u64 {
        let paid = dollars.min(self.debt).min(*money);
        self.debt -= paid;
        *money -= paid;
        if paid > 0 && self.debt == 0 {
            self.credit = (self.credit + 25).min(100);
        }
        paid
    }

    /// A Friday: the week's repayment comes out of what is in hand. What was paid and what
    /// was not. Paid in full, credit rises; short, what was not paid goes back on the debt
    /// with a penalty, and credit falls.
    pub fn friday(&mut self, money: &mut u64) -> (u64, u64) {
        let due = REPAYMENT.min(self.debt);
        let paid = due.min(*money);
        *money -= paid;
        self.debt -= paid;
        let short = due - paid;
        if short > 0 {
            self.debt += (short as f32 * SHORT_PENALTY).round() as u64;
            self.credit = self.credit.saturating_sub(8);
        } else if due > 0 {
            self.credit = (self.credit + 2).min(100);
        }
        (paid, short)
    }

    /// Pays for something: out of what is in hand, and, if that is not enough and the bank
    /// will lend the rest, the rest on the debt.
    fn pay(&mut self, money: &mut u64, dollars: u64) -> Result<(), No> {
        let short = dollars.saturating_sub(*money);
        if short > 0 {
            if self.debt + short > self.limit() {
                return Err(No::Money);
            }
            self.debt += short;
        }
        *money -= dollars - short;
        Ok(())
    }

    /// The used yard today, each truck with whether it is still for sale.
    pub fn lot(&self, seed: u64, day: u32, trucks: &[Truck]) -> Vec<(Listing, bool)> {
        let sold = |place: usize| self.bought.0 == day && self.bought.1.contains(&place);
        let lot = used(seed, day, trucks);
        lot.into_iter()
            .enumerate()
            .map(|(place, listing)| (listing, !sold(place)))
            .collect()
    }

    /// Buys a truck off the used yard, by its place on today's lot. It is owned, and is not
    /// yet the one driven.
    pub fn buy(
        &mut self,
        money: &mut u64,
        seed: u64,
        day: u32,
        trucks: &[Truck],
        place: usize,
    ) -> Result<usize, No> {
        let lot = self.lot(seed, day, trucks);
        let Some((listing, true)) = lot.get(place) else {
            return Err(No::Nothing);
        };
        self.pay(money, listing.price)?;
        if self.bought.0 != day {
            self.bought = (day, Vec::new());
        }
        self.bought.1.push(place);
        Ok(self.own(
            &listing.truck,
            listing.km,
            listing.roll,
            Some(listing.paint),
        ))
    }

    /// What a truck that is owned would fetch: four fifths of what it is worth, and half of
    /// what its parts cost.
    pub fn fetches(&self, index: usize, trucks: &[Truck]) -> Option<u64> {
        let owned = self.trucks.get(index)?;
        let truck = trucks.iter().find(|truck| truck.name == owned.truck)?;
        let parts: u64 = owned
            .parts
            .iter()
            .filter_map(|part| part.split_once('.'))
            .map(|(group, choice)| part_price(group, choice, truck.grade))
            .sum();
        Some((worth(truck, owned.roll, owned.km) as f32 * 0.8) as u64 / 50 * 50 + parts / 2)
    }

    /// Sells a truck that is owned: what it fetched. The last one is not sold.
    pub fn sell(&mut self, money: &mut u64, index: usize, trucks: &[Truck]) -> Result<u64, No> {
        if self.trucks.len() <= 1 {
            return Err(No::LastTruck);
        }
        let fetched = self.fetches(index, trucks).ok_or(No::Nothing)?;
        self.trucks.remove(index);
        *money += fetched;
        if self.driving == index {
            self.driving = 0;
        } else if self.driving > index {
            self.driving -= 1;
        }
        Ok(fetched)
    }

    /// Takes another truck out: it is the one driven.
    pub fn drive(&mut self, index: usize) -> Result<(), No> {
        if index >= self.trucks.len() {
            return Err(No::Nothing);
        }
        self.driving = index;
        Ok(())
    }

    /// Fits a part to the truck that is driven: a choice of a group of its kit, or none of
    /// the group. One that has not been bought for it is bought; one that has, or that it
    /// was sold with, costs nothing to put back. What was paid.
    pub fn fit(
        &mut self,
        money: &mut u64,
        kit: &Kit,
        grade: u32,
        group: &str,
        choice: Option<&str>,
    ) -> Result<u64, No> {
        let index = self.driving;
        let Some(choice) = choice else {
            let owned = self.trucks.get_mut(index).ok_or(No::Nothing)?;
            owned.fitted.set(group, None);
            return Ok(0);
        };
        let mut asked = Build::default();
        asked.set(group, Some(choice));
        if kit.check(&asked).is_err() {
            return Err(No::Nothing);
        }
        let part = format!("{group}.{choice}");
        let owned = self.trucks.get(index).ok_or(No::Nothing)?;
        let have = owned.parts.contains(&part) || kit.stock.fitted(group) == Some(choice);
        let price = if have {
            0
        } else {
            part_price(group, choice, grade)
        };
        self.pay(money, price)?;
        let owned = &mut self.trucks[index];
        if !have {
            owned.parts.push(part);
        }
        owned.fitted.set(group, Some(choice));
        Ok(price)
    }

    /// Resprays the truck that is driven.
    pub fn respray(&mut self, money: &mut u64, colour: u32) -> Result<(), No> {
        if self.driven().is_none() {
            return Err(No::Nothing);
        }
        self.pay(money, RESPRAY)?;
        self.trucks[self.driving].paint = Some(colour);
        Ok(())
    }

    /// Changes the oil of the truck that is driven.
    pub fn oil(&mut self, money: &mut u64) -> Result<(), No> {
        if self.driven().is_none() {
            return Err(No::Nothing);
        }
        self.pay(money, OIL_CHANGE)?;
        let owned = &mut self.trucks[self.driving];
        owned.oiled = owned.km;
        Ok(())
    }

    /// The truck that is driven has gone so far, in kilometres.
    pub fn went(&mut self, km: f32) {
        if let Some(owned) = self.trucks.get_mut(self.driving) {
            owned.km += km.max(0.0);
        }
    }

    /// A truck that is owned is given what it was sold with, the first time it is seen with
    /// its kit: so that what is fitted to it is what is on it.
    pub fn stocked(&mut self, kit: &Kit) {
        if let Some(owned) = self.trucks.get_mut(self.driving) {
            if owned.fitted.0.is_empty() {
                owned.fitted = kit.stock.clone();
            }
        }
    }

    /// Reads what was kept: the lines [`Yard::keep`] wrote, among others.
    pub fn read(kept: &str) -> Self {
        let mut yard = Self::default();
        for line in kept.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let value = |key: &str| {
                let found = words.iter().find_map(|word| word.strip_prefix(key));
                found.and_then(|rest| rest.strip_prefix('='))
            };
            let number = |key: &str| value(key).and_then(|text| text.parse::<f32>().ok());
            match words.first() {
                Some(&"yard") => {
                    yard.debt = value("debt").and_then(|t| t.parse().ok()).unwrap_or(0);
                    yard.credit = number("credit").map_or(20, |n| n as u32).min(100);
                    yard.driving = number("driving").map_or(0, |n| n as usize);
                    yard.next = number("next").map_or(1, |n| n as u32).max(1);
                    let day = number("bought").map_or(0, |n| n as u32);
                    let places = value("places").unwrap_or("");
                    let places = places.split(';').filter_map(|p| p.parse().ok()).collect();
                    yard.bought = (day, places);
                }
                Some(&"owned") => {
                    let (Some(id), Some(truck)) = (number("id"), value("truck")) else {
                        continue;
                    };
                    let paint = value("paint").and_then(|hex| u32::from_str_radix(hex, 16).ok());
                    let fitted = value("fitted").and_then(|text| Build::parse(text).ok());
                    let parts = value("parts").unwrap_or("");
                    let km = number("km").unwrap_or(0.0);
                    yard.trucks.push(Owned {
                        id: id as u32,
                        truck: truck.to_owned(),
                        paint,
                        fitted: fitted.unwrap_or_default(),
                        parts: parts
                            .split(';')
                            .filter(|p| !p.is_empty())
                            .map(str::to_owned)
                            .collect(),
                        km,
                        oiled: number("oiled").unwrap_or(km),
                        roll: number("roll").unwrap_or(0.5),
                        dirt: [
                            number("dust").unwrap_or(0.0),
                            number("mud").unwrap_or(0.0),
                            number("rust").unwrap_or(0.0),
                        ],
                    });
                }
                _ => {}
            }
        }
        yard.driving = yard.driving.min(yard.trucks.len().saturating_sub(1));
        yard
    }

    /// Writes what is to be kept, as lines.
    pub fn keep(&self, kept: &mut String) {
        if !self.begun() && self.debt == 0 {
            return;
        }
        let places: Vec<String> = self.bought.1.iter().map(usize::to_string).collect();
        *kept += &format!(
            "yard debt={} credit={} driving={} next={} bought={} places={}\n",
            self.debt,
            self.credit,
            self.driving,
            self.next,
            self.bought.0,
            places.join(";")
        );
        for owned in &self.trucks {
            *kept += &format!(
                "owned id={} truck={} km={:.1} oiled={:.1} roll={:.3}",
                owned.id, owned.truck, owned.km, owned.oiled, owned.roll
            );
            if let Some(paint) = owned.paint {
                *kept += &format!(" paint={paint:06x}");
            }
            if owned.dirt != [0.0; 3] {
                let [dust, mud, rust] = owned.dirt;
                *kept += &format!(" dust={dust:.3} mud={mud:.3} rust={rust:.3}");
            }
            if !owned.fitted.0.is_empty() {
                *kept += &format!(" fitted={}", owned.fitted.to_text());
            }
            if !owned.parts.is_empty() {
                *kept += &format!(" parts={}", owned.parts.join(";"));
            }
            *kept += "\n";
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trucks() -> Vec<Truck> {
        let truck = |name: &str, grade: u32, price: u32, year: u32| Truck {
            name: name.to_owned(),
            grade,
            price,
            year,
            title: name.to_owned(),
        };
        vec![
            truck("bendoc", 1, 6_000, 1981),
            truck("tambo", 1, 9_000, 1986),
            truck("licola", 1, 9_500, 1976),
            truck("wonnangatta", 1, 10_500, 1978),
            truck("mitta", 2, 16_000, 1995),
            truck("ovens", 3, 42_000, 2014),
            truck("bogong", 4, 92_000, 2019),
            truck("kosciuszko", 5, 160_000, 2022),
        ]
    }

    const KIT: &str = "base cab\n\
        choice bullbar tube acc_bullbar_tube\nchoice bullbar plate acc_bullbar_plate\n\
        choice tyres at tyre_at_FL\nchoice tyres mud tyre_mud_FL\n\
        stock tyres at\n";

    #[test]
    fn a_story_begins_in_debt_with_one_of_three_cheap_trucks() {
        let trucks = trucks();
        let offered = starters(&trucks);
        let names: Vec<&str> = offered.iter().map(|truck| truck.name.as_str()).collect();
        // The three nearest ten thousand dollars, cheapest first: not the dear ones.
        assert_eq!(names, ["tambo", "licola", "wonnangatta"]);
        let mut yard = Yard::default();
        assert!(!yard.begun());
        yard.begin(offered[1]);
        assert!(yard.begun());
        assert_eq!(yard.debt, BLOCK + 9_500);
        let driven = yard.driven().unwrap();
        assert_eq!((driven.truck.as_str(), driven.paint), ("licola", None));
        assert!(
            driven.km > 250_000.0 && driven.km < 500_000.0,
            "an old truck has been a long way"
        );
        assert!(driven.oil_overdue() < 0.0);
    }

    #[test]
    fn the_bank_lends_what_credit_allows_and_fridays_come_round() {
        let trucks = trucks();
        let mut yard = Yard::default();
        yard.begin(&trucks[1]);
        let mut money = 0;
        // A little can be borrowed with poor credit: not a great deal.
        assert_eq!(yard.borrow(&mut money, 5_000), Ok(()));
        assert_eq!((money, yard.debt), (5_000, BLOCK + 14_000));
        assert_eq!(yard.borrow(&mut money, 200_000), Err(No::Credit));
        // A Friday paid in full: the debt is down, and credit up.
        let (credit, owed) = (yard.credit, yard.debt);
        assert_eq!(yard.friday(&mut money), (REPAYMENT, 0));
        assert_eq!((money, yard.debt), (5_000 - REPAYMENT, owed - REPAYMENT));
        assert!(yard.credit > credit);
        // One that cannot be paid: what there is goes, the rest is owed with more on it.
        money = 350;
        let (credit, owed) = (yard.credit, yard.debt);
        assert_eq!(yard.friday(&mut money), (350, REPAYMENT - 350));
        assert_eq!(money, 0);
        assert!(yard.debt > owed - 350 && yard.credit < credit);
        // Paid back early, as much as there is to pay with.
        money = 2_000;
        assert_eq!(yard.repay(&mut money, 50_000), 2_000);
        assert_eq!(money, 0);
        // And paid out, it is worth a good deal of credit.
        money = yard.debt + 10;
        let credit = yard.credit;
        yard.repay(&mut money, u64::MAX);
        assert_eq!((yard.debt, money), (0, 10));
        assert!(yard.credit >= credit + 25 || yard.credit == 100);
        assert_eq!(yard.friday(&mut money), (0, 0));
    }

    #[test]
    fn the_used_yard_is_different_every_day_and_now_and_then_has_a_good_one_cheap() {
        let trucks = trucks();
        let today = used(7, 3, &trucks);
        assert_eq!(today.len(), ON_THE_LOT);
        assert_eq!(today, used(7, 3, &trucks), "the same all day");
        assert_ne!(today, used(7, 4, &trucks), "and not tomorrow");
        assert_ne!(today, used(8, 3, &trucks), "nor in another game");
        // Over a year: cheap trucks oftener than dear ones, every grade sometimes, and
        // bargains now and then, each a good truck for well under what it is worth.
        let (mut cheap, mut dear, mut bargains) = (0, 0, 0);
        for day in 1..=365 {
            for listing in used(7, day, &trucks) {
                let truck = trucks.iter().find(|t| t.name == listing.truck).unwrap();
                cheap += usize::from(truck.grade == 1);
                dear += usize::from(truck.grade == 5);
                assert!((0.0..=1.0).contains(&listing.roll) && listing.km > 0.0);
                assert!(COLOURS.iter().any(|(_, colour)| *colour == listing.paint));
                if listing.bargain {
                    bargains += 1;
                    assert!(listing.roll >= 0.8);
                    assert!(listing.price < worth(truck, listing.roll, listing.km) * 8 / 10);
                }
            }
        }
        assert!(cheap > 5 * dear && dear > 0, "{cheap} cheap, {dear} dear");
        assert!(
            (60..=320).contains(&bargains),
            "{bargains} bargains in a year"
        );
    }

    #[test]
    fn trucks_are_bought_driven_and_sold_and_the_last_is_kept() {
        let trucks = trucks();
        let mut yard = Yard::default();
        yard.begin(&trucks[1]);
        let mut money = 500_000;
        let lot = yard.lot(7, 3, &trucks);
        let (asked, name) = (lot[2].0.price, lot[2].0.truck.clone());
        let bought = yard.buy(&mut money, 7, 3, &trucks, 2).unwrap();
        assert_eq!(money, 500_000 - asked);
        assert_eq!(yard.trucks[bought].truck, name);
        assert_eq!(yard.trucks[bought].paint, Some(lot[2].0.paint));
        // It is gone from the lot today, and cannot be bought twice; tomorrow's lot is whole.
        assert!(!yard.lot(7, 3, &trucks)[2].1);
        assert_eq!(yard.buy(&mut money, 7, 3, &trucks, 2), Err(No::Nothing));
        assert!(yard.lot(7, 4, &trucks).iter().all(|(_, here)| *here));
        // It is taken out, and the first is sold: the one driven is still the one driven.
        yard.drive(bought).unwrap();
        let fetched = yard.sell(&mut money, 0, &trucks).unwrap();
        assert!(fetched > 0 && fetched < 9_000);
        assert_eq!(yard.driven().unwrap().truck, name);
        assert_eq!(yard.sell(&mut money, 0, &trucks), Err(No::LastTruck));
        // With no money, a truck goes on the debt if the bank will lend it: and not if not.
        let mut broke = 0;
        let owed = yard.debt;
        let cheap = (0..ON_THE_LOT).min_by_key(|place| yard.lot(7, 9, &trucks)[*place].0.price);
        let cheap = cheap.unwrap();
        let price = yard.lot(7, 9, &trucks)[cheap].0.price;
        if owed + price <= yard.limit() {
            yard.buy(&mut broke, 7, 9, &trucks, cheap).unwrap();
            assert_eq!(yard.debt, owed + price);
        }
        yard.debt = yard.limit();
        assert_eq!(yard.buy(&mut broke, 7, 10, &trucks, 0), Err(No::Money));
    }

    #[test]
    fn parts_are_bought_once_and_paint_and_oil_are_paid_for() {
        let trucks = trucks();
        let kit = Kit::parse(KIT).unwrap();
        let mut yard = Yard::default();
        yard.begin(&trucks[1]);
        yard.stocked(&kit);
        assert_eq!(yard.driven().unwrap().fitted.fitted("tyres"), Some("at"));
        // (Before it had been seen with its kit it was put together as it was sold, with
        // its tyres on: not with everything it was sold with taken off.)
        let mut unseen = Yard::default();
        unseen.begin(&trucks[1]);
        assert_eq!(unseen.driven().unwrap().build_text(Some(&kit)), "stock");
        let mut money = 10_000;
        // A bar is bought and fitted; taken off and put back, it costs nothing more.
        let price = part_price("bullbar", "plate", 1);
        assert!(price > 0);
        assert_eq!(
            yard.fit(&mut money, &kit, 1, "bullbar", Some("plate")),
            Ok(price)
        );
        assert_eq!(yard.fit(&mut money, &kit, 1, "bullbar", None), Ok(0));
        assert_eq!(yard.driven().unwrap().fitted.fitted("bullbar"), None);
        assert_eq!(
            yard.fit(&mut money, &kit, 1, "bullbar", Some("plate")),
            Ok(0)
        );
        assert_eq!(money, 10_000 - price);
        // The tyres it was sold with are its own: mud ones are paid for, and going back is free.
        assert!(yard.fit(&mut money, &kit, 1, "tyres", Some("mud")).unwrap() > 0);
        assert_eq!(yard.fit(&mut money, &kit, 1, "tyres", Some("at")), Ok(0));
        assert_eq!(
            yard.fit(&mut money, &kit, 1, "wings", Some("big")),
            Err(No::Nothing)
        );
        // What it is put together with: what is fitted, and nothing of what was taken off.
        assert_eq!(
            yard.driven().unwrap().build_text(Some(&kit)),
            "bullbar=plate,tyres=at"
        );
        yard.fit(&mut money, &kit, 1, "tyres", None).unwrap();
        assert_eq!(
            yard.driven().unwrap().build_text(Some(&kit)),
            "bullbar=plate,tyres=none"
        );
        // Its parts are worth something when it is sold.
        let with = yard.fetches(0, &trucks).unwrap();
        yard.trucks[0].parts.clear();
        assert!(with > yard.fetches(0, &trucks).unwrap());
        // A respray, and oil: driven far enough it is overdue, and changed it is not.
        let before = money;
        yard.respray(&mut money, 0x1c2f52).unwrap();
        assert_eq!(
            (yard.driven().unwrap().paint, money),
            (Some(0x1c2f52), before - RESPRAY)
        );
        yard.went(OIL_EVERY + 250.0);
        assert!(yard.driven().unwrap().oil_overdue() > 0.0);
        yard.oil(&mut money).unwrap();
        assert!(yard.driven().unwrap().oil_overdue() < 0.0);
        assert_eq!(money, before - RESPRAY - OIL_CHANGE);
    }

    #[test]
    fn what_is_kept_is_read_back_as_it_was() {
        let trucks = trucks();
        let kit = Kit::parse(KIT).unwrap();
        let mut yard = Yard::default();
        yard.begin(&trucks[2]);
        yard.stocked(&kit);
        let mut money = 400_000;
        yard.fit(&mut money, &kit, 1, "bullbar", Some("tube"))
            .unwrap();
        yard.respray(&mut money, 0xb01818).unwrap();
        yard.buy(&mut money, 7, 3, &trucks, 1).unwrap();
        yard.went(1234.5);
        yard.trucks[0].dirt = [0.25, 0.5, 0.125];
        let mut kept = String::from("day 3\nmoney 12\n");
        yard.keep(&mut kept);
        assert_eq!(Yard::read(&kept), yard);
        // A game with no story yet keeps nothing of it.
        let mut nothing = String::new();
        Yard::default().keep(&mut nothing);
        assert!(nothing.is_empty());
        assert_eq!(Yard::read("day 3\n"), Yard::default());
    }
}
