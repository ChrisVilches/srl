//! Finding and creating a store, and listing its notes (see
//! `docs/store.md`).

use std::fs;
use std::path::{Path, PathBuf};

use globset::GlobSet;
use unicode_normalization::UnicodeNormalization;

use crate::{Error, Result};

/// The folder that marks a directory as a store and holds everything `srl`
/// owns.
const SRL: &str = ".srl";

/// The configuration file, in `.srl/`.
const CONFIG_FILE: &str = "config.toml";

/// The folder of the review log, in `.srl/`.
const LOG: &str = "log";

/// The folder of derived data, in `.srl/`.
const CACHE: &str = "cache";

/// The default store, relative to the platform's data directory.
const DEFAULT_STORE: &str = "srl";

/// The `config.toml` that `srl init` writes: every key at its default,
/// commented out (see `docs/store.md`, "Configuration").
pub(crate) const DEFAULT_CONFIG: &str = r#"# srl configuration. Every key is optional; these are the defaults.

[notes]
# include = ["**/*.md"]      # globs of notes, relative to the store's folder
# exclude = []               # globs of notes to ignore; they win over include

[schedule]
# desired_retention = 0.9    # FSRS target probability of recall
# day_starts_at = 4          # hour (local time) when a new study day begins
# new_per_day = 10           # new cards introduced per day, across all selectors
# leech_lapses = 8           # a card with this many lapses is a leech
# mature_days = 21           # a card with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights (21, or 19 or 17); default weights if absent

[kinds]
# ja-words = ["python3", "kinds/ja_words.py"]   # kind:ja-words runs this command

[kind_programs]
# card_timeout = 60          # seconds a kind program may take to make a card
"#;

/// Everything needed to find a store, gathered by the caller.
///
/// The core never reads the process environment itself; the CLI fills this
/// in from `--dir`, `SRL_DIR`, the current directory and the platform's data
/// directory.
#[derive(Debug, Clone, Default)]
pub struct Lookup {
    /// The `--dir` flag.
    pub dir: Option<PathBuf>,
    /// The `SRL_DIR` environment variable.
    pub env_dir: Option<PathBuf>,
    /// The current directory.
    pub cwd: PathBuf,
    /// The platform's directory for application data, if there is one:
    /// `$XDG_DATA_HOME` or `~/.local/share` on Linux, `~/Library/Application
    /// Support` on macOS, `%APPDATA%` on Windows (see `docs/store.md`).
    pub data_dir: Option<PathBuf>,
}

