//! Autosave: keeps the saved day up to date with the game, without the player doing anything.
//!
//! The current day is saved every [`INTERVAL`] seconds if anything in it has changed since it was
//! last saved, and once more when the game closes. A save that fails is tried again at the next
//! interval, and the game says so meanwhile.

use crate::day::DaySave;
use crate::saves::Saves;

/// How often a changed day is saved, in seconds.
pub const INTERVAL: f64 = 30.0;

pub struct Autosave {
    /// The day as last written to its file.
    saved: DaySave,
    /// When a save was last looked at, in seconds of game time.
    checked_at: f64,
    /// What went wrong with the last save, until one succeeds.
    error: Option<String>,
}

impl Autosave {
    /// For a game whose current day is `day`, as it is saved now.
    pub fn new(day: &DaySave, now: f64) -> Self {
        Self { saved: day.clone(), checked_at: now, error: None }
    }

    /// Carries on with `day`, which has just been saved by something else, such as the start of a new day.
    pub fn reset(&mut self, day: &DaySave, now: f64) {
        *self = Self::new(day, now);
    }

    /// Call every frame with the current day and time. Saves it if it has changed and it is time.
    pub fn update(&mut self, saves: &Saves, day: &DaySave, now: f64) {
        if now - self.checked_at >= INTERVAL {
            self.checked_at = now;
            self.flush(saves, day);
        }
    }

    /// Saves `day` now if it has changed since it was last saved. Used when the game closes.
    pub fn flush(&mut self, saves: &Saves, day: &DaySave) {
        if *day != self.saved {
            // A failure is kept for the game to show, and the save is tried again later.
            let _ = self.save_now(saves, day);
        }
    }

    /// Saves `day` now, changed or not, for something that must not be lost, like founding a country.
    pub fn save_now(&mut self, saves: &Saves, day: &DaySave) -> Result<(), String> {
        match saves.save_day(day) {
            Ok(()) => {
                self.saved = day.clone();
                self.error = None;
                Ok(())
            }
            Err(message) => {
                self.error = Some(message.clone());
                Err(message)
            }
        }
    }

    /// Why the last save failed, if it did.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::test_support::TempRoot;
    use std::fs;

    const MAP: &str = "Assets/map.nworld";

    fn opened(root: &TempRoot) -> (Saves, DaySave) {
        let saves = Saves::new(&root.0);
        let day = saves.open(MAP).unwrap().day;
        (saves, day)
    }

    fn day_file(root: &TempRoot) -> std::path::PathBuf {
        root.0.join("Saves/Days").join(format!("{}.json", Date::FIRST.file_stem()))
    }

    #[test]
    fn a_changed_day_is_saved_once_the_interval_has_passed() {
        let root = TempRoot::new();
        let (saves, mut day) = opened(&root);
        let mut autosave = Autosave::new(&day, 0.0);

        day.found_country("Aldoria").unwrap();
        autosave.update(&saves, &day, INTERVAL - 1.0);
        assert!(saves.open(MAP).unwrap().day.countries.is_empty(), "saved too early");

        autosave.update(&saves, &day, INTERVAL);
        assert_eq!(saves.open(MAP).unwrap().day, day);
    }

    #[test]
    fn an_unchanged_day_is_not_written_again() {
        let root = TempRoot::new();
        let (saves, day) = opened(&root);
        let mut autosave = Autosave::new(&day, 0.0);

        fs::remove_file(day_file(&root)).unwrap();
        autosave.update(&saves, &day, INTERVAL * 3.0);
        assert!(!day_file(&root).exists());
    }

    #[test]
    fn closing_the_game_saves_a_change_that_is_not_yet_due() {
        let root = TempRoot::new();
        let (saves, mut day) = opened(&root);
        let mut autosave = Autosave::new(&day, 0.0);

        day.found_country("Aldoria").unwrap();
        autosave.flush(&saves, &day);
        assert_eq!(saves.open(MAP).unwrap().day.countries.len(), 1);
    }

    #[test]
    fn each_interval_looks_again_for_changes() {
        let root = TempRoot::new();
        let (saves, mut day) = opened(&root);
        let mut autosave = Autosave::new(&day, 0.0);

        day.found_country("Aldoria").unwrap();
        autosave.update(&saves, &day, INTERVAL);
        day.found_country("Brevon").unwrap();
        autosave.update(&saves, &day, INTERVAL + 1.0);
        assert_eq!(saves.open(MAP).unwrap().day.countries.len(), 1, "the second change had to wait for its interval");
        autosave.update(&saves, &day, INTERVAL * 2.0);
        assert_eq!(saves.open(MAP).unwrap().day.countries.len(), 2);
    }

    #[test]
    fn a_failed_save_is_reported_and_tried_again() {
        let root = TempRoot::new();
        let (saves, mut day) = opened(&root);
        let mut autosave = Autosave::new(&day, 0.0);

        // A folder where the day's file should go makes writing it fail.
        fs::remove_file(day_file(&root)).unwrap();
        fs::create_dir(day_file(&root)).unwrap();
        day.found_country("Aldoria").unwrap();
        autosave.update(&saves, &day, INTERVAL);
        assert!(autosave.error().is_some());

        fs::remove_dir(day_file(&root)).unwrap();
        autosave.update(&saves, &day, INTERVAL * 2.0);
        assert!(autosave.error().is_none());
        assert_eq!(saves.open(MAP).unwrap().day, day);
    }

    #[test]
    fn saving_now_records_the_day_so_autosave_does_not_repeat_it() {
        let root = TempRoot::new();
        let (saves, mut day) = opened(&root);
        let mut autosave = Autosave::new(&day, 0.0);

        day.found_country("Aldoria").unwrap();
        autosave.save_now(&saves, &day).unwrap();
        fs::remove_file(day_file(&root)).unwrap();
        autosave.update(&saves, &day, INTERVAL);
        assert!(!day_file(&root).exists());
    }
}
