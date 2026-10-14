//! Opening and saving case files through a native file dialog (no drawing code).
//!
//! The dialog sits behind the [`FileChooser`] trait so the decisions around it (start folder, suggested
//! name, extension, cancel handling, errors) are unit-tested without a window. [`NativeChooser`] uses
//! the `rfd` crate (Win32 common dialogs, macOS `NSOpenPanel`, the XDG desktop portal on Linux). It
//! blocks the UI thread while the dialog is open. The typed-path prompt in the File menu stays as the
//! fallback for systems where no dialog can be shown (for example Linux without a desktop portal,
//! where `rfd` gives the same `None` as a cancel).

use crate::controller::Controller;
use std::path::{Path, PathBuf};

/// Case file extensions offered in dialogs.
pub const CASE_EXTENSIONS: &[&str] = &["toml", "json"];

/// Something that can ask the user for a case file path.
pub trait FileChooser {
    /// Ask for a case file to open; `None` when cancelled or unavailable.
    fn pick_open(&self, start_dir: Option<&Path>) -> Option<PathBuf>;
    /// Ask where to save; `None` when cancelled or unavailable.
    fn pick_save(&self, start_dir: Option<&Path>, suggested_name: &str) -> Option<PathBuf>;
}

/// The operating system's file dialog.
pub struct NativeChooser;

impl FileChooser for NativeChooser {
    fn pick_open(&self, start_dir: Option<&Path>) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new()
            .set_title("Open case")
            .add_filter("rcfd case (.toml, .json)", CASE_EXTENSIONS);
        if let Some(s) = start_dir {
            d = d.set_directory(s);
        }
        d.pick_file()
    }

    fn pick_save(&self, start_dir: Option<&Path>, suggested_name: &str) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new()
            .set_title("Save case as")
            .set_file_name(suggested_name)
            .add_filter("rcfd case (.toml, .json)", CASE_EXTENSIONS);
        if let Some(s) = start_dir {
            d = d.set_directory(s);
        }
        d.save_file()
    }
}

/// Folder a dialog should start in: the folder of the current file if it exists, else `fallback` if it exists.
pub fn start_dir(current: Option<&Path>, fallback: Option<&Path>) -> Option<PathBuf> {
    current
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty() && p.is_dir())
        .or_else(|| fallback.filter(|p| p.is_dir()))
        .map(Path::to_path_buf)
}

/// File name proposed in the save dialog.
pub fn suggested_name(current: Option<&Path>) -> String {
    current
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "case.toml".to_string())
}

/// Give a saved path a case extension when it has none (the save dialog of some systems
/// does not add one). A recognised extension, in any letter case, is kept.
pub fn with_case_extension(p: PathBuf) -> PathBuf {
    let known = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| CASE_EXTENSIONS.iter().any(|k| k.eq_ignore_ascii_case(e)));
    if known {
        p
    } else {
        let mut s = p.into_os_string();
        s.push(".toml");
        PathBuf::from(s)
    }
}

/// What a dialog-driven action did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The user cancelled (or no dialog was available); nothing changed.
    Cancelled,
    /// The file was opened or saved.
    Done(PathBuf),
    /// The file was chosen but the operation failed; the message is for the status line.
    Failed(String),
}

/// File > Open: ask for a file and open it.
pub fn open_with_dialog(ctrl: &mut Controller, chooser: &dyn FileChooser, fallback_dir: Option<&Path>) -> Outcome {
    let start = start_dir(ctrl.file.as_deref(), fallback_dir);
    let Some(path) = chooser.pick_open(start.as_deref()) else {
        return Outcome::Cancelled;
    };
    match ctrl.open_path(&path) {
        Ok(()) => Outcome::Done(path),
        Err(e) => Outcome::Failed(e),
    }
}

/// File > Save as: ask for a path and save there.
pub fn save_as_with_dialog(ctrl: &mut Controller, chooser: &dyn FileChooser, fallback_dir: Option<&Path>) -> Outcome {
    let start = start_dir(ctrl.file.as_deref(), fallback_dir);
    let name = suggested_name(ctrl.file.as_deref());
    let Some(path) = chooser.pick_save(start.as_deref(), &name) else {
        return Outcome::Cancelled;
    };
    let path = with_case_extension(path);
    match ctrl.save_to(&path) {
        Ok(()) => Outcome::Done(path),
        Err(e) => Outcome::Failed(e),
    }
}