/// One `srl` store: a folder of notes with a `.srl/` folder in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Finds the store, using the first match of: `--dir`, `SRL_DIR`, the
    /// nearest directory from `cwd` upwards that contains `.srl/`, and the
    /// default store in the platform's data directory.
    pub fn locate(lookup: &Lookup) -> Result<Self> {
        lookup
            .dir
            .clone()
            .or_else(|| lookup.env_dir.clone())
            .or_else(|| marked_ancestor(&lookup.cwd))
            .or_else(|| {
                lookup
                    .data_dir
                    .as_ref()
                    .map(|data| data.join(DEFAULT_STORE))
            })
            .map(|root| Self { root })
            .ok_or_else(|| Error::NoStore {
                cwd: lookup.cwd.clone(),
            })
    }

    /// Finds the store like [`Store::locate`], and checks that it exists.
    pub fn open(lookup: &Lookup) -> Result<Self> {
        let store = Self::locate(lookup)?;
        if store.config_file().is_file() {
            Ok(store)
        } else {
            Err(Error::NotAStore { root: store.root })
        }
    }

    /// The directory `srl init [DIR]` creates a store in: `DIR`, `--dir`,
    /// `SRL_DIR` or the current directory, whichever comes first.
    pub fn init_target(lookup: &Lookup, dir: Option<PathBuf>) -> PathBuf {
        dir.or_else(|| lookup.dir.clone())
            .or_else(|| lookup.env_dir.clone())
            .map_or_else(|| lookup.cwd.clone(), |dir| lookup.cwd.join(dir))
    }

    /// Creates a store in `root`, which may already exist and hold notes
    /// and other files, and returns the paths it creates. With `dry_run`,
    /// only returns them.
    pub fn init(root: &Path, dry_run: bool) -> Result<Vec<PathBuf>> {
        let store = Self {
            root: root.to_path_buf(),
        };
        if store.srl_dir().exists() {
            return Err(Error::StoreExists {
                root: root.to_path_buf(),
            });
        }
        let config = store.config_file();
        let log = store.log_dir();
        if !dry_run {
            fs::create_dir_all(&log).map_err(Error::io(&log))?;
            fs::write(&config, DEFAULT_CONFIG).map_err(Error::io(&config))?;
        }
        Ok(vec![config, log])
    }

    /// The store's folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder of everything `srl` owns, `.srl/`.
    pub fn srl_dir(&self) -> PathBuf {
        self.root.join(SRL)
    }

    /// The configuration file, `.srl/config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.srl_dir().join(CONFIG_FILE)
    }

    /// The folder of the review log.
    pub fn log_dir(&self) -> PathBuf {
        self.srl_dir().join(LOG)
    }

    /// The folder of derived data, safe to delete.
    pub fn cache_dir(&self) -> PathBuf {
        self.srl_dir().join(CACHE)
    }

    /// Every note: each file under the store's folder, at any depth,
    /// outside folders whose name starts with `.`, matched by `include` and
    /// not by `exclude` (see `docs/notes.md`, "Which files are notes"). Paths are relative to the store's folder, with `/`
    /// between folders, as named on disk, and sorted by their [NFC](nfc)
    /// form, so that the Unicode form a file system gives names in never
    /// changes the order (see `docs/store.md`, "File names").
    pub fn note_paths(&self, include: &GlobSet, exclude: &GlobSet) -> Result<Vec<String>> {
        let mut paths = files(&self.root, "")?;
        paths.retain(|path| {
            let path = nfc(path);
            include.is_match(&path) && !exclude.is_match(&path)
        });
        paths.sort_by_cached_key(|path| (nfc(path), path.clone()));
        Ok(paths)
    }

    /// The file of a note path, with the platform's separator between
    /// folders.
    pub fn note_file(&self, path: &str) -> PathBuf {
        path.split('/')
            .fold(self.root.clone(), |file, part| file.join(part))
    }
}

