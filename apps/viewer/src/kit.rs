//! A truck built from its pieces.
//!
//! One of the game's own trucks is a bare truck and a kit: everything that can be bolted to
//! it, every kind of it, each a named piece of the one model (`tools/blender/truck/build.py`).
//! Its file has them all. What is on the truck at any time is its **build**: of each group
//! of things (its bull bar, its tyres, its springs) the one that is fitted, or none. The
//! truck is drawn as its base and the pieces of its build, and the rest are hidden.
//!
//! Two of the scene's gameplay files say what is what. The truck's **kit** (`NAME.kit`,
//! named by its actor file's `kit` line, written with its model) says which pieces are the
//! base and which are which choice of which group, what it wears as it is sold (`stock`)
//! and in its pictures (`showroom`), what a choice takes off (`replaces`), what swings and
//! where on (`hinge`), and where gold can be put (`hold`). Its **pieces** (written by the
//! bake, `solstice_game::actor::pieces_file`) say which of its instances draw each piece.

use glam::{Mat4, Vec3};
use solstice_asset::{pack, GameFile, LoadedScene};
use solstice_game::actor::pieces_file;
use solstice_game::scene::ActorLine;
use solstice_game::{resolve, ActorFile, GAME_FILE};
use solstice_render::Renderer;
use std::collections::HashSet;

/// A piece that swings: a door, a tailgate.
#[derive(Clone, Debug, PartialEq)]
pub struct Hinge {
    pub piece: String,
    /// Where it hangs, in the truck's own space. The piece is made about this point.
    pub at: Vec3,
    /// Whether it swings about an upright line, as a door does, or one across the truck, as
    /// a tailgate does.
    pub upright: bool,
    /// How far it opens, in degrees.
    pub opens: f32,
}

/// A place gold can be put: a seat, a boot, a tray.
#[derive(Clone, Debug, PartialEq)]
pub struct Hold {
    pub name: String,
    /// Its size, across, up and along, and its middle, in the truck's own space.
    pub size: Vec3,
    pub at: Vec3,
    /// The doors it is behind: any one of them open, and it can be reached. None, and it is
    /// open to the sky.
    pub doors: Vec<String>,
}

/// What a truck is made of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kit {
    pub base: Vec<String>,
    /// Each choice: its group, its name, and its pieces.
    pub choices: Vec<(String, String, Vec<String>)>,
    pub stock: Build,
    pub showroom: Build,
    /// What a choice takes off the truck: its group, its name, and the pieces.
    pub replaces: Vec<(String, String, Vec<String>)>,
    /// How far each kind of spring lifts the body, in metres.
    pub lifts: Vec<(String, f32)>,
    pub hinges: Vec<Hinge>,
    pub holds: Vec<Hold>,
    /// What rides on a door and swings with it, as a spare wheel does: the piece, and the
    /// piece it is on.
    pub rides: Vec<(String, String)>,
    /// The colour its paint was made in, as light: what a respray is told from.
    pub paint: Option<Vec3>,
}

impl Hinge {
    /// Where what hangs on it is when it is open so far of all the way, from where it is
    /// shut: turned about the hinge, in the truck's own space.
    pub fn swung(&self, share: f32) -> Mat4 {
        let angle = (self.opens * share).to_radians();
        let turn = if self.upright {
            Mat4::from_rotation_y(angle)
        } else {
            Mat4::from_rotation_x(angle)
        };
        Mat4::from_translation(self.at) * turn * Mat4::from_translation(-self.at)
    }
}

/// What is fitted to a truck: of each group, which choice. A group that is not named has
/// nothing fitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Build(pub Vec<(String, String)>);

impl Build {
    /// Reads `GROUP=CHOICE,GROUP=CHOICE`. `GROUP=none` takes that group off.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut build = Self::default();
        for word in text.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            let (group, choice) = word
                .split_once('=')
                .ok_or_else(|| format!("`{word}`: expected GROUP=CHOICE"))?;
            build.set(group.trim(), Some(choice.trim()).filter(|c| *c != "none"));
        }
        Ok(build)
    }

    pub fn to_text(&self) -> String {
        let words: Vec<String> = self.0.iter().map(|(g, c)| format!("{g}={c}")).collect();
        words.join(",")
    }

    pub fn fitted(&self, group: &str) -> Option<&str> {
        let found = self.0.iter().find(|(named, _)| named == group);
        found.map(|(_, choice)| choice.as_str())
    }

    /// Fits a choice of a group, or with none takes the group off.
    pub fn set(&mut self, group: &str, choice: Option<&str>) {
        self.0.retain(|(named, _)| named != group);
        if let Some(choice) = choice {
            self.0.push((group.to_owned(), choice.to_owned()));
        }
    }

    /// This build with another's choices put over it.
    pub fn with(&self, over: &Build) -> Build {
        let mut build = self.clone();
        for (group, choice) in &over.0 {
            build.set(group, Some(choice));
        }
        build
    }
}

