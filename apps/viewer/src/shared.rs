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
pub fn swap_truck(
    scene: &mut solstice_asset::LoadedScene,
    truck: crate::kit::Assembled,
) -> Result<(), String> {
    let text = |files: &[GameFile], name: &str| {
        files
            .iter()
            .find(|file| file.name == name)
            .map(|file| file.text.clone())
    };
    // The map's car: the actor whose file makes it one, and that something draws.
    let (line, mut actor) = actors(&scene.scene.gameplay)
        .into_iter()
        .find(|(_, actor)| {
            let file = text(&scene.scene.gameplay, &actor.file);
            let car = file.and_then(|text| solstice_game::ActorFile::parse(&text).ok());
            car.is_some_and(|car| car.vehicle.is_some())
        })
        .ok_or("the map has no car to put the truck in place of")?;
    let was = actor.instances.clone();
    let count = truck.scene.scene.instances.len() as u32;
    let joined = scene.join(truck.scene).map_err(|error| error.to_string())?;
    let instances = joined.instances..joined.instances + count;
    let place = Mat4::from_rotation_translation(
        Quat::from_rotation_y(actor.yaw_degrees.to_radians()),
        actor.position,
    );
    for id in instances.clone() {
        let instance = &mut scene.scene.instances[id as usize];
        instance.transform = place * instance.transform;
    }
    actor.file.clone_from(&truck.actor);
    actor.instances = Some(instances);
    actor.shared = false;
    let files = &mut scene.scene.gameplay;
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
    scene: &mut solstice_asset::LoadedScene,
    trucks: Vec<crate::kit::Assembled>,
    stand: Option<&str>,
) -> Result<(), String> {
    let text = |files: &[GameFile], name: &str| {
        files
            .iter()
            .find(|file| file.name == name)
            .map(|file| file.text.clone())
    };
    let (_, actor) = actors(&scene.scene.gameplay)
        .into_iter()
        .find(|(_, actor)| {
            let file = text(&scene.scene.gameplay, &actor.file);
            let car = file.and_then(|text| solstice_game::ActorFile::parse(&text).ok());
            car.is_some_and(|car| car.vehicle.is_some())
        })
        .ok_or("the map has no car to stand the trucks in place of")?;
    // Where the map's car is; or, if the map says where trucks stand for what is being
    // looked at (`place stand:WHAT X Y Z YAW` among its places), there.
    let stood = stand.and_then(|stand| {
        let places = scene.scene.gameplay.iter();
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
        let joined = scene.join(truck.scene).map_err(|error| error.to_string())?;
        for id in joined.instances..joined.instances + count {
            let instance = &mut scene.scene.instances[id as usize];
            instance.transform = place * instance.transform;
        }
        // Each has its files in a folder of its own: two of them may be the same truck.
        let folder = format!("show{number}/");
        let renamed = |name: &str| name.replacen(crate::kit::FOLDER, &folder, 1);
        list.push_str(&format!("{} {}\n", renamed(&truck.actor), joined.instances));
        for file in truck.files {
            let name = renamed(&file.name);
            scene.scene.gameplay.retain(|other| other.name != name);
            scene.scene.gameplay.push(GameFile {
                name,
                text: file.text,
            });
        }
    }
    let files = &mut scene.scene.gameplay;
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
}
