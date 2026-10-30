//! The saved game: a main file that says which day the game is on, and one file for every day.
//!
//! Everything is under the `Saves` folder of the game's data folder (what a day's file holds is
//! in [`crate::day`]):
//!
//! ```text
//! Saves/game.json                          the main file: the current day
//! Saves/Days/year-0001_month-01_day-01.json  the state of each day, kept forever
//! ```
//!
//! All of them are plain JSON with a version number, so other programs can read them for
//! statistics, and old days stay readable when new things are added to the game.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::day::DaySave;

const SAVES_DIR: &str = "Saves";
const DAYS_DIR: &str = "Days";
const MAIN_FILE: &str = "game.json";
/// The version of the main file, and of a day's file. A day's file is version 2: version 1 named
/// the things that belong to a country by its name, and version 2 refers to everything by id.
const MAIN_VERSION: u32 = 1;
pub const DAY_VERSION: u32 = 2;

/// The main file: what the game needs to know to carry on where it left off.
#[derive(Serialize, Deserialize)]
struct MainFile {
    version: u32,
    /// The day the game is on.
    date: Date,
}

/// The saved game as found at startup.
#[derive(Debug)]
pub struct Opened {
    /// The current day's save.
    pub day: DaySave,
    /// Whether there was no saved game, so a new one was started.
    pub new_game: bool,
}

pub struct Saves {
    dir: PathBuf,
}

impl Saves {
    /// The saves of the game whose files are in `root`.
    pub fn new(root: &Path) -> Self {
        Self { dir: root.join(SAVES_DIR) }
    }

    fn main_path(&self) -> PathBuf {
        self.dir.join(MAIN_FILE)
    }

    fn day_path(&self, date: Date) -> PathBuf {
        self.dir.join(DAYS_DIR).join(format!("{}.json", date.file_stem()))
    }

    /// Saves `day`, replacing that day's file. The main file doesn't change: it says which day the
    /// game is on, not what is in it.
    pub fn save_day(&self, day: &DaySave) -> Result<(), String> {
        write_json(&self.day_path(day.date), day)
    }

    /// Ends the day `current` and starts the day after it, which begins as `current` is with the day's rules
    /// run on it (see [`DaySave::next_day`]), and makes it the day the game is on. `current` is saved first, as it ended,
    /// then the new day's file, then the main file, so that stopping at any point leaves the game on a day
    /// that is saved. Returns the new day. An error says what couldn't be written, and the game stays on
    /// `current`.
    pub fn advance(&self, current: &DaySave) -> Result<DaySave, String> {
        let next = current.next_day();
        self.save_day(current)?;
        self.save_day(&next)?;
        write_json(&self.main_path(), &MainFile { version: MAIN_VERSION, date: next.date })?;
        Ok(next)
    }

    /// Opens the saved game. With no main file this is a new game: the day is set to 1-1-1 and the
    /// main file and the first day's save are written. Otherwise the main file says which day to
    /// load. A file that exists but can't be read is an error, never replaced, so a bad file can't
    /// cost the game its history.
    pub fn open(&self, map: &str) -> Result<Opened, String> {
        let (date, new_game) = match read_json::<MainFile>(&self.main_path())? {
            Some(main) => {
                if main.version != MAIN_VERSION {
                    return Err(format!("{} is version {}, and this game reads version {MAIN_VERSION}.", self.main_path().display(), main.version));
                }
                if !main.date.is_valid() {
                    return Err(format!("{} says the day is {}, which isn't a day of the calendar.", self.main_path().display(), main.date));
                }
                (main.date, false)
            }
            None => (Date::FIRST, true),
        };

        let day = match read_json::<DaySave>(&self.day_path(date))? {
            Some(day) if day.version != DAY_VERSION => {
                return Err(format!("{} is version {}, and this game reads version {DAY_VERSION}.", self.day_path(date).display(), day.version));
            }
            Some(day) => day,
            None => {
                // The first day of a new game, or a day whose save went missing: start it from here.
                let day = DaySave::new(date, map);
                write_json(&self.day_path(date), &day)?;
                day
            }
        };
        if new_game {
            // After the day's save, so a game that stopped in between starts over cleanly.
            write_json(&self.main_path(), &MainFile { version: MAIN_VERSION, date })?;
        }
        Ok(Opened { day, new_game })
    }
}

