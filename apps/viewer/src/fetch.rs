//! Fetches the game's maps from its site, before it is played: `viewer --fetch-maps SITE...`.
//!
//! A download of the game is its program, pages and sounds; its maps, which are most of it,
//! are fetched here the first time it is run, and again when one has changed. They are
//! fetched in pieces, and only the pieces that are not here already (docs/hosting.md):
//!
//! * The site has `scenes/maps.txt`, a line a map, `NAME VERSION MEGABYTES TITLE`, and beside
//!   each map `NAME.solstice.chunks`, a line a piece of it, `OFFSET LENGTH HASH`: the file cut
//!   where its own bytes say to, so that what two maps have in common, the truck, the
//!   textures, the trees, is the same pieces in both (tools/hosting/chunks.py).
//! * A map is put together from pieces. A piece that is in a map already on the disk, another
//!   map or the last version of this one, is copied from there. One that was fetched before
//!   this was last stopped is kept. Only the rest are downloaded, pieces side by side in one
//!   request. Every piece is checked against its hash before it is used.
//!
//! So the first time, what the maps share comes down once; a map that has changed costs only
//! its changed pieces; and a download that is stopped carries on where it stopped.
//!
//! What goes over the wire is `curl`'s to send, which every Windows since 2018 has: this needs
//! no web client of its own, and uses the system's certificates and proxy as it finds them.
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The most that is asked for in one request, when pieces side by side are all wanted.
const RUN_BYTES: u64 = 32 * 1024 * 1024;

/// One of the game's maps, as the site lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub name: String,
    pub version: String,
    pub megabytes: u32,
    pub title: String,
}

/// A piece of a map's file.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub offset: u64,
    pub length: u64,
    pub hash: String,
}

/// The maps a `maps.txt` lists: `NAME VERSION MEGABYTES TITLE` a line.
pub fn listed(text: &str) -> Vec<Listed> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.trim().splitn(4, ' ');
            Some(Listed {
                name: words.next().filter(|name| is_plain(name))?.to_owned(),
                version: words.next().filter(|version| is_plain(version))?.to_owned(),
                megabytes: words.next()?.parse().ok()?,
                title: words.next().unwrap_or("").trim().to_owned(),
            })
        })
        .collect()
}

/// Whether a word can be part of a file's name here: what a site says is not trusted to be.
fn is_plain(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The pieces a `.chunks` list gives: `OFFSET LENGTH HASH` a line. `None` unless they are a
/// whole file's, one after another with nothing between.
pub fn pieces(text: &str) -> Option<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut at = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut words = line.split_whitespace();
        let piece = Piece {
            offset: words.next()?.parse().ok()?,
            length: words.next()?.parse().ok()?,
            hash: words.next()?.to_owned(),
        };
        if piece.offset != at || piece.length == 0 {
            return None;
        }
        at += piece.length;
        pieces.push(piece);
    }
    (!pieces.is_empty()).then_some(pieces)
}

/// What a piece is known by: the first 24 hex digits of its SHA-256.
pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The pieces to download, as runs of pieces side by side, none longer than `most` bytes
/// unless it is one piece: each run is one request.
pub fn runs(wanted: &[usize], pieces: &[Piece], most: u64) -> Vec<std::ops::Range<usize>> {
    let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
    for &index in wanted {
        match runs.last_mut() {
            Some(run)
                if run.end == index
                    && pieces[index].offset + pieces[index].length - pieces[run.start].offset
                        <= most =>
            {
                run.end = index + 1;
            }
            _ => runs.push(index..index + 1),
        }
    }
    runs
}

/// What fetches things from the site: `curl`, or, to try this without a site, a folder.
pub trait Wire {
    /// The whole of a small file, or a range of bytes of a big one, first to one past last.
    fn get(&self, url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, String>;
    /// A whole big file, into `to`, carrying on if part of it is there.
    fn download(&self, url: &str, to: &Path) -> Result<(), String>;
}

/// The system's `curl`.
pub struct Curl;

impl Curl {
    fn run(arguments: &[&str]) -> Result<Vec<u8>, String> {
        let output = Command::new("curl")
            .args(["-s", "-S", "-f", "-L", "--retry", "3", "--retry-delay", "2"])
            .args(arguments)
            .output()
            .map_err(|error| format!("curl could not be run: {error}"))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
        }
    }
}

