//! What every map shares: the truck, and what its driver carries.
//!
//! They are not in a map's file. They are in one file of their own beside the maps,
//! [`FILE`], baked from `scenes/shared.txt`, and whichever map is played is joined with it as
//! it is read (`solstice_asset::join`). A map's text says which of its actors are there
//! rather than in it: `actor outback/raptor.actor X Y Z YAW shared`. The shared file has an
//! actor of the same file, with the instances that draw it; joined, the map's actor is given
//! those instances, and they are put where the map puts the actor ([`adopt`]).
//!
//! A map that is read with the shared file also leaves out of its own whatever else that
//! file has: its trees, grass and stone, and the textures of its ground. The shared file
//! lends them, and they are put back as the two are joined (`solstice_asset::join::fill`).
use glam::{Mat4, Quat, Vec3};
use solstice_asset::join::Joined;
use solstice_asset::{GameFile, PagedScene};
use solstice_game::scene::ActorLine;
use solstice_game::GAME_FILE;

/// The file of what is shared, beside the maps'.
pub const FILE: &str = "shared.solstice";

/// Where the shared file's bytes are numbered from, in a browser. A scene read as it is
/// drawn asks for ranges of bytes, and the shared file's are numbered on past the map's: on
/// a disk, from the map's length, which a browser would have to ask the server for. Any
/// number past the end of the map does as well, so there it is this one, which no map
/// reaches. The page reads a range from here on out of the shared file
/// (`apps/viewer/web/loader.js`).
pub const WEB_AT: u64 = 1 << 40;

/// Where scenes that are held in memory are numbered from, in the requests of a scene that
/// is read as it is drawn, and how far apart: a truck put together from its pack is such a
/// scene, joined onto a map whose own bytes are in a file (see [`Memory`]). Past anything a
/// map's file or the shared file reaches, on a disk or in a browser.
pub const MEMORY_AT: u64 = 1 << 41;
const MEMORY_EACH: u64 = 1 << 32;

/// Scenes that are all in memory, joined onto one that is read from its file as it is drawn.
///
/// Such a scene asks whoever runs it for ranges of bytes of its file. What is joined onto it
/// here has no file: it is written out as a file would be, kept, and numbered from
/// [`MEMORY_AT`] on, so that a range asked for from there is answered from what is kept
/// ([`Memory::read`]) and never asked of a disk or a site.
#[derive(Clone, Default)]
pub struct Memory {
    files: Vec<std::sync::Arc<Vec<u8>>>,
}

impl Memory {
    /// Joins a scene that is all in memory onto one that is not.
    pub fn join(
        &mut self,
        scene: &mut PagedScene,
        loaded: &solstice_asset::LoadedScene,
    ) -> Result<Joined, String> {
        use solstice_asset::file;
        let said = |error: file::DecodeError| error.to_string();
        let bytes = file::encode(loaded);
        let header = file::decode_header(&bytes).map_err(said)?;
        let (from, to) = (
            header.manifest_offset as usize,
            (header.manifest_offset + header.manifest_bytes) as usize,
        );
        let manifest =
            file::inflate(bytes.get(from..to).ok_or("its manifest is cut short")?).map_err(said)?;
        let other = file::decode_manifest(&manifest, to as u64).map_err(said)?;
        let at = MEMORY_AT + self.files.len() as u64 * MEMORY_EACH;
        let joined =
            solstice_asset::join::join(scene, other, at).map_err(|error| error.to_string())?;
        self.files.push(std::sync::Arc::new(bytes));
        Ok(joined)
    }

    /// The bytes of a range that was asked for, if it is of what is kept here.
    pub fn read(&self, start: u64, end: u64) -> Option<&[u8]> {
        let within = start.checked_sub(MEMORY_AT)?;
        let file = self.files.get((within / MEMORY_EACH) as usize)?;
        let from = (within % MEMORY_EACH) as usize;
        file.get(from..from + (end - start) as usize)
    }

    /// Whether nothing is kept here.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// A scene that others can be joined onto: one that is all in memory, or one that is read
/// as it is drawn, with a [`Memory`] for what is joined onto it. What puts trucks in a map
/// (below) does it to either.
pub trait Joins {
    /// The scene's tables.
    fn paged(&mut self) -> &mut PagedScene;
    /// Joins a scene that is all in memory onto it.
    fn join_loaded(&mut self, other: solstice_asset::LoadedScene) -> Result<Joined, String>;
}

impl Joins for solstice_asset::LoadedScene {
    fn paged(&mut self) -> &mut PagedScene {
        &mut self.scene
    }

