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

use glam::{Mat4, Quat, Vec3};
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

/// A piece that runs between two places, and turns to go on reaching from one to the other
/// as its axle moves: a suspension link, held on the truck and reaching to the axle; the
/// body of a shock absorber, the same; its shaft, held on the axle and reaching up to the
/// truck.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub piece: String,
    /// Whether it is the rear axle's, or the front's.
    pub rear: bool,
    /// Where it is held, and where it reaches to, in the truck's own space with the truck
    /// at rest; and of each, whether it is on the axle, and moves with it.
    pub held: (Vec3, bool),
    pub reaches: (Vec3, bool),
}

/// How an axle lies when the wheels at its two ends have moved so far up from where they
/// rest: from where it lies at rest, in the truck's own space. It goes up with them, and
/// leans as one is higher than the other.
pub fn axle_lies(hubs: (Vec3, Vec3), travel: (f32, f32)) -> Mat4 {
    let middle = (hubs.0 + hubs.1) * 0.5;
    let across = hubs.1.x - hubs.0.x;
    if across.abs() < 1.0e-3 {
        return Mat4::IDENTITY;
    }
    let lean = ((travel.1 - travel.0) / across).clamp(-0.9, 0.9).asin();
    Mat4::from_translation(middle + Vec3::Y * (travel.0 + travel.1) * 0.5)
        * Mat4::from_rotation_z(lean)
        * Mat4::from_translation(-middle)
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
    /// What is on an axle and goes where it goes, up and down and leaning, as its wheels
    /// move: the piece, and whether the axle is the rear one. And what runs between the
    /// truck and an axle. A truck whose kit names none is drawn with its axles still.
    pub axles: Vec<(String, bool)>,
    pub links: Vec<Link>,
    /// For a kit that is not a truck's own but what another truck is stood on (a monster
    /// truck's frame, axles and wheels): the wheelbase it was made at, how high a truck's
    /// own chassis sits on it, and how long its frame's middle is, in metres.
    pub fits: Option<[f32; 3]>,
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
                ("fits", [wheelbase, sits, middle]) => {
                    kit.fits = Some([number_of(wheelbase)?, number_of(sits)?, number_of(middle)?]);
                }
                ("axle", [piece, end @ ("front" | "rear")]) => {
                    kit.axles.push(((*piece).to_owned(), *end == "rear"));
                }
                (
                    "link",
                    [piece, end @ ("front" | "rear"), held @ ("frame" | "axle" | "truck"), ax, ay, az, reaches @ ("frame" | "axle" | "truck"), bx, by, bz],
                ) => kit.links.push(Link {
                    piece: (*piece).to_owned(),
                    rear: *end == "rear",
                    held: (
                        Vec3::new(number_of(ax)?, number_of(ay)?, number_of(az)?),
                        *held == "axle",
                    ),
                    reaches: (
                        Vec3::new(number_of(bx)?, number_of(by)?, number_of(bz)?),
                        *reaches == "axle",
                    ),
                }),
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

/// The name of the gameplay file that says where each piece of a truck is: a line a piece,
/// `NAME LOW_X LOW_Y LOW_Z HIGH_X HIGH_Y HIGH_Z`, the least and the greatest corners of a
/// box round it in the truck's own space, in metres. What a blow lands on is told from it
/// (`damage.rs`). It is measured from the pieces themselves ([`measured`]) as a truck is put
/// together from its pack.
pub fn boxes_file(actor_file: &str) -> String {
    format!("{actor_file}.boxes")
}

/// The box round one of a scene's meshes, in the mesh's own space: its least corner and its
/// greatest. Every point of every level of detail of it is looked at.
pub fn mesh_box(scene: &LoadedScene, mesh: u32) -> Option<(Vec3, Vec3)> {
    use solstice_asset::gpu::{PackedVertex, NODE_LEAF, NODE_PAGE_MASK};
    let paged = &scene.scene;
    let info = paged.meshes.get(mesh as usize)?;
    let mut found: Option<(Vec3, Vec3)> = None;
    let mut nodes = vec![info.root_node?];
    while let Some(node) = nodes.pop() {
        let node = paged.nodes.get(node as usize)?;
        if node.is_leaf & NODE_LEAF == 0 {
            nodes.extend(node.first..node.first + node.count);
            continue;
        }
        let page = scene.page(node.is_leaf & NODE_PAGE_MASK);
        for cluster in node.first..node.first + node.count {
            let cluster = paged.clusters.get(cluster as usize)?;
            // A cluster's vertices are so many words from the start of its page.
            let from = cluster.vertex_offset as usize * 4;
            let size = std::mem::size_of::<PackedVertex>();
            let bytes = page.get(from..from + cluster.vertex_count() as usize * size)?;
            for vertex in bytes.chunks_exact(size) {
                let word = |at: usize| f32::from(u16::from_le_bytes([vertex[at], vertex[at + 1]]));
                let point =
                    info.quant_origin + Vec3::new(word(0), word(2), word(4)) * info.quant_step;
                let (low, high) = found.get_or_insert((point, point));
                (*low, *high) = (low.min(point), high.max(point));
            }
        }
    }
    found
}

/// The box round some of a scene's instances, as `within` has them: where each is, in the
/// space the box is wanted in (a truck's own, for its pieces).
pub fn measured(
    scene: &LoadedScene,
    instances: std::ops::Range<u32>,
    within: impl Fn(Mat4) -> Mat4,
) -> Option<(Vec3, Vec3)> {
    let mut found: Option<(Vec3, Vec3)> = None;
    for instance in instances {
        let instance = scene.scene.instances.get(instance as usize)?;
        let Some((low, high)) = mesh_box(scene, instance.mesh) else {
            continue;
        };
        let placed = within(instance.transform);
        for corner in 0..8 {
            let pick = |bit: u32, low: f32, high: f32| if corner & bit == 0 { low } else { high };
            let point = placed.transform_point3(Vec3::new(
                pick(1, low.x, high.x),
                pick(2, low.y, high.y),
                pick(4, low.z, high.z),
            ));
            let (low, high) = found.get_or_insert((point, point));
            (*low, *high) = (low.min(point), high.max(point));
        }
    }
    found
}

/// A line of a truck's boxes: a piece, and the box round it.
fn box_line(name: &str, (low, high): (Vec3, Vec3)) -> String {
    format!(
        "{name} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}\n",
        low.x, low.y, low.z, high.x, high.y, high.z
    )
}

/// A truck's boxes, read from their file ([`boxes_file`]). A line that is not one is passed
/// over: the file is written by the game, and a truck with fewer boxes is still a truck.
fn read_boxes(text: &str) -> Vec<(String, Vec3, Vec3)> {
    let mut boxes = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let Some(name) = words.next() else { continue };
        let numbers: Vec<f32> = words.filter_map(|word| word.parse().ok()).collect();
        if let [lx, ly, lz, hx, hy, hz] = numbers[..] {
            boxes.push((
                name.to_owned(),
                Vec3::new(lx, ly, lz),
                Vec3::new(hx, hy, hz),
            ));
        }
    }
    boxes
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
    ("heavy", [1.12, 1.15, 1.0]),
    ("lift", [1.0, 1.1, 1.1]),
    ("long", [0.96, 1.15, 1.22]),
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

/// The kit of the truck that is in a pack, without anything of it being put together.
pub fn pack_kit(file: &mut (impl std::io::Read + std::io::Seek)) -> Result<Kit, String> {
    let said = |error: std::io::Error| error.to_string();
    let list = pack::read_list(file).map_err(said)?;
    let entry = list
        .iter()
        .find(|entry| entry.kind == "text" && entry.name.ends_with(".kit"))
        .ok_or("the pack has no kit")?;
    let bytes = pack::read(file, entry).map_err(said)?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{} is not text", entry.name))?;
    Kit::parse(&text).map_err(|error| format!("{}: {error}", entry.name))
}

pub fn assemble(
    file: &mut (impl std::io::Read + std::io::Seek),
    build: Option<&str>,
    power: f32,
) -> Result<Assembled, String> {
    assemble_but(file, build, power, &[])
}

/// What a truck stood on a monster kit leaves behind: its own axles, springs and wheels.
const UNDER: [&str; 4] = ["axle_", "susp_", "tyre_", "rim_"];

/// The pack of what a monster truck stands on, by its name (`tools/blender/monster/build.py`).
pub const MONSTER: &str = "monster";

/// A build that is asked for with this before it is of a monster truck: `monster:stock` is
/// the truck as it is sold, stood on the monster kit.
pub const MONSTER_BUILD: &str = "monster:";

/// Puts a truck together, as [`assemble`] does or, for a build that begins
/// [`MONSTER_BUILD`], as [`assemble_monster`] does. `pack` gives a truck's pack by its
/// name, all of it.
pub fn put_together(
    pack: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    truck: &str,
    build: Option<&str>,
    power: f32,
) -> Result<Assembled, String> {
    let named = |error: String| format!("{truck}: {error}");
    let mut own = std::io::Cursor::new(pack(truck)?);
    match build.and_then(|build| build.strip_prefix(MONSTER_BUILD)) {
        Some(build) => {
            let mut under = std::io::Cursor::new(pack(MONSTER)?);
            assemble_monster(&mut own, &mut under, Some(build), power).map_err(named)
        }
        None => assemble(&mut own, build, power).map_err(named),
    }
}

/// A number said in a line of a truck's numbers as `KEY=NUMBER` (or `KEY=NUMBER,NUMBER`:
/// the first).
fn said(line: &str, key: &str) -> Option<f32> {
    let words = line.split_whitespace();
    let found = words
        .filter_map(|word| word.split_once('='))
        .find(|(named, _)| *named == key);
    found.and_then(|(_, value)| value.split(',').next()?.parse().ok())
}

/// A line of numbers with one of them raised: the word `at` words along is that much more.
fn raised(line: &str, at: usize, by: f32) -> String {
    let words = line.split_whitespace().enumerate();
    let each = words.map(|(index, word)| match word.parse::<f32>() {
        Ok(number) if index == at => format!("{:.3}", number + by),
        _ => word.to_owned(),
    });
    each.collect::<Vec<_>>().join(" ")
}

/// Where a piece that runs from `held` to `reaches` is put so that it runs from `held_now`
/// to `reaches_now`: moved, turned, and drawn out along its length to reach.
fn refit(held: Vec3, reaches: Vec3, held_now: Vec3, reaches_now: Vec3) -> Mat4 {
    let (was, now) = (reaches - held, reaches_now - held_now);
    let (Some(along), Some(towards)) = (was.try_normalize(), now.try_normalize()) else {
        return Mat4::from_translation(held_now - held);
    };
    let grow = now.length() / was.length() - 1.0;
    let drawn = glam::Mat3::from_cols(
        Vec3::X + along * (along.x * grow),
        Vec3::Y + along * (along.y * grow),
        Vec3::Z + along * (along.z * grow),
    );
    Mat4::from_translation(held_now)
        * Mat4::from_quat(Quat::from_rotation_arc(along, towards))
        * Mat4::from_mat3(drawn)
        * Mat4::from_translation(-held)
}

/// Puts a monster truck together: a truck from its pack (`file`), as [`assemble`] would
/// but for its axles, its springs and its wheels, stood on the monster kit from its pack
/// (`under`). Every other piece of the truck is kept, its own chassis among them and
/// whatever of its kit its build has fitted, and lifted so that its chassis sits on the
/// kit's frame. The frame's ends go where the truck's axles are, and its middle and its
/// driveshafts are drawn out to reach them: so one kit fits any truck. The truck's numbers
/// are the kit's, with the truck's wheelbase and body and a weight and springs for it.
///
/// What comes of it is a truck like any other: one scene, and the files that say what it
/// is, by the truck's own names.
pub fn assemble_monster(
    file: &mut (impl std::io::Read + std::io::Seek),
    under: &mut (impl std::io::Read + std::io::Seek),
    build: Option<&str>,
    power: f32,
) -> Result<Assembled, String> {
    let mut truck = assemble_but(file, build, power, &UNDER)?;
    let kit = assemble_but(under, Some("stock"), 1.0, &[])?;
    let text = |files: &[GameFile], ending: &str| {
        let found = files.iter().find(|file| file.name.ends_with(ending));
        found
            .map(|file| file.text.clone())
            .ok_or_else(|| format!("it has no {ending} file"))
    };
    let on = Kit::parse(&text(&kit.files, ".kit")?)?;
    let [made_at, sits, middle] = on
        .fits
        .ok_or("the kit it is to stand on does not say what it fits")?;
    let (numbers, under_numbers) = (
        text(&truck.files, ".vehicle")?,
        text(&kit.files, ".vehicle")?,
    );
    let line_of = |text: &str, first: &str| {
        let lines = text.lines();
        let mut found = lines.filter(|line| line.split_whitespace().next() == Some(first));
        found.next().map(str::to_owned)
    };
    let axle_at = |text: &str, end: &str| {
        let lines = text
            .lines()
            .filter(|line| line.split_whitespace().next() == Some(end));
        lines.filter_map(|line| said(line, "at")).next()
    };
    let (Some(front), Some(rear)) = (axle_at(&numbers, "front"), axle_at(&numbers, "rear")) else {
        return Err("its numbers do not say where its axles are".to_owned());
    };
    let (Some(front_made), Some(rear_made)) = (
        axle_at(&under_numbers, "front"),
        axle_at(&under_numbers, "rear"),
    ) else {
        return Err("the kit's numbers do not say where its axles are".to_owned());
    };
    let _ = made_at;
    // How far out each end of the kit goes, to be where the truck's axle is: -z is ahead.
    let shift = |rear_end: bool| {
        if rear_end {
            Vec3::Z * (rear - rear_made)
        } else {
            Vec3::NEG_Z * (front - front_made)
        }
    };
    // Its own chassis sits on the frame: it is lifted by as much as that takes.
    let boxes = read_boxes(&text(&truck.files, ".boxes").unwrap_or_default());
    let chassis = boxes.iter().find(|(name, ..)| name == "chassis");
    let low = match chassis {
        Some((_, low, _)) => low.y,
        None => {
            let each = boxes.iter().filter(|(name, ..)| !name.starts_with("acc_"));
            each.map(|(_, low, _)| low.y).fold(f32::MAX, f32::min)
        }
    };
    if low == f32::MAX {
        return Err(
            "it has not been measured: its pack has no pieces to stand on the kit".to_owned(),
        );
    }
    let lift = sits - low;
    let up = Mat4::from_translation(Vec3::Y * lift);
    for instance in &mut truck.scene.scene.instances {
        instance.transform = up * instance.transform;
    }

    // ---- The kit goes under it, each piece of it where this truck has it.
    let pieces = Pieces::parse(&text(&kit.files, ".pieces")?)?;
    let joined = truck
        .scene
        .join(kit.scene)
        .map_err(|error| error.to_string())?;
    // Which end of the truck a piece is at, by the end of its name: `_FL`, `_front`, `_R`.
    let end_of = |name: &str| {
        let ending = name.rsplit('_').next().unwrap_or("");
        match ending {
            "front" => Some(false),
            "rear" => Some(true),
            _ if ending.starts_with('F') => Some(false),
            _ if ending.starts_with('R') => Some(true),
            _ => None,
        }
    };
    let drawn_out = ((middle + (front - front_made) + (rear - rear_made)) / middle).max(0.05);
    // The middle of the frame is not in the middle of a truck whose axles moved unevenly.
    let centre = Vec3::Z * ((rear - rear_made) - (front - front_made)) * 0.5;
    let mut placed: Vec<(String, Mat4)> = pieces
        .0
        .iter()
        .map(|(name, ..)| {
            let place = match end_of(name) {
                Some(rear_end) => Mat4::from_translation(shift(rear_end)),
                None if name.ends_with("_M") => {
                    Mat4::from_translation(centre)
                        * Mat4::from_scale(Vec3::new(1.0, 1.0, drawn_out))
                }
                None => Mat4::from_translation(centre),
            };
            (name.clone(), place)
        })
        .collect();
    // What runs between two places goes on reaching both: a driveshaft is held in the
    // middle of the truck and reaches an axle that has moved.
    let mut links = Vec::new();
    for link in &on.links {
        links.push(link.clone());
    }
    let kit_text = text(&kit.files, ".kit")?;
    let mut link_lines = Vec::new();
    for line in kit_text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let ["link", piece, end, held_on, ax, ay, az, reaches_on, bx, by, bz] = words[..] else {
            continue;
        };
        let point = |x: &str, y: &str, z: &str| {
            let number = |word: &str| {
                word.parse::<f32>()
                    .map_err(|_| format!("`{word}` is not a number"))
            };
            Ok::<Vec3, String>(Vec3::new(number(x)?, number(y)?, number(z)?))
        };
        let (held, reaches) = (point(ax, ay, az)?, point(bx, by, bz)?);
        let moved = |at: Vec3, on: &str| {
            if on == "truck" {
                at + centre
            } else {
                at + shift(end == "rear")
            }
        };
        let (held_now, reaches_now) = (moved(held, held_on), moved(reaches, reaches_on));
        if let Some((_, place)) = placed.iter_mut().find(|(name, _)| name == piece) {
            *place = refit(held, reaches, held_now, reaches_now);
        }
        let word = |on: &str| if on == "axle" { "axle" } else { "frame" };
        let at = |at: Vec3| format!("{:.3} {:.3} {:.3}", at.x, at.y, at.z);
        link_lines.push(format!(
            "link {piece} {end} {} {} {} {}",
            word(held_on),
            at(held_now),
            word(reaches_on),
            at(reaches_now)
        ));
    }
    let _ = links;
    for (name, first, count) in &pieces.0 {
        let Some((_, place)) = placed.iter().find(|(named, _)| named == name) else {
            continue;
        };
        for id in joined.instances + first..joined.instances + first + count {
            let instance = &mut truck.scene.scene.instances[id as usize];
            instance.transform = *place * instance.transform;
        }
    }

    // ---- Its files: the truck's own, with what the kit changes of each.
    let left = |piece: &str| UNDER.iter().any(|start| piece.starts_with(start));
    let mut kit_lines = Vec::new();
    for line in text(&truck.files, ".kit")?.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words[..] {
            ["base", piece] if left(piece) => {}
            ["choice" | "stock" | "showroom", "suspension" | "tyres" | "rims", ..] => {}
            ["lift", ..] => {}
            // Its doors hang, and its gold is put, as much higher as it stands.
            ["hinge", ..] => kit_lines.push(raised(line, 3, lift)),
            ["hold", ..] => kit_lines.push(raised(line, 7, lift)),
            _ => kit_lines.push(line.to_owned()),
        }
    }
    for line in kit_text.lines() {
        let first = line.split_whitespace().next().unwrap_or("#");
        if !first.starts_with('#') && !matches!(first, "paint" | "fits" | "link") {
            kit_lines.push(line.to_owned());
        }
    }
    kit_lines.extend(link_lines);

    let mut piece_lines = text(&truck.files, ".pieces")?;
    for (name, first, count) in &pieces.0 {
        piece_lines.push_str(&format!("{name} {} {count}\n", joined.instances + first));
    }
    let mut box_lines = String::new();
    for (name, low, high) in &boxes {
        box_lines.push_str(&box_line(
            name,
            (*low + Vec3::Y * lift, *high + Vec3::Y * lift),
        ));
    }
    for (name, low, high) in read_boxes(&text(&kit.files, ".boxes").unwrap_or_default()) {
        let by = end_of(&name).map_or(centre, shift);
        box_lines.push_str(&box_line(&name, (low + by, high + by)));
    }
    // What it wears: what it wore, on the kit's springs, tyres and rims.
    let mut wears = Build::parse(&text(&truck.files, ".build")?)?;
    for group in ["suspension", "tyres", "rims"] {
        wears.set(group, on.stock.fitted(group));
    }

    // Its numbers: the kit's, with this truck on it.
    let mass = |text: &str| {
        line_of(text, "mass").and_then(|line| line.split_whitespace().nth(1)?.parse::<f32>().ok())
    };
    let (Some(own_mass), Some(kit_mass)) = (mass(&numbers), mass(&under_numbers)) else {
        return Err("its numbers do not say what it weighs".to_owned());
    };
    // The kit weighs what it weighs, and the truck on it its own weight less the axles,
    // the springs and the wheels it left behind.
    let share = (2600.0 + 0.65 * own_mass) / kit_mass;
    let longer = ((front + rear) / (front_made + rear_made)).powi(2);
    let scaled = |line: &str, keys: &[&str], by: f32| {
        let words = line.split_whitespace();
        let each = words.map(|word| match word.split_once('=') {
            Some((key, value)) if keys.contains(&key) => {
                let values = value.split(',').map(|value| match value.parse::<f32>() {
                    Ok(number) => format!("{:.0}", number * by),
                    Err(_) => value.to_owned(),
                });
                format!("{key}={}", values.collect::<Vec<_>>().join(","))
            }
            _ => word.to_owned(),
        });
        each.collect::<Vec<_>>().join(" ")
    };
    let at_axle = |line: &str, at: f32| {
        let words = line.split_whitespace();
        let each = words.map(|word| match word.split_once('=') {
            Some(("at", _)) => format!("at={at:.3}"),
            _ => word.to_owned(),
        });
        each.collect::<Vec<_>>().join(" ")
    };
    let mut vehicle = String::new();
    for line in under_numbers.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let written = match words[..] {
            ["mass", _] => format!("mass {:.0}", kit_mass * share),
            ["inertia", pitch, yaw, roll] => {
                let number = |word: &str, by: f32| word.parse::<f32>().map_or(0.0, |n| n * by);
                format!(
                    "inertia {:.0} {:.0} {:.0}",
                    number(pitch, share * longer),
                    number(yaw, share * longer),
                    number(roll, share)
                )
            }
            // Its body: the truck's own, as much higher as it stands, down to the kit's belly.
            ["body", _, high, _, "at", _, y, _] => {
                let belly =
                    y.parse::<f32>().unwrap_or(2.0) - high.parse::<f32>().unwrap_or(2.0) * 0.5;
                let own: Vec<f32> = line_numbers(&line_of(&numbers, "body").unwrap_or_default());
                match own[..] {
                    [wide, tall, long, _, y, z] => {
                        let top = y + tall * 0.5 + lift;
                        format!(
                            "body {wide:.2} {:.2} {long:.2} at 0 {:.2} {z:.2}",
                            top - belly,
                            (top + belly) * 0.5
                        )
                    }
                    _ => line.to_owned(),
                }
            }
            ["driver", ..] => line_of(&numbers, "driver").unwrap_or_else(|| line.to_owned()),
            ["front", ..] => scaled(
                &at_axle(line, front),
                &["spring", "bump", "rebound", "bar", "brake"],
                share,
            ),
            ["rear", ..] => scaled(
                &at_axle(line, rear),
                &["spring", "bump", "rebound", "bar", "brake"],
                share,
            ),
            ["tyre", ..] => scaled(line, &["load"], share),
            _ => line.to_owned(),
        };
        vehicle.push_str(&written);
        vehicle.push('\n');
    }
    // Its tray, and the places in its cab that gold can be put, are as much higher.
    for own in numbers.lines() {
        match own.split_whitespace().next() {
            Some("tray" | "hold") => {
                vehicle.push_str(&raised(own, 6, lift));
                vehicle.push('\n');
            }
            _ => {}
        }
    }

    let actor = truck.actor.clone();
    for file in &mut truck.files {
        if file.name.ends_with(".kit") {
            file.text = kit_lines.join("\n") + "\n";
        } else if file.name.ends_with(".vehicle") {
            file.text.clone_from(&vehicle);
        } else if file.name.ends_with(".pieces") {
            file.text.clone_from(&piece_lines);
        } else if file.name.ends_with(".boxes") {
            file.text.clone_from(&box_lines);
        } else if file.name.ends_with(".build") {
            file.text = wears.to_text();
        } else if file.name == actor {
            // Whoever drives it sits as much higher.
            let lines = file
                .text
                .lines()
                .map(|line| match line.split_whitespace().next() {
                    Some("camera") => raised(line, 2, lift),
                    _ => line.to_owned(),
                });
            file.text = lines.collect::<Vec<_>>().join("\n") + "\n";
        }
    }
    Ok(truck)
}