/// Reads a JSON file, or `None` if there isn't one.
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Couldn't read {}: {e}", path.display())),
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("Couldn't understand {}: {e}", path.display()))
}

/// Writes a JSON file through a temporary file, so a crash can't leave half of one behind.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("Couldn't write {}: {e}", path.display());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
    text.push('\n');
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text).map_err(fail)?;
    fs::rename(&temp, path).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempRoot;

    const MAP: &str = "Assets/map.nworld";

    fn saves(root: &TempRoot) -> Saves {
        Saves::new(&root.0)
    }

    #[test]
    fn a_new_game_starts_on_1_1_1_and_writes_both_files() {
        let root = TempRoot::new();
        let saves = saves(&root);
        let opened = saves.open(MAP).unwrap();

        assert!(opened.new_game);
        assert_eq!(opened.day.date, Date::FIRST);
        assert_eq!(opened.day.map, MAP);
        let main = fs::read_to_string(root.0.join("Saves/game.json")).unwrap();
        assert!(main.contains("\"year\": 1") && main.contains("\"month\": 1") && main.contains("\"day\": 1"));
        assert!(root.0.join("Saves/Days/year-0001_month-01_day-01.json").is_file());
    }

    #[test]
    fn advancing_saves_the_day_that_ended_and_starts_the_next_from_it() {
        let root = TempRoot::new();
        let saves = saves(&root);
        let mut day = saves.open(MAP).unwrap().day;
        day.found_country("Aldoria").unwrap();

        let next = saves.advance(&day).unwrap();
        assert_eq!(next.date.to_string(), "1-1-2");
        assert_eq!(next.countries, day.countries, "what carries forward does");
        assert!(next.newsreel.is_empty(), "the news doesn't");
        // The day that ended was saved as it was, news and all, and stays as it was.
        let first: DaySave = serde_json::from_str(&fs::read_to_string(root.0.join("Saves/Days/year-0001_month-01_day-01.json")).unwrap()).unwrap();
        assert_eq!(first, day);
        assert!(root.0.join("Saves/Days/year-0001_month-01_day-02.json").is_file());
        // The game is now on the new day, and opens on it.
        let opened = saves.open(MAP).unwrap();
        assert!(!opened.new_game);
        assert_eq!(opened.day, next);
    }

    #[test]
    fn the_rules_run_on_the_new_day_and_not_on_the_one_that_ended() {
        let root = TempRoot::new();
        let saves = saves(&root);
        let mut day = saves.open(MAP).unwrap().day;
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 250;

        let next = saves.advance(&day).unwrap();
        assert_eq!(next.countries[0].money, 250, "a coin for each person");
        // The day that ended is as it was, with no income, and the day after is saved with it.
        let ended: DaySave = serde_json::from_str(&fs::read_to_string(root.0.join("Saves/Days/year-0001_month-01_day-01.json")).unwrap()).unwrap();
        assert_eq!(ended.countries[0].money, 0);
        assert_eq!(saves.open(MAP).unwrap().day.countries[0].money, 250);
        // And again the next day, on the people the city has grown to by then.
        let after = saves.advance(&next).unwrap();
        assert_eq!(after.countries[0].money, 250 + i64::from(next.cities[0].population));
        assert!(next.cities[0].population > 250, "a free city grows");
    }

    #[test]
    fn advancing_goes_on_day_after_day() {
        let root = TempRoot::new();
        let saves = saves(&root);
        let mut day = saves.open(MAP).unwrap().day;
        for _ in 0..25 {
            day = saves.advance(&day).unwrap();
        }
        assert_eq!(day.date.to_string(), "1-2-6");
        assert_eq!(saves.open(MAP).unwrap().day.date, day.date);
        assert_eq!(fs::read_dir(root.0.join("Saves/Days")).unwrap().count(), 26, "every day has its file");
    }

    #[test]
    fn opening_again_carries_on_with_what_was_saved() {
        let root = TempRoot::new();
        let saves = saves(&root);
        saves.open(MAP).unwrap();
        let opened = saves.open("Assets/other.nworld").unwrap();

        assert!(!opened.new_game);
        assert_eq!(opened.day.date, Date::FIRST);
        // The day's save is what was written the first time, not remade.
        assert_eq!(opened.day.map, MAP);
    }

    #[test]
    fn the_main_file_decides_which_day_is_loaded() {
        let root = TempRoot::new();
        let saves = saves(&root);
        let date = Date { year: 2, month: 7, day: 13 };
        write_json(&saves.main_path(), &MainFile { version: MAIN_VERSION, date }).unwrap();
        write_json(&saves.day_path(date), &DaySave::new(date, "Assets/later.nworld")).unwrap();

        let opened = saves.open(MAP).unwrap();
        assert!(!opened.new_game);
        assert_eq!(opened.day, DaySave::new(date, "Assets/later.nworld"));
    }

    #[test]
    fn a_saved_day_keeps_its_countries_and_newsreel() {
        let root = TempRoot::new();
        let saves = saves(&root);
        let mut day = saves.open(MAP).unwrap().day;
        day.found_country("Aldoria").unwrap();
        saves.save_day(&day).unwrap();

        let reopened = saves.open(MAP).unwrap();
        assert!(!reopened.new_game);
        assert_eq!(reopened.day, day);
        assert_eq!(reopened.day.countries.len(), 1);
        assert_eq!(reopened.day.newsreel.len(), 1);
    }

    #[test]
    fn a_day_whose_save_is_missing_is_started_afresh() {
        let root = TempRoot::new();
        let saves = saves(&root);
        saves.open(MAP).unwrap();
        fs::remove_file(saves.day_path(Date::FIRST)).unwrap();

        let opened = saves.open(MAP).unwrap();
        assert!(!opened.new_game);
        assert!(saves.day_path(Date::FIRST).is_file());
    }

    #[test]
    fn a_game_that_stopped_before_the_main_file_was_written_starts_over() {
        let root = TempRoot::new();
        let saves = saves(&root);
        write_json(&saves.day_path(Date::FIRST), &DaySave::new(Date::FIRST, MAP)).unwrap();

        let opened = saves.open(MAP).unwrap();
        assert!(opened.new_game);
        assert!(saves.main_path().is_file());
    }

    #[test]
    fn a_broken_main_file_is_an_error_and_is_left_alone() {
        let root = TempRoot::new();
        let saves = saves(&root);
        fs::create_dir_all(&saves.dir).unwrap();
        fs::write(saves.main_path(), "not json").unwrap();

        assert!(saves.open(MAP).is_err());
        assert_eq!(fs::read_to_string(saves.main_path()).unwrap(), "not json");
        assert!(!saves.day_path(Date::FIRST).exists());
    }

    #[test]
    fn a_day_that_is_not_in_the_calendar_is_an_error() {
        let root = TempRoot::new();
        let saves = saves(&root);
        write_json(&saves.main_path(), &MainFile { version: MAIN_VERSION, date: Date { year: 1, month: 11, day: 1 } }).unwrap();
        assert!(saves.open(MAP).unwrap_err().contains("isn't a day of the calendar"));
    }

    #[test]
    fn a_file_from_a_newer_version_is_an_error() {
        let root = TempRoot::new();
        let saves = saves(&root);
        write_json(&saves.main_path(), &MainFile { version: MAIN_VERSION + 1, date: Date::FIRST }).unwrap();
        assert!(saves.open(MAP).unwrap_err().contains("version"));
    }

    #[test]
    fn writing_leaves_no_temporary_file_behind() {
        let root = TempRoot::new();
        let saves = saves(&root);
        saves.open(MAP).unwrap();
        let leftovers: Vec<_> = [saves.dir.clone(), saves.dir.join(DAYS_DIR)]
            .iter()
            .flat_map(|d| fs::read_dir(d).unwrap())
            .filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