    fn join_loaded(&mut self, other: solstice_asset::LoadedScene) -> Result<Joined, String> {
        self.join(other).map_err(|error| error.to_string())
    }
}

/// A scene that is read as it is drawn, with where what is joined onto it is kept.
pub struct Streamed<'a> {
    pub scene: &'a mut PagedScene,
    pub memory: &'a mut Memory,
}

impl Joins for Streamed<'_> {
    fn paged(&mut self) -> &mut PagedScene {
        self.scene
    }

    fn join_loaded(&mut self, other: solstice_asset::LoadedScene) -> Result<Joined, String> {
        self.memory.join(self.scene, &other)
    }
}

/// The actors of a scene's game, each with the line it is on.
fn actors(files: &[GameFile]) -> Vec<(usize, ActorLine)> {
    let Some(game) = files.iter().find(|file| file.name == GAME_FILE) else {
        return Vec::new();
    };
    game.text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.split_first() {
                Some((&"actor", rest)) => ActorLine::parse(rest).ok().map(|actor| (index, actor)),
                _ => None,
            }
        })
        .collect()
}

/// The gameplay file that says which instances of a scene are not to be drawn: a line a
/// run of them, `FIRST COUNT`. A truck that another has taken the place of is in it.
pub const HIDDEN: &str = "truck.hidden";

/// The instances a scene's gameplay says are not to be drawn.
pub fn hidden(files: &[GameFile]) -> Vec<u32> {
    let Some(file) = files.iter().find(|file| file.name == HIDDEN) else {
        return Vec::new();
    };
    let mut hidden = Vec::new();
    for line in file.text.lines() {
        let numbers: Vec<u32> = line
            .split_whitespace()
            .filter_map(|w| w.parse().ok())
            .collect();
        if let [first, count] = numbers[..] {
            hidden.extend(first..first + count);
        }
    }
    hidden
}

/// Puts a truck that was put together from its pack (`kit::assemble`) in a map, in place of
/// the truck the map has: the map's car is given the new truck's instances, where the map
/// has its car, and its actor file, its numbers and its kit; and the truck that was there
/// is no longer drawn.
pub fn swap_truck(scene: &mut impl Joins, truck: crate::kit::Assembled) -> Result<(), String> {
    let text = |files: &[GameFile], name: &str| {
        files
            .iter()
            .find(|file| file.name == name)
            .map(|file| file.text.clone())
    };
    // The map's car: the actor whose file makes it one, and that something draws.
    let files = &scene.paged().gameplay;
    let (line, mut actor) = actors(files)
        .into_iter()
        .find(|(_, actor)| {
            let file = text(files, &actor.file);
            let car = file.and_then(|text| solstice_game::ActorFile::parse(&text).ok());
            car.is_some_and(|car| car.vehicle.is_some())
        })
        .ok_or("the map has no car to put the truck in place of")?;
    let was = actor.instances.clone();
    let count = truck.scene.scene.instances.len() as u32;
    let joined = scene.join_loaded(truck.scene)?;
    let scene = scene.paged();
    let instances = joined.instances..joined.instances + count;
    let place = Mat4::from_rotation_translation(
        Quat::from_rotation_y(actor.yaw_degrees.to_radians()),
        actor.position,
    );
    for id in instances.clone() {
        let instance = &mut scene.instances[id as usize];
        instance.transform = place * instance.transform;
    }
    actor.file.clone_from(&truck.actor);
    actor.instances = Some(instances);
    actor.shared = false;
    let files = &mut scene.gameplay;
    if let Some(game) = files.iter_mut().find(|file| file.name == GAME_FILE) {
        let mut lines: Vec<String> = game.text.lines().map(str::to_owned).collect();
        lines[line] = actor.to_text();
        game.text = lines.join("\n") + "\n";
    }
    for file in truck.files {
        files.retain(|other| other.name != file.name);
        files.push(file);
    }
    if let Some(was) = was {
        files.retain(|other| other.name != HIDDEN);
        files.push(GameFile {
            name: HIDDEN.to_owned(),
            text: format!("{} {}\n", was.start, was.end - was.start),
        });
    }
    Ok(())
}

