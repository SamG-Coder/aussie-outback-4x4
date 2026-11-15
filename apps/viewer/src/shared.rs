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
use glam::{Mat4, Quat};
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

/// Whether a scene's game has actors that are in the shared file.
pub fn wanted(files: &[GameFile]) -> bool {
    actors(files).iter().any(|(_, actor)| actor.shared)
}

/// A map has been joined with the shared file, whose gameplay files `shared` are: gives each
/// of the map's shared actors the instances that draw it, and puts them where the map puts
/// the actor.
pub fn adopt(scene: &mut PagedScene, shared: &[GameFile], joined: Joined) -> Result<(), String> {
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
