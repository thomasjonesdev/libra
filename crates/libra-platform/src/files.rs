//! `Effect::PickFile` and `Effect::SaveFile` (PLAN-NATIVE-INTEGRATIONS).
//!
//! OS sheets via `rfd`. Tests do not call [`pick`] or [`save`].

use std::path::PathBuf;

use libra_core::FilePick;

pub fn pick(title: &str) -> FilePick {
    path_result(rfd::FileDialog::new().set_title(title).pick_file())
}

pub fn save(title: &str, name: Option<&str>) -> FilePick {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(name) = name {
        dialog = dialog.set_file_name(name);
    }
    path_result(dialog.save_file())
}

fn path_result(path: Option<PathBuf>) -> FilePick {
    match path {
        Some(path) => FilePick::Picked(path.to_string_lossy().into_owned()),
        None => FilePick::Cancelled,
    }
}