impl Kit {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut kit = Kit::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("");
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some((&keyword, rest)) = words.split_first() else {
                continue;
            };
            let wrong = |message: &str| format!("line {}: {message}", number + 1);
            let number_of = |word: &str| {
                word.parse::<f32>()
                    .map_err(|_| wrong(&format!("`{word}` is not a number")))
            };
            let names = |from: &[&str]| from.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
            match (keyword, rest) {
                ("base", [piece]) => kit.base.push((*piece).to_owned()),
                ("choice", [group, choice, pieces @ ..]) if !pieces.is_empty() => kit
                    .choices
                    .push(((*group).to_owned(), (*choice).to_owned(), names(pieces))),
                ("stock", [group, choice]) => kit.stock.set(group, Some(choice)),
                ("showroom", [group, choice]) => kit.showroom.set(group, Some(choice)),
                ("replaces", [group, choice, pieces @ ..]) => {
                    kit.replaces
                        .push(((*group).to_owned(), (*choice).to_owned(), names(pieces)));
                }
                ("lift", [choice, metres]) => {
                    kit.lifts.push(((*choice).to_owned(), number_of(metres)?));
                }
                ("hinge", [piece, x, y, z, axis, opens]) => kit.hinges.push(Hinge {
                    piece: (*piece).to_owned(),
                    at: Vec3::new(number_of(x)?, number_of(y)?, number_of(z)?),
                    upright: match *axis {
                        "up" => true,
                        "across" => false,
                        _ => return Err(wrong("a hinge is `up` or `across`")),
                    },
                    opens: number_of(opens)?,
                }),
                ("hold", [name, w, h, l, "at", x, y, z, doors @ ..]) => {
                    let mut behind = Vec::new();
                    for pair in doors.chunks(2) {
                        match pair {
                            ["door", piece] => behind.push((*piece).to_owned()),
                            _ => return Err(wrong("after a hold's place, `door PIECE`")),
                        }
                    }
                    kit.holds.push(Hold {
                        name: (*name).to_owned(),
                        size: Vec3::new(number_of(w)?, number_of(h)?, number_of(l)?),
                        at: Vec3::new(number_of(x)?, number_of(y)?, number_of(z)?),
                        doors: behind,
                    });
                }
                ("paint", [r, g, b]) => {
                    kit.paint = Some(Vec3::new(number_of(r)?, number_of(g)?, number_of(b)?));
                }
                ("rides", [piece, on]) => {
                    kit.rides.push(((*piece).to_owned(), (*on).to_owned()));
                }
                _ => return Err(wrong(&format!("`{keyword}` is not a line of a kit"))),
            }
        }
        if kit.base.is_empty() {
            return Err("a kit has no base".to_owned());
        }
        Ok(kit)
    }

    /// A build of this truck from what is asked for: `stock` or `showroom` for what the kit
    /// calls those, or choices to put over what it is sold with, where `GROUP=none` takes
    /// off what it is sold with. A group or a choice the truck has not is an error.
    pub fn built(&self, asked: &str) -> Result<Build, String> {
        let build = match asked.trim() {
            "" | "stock" => self.stock.clone(),
            "showroom" => self.showroom.clone(),
            choices => {
                let mut build = self.stock.with(&Build::parse(choices)?);
                for word in choices.split(',') {
                    if let Some((group, "none")) = word.trim().split_once('=') {
                        build.set(group.trim(), None);
                    }
                }
                build
            }
        };
        self.check(&build)?;
        Ok(build)
    }

    /// The groups, in the order the kit names them, each with its choices.
    pub fn groups(&self) -> Vec<(&str, Vec<&str>)> {
        let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
        for (group, choice, _) in &self.choices {
            match groups.iter_mut().find(|(named, _)| named == group) {
                Some((_, choices)) => choices.push(choice),
                None => groups.push((group, vec![choice])),
            }
        }
        groups
    }

    /// What is wrong with a build for this truck, if anything: a group or a choice it has not.
    pub fn check(&self, build: &Build) -> Result<(), String> {
        for (group, choice) in &build.0 {
            let groups = self.groups();
            let Some((_, choices)) = groups.iter().find(|(named, _)| named == group) else {
                let known: Vec<&str> = groups.iter().map(|(named, _)| *named).collect();
                return Err(format!("no `{group}` to fit: {}", known.join(", ")));
            };
            if !choices.contains(&choice.as_str()) {
                return Err(format!(
                    "no `{choice}` among its {group}: {}",
                    choices.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// The pieces a build shows: the base, and each fitted choice's, less what they take off.
    pub fn shown(&self, build: &Build) -> HashSet<&str> {
        let mut shown: HashSet<&str> = self.base.iter().map(String::as_str).collect();
        let fitted = |group: &str, choice: &str| build.fitted(group) == Some(choice);
        for (group, choice, pieces) in &self.choices {
            if fitted(group, choice) {
                shown.extend(pieces.iter().map(String::as_str));
            }
        }
        for (group, choice, pieces) in &self.replaces {
            if fitted(group, choice) {
                for piece in pieces {
                    shown.remove(piece.as_str());
                }
            }
        }
        shown
    }

    /// The truck's holds and its doors, as the gold has them (`gold.rs`): each place gold
    /// can be put that is behind a door, with its doorways, and each door that leads to one.
    /// A place open to the sky, as a tray is, is not among them: it is the truck's tray.
    pub fn holds(&self) -> (Vec<crate::gold::Hold>, Vec<crate::gold::Door>) {
        let mut doors: Vec<crate::gold::Door> = Vec::new();
        let mut holds = Vec::new();
        for hold in self.holds.iter().filter(|hold| !hold.doors.is_empty()) {
            let mut ways = Vec::new();
            for piece in &hold.doors {
                let Some(hinge) = self.hinges.iter().find(|hinge| hinge.piece == *piece) else {
                    continue;
                };
                let door = doors.iter().position(|door| door.piece == *piece);
                let door = door.unwrap_or_else(|| {
                    doors.push(crate::gold::Door {
                        piece: piece.clone(),
                        open: false,
                    });
                    doors.len() - 1
                });
                // Its doorway: in the truck's side beside the hold, or, for a door whose
                // hinge is behind the hold, in its back.
                let behind = hinge.at.z > hold.at.z + hold.size.z * 0.5 - 0.3;
                let way = if behind {
                    Vec3::new(hold.at.x, hold.at.y, hinge.at.z)
                } else {
                    Vec3::new(hinge.at.x, hold.at.y, hold.at.z)
                };
                ways.push((door, way));
            }
            holds.push(crate::gold::Hold {
                name: hold.name.clone(),
                size: hold.size,
                floor: hold.at - Vec3::Y * hold.size.y * 0.5,
                ways,
            });
        }
        (holds, doors)
    }

    /// How far a build's springs lift the body.
    pub fn lift(&self, build: &Build) -> f32 {
        let springs = build.fitted("suspension");
        let found = springs.and_then(|kind| self.lifts.iter().find(|(named, _)| named == kind));
        found.map_or(0.0, |(_, lift)| *lift)
    }
}

/// The folder a truck's texts are in, among a scene's gameplay files, once it has been put
/// together from its pack.
pub const FOLDER: &str = "trucks/";

/// The name of the gameplay file that says what a truck put together from its pack is
/// wearing: its build, as [`Build::to_text`] writes it.
fn build_file(actor_file: &str) -> String {
    format!("{actor_file}.build")
}

/// A truck put together from its pack: its pieces as one scene, the gameplay files that say
/// what it is, and the name of its actor file among them.
pub struct Assembled {
    pub scene: LoadedScene,
    pub files: Vec<GameFile>,
    pub actor: String,
}

/// Puts a truck together from its pack (`solstice_asset::pack`, `bake --kit`). Its texts
/// are read, and then the pieces of its build and no others: what is not fitted to it is
/// never read. `build` is `stock`, `showroom`, or choices to put over what it is sold with;
/// with none it is as it is sold.
/// What each kind of tyre makes of the ground: times its grip on firm ground, and on soft.
const TREADS: [(&str, (f32, f32)); 3] = [
    ("road", (1.06, 0.8)),
    ("at", (1.0, 1.0)),
    ("mud", (0.93, 1.18)),
];
/// What each suspension does to a truck: times its springs, times its dampers, and times
/// how far its wheels travel. (How much higher it stands is the kit's own `lift`.)
const SUSPENSIONS: [(&str, [f32; 3]); 4] = [
    ("stock", [1.0, 1.0, 1.0]),
    ("heavy", [1.25, 1.2, 1.0]),
    ("lift", [1.05, 1.1, 1.25]),
    ("long", [0.9, 1.1, 1.6]),
];

/// A truck's numbers (its `.vehicle` file) as it is fitted: its tyres' tread, its suspension's
/// springs, dampers and travel, how high it stands (its weight is as much higher, and its
/// belly as much further off the ground), and its engine's pull, of which `power` is the
/// share an engine overdue for its oil still has.
pub fn tuned(vehicle: &str, kit: &Kit, build: &Build, power: f32) -> String {
    let lift = kit.lift(build);
    let tread = build
        .fitted("tyres")
        .and_then(|fitted| TREADS.iter().find(|(name, _)| *name == fitted))
        .map_or((1.0, 1.0), |(_, tread)| *tread);
    let [spring, damper, travel] = build
        .fitted("suspension")
        .and_then(|fitted| SUSPENSIONS.iter().find(|(name, _)| *name == fitted))
        .map_or([1.0; 3], |(_, does)| *does);
    let number = |word: &str| word.parse::<f32>().ok();
    // A `key=value` word of a line, its value (or each of its values) times something.
    let scaled = |word: &str, key: &str, by: f32| -> Option<String> {
        let values = word.strip_prefix(key)?.strip_prefix('=')?;
        let each: Option<Vec<String>> = values
            .split(',')
            .map(|value| number(value).map(|value| format!("{:.4}", value * by)))
            .collect();
        Some(format!("{key}={}", each?.join(",")))
    };
    let mut out = String::new();
    for line in vehicle.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let written = match words[..] {
            ["centre", x, y, z] => match number(y) {
                Some(y) => format!("centre {x} {:.3} {z}", y + lift),
                None => line.to_owned(),
            },
            // Its body's underside is as much higher as it stands: the box is that much
            // shallower, from below.
            ["body", w, h, l, "at", x, y, z] => match (number(h), number(y)) {
                (Some(h), Some(y)) if h > lift + 0.2 => {
                    format!(
                        "body {w} {:.3} {l} at {x} {:.3} {z}",
                        h - lift,
                        y + lift * 0.5
                    )
                }
                _ => line.to_owned(),
            },
            ["torque", rpm, pull] => match number(pull) {
                Some(pull) => format!("torque {rpm} {:.1}", pull * power.clamp(0.5, 1.0)),
                None => line.to_owned(),
            },
            ["front" | "rear", ..] => words
                .iter()
                .map(|word| {
                    scaled(word, "spring", spring)
                        .or_else(|| scaled(word, "bump", damper))
                        .or_else(|| scaled(word, "rebound", damper))
                        .or_else(|| scaled(word, "travel", travel))
                        .unwrap_or_else(|| (*word).to_owned())
                })
                .collect::<Vec<_>>()
                .join(" "),
            ["tyre", ..] if line.contains("relaxation=") => {
                format!("{line} tread={},{}", tread.0, tread.1)
            }
            _ => line.to_owned(),
        };
        out.push_str(&written);
        out.push('\n');
    }
    out
}

pub fn assemble(
    file: &mut (impl std::io::Read + std::io::Seek),
    build: Option<&str>,
    power: f32,
) -> Result<Assembled, String> {
    let said = |error: std::io::Error| error.to_string();
    let list = pack::read_list(file).map_err(said)?;
    let mut texts = Vec::new();
    for entry in list.iter().filter(|entry| entry.kind == "text") {
        let bytes = pack::read(file, entry).map_err(said)?;
        let text = String::from_utf8(bytes).map_err(|_| format!("{} is not text", entry.name))?;
        texts.push((entry.name.clone(), text));
    }
    let text = |name: &str| {
        texts
            .iter()
            .find(|(named, _)| named == name)
            .map(|(_, text)| text)
    };
    let (actor_name, actor) = texts
        .iter()
        .find(|(name, _)| name.ends_with(".actor"))
        .ok_or("the pack has no actor file")?;
    let actor = ActorFile::parse(actor).map_err(|error| format!("{actor_name}: {error}"))?;
    let kit_name = resolve(
        actor_name,
        actor.kit.as_ref().ok_or("its actor names no kit")?,
    );
    let kit = text(&kit_name).ok_or_else(|| format!("{kit_name} is not in the pack"))?;
    let kit = Kit::parse(kit).map_err(|error| format!("{kit_name}: {error}"))?;
    // `all` is every piece of it, as it is sold: for a truck that parts are to be tried on,
    // where it stands, with nothing more read.
    let all = build == Some("all");
    let build = kit.built(build.filter(|_| !all).unwrap_or("stock"))?;
    let shown = kit.shown(&build);

    // Its pieces, one after another: each a scene of its own, joined to those before it.
    let mut truck: Option<LoadedScene> = None;
    let mut pieces = String::new();
    for entry in list.iter().filter(|entry| entry.kind == "piece") {
        if !all && !shown.contains(entry.name.as_str()) {
            continue;
        }
        let bytes = pack::read(file, entry).map_err(said)?;
        let piece = solstice_asset::file::decode(&bytes)
            .map_err(|error| format!("{}: {error}", entry.name))?;
        let count = piece.scene.instances.len();
        let first = match &mut truck {
            None => {
                truck = Some(piece);
                0
            }
            Some(truck) => {
                let joined = truck.join(piece).map_err(|error| error.to_string())?;
                joined.instances
            }
        };
        pieces.push_str(&format!("{} {first} {count}\n", entry.name));
    }
    let scene = truck.ok_or("none of the truck's pieces are in its pack")?;
    let actor = format!("{FOLDER}{actor_name}");
    let mut files: Vec<GameFile> = texts
        .iter()
        .map(|(name, text)| GameFile {
            name: format!("{FOLDER}{name}"),
            // Its numbers are as it is fitted.
            text: if name.ends_with(".vehicle") {
                tuned(text, &kit, &build, power)
            } else {
                text.clone()
            },
        })
        .collect();
    files.push(GameFile {
        name: pieces_file(&actor),
        text: pieces,
    });
    files.push(GameFile {
        name: build_file(&actor),
        text: build.to_text(),
    });
    Ok(Assembled {
        scene,
        files,
        actor,
    })
}

/// The gameplay file that lists the trucks stood in a showroom to be looked at, one shown at
/// a time: a line a truck, `ACTOR_FILE FIRST`, its actor file among the scene's and the
/// first of its instances (`shared::show_trucks`).
pub const SHOWN: &str = "showroom.trucks";

/// The trucks stood in a scene to be looked at, in their order: each with its kit, its
/// pieces, and what it is wearing.
pub fn displayed(files: &[GameFile]) -> Vec<Fitted> {
    let Some(list) = files.iter().find(|file| file.name == SHOWN) else {
        return Vec::new();
    };
    let mut trucks = Vec::new();
    for line in list.text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let [actor, first] = words[..] else { continue };
        let Ok(first) = first.parse::<u32>() else {
            continue;
        };
        match Fitted::of(files, actor, first) {
            Ok(fitted) => trucks.push(fitted),
            Err(error) => log::warn!("{actor}: {error}"),
        }
    }
    trucks
}

/// Which of an actor's instances draw each named piece, counted from its first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pieces(pub Vec<(String, u32, u32)>);

impl Pieces {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut pieces = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let [name, first, count] = words[..] else {
                if words.is_empty() {
                    continue;
                }
                return Err(format!("line {}: expected NAME FIRST COUNT", number + 1));
            };
            let read = |word: &str| {
                word.parse::<u32>()
                    .map_err(|_| format!("line {}: `{word}` is not a count", number + 1))
            };
            pieces.push((name.to_owned(), read(first)?, read(count)?));
        }
        Ok(Self(pieces))
    }
}