/// The game's own code is kept once, in the shared file: who drives and what the keys and a
/// controller do (an actor's file, its script, the input actions), and a truck's numbers. A
/// map has copies of them, from when it was baked; the shared file's take their place as the
/// two are joined, and any it has that the map has not are added. So what the controller
/// does is changed by baking the shared file alone, and is the same on every map.
///
/// (What a map lists of its own is its own: its actors and where they stand, its places,
/// its track.)
pub fn share_code(files: &mut Vec<GameFile>, shared: &[GameFile]) {
    let code = |name: &str| {
        [".actor", ".star", ".input", ".vehicle"]
            .iter()
            .any(|kind| name.ends_with(kind))
    };
    for theirs in shared.iter().filter(|file| code(&file.name)) {
        match files.iter_mut().find(|own| own.name == theirs.name) {
            Some(own) => own.text.clone_from(&theirs.text),
            None => files.push(theirs.clone()),
        }
    }
}

/// The file that says where trucks stood to be looked at stand: `X Y Z YAW`.
pub const STAND: &str = "showroom.stand";

/// Where the trucks that stand to be looked at are, and how they are turned, if a scene's
/// files say.
pub fn stand(files: &[GameFile]) -> Option<(Vec3, Quat)> {
    let file = files.iter().find(|file| file.name == STAND)?;
    let numbers: Vec<f32> = file
        .text
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect();
    let [x, y, z, yaw] = numbers[..] else {
        return None;
    };
    Some((Vec3::new(x, y, z), Quat::from_rotation_y(yaw.to_radians())))
}

/// Stands trucks in a map to be looked at, where the map has its car: each was put together
/// from its pack (`kit::assemble`), and all of them stand in the one place, to be shown one
/// at a time (`kit::displayed`). The map's own car is no longer drawn. Nothing drives them:
/// they are as they were made, at rest.
pub fn show_trucks(
    scene: &mut impl Joins,
    trucks: Vec<crate::kit::Assembled>,
    stand: Option<&str>,
) -> Result<(), String> {
    let text = |files: &[GameFile], name: &str| {
        files
            .iter()
            .find(|file| file.name == name)
            .map(|file| file.text.clone())
    };
    let files = &scene.paged().gameplay;
    let (_, actor) = actors(files)
        .into_iter()
        .find(|(_, actor)| {
            let file = text(files, &actor.file);
            let car = file.and_then(|text| solstice_game::ActorFile::parse(&text).ok());
            car.is_some_and(|car| car.vehicle.is_some())
        })
        .ok_or("the map has no car to stand the trucks in place of")?;
    // Where the map's car is; or, if the map says where trucks stand for what is being
    // looked at (`place stand:WHAT X Y Z YAW` among its places), there.
    let stood = stand.and_then(|stand| {
        let places = files.iter();
        let mut lines = places
            .filter(|file| file.name.ends_with(".places"))
            .flat_map(|file| file.text.lines());
        lines.find_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let ["place", what, x, y, z, yaw] = words[..] else {
                return None;
            };
            if what.strip_prefix("stand:") != Some(stand) {
                return None;
            }
            let number = |word: &str| word.parse::<f32>().ok();
            Some((Vec3::new(number(x)?, number(y)?, number(z)?), number(yaw)?))
        })
    });
    let (position, yaw_degrees) = stood.unwrap_or((actor.position, actor.yaw_degrees));
    let place =
        Mat4::from_rotation_translation(Quat::from_rotation_y(yaw_degrees.to_radians()), position);
    let mut list = String::new();
    for (number, truck) in trucks.into_iter().enumerate() {
        let count = truck.scene.scene.instances.len() as u32;
        let joined = scene.join_loaded(truck.scene)?;
        let scene = scene.paged();
        for id in joined.instances..joined.instances + count {
            let instance = &mut scene.instances[id as usize];
            instance.transform = place * instance.transform;
        }
        // Each has its files in a folder of its own: two of them may be the same truck.
        let folder = format!("show{number}/");
        let renamed = |name: &str| name.replacen(crate::kit::FOLDER, &folder, 1);
        list.push_str(&format!("{} {}\n", renamed(&truck.actor), joined.instances));
        for file in truck.files {
            let name = renamed(&file.name);
            scene.gameplay.retain(|other| other.name != name);
            scene.gameplay.push(GameFile {
                name,
                text: file.text,
            });
        }
    }
    let files = &mut scene.paged().gameplay;
    files.retain(|other| {
        other.name != crate::kit::SHOWN && other.name != HIDDEN && other.name != STAND
    });
    // Where they stand, for whoever looks at them: `X Y Z YAW`.
    files.push(GameFile {
        name: STAND.to_owned(),
        text: format!(
            "{} {} {} {yaw_degrees}\n",
            position.x, position.y, position.z
        ),
    });
    files.push(GameFile {
        name: crate::kit::SHOWN.to_owned(),
        text: list,
    });
    if let Some(was) = actor.instances {
        files.push(GameFile {
            name: HIDDEN.to_owned(),
            text: format!("{} {}\n", was.start, was.end - was.start),
        });
    }
    Ok(())
}