/// The files under `dir`, whose path relative to the store is `prefix`,
/// skipping folders whose name starts with `.`.
fn files(dir: &Path, prefix: &str) -> Result<Vec<String>> {
    let entries = fs::read_dir(dir).map_err(Error::io(dir))?;
    entries
        .map(|entry| {
            let path = entry.map_err(Error::io(dir))?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| Error::NonUtf8Path { path: path.clone() })?;
            let relative = format!("{prefix}{name}");
            if path.is_dir() {
                if name.starts_with('.') {
                    Ok(Vec::new())
                } else {
                    files(&path, &format!("{relative}/"))
                }
            } else if path.is_file() {
                Ok(vec![relative])
            } else {
                Ok(Vec::new())
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(|lists| lists.concat())
}

/// `text` in Unicode NFC, the composed form most systems write. macOS often
/// gives file names in the decomposed form (NFD) instead; paths are
/// compared in NFC so that both match the same way.
pub(crate) fn nfc(text: &str) -> String {
    text.nfc().collect()
}

/// The nearest of `dir` and its parents that contains `.srl/`.
fn marked_ancestor(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|candidate| candidate.join(SRL).is_dir())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// A temporary tree with a store at `notes/` and a plain directory at
    /// `notes/japanese/grammar/`.
    fn tree() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("notes").join("japanese").join("grammar")).unwrap();
        Store::init(&tmp.path().join("notes"), false).unwrap();
        tmp
    }

    fn root(lookup: &Lookup) -> PathBuf {
        Store::locate(lookup).unwrap().root().to_path_buf()
    }

    fn open(dir: &Path) -> Store {
        Store::open(&Lookup {
            dir: Some(dir.to_path_buf()),
            ..Lookup::default()
        })
        .unwrap()
    }

    #[test]
    fn dir_flag_comes_first() {
        let tmp = tree();
        let lookup = Lookup {
            dir: Some("/flag".into()),
            env_dir: Some("/env".into()),
            cwd: tmp.path().join("notes"),
            data_dir: Some("/data".into()),
        };
        assert_eq!(root(&lookup), PathBuf::from("/flag"));
    }

    #[test]
    fn env_dir_comes_before_the_current_directory() {
        let tmp = tree();
        let lookup = Lookup {
            env_dir: Some("/env".into()),
            cwd: tmp.path().join("notes"),
            ..Lookup::default()
        };
        assert_eq!(root(&lookup), PathBuf::from("/env"));
    }

    #[test]
    fn nearest_parent_with_srl_folder_is_found() {
        let tmp = tree();
        let lookup = Lookup {
            cwd: tmp.path().join("notes").join("japanese").join("grammar"),
            data_dir: Some("/data".into()),
            ..Lookup::default()
        };
        assert_eq!(root(&lookup), tmp.path().join("notes"));
    }

    #[test]
    fn default_store_is_used_when_nothing_else_matches() {
        let tmp = TempDir::new().unwrap();
        let lookup = Lookup {
            cwd: tmp.path().to_path_buf(),
            data_dir: Some("/data".into()),
            ..Lookup::default()
        };
        assert_eq!(root(&lookup), PathBuf::from("/data/srl"));
    }

    #[test]
    fn open_refuses_a_directory_without_config() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".srl")).unwrap();
        let lookup = Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        };
        assert!(matches!(Store::open(&lookup), Err(Error::NotAStore { .. })));
    }

    #[test]
    fn init_target_is_the_argument_then_the_flag_then_the_current_directory() {
        let lookup = Lookup {
            env_dir: Some("/env".into()),
            cwd: "/cwd".into(),
            data_dir: Some("/data".into()),
            ..Lookup::default()
        };
        assert_eq!(
            Store::init_target(&lookup, Some("sub".into())),
            PathBuf::from("/cwd/sub")
        );
        assert_eq!(Store::init_target(&lookup, None), PathBuf::from("/env"));
        let bare = Lookup {
            env_dir: None,
            ..lookup
        };
        assert_eq!(Store::init_target(&bare, None), PathBuf::from("/cwd"));
    }

    #[test]
    fn init_creates_a_store_that_open_finds() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("new");
        let created = Store::init(&root, false).unwrap();
        assert_eq!(created.len(), 2);
        assert!(root.join(".srl").join("config.toml").is_file());
        assert!(root.join(".srl").join("log").is_dir());
        fs::create_dir(root.join("sub")).unwrap();
        let lookup = Lookup {
            cwd: root.join("sub"),
            ..Lookup::default()
        };
        assert_eq!(Store::open(&lookup).unwrap().root(), root);
    }

    #[test]
    fn init_refuses_an_existing_store() {
        let tmp = tree();
        assert!(matches!(
            Store::init(&tmp.path().join("notes"), false),
            Err(Error::StoreExists { .. })
        ));
    }

    #[test]
    fn init_dry_run_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(Store::init(tmp.path(), true).unwrap().len(), 2);
        assert!(fs::read_dir(tmp.path()).unwrap().next().is_none());
    }

    #[test]
    fn no_data_directory_and_no_store_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let lookup = Lookup {
            cwd: tmp.path().to_path_buf(),
            ..Lookup::default()
        };
        assert!(matches!(Store::locate(&lookup), Err(Error::NoStore { .. })));
    }

    #[test]
    fn notes_skip_dot_folders_other_files_and_excluded_ones() {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = open(tmp.path());
        for path in [
            "a.md",
            "b.txt",
            "sub/c.md",
            ".obsidian/d.md",
            "templates/e.md",
            "sub/.git/f.md",
        ] {
            let file = store.note_file(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "").unwrap();
        }
        let include = crate::config::globs(&["**/*.md"]).unwrap();
        let exclude = crate::config::globs(&["templates/**"]).unwrap();
        assert_eq!(
            store.note_paths(&include, &exclude).unwrap(),
            ["a.md", "sub/c.md"]
        );
        let include = crate::config::globs(&["**/*.md", "*.txt"]).unwrap();
        assert_eq!(
            store.note_paths(&include, &exclude).unwrap(),
            ["a.md", "b.txt", "sub/c.md"]
        );
        // exclude wins over include.
        assert!(store.note_paths(&include, &include).unwrap().is_empty());
    }

    #[test]
    fn note_paths_sort_by_their_composed_form_and_stay_as_named() {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = open(tmp.path());
        // "é" decomposed, as macOS often names files: by bytes it sorts
        // before "f", composed it sorts after.
        let decomposed = "e\u{301}.md";
        for name in ["ea.md", "f.md", decomposed] {
            fs::write(store.note_file(name), "").unwrap();
        }
        assert_eq!(
            store
                .note_paths(&crate::config::globs(&["*.md"]).unwrap(), &GlobSet::empty())
                .unwrap(),
            ["ea.md", "f.md", decomposed]
        );
    }
}
