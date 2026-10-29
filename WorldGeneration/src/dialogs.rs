//! Native file and message dialogs.

use std::path::{Path, PathBuf};

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::storage::EXTENSION;

fn file_dialog() -> FileDialog {
    FileDialog::new().add_filter("NWorld map", &[EXTENSION])
}

/// Asks where to save a map. `current` is the file the map was last saved to or opened from,
/// which the dialog starts in. Returns `None` if the dialog was cancelled.
pub fn pick_save(current: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = file_dialog().set_title("Save map");
    dialog = match current {
        Some(path) => {
            let dialog = match path.parent().filter(|dir| dir.is_dir()) {
                Some(dir) => dialog.set_directory(dir),
                None => dialog,
            };
            match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => dialog.set_file_name(name),
                None => dialog,
            }
        }
        None => dialog.set_file_name(format!("map.{EXTENSION}")),
    };
    let mut path = dialog.save_file()?;
    // The dialog only adds the extension if the platform feels like it.
    if path.extension().is_none() {
        path.set_extension(EXTENSION);
    }
    Some(path)
}

/// Asks which map file to open. Returns `None` if the dialog was cancelled.
pub fn pick_open() -> Option<PathBuf> {
    file_dialog().set_title("Open map").pick_file()
}

pub enum Unsaved {
    Save,
    Discard,
    Cancel,
}

/// "Do you want to save changes to `name` before you `action`?" with Save, Don't save and
/// Cancel.
pub fn ask_unsaved(name: &str, action: &str) -> Unsaved {
    let answer = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("Unsaved changes")
        .set_description(format!(
            "Do you want to save the changes to {name} before you {action}?\n\n\
             Yes saves them, No throws them away, and Cancel goes back to the map."
        ))
        .set_buttons(MessageButtons::YesNoCancel)
        .show();
    match answer {
        MessageDialogResult::Yes => Unsaved::Save,
        MessageDialogResult::No => Unsaved::Discard,
        _ => Unsaved::Cancel,
    }
}