/// The gameplay file that says which trucks are parked in a map's used-truck yard: a line a
/// truck, `PLACE FIRST COUNT PAINT WAS_R WAS_G WAS_B`: its place on the day's lot, the run
/// of instances that draws it, the colour it is in (six hex digits, or `-` for the colour
/// it was made in), and the colour its paint was made in. A first line `pulled` says that
/// whichever of them is being looked over is not in its row: it stands out before it.
pub const PARKED: &str = "lot.parked";

/// A truck to be parked in the used-truck yard: its place on the day's lot, the truck put
/// together from its pack, the colour it is in, what is asked for it and how far it has
/// been.
pub struct Parked {
    pub place: usize,
    pub truck: crate::kit::Assembled,
    pub paint: Option<u32>,
    pub price: u64,
    pub km: f32,
}

/// How much of the ground a parked truck takes up, for whoever walks or drives into it:
/// across, high, and long. And how far before its middle its price stands.
const PARKED_SIZE: Vec3 = Vec3::new(2.1, 1.8, 5.3);
const PRICE_AHEAD: f32 = 3.7;

/// Where a map's used-truck yard has its trucks stand, in the order of their places: each
/// where, and its yaw in degrees (`place lot:N X Y Z YAW` among the map's places). None, in
/// a map with no such yard.
pub fn lot(files: &[GameFile]) -> Vec<(Vec3, f32)> {
    let places = files.iter().filter(|file| file.name.ends_with(".places"));
    let mut spots: Vec<(usize, Vec3, f32)> = places
        .flat_map(|file| file.text.lines())
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let ["place", what, x, y, z, yaw] = words[..] else {
                return None;
            };
            let spot = what.strip_prefix("lot:")?.parse().ok()?;
            let number = |word: &str| word.parse::<f32>().ok();
            Some((
                spot,
                Vec3::new(number(x)?, number(y)?, number(z)?),
                number(yaw)?,
            ))
        })
        .collect();
    spots.sort_by_key(|(spot, ..)| *spot);
    spots.into_iter().map(|(_, at, yaw)| (at, yaw)).collect()
}

/// Parks the day's used trucks in a map's yard, each in its place in the row (`spots`, from
/// [`lot`]): the truck as it was made, at rest, with a box round it that stops whoever
/// walks or drives into it, and before it a board with its price on (`board::board`).
/// Nothing drives them. What they are and what colour each is in is said in [`PARKED`],
/// for whoever draws them (`PlayMode`).
pub fn park_trucks(
    scene: &mut impl Joins,
    trucks: Vec<Parked>,
    spots: &[(Vec3, f32)],
    pulled: bool,
) -> Result<(), String> {
    let mut list = String::new();
    if pulled {
        list.push_str("pulled\n");
    }
    let mut blockers = String::new();
    for parked in trucks {
        let Some(&(at, yaw_degrees)) = spots.get(parked.place) else {
            continue;
        };
        let turned = Quat::from_rotation_y(yaw_degrees.to_radians());
        let place = Mat4::from_rotation_translation(turned, at);
        // The colour its paint was made in is its kit's to say.
        let kit = parked
            .truck
            .files
            .iter()
            .find(|file| file.name.ends_with(".kit"));
        let was = kit
            .and_then(|file| crate::kit::Kit::parse(&file.text).ok())
            .and_then(|kit| kit.paint);
        let count = parked.truck.scene.scene.instances.len() as u32;
        let joined = scene.join_loaded(parked.truck.scene)?;
        for id in joined.instances..joined.instances + count {
            let instance = &mut scene.paged().instances[id as usize];
            instance.transform = place * instance.transform;
        }
        let paint = match parked.paint {
            Some(colour) => format!("{colour:06x}"),
            None => "-".to_owned(),
        };
        let was = was.map_or("- - -".to_owned(), |was| {
            format!("{} {} {}", was.x, was.y, was.z)
        });
        list.push_str(&format!(
            "{} {} {count} {paint} {was}\n",
            parked.place, joined.instances
        ));
        let stops = |centre: Vec3, size: Vec3| {
            solstice_game::Blocker {
                centre,
                size,
                yaw_degrees,
            }
            .to_text()
        };
        blockers.push_str(&stops(at + Vec3::Y * PARKED_SIZE.y * 0.5, PARKED_SIZE));
        blockers.push('\n');
        // Its price, on a board before its nose.
        let ahead = at + turned * Vec3::new(0.0, 0.0, -PRICE_AHEAD);
        let board = crate::board::board(parked.price, parked.km);
        let count = board.scene.instances.len() as u32;
        let joined = scene.join_loaded(board)?;
        let stood = Mat4::from_rotation_translation(turned, ahead);
        for id in joined.instances..joined.instances + count {
            let instance = &mut scene.paged().instances[id as usize];
            instance.transform = stood * instance.transform;
        }
        blockers.push_str(&stops(ahead + Vec3::Y * 0.55, Vec3::new(1.1, 1.1, 0.5)));
        blockers.push('\n');
    }
    let files = &mut scene.paged().gameplay;
    if let Some(game) = files.iter_mut().find(|file| file.name == GAME_FILE) {
        if !game.text.ends_with('\n') {
            game.text.push('\n');
        }
        game.text.push_str(&blockers);
    }
    files.retain(|other| other.name != PARKED);
    files.push(GameFile {
        name: PARKED.to_owned(),
        text: list,
    });
    Ok(())
}

