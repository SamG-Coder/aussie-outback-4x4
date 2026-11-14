//! A game that is downloaded: what its program does when it is opened with nothing asked.
//!
//! The program is the engine's viewer, which does what its arguments say. A game somebody has
//! downloaded is opened with a double click, and is given none. So beside the program there is
//! a `game.txt`, which says what it is then to do; the program looks for it as it starts, and
//! with none there it is the viewer it always was.
//!
//! ```text
//! title Aussie Outback 4x4
//! run   --world scenes/outback.world --ui ui --size 1920x1080 --weather fair
//! maps  https://rust4x4.com https://rust4x4.pages.dev
//! log   save/game.log
//! ```
//!
//! * `run`: the arguments it is run with. Any it is given by hand come after them, and so are
//!   the ones that count: `AussieOutback4x4.exe --size 2560x1440`.
//! * `maps`: the sites its maps are fetched from, the first that answers. They are not in the
//!   download: the game opens its window, fetches the maps it has not got (`fetch.rs`) while
//!   its loading screen says how far along that is, and then starts (the lobby in `main.rs`).
//! * `log`: where what the program says as it runs is written. A game opened with a double
//!   click has no console to say it on.
//! * `title`: what it is called, where something has to be said outside its own pages.
//!
//! The paths in it, and the game's own, are from the folder the program is in, wherever it
//! was opened from: the program goes there first.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// What a `game.txt` says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Game {
    pub title: String,
    pub run: Vec<String>,
    pub maps: Vec<String>,
    pub log: Option<PathBuf>,
}

/// The game's title, once a game is being run: what [`alert`] is headed with.
static TITLE: OnceLock<String> = OnceLock::new();

impl Game {
    /// Reads a `game.txt`. A line it does not know says nothing: an older program can be
    /// handed a newer file.
    pub fn parse(text: &str) -> Self {
        let mut game = Self::default();
        for line in text.lines() {
            let line = line.trim();
            let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let words = || rest.split_whitespace().map(str::to_owned);
            match word {
                "title" => game.title = rest.trim().to_owned(),
                "run" => game.run.extend(words()),
                "maps" => game.maps.extend(words()),
                "log" if !rest.trim().is_empty() => game.log = Some(PathBuf::from(rest.trim())),
                _ => {}
            }
        }
        game
    }

    /// The game beside the program, if there is one: read, and the program moved to its
    /// folder, which the game's paths are from.
    pub fn beside_program() -> Option<Self> {
        let program = std::env::current_exe().ok()?;
        let folder = program.parent()?;
        let text = std::fs::read_to_string(folder.join("game.txt")).ok()?;
        let game = Self::parse(&text);
        if let Err(error) = std::env::set_current_dir(folder) {
            // Said where it can be: there is no log yet.
            eprintln!("{}: {error}", folder.display());
        }
        let _ = TITLE.set(game.title.clone());
        Some(game)
    }

    /// What the program is run with: the game's own arguments, then the ones it was given.
    pub fn arguments(&self, given: Vec<String>) -> Vec<String> {
        self.run.iter().cloned().chain(given).collect()
    }

    /// The file the log is written to, made ready: its folder is there, and it is empty.
    pub fn log_file(&self) -> Option<std::fs::File> {
        let path = self.log.as_ref()?;
        if let Some(folder) = path.parent().filter(|folder| *folder != Path::new("")) {
            let _ = std::fs::create_dir_all(folder);
        }
        std::fs::File::create(path).ok()
    }
}

/// Whether the program has a console to say things on. One opened with a double click that
/// was built to have a window has none.
pub fn has_console() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetConsoleWindow() -> isize;
        }
        // SAFETY: it takes nothing and gives back a handle, or none.
        unsafe { GetConsoleWindow() != 0 }
    }
    #[cfg(not(windows))]
    true
}

/// Says that the program cannot go on, in a box of its own, to somebody who opened it with a
/// double click and has no console to read it on. Nothing, where there is a console and no
/// game is being run: there it has been said already.
pub fn alert(text: &str) {
    let title = match TITLE.get() {
        Some(title) => title.as_str(),
        None if !has_console() => "Solstice",
        None => return,
    };
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(owner: isize, text: *const u16, title: *const u16, kind: u32) -> i32;
        }
        const MB_ICONERROR: u32 = 0x10;
        let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain([0]).collect() };
        let (text, title) = (wide(text), wide(title));
        // SAFETY: both are strings that end in a zero, and outlive the call.
        unsafe {
            MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_ICONERROR);
        }
    }
    #[cfg(not(windows))]
    eprintln!("{title}: {text}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_file_is_read_and_what_is_given_by_hand_comes_last() {
        let game = Game::parse(
            "# The game.\r\ntitle Aussie Outback 4x4\r\nrun --world scenes/outback.world --ui ui\r\n\
             run --size 1920x1080\r\nmaps https://a.example  https://b.example\r\nlog save/game.log\r\n\
             colour ochre\r\n",
        );
        assert_eq!(game.title, "Aussie Outback 4x4");
        assert_eq!(game.maps, ["https://a.example", "https://b.example"]);
        assert_eq!(game.log, Some(PathBuf::from("save/game.log")));
        assert_eq!(
            game.arguments(vec!["--size".into(), "2560x1440".into()]),
            [
                "--world",
                "scenes/outback.world",
                "--ui",
                "ui",
                "--size",
                "1920x1080",
                "--size",
                "2560x1440"
            ]
        );
        // Nothing in it: nothing asked for, and no log.
        assert_eq!(Game::parse("log\n"), Game::default());
    }
}