impl Wire for Curl {
    fn get(&self, url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, String> {
        match range {
            Some((from, to)) => {
                let bytes = Self::run(&["-m", "600", "-r", &format!("{from}-{}", to - 1), url])?;
                if bytes.len() as u64 == to - from {
                    Ok(bytes)
                } else {
                    Err(format!(
                        "asked for {} bytes and was sent {}",
                        to - from,
                        bytes.len()
                    ))
                }
            }
            None => Self::run(&["-m", "30", url]),
        }
    }

    fn download(&self, url: &str, to: &Path) -> Result<(), String> {
        let to = to.to_string_lossy();
        Self::run(&["-C", "-", "-o", &to, url]).map(|_| ())
    }
}

/// Where every piece that is in a map on the disk can be read from: its hash, and the file,
/// the place in it and the length. From the list each map was put together by, kept beside it.
fn on_disk(folder: &Path) -> HashMap<String, (PathBuf, u64, u64)> {
    let mut found = HashMap::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let list = entry.path();
        let Some(map) = list
            .to_string_lossy()
            .strip_suffix(".chunks")
            .map(PathBuf::from)
        else {
            continue;
        };
        let (Ok(text), Ok(size)) = (std::fs::read_to_string(&list), std::fs::metadata(&map)) else {
            continue;
        };
        let Some(listed) = pieces(&text) else {
            continue;
        };
        // A list that is not this file's says nothing about it.
        if listed.last().map(|last| last.offset + last.length) != Some(size.len()) {
            continue;
        }
        for piece in listed {
            found.insert(piece.hash, (map.clone(), piece.offset, piece.length));
        }
    }
    found
}

/// Reads part of a file, if all of it is there.
fn read_at(file: &mut File, offset: u64, length: u64) -> Option<Vec<u8>> {
    let mut bytes = vec![0; length as usize];
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

/// How far along a map is, said on one line that is written over as it goes.
fn progress(done: u64, of: u64) {
    let share = if of == 0 {
        1.0
    } else {
        done as f64 / of as f64
    };
    let filled = (share * 30.0).round() as usize;
    print!(
        "\r    [{}{}] {:4.0} of {:.0} MB",
        "#".repeat(filled),
        ".".repeat(30 - filled),
        done as f64 / 1.0e6,
        of as f64 / 1.0e6
    );
    let _ = std::io::stdout().flush();
}

/// Makes sure a map is on the disk at the version listed: puts it together from what is here
/// and what has to be downloaded. Returns how many bytes were downloaded.
pub fn fetch_map(wire: &dyn Wire, site: &str, folder: &Path, map: &Listed) -> Result<u64, String> {
    let file = folder.join(format!("{}.solstice", map.name));
    let version = folder.join(format!("{}.version", map.name));
    let kept = folder.join(format!("{}.solstice.chunks", map.name));
    if file.exists()
        && std::fs::read_to_string(&version).is_ok_and(|have| have.trim() == map.version)
    {
        return Ok(0);
    }
    let url = format!("{site}/scenes/{}.solstice", map.name);
    let part = folder.join(format!("{}.solstice.part", map.name));
    let list = wire.get(&format!("{url}.chunks"), None).ok();
    let list = list.and_then(|bytes| String::from_utf8(bytes).ok());
    let Some((text, listed)) = list.and_then(|text| pieces(&text).map(|listed| (text, listed)))
    else {
        // No list of its pieces: the whole file, as it is.
        println!("  {}: {} MB to download", map.title, map.megabytes);
        wire.download(&url, &part)?;
        finish(&part, &file, &version, &map.version)?;
        let _ = std::fs::remove_file(&kept);
        return Ok(std::fs::metadata(&file).map_or(0, |m| m.len()));
    };
    let size = listed.last().map_or(0, |last| last.offset + last.length);

    // What is here already: in a download that was stopped, or in a map on the disk.
    let resumed = part.exists();
    let mut out = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&part)
        .map_err(|error| format!("{}: {error}", part.display()))?;
    out.set_len(size)
        .map_err(|error| format!("{}: {error}", part.display()))?;
    let here = on_disk(folder);
    let mut sources: HashMap<PathBuf, File> = HashMap::new();
    let mut wanted = Vec::new();
    let mut copied = 0;
    for (index, piece) in listed.iter().enumerate() {
        if resumed
            && read_at(&mut out, piece.offset, piece.length)
                .is_some_and(|bytes| hash(&bytes) == piece.hash)
        {
            copied += piece.length;
            continue;
        }
        let from_disk = here.get(&piece.hash).and_then(|(path, offset, length)| {
            if !sources.contains_key(path) {
                sources.insert(path.clone(), File::open(path).ok()?);
            }
            let bytes = read_at(sources.get_mut(path)?, *offset, *length)?;
            (hash(&bytes) == piece.hash).then_some(bytes)
        });
        match from_disk {
            Some(bytes) => {
                write_at(&mut out, piece.offset, &bytes, &part)?;
                copied += piece.length;
            }
            None => wanted.push(index),
        }
    }
    let to_fetch: u64 = wanted.iter().map(|&index| listed[index].length).sum();
    println!(
        "  {}: {:.0} MB to download, {:.0} MB already here",
        map.title,
        to_fetch as f64 / 1.0e6,
        copied as f64 / 1.0e6
    );

    let mut fetched = 0;
    for run in runs(&wanted, &listed, RUN_BYTES) {
        let (from, to) = (
            listed[run.start].offset,
            listed[run.end - 1].offset + listed[run.end - 1].length,
        );
        progress(fetched, to_fetch);
        let bytes = wire.get(&url, Some((from, to)))?;
        for piece in &listed[run] {
            let start = (piece.offset - from) as usize;
            let bytes = &bytes[start..start + piece.length as usize];
            if hash(bytes) != piece.hash {
                return Err(format!(
                    "{}'s list of pieces is not the list of the map on the site: try again in a minute",
                    map.title
                ));
            }
            write_at(&mut out, piece.offset, bytes, &part)?;
        }
        fetched += to - from;
    }
    if to_fetch > 0 {
        progress(fetched, to_fetch);
        println!();
    }
    out.flush()
        .map_err(|error| format!("{}: {error}", part.display()))?;
    drop(out);
    drop(sources);
    finish(&part, &file, &version, &map.version)?;
    std::fs::write(&kept, text).map_err(|error| format!("{}: {error}", kept.display()))?;
    Ok(fetched)
}