/// A truck in a scene, with its kit: what it is made of, where its instances are, and what
/// is fitted to it.
pub struct Fitted {
    pub kit: Kit,
    pieces: Pieces,
    /// The first of the truck's instances.
    first: u32,
    build: Build,
    /// Whether the renderer has yet to be told of the build.
    changed: bool,
    /// How far open each of the kit's hinges is, of all the way, how far it is drawn, and
    /// how far it is to be: a door takes a moment to swing.
    open: Vec<f32>,
    swung: Vec<f32>,
    aim: Vec<f32>,
}

/// How long a door takes to swing all the way, in seconds.
const SWINGS: f32 = 0.45;

impl Fitted {
    fn new(kit: Kit, pieces: Pieces, first: u32) -> Self {
        Self {
            build: kit.stock.clone(),
            open: vec![0.0; kit.hinges.len()],
            swung: vec![0.0; kit.hinges.len()],
            aim: vec![0.0; kit.hinges.len()],
            kit,
            pieces,
            first,
            changed: true,
        }
    }

    /// Opens what swings, so far of all the way: a door or a tailgate by its piece's name,
    /// or `all` of them.
    pub fn swing(&mut self, piece: &str, share: f32) -> Result<(), String> {
        let mut found = false;
        for (hinge, aim) in self.kit.hinges.iter().zip(&mut self.aim) {
            if piece == "all" || hinge.piece == piece {
                *aim = share.clamp(0.0, 1.0);
                found = true;
            }
        }
        if found {
            return Ok(());
        }
        let known: Vec<&str> = self.kit.hinges.iter().map(|h| h.piece.as_str()).collect();
        Err(format!(
            "nothing swings called `{piece}`: {}",
            known.join(", ")
        ))
    }