/// The trucks a scene's files say are parked in its used-truck yard: each its place on the
/// lot, the instances that draw it, the colour it is in and the colour its paint was made
/// in, where the file says; and whether the one being looked over is pulled out of its row.
#[allow(clippy::type_complexity)]
pub fn parked(files: &[GameFile]) -> (Vec<(usize, Vec<u32>, Option<u32>, Option<Vec3>)>, bool) {
    let Some(file) = files.iter().find(|file| file.name == PARKED) else {
        return (Vec::new(), false);
    };
    let mut pulled = false;
    let mut trucks = Vec::new();
    for line in file.text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words[..] {
            ["pulled"] => pulled = true,
            [place, first, count, paint, r, g, b] => {
                let (Ok(place), Ok(first), Ok(count)) = (
                    place.parse::<usize>(),
                    first.parse::<u32>(),
                    count.parse::<u32>(),
                ) else {
                    continue;
                };
                let number = |word: &str| word.parse::<f32>().ok();
                let was = (|| Some(Vec3::new(number(r)?, number(g)?, number(b)?)))();
                trucks.push((
                    place,
                    (first..first + count).collect(),
                    u32::from_str_radix(paint, 16).ok(),
                    was,
                ));
            }
            _ => {}
        }
    }
    (trucks, pulled)
}

/// Whether a scene's game has actors that are in the shared file.
pub fn wanted(files: &[GameFile]) -> bool {
    actors(files).iter().any(|(_, actor)| actor.shared)
}

