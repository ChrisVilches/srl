//! Finding a store (see `docs/store.md`, "Finding the store").

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The file whose presence marks a directory as a store.
const CONFIG_FILE: &str = "srl.toml";

/// The default store, relative to the home directory.
const DEFAULT_STORE: &str = ".local/share/srl";

/// Everything needed to find a store, gathered by the caller.
///
/// The core never reads the process environment itself; the CLI fills this
/// in from `--dir`, `SRL_DIR`, the current directory and `HOME`.
#[derive(Debug, Clone, Default)]
pub struct Lookup {
    /// The `--dir` flag.
    pub dir: Option<PathBuf>,
    /// The `SRL_DIR` environment variable.
    pub env_dir: Option<PathBuf>,
    /// The current directory.
    pub cwd: PathBuf,
    /// The home directory, if there is one.
    pub home: Option<PathBuf>,
}

/// One `srl` store: a directory holding atoms, the log and configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Finds the store, using the first match of: `--dir`, `SRL_DIR`, the
    /// nearest directory from `cwd` upwards that contains `srl.toml`, and the
    /// default store under the home directory.
    pub fn locate(lookup: &Lookup) -> Result<Self> {
        lookup
            .dir
            .clone()
            .or_else(|| lookup.env_dir.clone())
            .or_else(|| marked_ancestor(&lookup.cwd))
            .or_else(|| lookup.home.as_ref().map(|home| home.join(DEFAULT_STORE)))
            .map(|root| Self { root })
            .ok_or_else(|| Error::NoStore {
                cwd: lookup.cwd.clone(),
            })
    }

    /// The store's directory.
    pub fn root(&self) -> &Path {
        &self.root
    }
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
        fs::create_dir_all(tmp.path().join("notes/japanese/grammar")).unwrap();
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
            home: Some("/home/u".into()),
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
            cwd: tmp.path().join("notes/japanese/grammar"),
            home: Some("/home/u".into()),
            ..Lookup::default()
        };
        assert_eq!(root(&lookup), tmp.path().join("notes"));
    }

    #[test]
    fn default_store_is_used_when_nothing_else_matches() {
        let tmp = TempDir::new().unwrap();
        let lookup = Lookup {
            cwd: tmp.path().to_path_buf(),
            home: Some("/home/u".into()),
            ..Lookup::default()
        };
        assert_eq!(root(&lookup), PathBuf::from("/home/u/.local/share/srl"));
    }

    #[test]
    fn no_home_and_no_store_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let lookup = Lookup {
            cwd: tmp.path().to_path_buf(),
            ..Lookup::default()
        };
        assert!(matches!(Store::locate(&lookup), Err(Error::NoStore { .. })));
    }
}