    /// Time passes: what is swinging swings on. With no time, it is where it is to be.
    pub fn advance(&mut self, seconds: f32) {
        for (open, aim) in self.open.iter_mut().zip(&self.aim) {
            let step = if seconds > 0.0 { seconds / SWINGS } else { 1.0 };
            *open += (*aim - *open).clamp(-step, step);
        }
    }

    /// How far open a piece is, of all the way, if it swings.
    pub fn open(&self, piece: &str) -> Option<f32> {
        let found = self
            .kit
            .hinges
            .iter()
            .position(|hinge| hinge.piece == piece);
        found.map(|index| self.open[index])
    }

    /// What has swung since this was last asked: each instance that moved, and what to move
    /// it by, in the truck's own space. A door's own instances, and those of what rides on it.
    pub fn swings(&mut self) -> Vec<(u32, Mat4)> {
        let mut moves = Vec::new();
        for (index, hinge) in self.kit.hinges.iter().enumerate() {
            if self.open[index] == self.swung[index] {
                continue;
            }
            let by = hinge.swung(self.open[index]) * hinge.swung(self.swung[index]).inverse();
            let rides = |name: &str| {
                name == hinge.piece
                    || self
                        .kit
                        .rides
                        .iter()
                        .any(|(piece, on)| piece == name && *on == hinge.piece)
            };
            for (name, first, count) in &self.pieces.0 {
                if rides(name) {
                    moves.extend((*first..first + count).map(|i| (self.first + i, by)));
                }
            }
            self.swung[index] = self.open[index];
        }
        moves
    }

