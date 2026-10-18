//! Finding and creating a store (see `docs/store.md`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The file whose presence marks a directory as a store.
const CONFIG_FILE: &str = "srl.toml";

/// The default store, relative to the platform's data directory.
const DEFAULT_STORE: &str = "srl";

/// The folder of atom files.
const ATOMS: &str = "atoms";

/// The folder of the store's own drills.
const DRILLS: &str = "drills";

/// The folders `srl init` creates.
const FOLDERS: [&str; 2] = [ATOMS, "log"];

/// The `srl.toml` that `srl init` writes: every key at its default, commented
/// out (see `docs/store.md`, "Configuration").
pub(crate) const DEFAULT_CONFIG: &str = r#"# srl configuration. Every key is optional; these are the defaults.

[schedule]
# desired_retention = 0.9    # FSRS target probability of recall
# day_starts_at = 4          # hour (local time) when a new study day begins
# new_per_day = 10           # new atoms introduced per day, across all selectors
# leech_lapses = 8           # an atom with this many lapses is a leech
# mature_days = 21           # an atom with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights; default weights if absent

[grading]
# cmd = ""                   # external grader for `cmd` cards; empty means fall back to self-grading

[drills]
# timeout_seconds = 10       # a drill that takes longer is skipped
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

/// One `srl` store: a directory holding atoms, the log and configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Finds the store, using the first match of: `--dir`, `SRL_DIR`, the
    /// nearest directory from `cwd` upwards that contains `srl.toml`, and the
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
        if store.root.join(CONFIG_FILE).is_file() {
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

    /// Creates a store in `root`, which may already exist and hold other
    /// files, and returns the paths it creates. With `dry_run`, only returns
    /// them.
    pub fn init(root: &Path, dry_run: bool) -> Result<Vec<PathBuf>> {
        let config = root.join(CONFIG_FILE);
        if config.exists() {
            return Err(Error::StoreExists {
                root: root.to_path_buf(),
            });
        }
        let folders = FOLDERS
            .iter()
            .map(|folder| root.join(folder))
            .filter(|folder| !folder.is_dir())
            .collect::<Vec<_>>();
        if !dry_run {
            for folder in &folders {
                fs::create_dir_all(folder).map_err(Error::io(folder))?;
            }
            fs::write(&config, DEFAULT_CONFIG).map_err(Error::io(&config))?;
        }
        Ok(std::iter::once(config).chain(folders).collect())
    }

    /// The store's directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The configuration file, `srl.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.root.join(CONFIG_FILE)
    }

    /// The folder of drills that belong to this store. It may not exist.
    pub fn drills_dir(&self) -> PathBuf {
        self.root.join(DRILLS)
    }

    /// The folder that holds the atom files.
    pub fn atoms_dir(&self) -> PathBuf {
        self.root.join(ATOMS)
    }

    /// Every atom file: each `.md` file under `atoms/`, at any depth, as a
    /// path relative to `atoms/` with `/` between folders, sorted.
    pub fn atom_paths(&self) -> Result<Vec<String>> {
        let dir = self.atoms_dir();
        let mut paths = markdown_files(&dir, "")?;
        paths.sort();
        Ok(paths)
    }

    /// The file of an atom path from [`Store::atom_paths`], with the
    /// platform's separator between folders.
    pub fn atom_file(&self, path: &str) -> PathBuf {
        path.split('/')
            .fold(self.atoms_dir(), |file, part| file.join(part))
    }
}

/// The `.md` files under `dir`, whose path relative to `atoms/` is `prefix`.
///
// TODO(platform): macOS (HFS+) may return file names in Unicode NFD form, so
// `をものともせず.md` can be listed with decomposed characters. Paths would
// then sort differently and not match NFC text in `path:` globs. Normalize
// names to NFC here if that turns out to happen (see docs/rust.md, "Platform
// independence").
fn markdown_files(dir: &Path, prefix: &str) -> Result<Vec<String>> {
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
                markdown_files(&path, &format!("{relative}/"))
            } else if path.extension().is_some_and(|extension| extension == "md") && path.is_file()
            {
                Ok(vec![relative])
            } else {
                Ok(Vec::new())
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(|lists| lists.concat())
}

/// The nearest of `dir` and its parents that contains `srl.toml`.
fn marked_ancestor(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|candidate| candidate.join(CONFIG_FILE).is_file())
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
        fs::write(tmp.path().join("notes").join(CONFIG_FILE), "").unwrap();
        tmp
    }

    fn root(lookup: &Lookup) -> PathBuf {
        Store::locate(lookup).unwrap().root().to_path_buf()
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
    fn nearest_parent_with_config_is_found() {
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
        assert_eq!(created.len(), 3);
        assert!(root.join("atoms").is_dir());
        assert!(root.join("log").is_dir());
        let lookup = Lookup {
            cwd: root.join("atoms"),
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
        assert_eq!(Store::init(tmp.path(), true).unwrap().len(), 3);
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
}
