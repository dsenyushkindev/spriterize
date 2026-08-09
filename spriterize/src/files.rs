//! File dialogs and the list of recently used paths.
//!
//! The dialogs live here rather than in the menu so that both the menu and the
//! keyboard shortcuts can reach them.

use std::path::{Path, PathBuf};

/// Extension of a saved project, as opposed to an imported or exported image.
pub const PROJECT_EXTENSION: &str = "spriterize";
pub use crate::collection::COLLECTION_EXTENSION;

const MAX_RECENT: usize = 10;
const APP_DIR: &str = "spriterize";
const RECENT_FILE_NAME: &str = "recent.txt";

/// Whether a path names a project, rather than an image.
pub fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(PROJECT_EXTENSION))
}

pub fn is_collection(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(COLLECTION_EXTENSION))
}

/// Starts a dialog in the directory of the file currently in use, so that the
/// user isn't dropped somewhere unrelated each time.
fn dialog(near: Option<&Path>) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();

    if let Some(dir) = near.and_then(|path| path.parent()) {
        dialog = dialog.set_directory(dir);
    }

    dialog
}

pub fn open_project(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter("Spriterize projects", &[PROJECT_EXTENSION])
        .add_filter("All files", &["*"])
        .pick_file()
}

/// Opens any document the application understands. Used by the start screen,
/// where choosing a document should not require knowing its kind first.
pub fn open_document(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter(
            "Spriterize documents",
            &[
                PROJECT_EXTENSION,
                COLLECTION_EXTENSION,
                "png",
                "jpg",
                "jpeg",
            ],
        )
        .add_filter("Spriterize projects", &[PROJECT_EXTENSION])
        .add_filter("Spriterize asset collections", &[COLLECTION_EXTENSION])
        .add_filter("Images", &["png", "jpg", "jpeg"])
        .add_filter("All files", &["*"])
        .pick_file()
}

pub fn save_project(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter("Spriterize projects", &[PROJECT_EXTENSION])
        .add_filter("All files", &["*"])
        .set_file_name(&format!("project.{PROJECT_EXTENSION}"))
        .save_file()
}

pub fn open_collection(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter("Spriterize asset collections", &[COLLECTION_EXTENSION])
        .add_filter("All files", &["*"])
        .pick_file()
}

pub fn save_collection(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter("Spriterize asset collections", &[COLLECTION_EXTENSION])
        .add_filter("All files", &["*"])
        .set_file_name(&format!("assets.{COLLECTION_EXTENSION}"))
        .save_file()
}

pub fn export_collection_dir(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near).pick_folder()
}

pub fn export_image(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter("PNG files", &["png"])
        .add_filter("JPEG files", &["jpg", "jpeg"])
        .add_filter("All files", &["*"])
        .save_file()
}

/// Picks the directory to write one image per layer into. Layers are named
/// individually, so there is no file name to choose.
pub fn export_layers_dir(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near).pick_folder()
}

pub fn import_image(near: Option<&Path>) -> Option<PathBuf> {
    dialog(near)
        .add_filter("Images", &["png", "jpg", "jpeg"])
        .add_filter("All files", &["*"])
        .pick_file()
}

/// Paths the user has opened or saved, most recent first.
///
/// Persisted between runs in the user's config directory. Read and write
/// failures are ignored on purpose: not being able to remember recent files
/// should never keep the editor from starting or from saving actual work.
#[derive(Debug, Default)]
pub struct RecentFiles {
    paths: Vec<PathBuf>,
}

impl RecentFiles {
    pub fn load() -> Self {
        let Some(config) = config_path(RECENT_FILE_NAME) else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(config) else {
            return Self::default();
        };

        Self {
            paths: text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .take(MAX_RECENT)
                .collect(),
        }
    }

    /// Records a path as the most recently used one, moving it to the front if
    /// it was already in the list, and persists the result.
    pub fn push(&mut self, path: PathBuf) {
        self.remember(path);
        self.save();
    }

    /// The list update on its own, without touching the disk.
    fn remember(&mut self, path: PathBuf) {
        self.paths.retain(|known| known != &path);
        self.paths.insert(0, path);
        self.paths.truncate(MAX_RECENT);
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.save();
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    fn save(&self) {
        let Some(config) = config_path(RECENT_FILE_NAME) else {
            return;
        };

        if let Some(dir) = config.parent() {
            let _ = std::fs::create_dir_all(dir);
        }

        let text: String = self
            .paths
            .iter()
            .map(|path| format!("{}\n", path.to_string_lossy()))
            .collect();

        let _ = std::fs::write(config, text);
    }
}

/// Path of a config file: `%APPDATA%\spriterize\<name>` on Windows, and
/// `$XDG_CONFIG_HOME/spriterize/<name>` (falling back to `~/.config`)
/// elsewhere.
pub fn config_path(file_name: &str) -> Option<PathBuf> {
    let dir = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }?;

    Some(dir.join(APP_DIR).join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_project_paths() {
        assert!(is_project(Path::new("hero.spriterize")));
        assert!(is_project(Path::new("HERO.SPRITERIZE")));
        assert!(!is_project(Path::new("hero.png")));
        assert!(!is_project(Path::new("hero")));
    }

    #[test]
    fn recognizes_collection_paths() {
        assert!(is_collection(Path::new("game.spriterize-collection")));
        assert!(is_collection(Path::new("GAME.SPRITERIZE-COLLECTION")));
        assert!(!is_collection(Path::new("game.spriterize")));
    }

    #[test]
    fn most_recent_comes_first() {
        let mut recent = RecentFiles::default();
        recent.remember("a".into());
        recent.remember("b".into());

        assert_eq!(recent.paths(), &[PathBuf::from("b"), PathBuf::from("a")]);
    }

    #[test]
    fn reopening_moves_a_path_up_instead_of_duplicating_it() {
        let mut recent = RecentFiles::default();
        recent.remember("a".into());
        recent.remember("b".into());
        recent.remember("a".into());

        assert_eq!(recent.paths(), &[PathBuf::from("a"), PathBuf::from("b")]);
    }

    #[test]
    fn oldest_paths_fall_off_the_end() {
        let mut recent = RecentFiles::default();

        for i in 0..MAX_RECENT + 5 {
            recent.remember(PathBuf::from(format!("file{i}")));
        }

        assert_eq!(recent.paths().len(), MAX_RECENT);
        assert_eq!(
            recent.paths()[0],
            PathBuf::from(format!("file{}", MAX_RECENT + 4))
        );
    }
}