    /// The first actor of a scene's game whose file has a kit, as it is sold. Nothing if no
    /// actor has one; a mistake in its files is an error.
    pub fn find(files: &[GameFile]) -> Result<Option<Self>, String> {
        let text = |name: &str| files.iter().find(|file| file.name == name).map(|f| &f.text);
        let Some(game) = text(GAME_FILE) else {
            return Ok(None);
        };
        for line in game.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some((&"actor", rest)) = words.split_first() else {
                continue;
            };
            let Ok(placed) = ActorLine::parse(rest) else {
                continue;
            };
            let Some(Ok(actor)) = text(&placed.file).map(|text| ActorFile::parse(text)) else {
                continue;
            };
            let (Some(_), Some(instances)) = (&actor.kit, &placed.instances) else {
                continue;
            };
            return Self::of(files, &placed.file, instances.start).map(Some);
        }
        Ok(None)
    }

    /// A truck with a kit, from the scene's gameplay files: its actor file's name among
    /// them, and the first of its instances.
    pub fn of(files: &[GameFile], actor_file: &str, first: u32) -> Result<Self, String> {
        let text = |name: &str| {
            let found = files.iter().find(|file| file.name == name);
            found
                .map(|file| &file.text)
                .ok_or_else(|| format!("{name} is not in the scene"))
        };
        let actor = ActorFile::parse(text(actor_file)?).map_err(|error| error.to_string())?;
        let name = resolve(actor_file, actor.kit.as_ref().ok_or("it has no kit")?);
        let kit = Kit::parse(text(&name)?).map_err(|error| format!("{name}: {error}"))?;
        let name = pieces_file(actor_file);
        let pieces = Pieces::parse(text(&name)?).map_err(|error| format!("{name}: {error}"))?;
        let mut fitted = Self::new(kit, pieces, first);
        // A truck put together from its pack is wearing what it was put together in.
        if let Ok(build) = text(&build_file(actor_file)) {
            fitted.build = Build::parse(build)?;
        }
        Ok(fitted)
    }

    /// Every instance of it.
    pub fn instances(&self) -> Vec<u32> {
        let each = self.pieces.0.iter();
        each.flat_map(|(_, first, count)| self.first + first..self.first + first + count)
            .collect()
    }

    /// It is to be drawn again as it is fitted, whether or not that has changed: it was
    /// hidden, and is shown.
    pub fn redraw(&mut self) {
        self.changed = true;
    }

    pub fn build(&self) -> &Build {
        &self.build
    }

    /// Fits a build: `stock` or `showroom` for what the kit calls those, or choices to put
    /// over what it wears as it is sold.
    pub fn fit(&mut self, text: &str) -> Result<(), String> {
        let build = self.kit.built(text)?;
        self.changed |= build != self.build;
        self.build = build;
        Ok(())
    }

    /// Which instances are hidden under the build, each with whether it is.
    pub fn hidden(&self) -> Vec<(u32, bool)> {
        let shown = self.kit.shown(&self.build);
        let mut hidden = Vec::new();
        for (name, first, count) in &self.pieces.0 {
            for instance in *first..first + count {
                hidden.push((self.first + instance, !shown.contains(name.as_str())));
            }
        }
        hidden
    }

    /// Tells the renderer what to draw of the truck, if the build has changed.
    pub fn show(&mut self, renderer: &mut Renderer) {
        if !std::mem::take(&mut self.changed) {
            return;
        }
        for (instance, hidden) in self.hidden() {
            renderer.set_instance_hidden(instance, hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gold_is_told_of_the_holds_behind_doors_and_where_their_doorways_are() {
        let kit = Kit::parse(KIT).unwrap();
        let (holds, doors) = kit.holds();
        // The seat, behind its door: not the bed, which is open and is the truck's tray.
        assert_eq!(holds.len(), 1);
        assert_eq!(doors.len(), 1);
        assert_eq!(doors[0].piece, "door_FL");
        assert!(!doors[0].open);
        let seat = &holds[0];
        assert_eq!(seat.floor, Vec3::new(-0.4, 0.8, -0.2));
        // Its doorway is in the truck's side, beside the seat.
        assert_eq!(seat.ways, [(0, Vec3::new(-0.9, 1.0, -0.2))]);
        // A boot's is in its back, where its door's hinge is.
        let boot = Kit::parse(
            "base cab\nhinge door_back 0.76 0 2.49 up 100\n\
             hold boot 1.5 0.45 1.4 at 0 1.1 1.7 door door_back\n",
        )
        .unwrap();
        assert_eq!(boot.holds().0[0].ways, [(0, Vec3::new(0.0, 1.1, 2.49))]);
    }

    #[test]
    fn a_door_swings_on_its_hinge_and_takes_what_rides_on_it() {
        let kit = Kit::parse(
            "base cab\nchoice spare fitted acc_spare\nhinge door_back 0.8 0 2.5 up 90\n\
             hinge door_FL -0.9 0 -0.6 up -70\nrides acc_spare door_back\n",
        )
        .unwrap();
        let pieces = Pieces::parse("cab 0 1\ndoor_back 1 2\nacc_spare 3 1\ndoor_FL 4 1\n").unwrap();
        let mut fitted = Fitted::new(kit, pieces, 10);
        assert!(fitted.swings().is_empty());
        fitted.swing("door_back", 1.0).unwrap();
        // It takes a moment: half way there, it has moved, and then it is open.
        fitted.advance(SWINGS * 0.5);
        assert!((fitted.open("door_back").unwrap() - 0.5).abs() < 1e-5);
        assert_eq!(fitted.swings().len(), 3);
        fitted.swing("door_back", 0.0).unwrap();
        fitted.advance(0.0);
        assert_eq!(fitted.swings().len(), 3);
        fitted.swing("door_back", 1.0).unwrap();
        fitted.advance(0.0);
        let moves = fitted.swings();
        // The door's two instances and the spare on it: not the cab, not the other door.
        let moved: Vec<u32> = moves.iter().map(|(instance, _)| *instance).collect();
        assert_eq!(moved, [11, 12, 13]);
        // A point on the hinge stays; one a metre to its left comes a metre back.
        let by = moves[0].1;
        assert!(
            by.transform_point3(Vec3::new(0.8, 1.0, 2.5))
                .distance(Vec3::new(0.8, 1.0, 2.5))
                < 1e-5
        );
        assert!(
            by.transform_point3(Vec3::new(-0.2, 1.0, 2.5))
                .distance(Vec3::new(0.8, 1.0, 3.5))
                < 1e-5
        );
        // Told once: and shut again it comes back to where it was.
        assert!(fitted.swings().is_empty());
        fitted.swing("all", 0.0).unwrap();
        fitted.advance(0.0);
        let back = fitted.swings()[0].1;
        assert!((back * by).abs_diff_eq(Mat4::IDENTITY, 1e-5));
        assert_eq!(fitted.open("door_FL"), Some(0.0));
        assert!(fitted
            .swing("bonnet", 1.0)
            .unwrap_err()
            .contains("door_back, door_FL"));
    }

    const KIT: &str = "# A truck.\n\
        base cab\nbase bumper_front\nbase plate_front\n\
        choice bullbar tube acc_bullbar_tube\nchoice bullbar plate acc_bullbar_plate\n\
        choice tyres road tyre_road_FL tyre_road_FR\nchoice tyres mud tyre_mud_FL tyre_mud_FR\n\
        choice suspension stock susp_stock_F\nchoice suspension lift susp_lift_F\n\
        stock tyres road\nstock suspension stock\n\
        showroom tyres mud\nshowroom bullbar plate\nshowroom suspension lift\n\
        replaces bullbar plate bumper_front plate_front\n\
        lift stock 0\nlift lift 0.05\n\
        hinge door_FL -0.9 0 -0.6 up -70\nhinge tailgate 0 0.6 2.8 across 90\n\
        hold passenger_seat 0.5 0.4 0.45 at -0.4 1 -0.2 door door_FL\n\
        hold bed 1.6 0.35 1.4 at 0 1.3 2\n";

    #[test]
    fn a_kit_is_read_and_a_bare_truck_wears_only_what_it_is_sold_with() {
        let kit = Kit::parse(KIT).unwrap();
        assert_eq!(kit.groups().len(), 3);
        let shown = kit.shown(&kit.stock);
        assert!(shown.contains("cab") && shown.contains("bumper_front"));
        assert!(shown.contains("tyre_road_FL") && shown.contains("susp_stock_F"));
        // Nothing bolted on, and none of the tyres it is not on.
        assert!(!shown.contains("acc_bullbar_tube") && !shown.contains("tyre_mud_FL"));
        assert_eq!(kit.lift(&kit.stock), 0.0);
        assert_eq!(kit.hinges.len(), 2);
        assert!(kit.hinges[0].upright && !kit.hinges[1].upright);
        assert_eq!(kit.holds[0].doors, ["door_FL"]);
        assert!(kit.holds[1].doors.is_empty());
    }

    #[test]
    fn what_is_fitted_changes_a_trucks_numbers() {
        let kit = Kit::parse(KIT).unwrap();
        let vehicle = "centre 0 0.76 -0.14
body 1.74 1.28 3.99 at 0 1.20 0.05
            front spring=30000 bump=3000 rebound=6000 travel=0.10,0.08
            tyre load=3625 sensitivity=0.1 relaxation=0.5
torque 1000 100
";
        // As it is sold: road tyres, and everything else as it was.
        let stock = tuned(vehicle, &kit, &kit.stock, 1.0);
        assert!(stock.contains("centre 0 0.760 -0.14") && stock.contains("torque 1000 100.0"));
        assert!(stock.contains("tread=1.06,0.8") && stock.contains("travel=0.1000,0.0800"));
        // Built up, on mud tyres and a lift, with its oil overdue: it stands higher, its
        // belly is further off the ground, its wheels travel further, and it pulls less.
        let built = tuned(vehicle, &kit, &kit.showroom, 0.9);
        assert!(built.contains("centre 0 0.810 -0.14"), "{built}");
        assert!(
            built.contains("body 1.74 1.230 3.99 at 0 1.225 0.05"),
            "{built}"
        );
        assert!(
            built.contains("travel=0.1250,0.1000") && built.contains("bump=3300.0000"),
            "{built}"
        );
        assert!(built.contains("tread=0.93,1.18") && built.contains("torque 1000 90.0"));
    }

    #[test]
    fn fitting_one_thing_takes_off_what_it_replaces() {
        let kit = Kit::parse(KIT).unwrap();
        let shown = kit.shown(&kit.showroom);
        // A winch bar is the bumper: the bumper and its plate come off.
        assert!(shown.contains("acc_bullbar_plate"));
        assert!(!shown.contains("bumper_front") && !shown.contains("plate_front"));
        assert!(shown.contains("tyre_mud_FL") && !shown.contains("tyre_road_FL"));
        assert_eq!(kit.lift(&kit.showroom), 0.05);
        // A tube bar goes on in front of the bumper, which stays.
        let build = kit.stock.with(&Build::parse("bullbar=tube").unwrap());
        assert!(kit.shown(&build).contains("bumper_front"));
    }

    #[test]
    fn a_build_names_only_what_the_truck_has() {
        let kit = Kit::parse(KIT).unwrap();
        assert!(kit.check(&Build::parse("tyres=mud").unwrap()).is_ok());
        assert!(kit
            .check(&Build::parse("tyres=slick").unwrap())
            .unwrap_err()
            .contains("road, mud"));
        assert!(kit.check(&Build::parse("wings=big").unwrap()).is_err());
        assert!(Build::parse("tyres").is_err());
        assert!(Kit::parse("choice a b c\n").is_err());
        assert!(Kit::parse("base cab\nnonsense 1\n").is_err());
    }

    #[test]
    fn the_instances_of_what_is_not_fitted_are_hidden() {
        let pieces = Pieces::parse(
            "cab 0 2\nbumper_front 2 1\nacc_bullbar_tube 3 1\ntyre_road_FL 4 1\ntyre_mud_FL 5 1\n",
        )
        .unwrap();
        let kit = Kit::parse(KIT).unwrap();
        let mut fitted = Fitted::new(kit, pieces, 100);
        let hidden = |fitted: &Fitted| -> Vec<u32> {
            let all = fitted.hidden();
            all.iter().filter(|(_, h)| *h).map(|(i, _)| *i).collect()
        };
        assert_eq!(hidden(&fitted), [103, 105]);
        fitted.fit("bullbar=tube,tyres=mud").unwrap();
        assert_eq!(hidden(&fitted), [104]);
        // `none` takes off what it is sold with.
        fitted.fit("tyres=none").unwrap();
        assert_eq!(hidden(&fitted), [103, 104, 105]);
        assert!(fitted.fit("tyres=slick").is_err());
        fitted.fit("showroom").unwrap();
        assert_eq!(fitted.build().fitted("bullbar"), Some("plate"));
    }
}