fn write_at(file: &mut File, offset: u64, bytes: &[u8], path: &Path) -> Result<(), String> {
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.write_all(bytes))
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// A map that has all come: in its place, and marked with its version.
fn finish(part: &Path, file: &Path, version: &Path, is: &str) -> Result<(), String> {
    // Unmarked while it is neither one thing nor the other.
    let _ = std::fs::remove_file(version);
    let _ = std::fs::remove_file(file);
    std::fs::rename(part, file).map_err(|error| format!("{}: {error}", file.display()))?;
    std::fs::write(version, format!("{is}\n"))
        .map_err(|error| format!("{}: {error}", version.display()))
}

/// Fetches every map the first of `sites` that answers lists, into `folder`. Returns whether
/// every one of them is there, and says what it did as it goes. With no site answering it
/// says so, and the maps that are there can still be played.
pub fn fetch_maps(wire: &dyn Wire, sites: &[String], folder: &Path) -> bool {
    if let Err(error) = std::fs::create_dir_all(folder) {
        println!("{}: {error}", folder.display());
        return false;
    }
    let found = sites.iter().find_map(|site| {
        let site = site.trim_end_matches('/');
        let text =
            String::from_utf8(wire.get(&format!("{site}/scenes/maps.txt"), None).ok()?).ok()?;
        let maps = listed(&text);
        (!maps.is_empty()).then(|| (site.to_owned(), maps))
    });
    let Some((site, maps)) = found else {
        println!(
            "The maps could not be checked: {} did not answer.",
            sites.join(" and ")
        );
        return false;
    };
    let (mut whole, mut fetched) = (true, 0);
    println!("Checking the maps...");
    for map in &maps {
        match fetch_map(wire, &site, folder, map) {
            Ok(bytes) => fetched += bytes,
            Err(error) => {
                println!("\n  {} could not be fetched: {error}", map.title);
                whole = false;
            }
        }
    }
    if fetched > 0 {
        println!("Downloaded {:.0} MB.", fetched as f64 / 1.0e6);
    } else if whole {
        println!("The maps are up to date.");
    }
    whole
}