/// The numbers in a line, in their order: words that are not numbers are passed over.
fn line_numbers(line: &str) -> Vec<f32> {
    let words = line.split_whitespace();
    words.filter_map(|word| word.parse().ok()).collect()
}

/// As [`assemble`], but for the pieces whose names begin with any of `leave`, which are
/// left out as if the truck had none of them.
fn assemble_but(
    file: &mut (impl std::io::Read + std::io::Seek),
    build: Option<&str>,
    power: f32,
    leave: &[&str],
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
    let mut boxes = String::new();
    for entry in list.iter().filter(|entry| entry.kind == "piece") {
        if !all && !shown.contains(entry.name.as_str()) {
            continue;
        }
        if leave.iter().any(|start| entry.name.starts_with(start)) {
            continue;
        }
        let bytes = pack::read(file, entry).map_err(said)?;
        let piece = solstice_asset::file::decode(&bytes)
            .map_err(|error| format!("{}: {error}", entry.name))?;
        let count = piece.scene.instances.len();
        // Where it is on the truck: a piece is made in the truck's own space.
        if let Some(found) = measured(&piece, 0..count as u32, |placed| placed) {
            boxes.push_str(&box_line(&entry.name, found));
        }
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
    files.push(GameFile {
        name: boxes_file(&actor),
        text: boxes,
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
    /// How each axle is drawn to lie, the front and the rear, and how each of the kit's
    /// links is: from where they are at rest.
    rode: [Mat4; 2],
    linked: Vec<Mat4>,
    /// The box round each piece, in the truck's own space, where it is known
    /// ([`boxes_file`], [`Fitted::measure`]).
    boxes: Vec<(String, Vec3, Vec3)>,
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
            rode: [Mat4::IDENTITY; 2],
            linked: vec![Mat4::IDENTITY; kit.links.len()],
            kit,
            pieces,
            first,
            changed: true,
            boxes: Vec::new(),
        }
    }

    /// Every piece that is on the truck as it is fitted, each with the instances that draw
    /// it.
    pub fn fitted(&self) -> Vec<(&str, std::ops::Range<u32>)> {
        let shown = self.kit.shown(&self.build);
        let each = self.pieces.0.iter();
        each.filter(|(name, ..)| shown.contains(name.as_str()))
            .map(|(name, first, count)| {
                (
                    name.as_str(),
                    self.first + first..self.first + first + count,
                )
            })
            .collect()
    }

    /// The box round a piece, in the truck's own space, if it is known.
    pub fn piece_box(&self, piece: &str) -> Option<(Vec3, Vec3)> {
        let found = self.boxes.iter().find(|(name, ..)| name == piece);
        found.map(|(_, low, high)| (*low, *high))
    }

    /// Whether the truck's pieces have been measured.
    pub fn is_measured(&self) -> bool {
        !self.boxes.is_empty()
    }

    /// Measures every piece from the scene it is in: for a truck that is baked into its
    /// map, and has no boxes of its own. `stood` is where the truck stands in the scene.
    pub fn measure(&mut self, scene: &LoadedScene, stood: Mat4) {
        let to_truck = stood.inverse();
        self.boxes.clear();
        for (name, first, count) in &self.pieces.0 {
            let instances = self.first + first..self.first + first + count;
            if let Some((low, high)) = measured(scene, instances, |placed| to_truck * placed) {
                self.boxes.push((name.clone(), low, high));
            }
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

    /// What has moved with the axles since this was last asked: each instance that moved,
    /// and what to move it by, in the truck's own space. `hubs` is where the middle of each
    /// wheel now is, as its suspension has it: front left, front right, rear left, rear
    /// right; at rest each is its `radius` above the truck's origin.
    ///
    /// What is on an axle goes up and down with its two wheels and leans as they differ.
    /// What runs between the truck and an axle stays held where it is held and turns to go
    /// on reaching to where it reaches. Nothing, for a truck whose kit names neither.
    pub fn rides(&mut self, hubs: [Vec3; 4], radius: [f32; 4]) -> Vec<(u32, Mat4)> {
        if self.kit.axles.is_empty() && self.kit.links.is_empty() {
            return Vec::new();
        }
        let lies = [0, 2].map(|left| {
            let rest = |wheel: usize| Vec3::new(hubs[wheel].x, radius[wheel], hubs[wheel].z);
            let travel = |wheel: usize| hubs[wheel].y - radius[wheel];
            axle_lies(
                (rest(left), rest(left + 1)),
                (travel(left), travel(left + 1)),
            )
        });
        let mut moves = Vec::new();
        let first = self.first;
        let pieces = &self.pieces.0;
        let mut moved = |piece: &str, by: Mat4| {
            for (name, start, count) in pieces {
                if name == piece {
                    moves.extend((*start..start + count).map(|i| (first + i, by)));
                }
            }
        };
        for (end, lie) in lies.iter().enumerate() {
            if lie.abs_diff_eq(self.rode[end], 1.0e-5) {
                continue;
            }
            let by = *lie * self.rode[end].inverse();
            for (piece, rear) in &self.kit.axles {
                if usize::from(*rear) == end {
                    moved(piece, by);
                }
            }
        }
        for (link, was) in self.kit.links.iter().zip(&mut self.linked) {
            let axle = lies[usize::from(link.rear)];
            let now = |(at, on_axle): (Vec3, bool)| {
                if on_axle {
                    axle.transform_point3(at)
                } else {
                    at
                }
            };
            let (held, reaches) = (now(link.held), now(link.reaches));
            let rested = (link.reaches.0 - link.held.0).normalize_or_zero();
            let turned = (reaches - held).normalize_or_zero();
            if rested == Vec3::ZERO || turned == Vec3::ZERO {
                continue;
            }
            let lie = Mat4::from_translation(held)
                * Mat4::from_quat(Quat::from_rotation_arc(rested, turned))
                * Mat4::from_translation(-link.held.0);
            if !lie.abs_diff_eq(*was, 1.0e-5) {
                moved(&link.piece, lie * was.inverse());
                *was = lie;
            }
        }
        self.rode = lies;
        moves
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
        // And it comes with where each of its pieces is.
        if let Ok(boxes) = text(&boxes_file(actor_file)) {
            fitted.boxes = read_boxes(boxes);
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
        // (A monster truck's build is said with `monster:` before it: it is one already,
        // and what it wears on its frame is the rest.)
        let text = text.strip_prefix(MONSTER_BUILD).unwrap_or(text);
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
    fn a_trucks_boxes_are_read_and_only_what_is_fitted_is_on_it() {
        let text = format!(
            "{}{}not a box\n",
            box_line(
                "cab",
                (Vec3::new(-0.9, 0.5, -1.0), Vec3::new(0.9, 1.9, 2.5))
            ),
            box_line(
                "acc_bullbar_tube",
                (Vec3::new(-0.9, 0.4, -2.6), Vec3::new(0.9, 1.1, -2.3))
            ),
        );
        let boxes = read_boxes(&text);
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[1].1, Vec3::new(-0.9, 0.4, -2.6));
        let kit = Kit::parse(KIT).unwrap();
        let pieces = Pieces::parse("cab 0 2\nbumper_front 2 1\nacc_bullbar_tube 3 1\n").unwrap();
        let mut fitted = Fitted::new(kit, pieces, 100);
        assert!(!fitted.is_measured());
        fitted.boxes = boxes;
        assert!(fitted.is_measured());
        assert_eq!(
            fitted.piece_box("cab"),
            Some((Vec3::new(-0.9, 0.5, -1.0), Vec3::new(0.9, 1.9, 2.5)))
        );
        assert_eq!(fitted.piece_box("door_FL"), None);
        // As it is sold it has no bar: the cab and the bumper, with their instances.
        assert_eq!(
            fitted.fitted(),
            [("cab", 100..102), ("bumper_front", 102..103)]
        );
        fitted.fit("bullbar=tube").unwrap();
        assert_eq!(fitted.fitted().len(), 3);
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
            built.contains("travel=0.1100,0.0880") && built.contains("bump=3300.0000"),
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

    #[test]
    fn an_axle_rides_with_its_wheels_and_its_links_go_on_reaching_it() {
        let kit = Kit::parse(
            "base cab
base axle_front
base link
base rod
             axle axle_front front
             link link front frame -0.5 1 -1 axle -0.8 0.8 -2
             link rod front axle -0.9 0.9 -2 frame -0.7 2.2 -2
",
        )
        .unwrap();
        assert_eq!(kit.axles, [("axle_front".to_owned(), false)]);
        assert_eq!(kit.links.len(), 2);
        assert!(Kit::parse(
            "base cab
axle axle_front middle
"
        )
        .is_err());
        let pieces = Pieces::parse(
            "cab 0 1
axle_front 1 1
link 2 1
rod 3 1
",
        )
        .unwrap();
        let mut fitted = Fitted::new(kit, pieces, 10);
        // Its wheels, each 0.84 m in radius and 2.8 m apart: at rest nothing moves.
        let radius = [0.84; 4];
        let rest = [
            Vec3::new(-1.4, 0.84, -2.0),
            Vec3::new(1.4, 0.84, -2.0),
            Vec3::new(-1.4, 0.84, 2.0),
            Vec3::new(1.4, 0.84, 2.0),
        ];
        assert!(fitted.rides(rest, radius).is_empty());
        // The left front wheel goes up 0.4 m: the axle's left end goes up with it, its
        // right end stays, and its middle is half way.
        let mut up = rest;
        up[0].y += 0.4;
        let moves = fitted.rides(up, radius);
        let by = |instance: u32| moves.iter().find(|(id, _)| *id == instance).unwrap().1;
        let axle = by(11);
        assert!((axle.transform_point3(rest[0]) - up[0]).length() < 0.03);
        assert!((axle.transform_point3(rest[1]) - rest[1]).length() < 0.03);
        assert!((axle.transform_point3(Vec3::new(0.0, 0.84, -2.0)).y - 1.04).abs() < 1.0e-3);
        // A link stays held on the frame, and still points at where it is held on the axle.
        let (held, reached) = (Vec3::new(-0.5, 1.0, -1.0), Vec3::new(-0.8, 0.8, -2.0));
        let link = by(12);
        assert!((link.transform_point3(held) - held).length() < 1.0e-4);
        let now = axle.transform_point3(reached) - held;
        let drawn = link.transform_point3(reached) - held;
        assert!(drawn.normalize().dot(now.normalize()) > 0.9999);
        // A rod is held on the axle, and goes with it.
        let foot = Vec3::new(-0.9, 0.9, -2.0);
        assert!((by(13).transform_point3(foot) - axle.transform_point3(foot)).length() < 1.0e-4);
        // Asked again with nothing changed, nothing moves; back at rest, it all goes back.
        assert!(fitted.rides(up, radius).is_empty());
        let back = fitted.rides(rest, radius);
        let undone = back.iter().find(|(id, _)| *id == 11).unwrap().1 * axle;
        assert!(undone.abs_diff_eq(Mat4::IDENTITY, 1.0e-4));
    }

    #[test]
    fn what_runs_between_two_places_is_refitted_to_reach_them_where_they_now_are() {
        // Moved bodily, it is only moved.
        let (held, reaches) = (Vec3::new(0.25, 1.08, -0.62), Vec3::new(0.25, 0.84, -1.5));
        let by = Vec3::new(0.0, 0.0, -0.4);
        let moved = refit(held, reaches, held + by, reaches + by);
        assert!(moved.abs_diff_eq(Mat4::from_translation(by), 1.0e-5));
        // Held where it was and reaching further, it is turned and drawn out: both its ends
        // are where they are to be, and it is no fatter.
        let further = reaches + by;
        let drawn = refit(held, reaches, held, further);
        assert!((drawn.transform_point3(held) - held).length() < 1.0e-4);
        assert!((drawn.transform_point3(reaches) - further).length() < 1.0e-4);
        let across = Vec3::X * 0.045;
        assert!(((drawn.transform_point3(held + across) - held).length() - 0.045).abs() < 1.0e-4);
        // A line of a truck's numbers, read and raised.
        assert_eq!(
            said("front at=1.485 track=1.56 travel=0.11,0.10", "at"),
            Some(1.485)
        );
        assert_eq!(
            said("front at=1.485 travel=0.11,0.10", "travel"),
            Some(0.11)
        );
        assert_eq!(raised("camera 0 1.54 0", 2, 1.2), "camera 0 2.740 0");
        assert_eq!(
            raised(
                "hold seat 0.9 0.4 0.4 at -0.5 0.86 -0.2 door door_FL",
                7,
                1.0
            ),
            "hold seat 0.9 0.4 0.4 at -0.5 1.860 -0.2 door door_FL"
        );
        // A kit says what it fits, and a link may be held in the middle of the truck.
        let kit = Kit::parse(
            "base chassis_frame_M
fits 3.69 1.745 1.69
             link susp_drive_F front truck 0.25 1.08 -0.62 axle 0.25 0.84 -1.5
",
        )
        .unwrap();
        assert_eq!(kit.fits, Some([3.69, 1.745, 1.69]));
        assert_eq!(kit.links[0].held, (Vec3::new(0.25, 1.08, -0.62), false));
        assert!(kit.links[0].reaches.1);
    }
}