/// A map has been joined with the shared file, whose gameplay files `shared` are: gives each
/// of the map's shared actors the instances that draw it, and puts them where the map puts
/// the actor.
pub fn adopt(scene: &mut PagedScene, shared: &[GameFile], joined: Joined) -> Result<(), String> {
    share_code(&mut scene.gameplay, shared);
    let there = actors(shared);
    let mut changed = Vec::new();
    for (line, mut actor) in actors(&scene.gameplay) {
        if !actor.shared {
            continue;
        }
        let (_, theirs) = there
            .iter()
            .find(|(_, theirs)| theirs.file == actor.file)
            .ok_or_else(|| format!("{} is not in {FILE}", actor.file))?;
        let Some(instances) = theirs.instances.clone() else {
            // It has no model: nothing draws it, there or here.
            actor.shared = false;
            changed.push((line, actor));
            continue;
        };
        let instances = instances.start + joined.instances..instances.end + joined.instances;
        // The shared file has it where it was baked; the map has it where it starts.
        let place = |actor: &ActorLine| {
            Mat4::from_rotation_translation(
                Quat::from_rotation_y(actor.yaw_degrees.to_radians()),
                actor.position,
            )
        };
        let moved = place(&actor) * place(theirs).inverse();
        for id in instances.clone() {
            let instance = scene
                .instances
                .get_mut(id as usize)
                .ok_or_else(|| format!("{FILE} has no instance {id} for {}", actor.file))?;
            instance.transform = moved * instance.transform;
        }
        // If it is a kit, the list of its pieces comes with it: it counts from the actor's
        // first instance, wherever that now is.
        let pieces = solstice_game::actor::pieces_file(&actor.file);
        if let Some(file) = shared.iter().find(|file| file.name == pieces) {
            if !scene.gameplay.iter().any(|f| f.name == pieces) {
                scene.gameplay.push(file.clone());
            }
        }
        actor.instances = Some(instances);
        actor.shared = false;
        changed.push((line, actor));
    }
    if let Some(game) = scene
        .gameplay
        .iter_mut()
        .find(|file| file.name == GAME_FILE)
    {
        let mut lines: Vec<String> = game.text.lines().map(str::to_owned).collect();
        for (line, actor) in changed {
            lines[line] = actor.to_text();
        }
        game.text = lines.join("\n") + "\n";
    }
    // And what the map left out because the shared file has it: its meshes and textures.
    let text = |files: &[GameFile], name: &str| {
        files
            .iter()
            .find(|file| file.name == name)
            .map(|file| file.text.clone())
    };
    if let Some(borrows) = text(&scene.gameplay, solstice_asset::join::BORROWS) {
        let lends = text(shared, solstice_asset::join::LENDS).unwrap_or_default();
        solstice_asset::join::fill(scene, &borrows, &lends, joined)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_files_code_takes_the_place_of_a_maps_copy_of_it() {
        let file = |name: &str, text: &str| GameFile {
            name: name.to_owned(),
            text: text.to_owned(),
        };
        let mut map = vec![
            file(GAME_FILE, "actor outback/driver.actor 1 2 3 0\n"),
            file("outback/driver.star", "old\n"),
            file("outback.places", "place shed 0 0 0 3\n"),
        ];
        let shared = [
            file(GAME_FILE, "actor outback/driver.actor 0 0 0 0\n"),
            file("outback/driver.star", "new\n"),
            file("outback/game.input", "sprint gamepad l3\n"),
            file("outback.places", "place nowhere 9 9 9 9\n"),
        ];
        share_code(&mut map, &shared);
        let text = |name: &str| {
            let found = map.iter().find(|file| file.name == name);
            found.map(|file| file.text.as_str())
        };
        // Its script is the shared one, and the actions it had none of are there.
        assert_eq!(text("outback/driver.star"), Some("new\n"));
        assert_eq!(text("outback/game.input"), Some("sprint gamepad l3\n"));
        // Where its actors stand, and its places, are its own.
        assert_eq!(
            text(GAME_FILE),
            Some("actor outback/driver.actor 1 2 3 0\n")
        );
        assert_eq!(text("outback.places"), Some("place shed 0 0 0 3\n"));
    }

    #[test]
    fn a_maps_yard_says_where_its_trucks_stand_and_which_are_parked() {
        let file = |name: &str, text: &str| GameFile {
            name: name.to_owned(),
            text: text.to_owned(),
        };
        // Its places, in whatever order they were written, are in the order of the lot.
        let files = [file(
            "town.places",
            "place buyer 1 2 3 5.5\nplace lot:1 10 80 -20 90\nplace lot:0 4 80 -20 90\nplace door:used 0 0 0 1.7\n",
        )];
        assert_eq!(
            lot(&files),
            vec![
                (Vec3::new(4.0, 80.0, -20.0), 90.0),
                (Vec3::new(10.0, 80.0, -20.0), 90.0)
            ]
        );
        assert!(lot(&[file("hub.places", "place shed 0 0 0 3\n")]).is_empty());
        // What is parked is read back as it was written: one resprayed, one as it was made.
        let files = [file(
            PARKED,
            "pulled\n2 40 3 c8102e 0.16 0 0\n4 60 2 - - - -\n",
        )];
        let (trucks, pulled) = parked(&files);
        assert!(pulled);
        assert_eq!(
            trucks,
            vec![
                (
                    2,
                    vec![40, 41, 42],
                    Some(0xc8102e),
                    Some(Vec3::new(0.16, 0.0, 0.0))
                ),
                (4, vec![60, 61], None, None),
            ]
        );
        assert_eq!(parked(&[]), (Vec::new(), false));
    }
}