/// `viewer --fetch-maps [--into FOLDER] SITE...`: what the process ends with, 0 if every map
/// is there.
pub fn run(arguments: &[String]) -> i32 {
    let mut folder = PathBuf::from("assets");
    let mut sites = Vec::new();
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--into" => match arguments.next() {
                Some(path) => folder = PathBuf::from(path),
                None => {
                    println!("--into expects a folder");
                    return 2;
                }
            },
            site => sites.push(site.to_owned()),
        }
    }
    if sites.is_empty() {
        println!("--fetch-maps expects the site the maps are on");
        return 2;
    }
    i32::from(!fetch_maps(&Curl, &sites, &folder))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A site that is a set of files in memory, which counts what is asked of it.
    #[derive(Default)]
    struct Site {
        files: HashMap<String, Vec<u8>>,
        asked: RefCell<Vec<(String, Option<(u64, u64)>)>>,
    }

    impl Wire for Site {
        fn get(&self, url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, String> {
            self.asked.borrow_mut().push((url.to_owned(), range));
            let file = self.files.get(url).ok_or("404")?;
            Ok(match range {
                Some((from, to)) => file[from as usize..to as usize].to_vec(),
                None => file.clone(),
            })
        }

        fn download(&self, url: &str, to: &Path) -> Result<(), String> {
            self.asked.borrow_mut().push((url.to_owned(), None));
            std::fs::write(to, self.files.get(url).ok_or("404")?).map_err(|e| e.to_string())
        }
    }

    /// A file made of these parts, and the list that cuts it into them.
    fn cut(parts: &[&[u8]]) -> (Vec<u8>, String) {
        let (mut file, mut list) = (Vec::new(), String::new());
        for part in parts {
            list.push_str(&format!("{} {} {}\n", file.len(), part.len(), hash(part)));
            file.extend_from_slice(part);
        }
        (file, list)
    }

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("outback-fetch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a folder to try it in");
        folder
    }

    fn map(name: &str, version: &str) -> Listed {
        Listed {
            name: name.to_owned(),
            version: version.to_owned(),
            megabytes: 1,
            title: name.to_owned(),
        }
    }

    #[test]
    fn lists_are_read_and_a_bad_one_is_not_trusted() {
        assert_eq!(
            listed("hub ab12 469 The Crossroads\r\noutback 7c2e 451 Wombat Creek Track\n\n../x y 1 Bad\n"),
            vec![
                Listed { name: "hub".into(), version: "ab12".into(), megabytes: 469, title: "The Crossroads".into() },
                Listed { name: "outback".into(), version: "7c2e".into(), megabytes: 451, title: "Wombat Creek Track".into() },
            ]
        );
        assert_eq!(pieces("0 4 aa\n4 6 bb\n").map(|p| p.len()), Some(2));
        // A gap, or nothing at all, is not a file's pieces.
        assert_eq!(pieces("0 4 aa\n5 6 bb\n"), None);
        assert_eq!(pieces(""), None);
    }

    #[test]
    fn pieces_side_by_side_are_one_request_up_to_a_size() {
        let (_, list) = cut(&[&[0; 10], &[1; 10], &[2; 10], &[3; 10], &[4; 10]]);
        let listed = pieces(&list).expect("a whole list");
        assert_eq!(runs(&[0, 1, 3, 4], &listed, 100), vec![0..2, 3..5]);
        assert_eq!(runs(&[0, 1, 2, 3], &listed, 20), vec![0..2, 2..4]);
        assert_eq!(
            runs(&[], &listed, 100),
            Vec::<std::ops::Range<usize>>::new()
        );
    }

    #[test]
    fn a_second_map_downloads_only_what_the_first_has_not_got() {
        let folder = folder("shared");
        let (truck, trees): (&[u8], &[u8]) = (&[7; 3000], &[9; 2000]);
        let (hub, hub_list) = cut(&[b"the hub's own land", truck, trees]);
        let (track, track_list) = cut(&[
            b"a track's land, which is its own",
            truck,
            b"its creek",
            trees,
        ]);
        let mut site = Site::default();
        for (name, file, list) in [("hub", &hub, &hub_list), ("track", &track, &track_list)] {
            site.files
                .insert(format!("s/scenes/{name}.solstice"), file.clone());
            site.files.insert(
                format!("s/scenes/{name}.solstice.chunks"),
                list.clone().into_bytes(),
            );
        }
        // The first map: all of it.
        assert_eq!(
            fetch_map(&site, "s", &folder, &map("hub", "1")),
            Ok(hub.len() as u64)
        );
        assert_eq!(
            std::fs::read(folder.join("hub.solstice")).expect("it is there"),
            hub
        );
        // The second: its own land and its creek, and neither the truck nor the trees.
        let fetched = fetch_map(&site, "s", &folder, &map("track", "1")).expect("it is fetched");
        assert_eq!(
            fetched,
            (b"a track's land, which is its own".len() + b"its creek".len()) as u64
        );
        assert_eq!(
            std::fs::read(folder.join("track.solstice")).expect("it is there"),
            track
        );
        // Asked for again at the same version: nothing is asked of the site at all.
        site.asked.borrow_mut().clear();
        assert_eq!(fetch_map(&site, "s", &folder, &map("track", "1")), Ok(0));
        assert!(site.asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_changed_map_downloads_only_its_changed_pieces_and_a_stopped_download_carries_on() {
        let folder = folder("changed");
        let (land, trees): (&[u8], &[u8]) = (&[3; 4000], &[5; 2500]);
        let (old, old_list) = cut(&[land, b"the track as it was", trees]);
        let (new, new_list) = cut(&[land, b"the track, cut again by rain", trees]);
        let mut site = Site::default();
        site.files.insert("s/scenes/m.solstice".into(), old.clone());
        site.files
            .insert("s/scenes/m.solstice.chunks".into(), old_list.into_bytes());
        assert_eq!(
            fetch_map(&site, "s", &folder, &map("m", "1")),
            Ok(old.len() as u64)
        );
        // The map is baked again: only the piece that changed comes down.
        site.files.insert("s/scenes/m.solstice".into(), new.clone());
        site.files.insert(
            "s/scenes/m.solstice.chunks".into(),
            new_list.clone().into_bytes(),
        );
        assert_eq!(
            fetch_map(&site, "s", &folder, &map("m", "2")),
            Ok(b"the track, cut again by rain".len() as u64)
        );
        assert_eq!(
            std::fs::read(folder.join("m.solstice")).expect("it is there"),
            new
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("m.version"))
                .expect("marked")
                .trim(),
            "2"
        );

        // A download stopped part of the way: the part that came is not asked for again.
        let other = folder.join("fresh");
        std::fs::create_dir_all(&other).expect("a folder");
        let mut part = vec![0; new.len()];
        part[..land.len()].copy_from_slice(land);
        std::fs::write(other.join("m.solstice.part"), &part).expect("written");
        assert_eq!(
            fetch_map(&site, "s", &other, &map("m", "2")),
            Ok((new.len() - land.len()) as u64)
        );
        assert_eq!(
            std::fs::read(other.join("m.solstice")).expect("it is there"),
            new
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_list_that_is_not_the_maps_is_refused_and_a_map_with_no_list_comes_whole() {
        let folder = folder("lists");
        let (file, _) = cut(&[b"what the map is now"]);
        let (_, stale) = cut(&[b"what the map was!!!"]);
        let mut site = Site::default();
        site.files
            .insert("s/scenes/m.solstice".into(), file.clone());
        site.files
            .insert("s/scenes/m.solstice.chunks".into(), stale.into_bytes());
        assert!(fetch_map(&site, "s", &folder, &map("m", "1")).is_err());
        assert!(!folder.join("m.solstice").exists());
        // With no list at all, the whole file.
        site.files.remove("s/scenes/m.solstice.chunks");
        let _ = std::fs::remove_file(folder.join("m.solstice.part"));
        assert_eq!(
            fetch_map(&site, "s", &folder, &map("m", "1")),
            Ok(file.len() as u64)
        );
        assert_eq!(
            std::fs::read(folder.join("m.solstice")).expect("it is there"),
            file
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}
