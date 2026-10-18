use std::path::PathBuf;

/// Everything that can go wrong in the core.
///
/// Each message is one plain sentence that says what to do about it, because
/// the CLI prints it to standard error as is (see `docs/cli.md`).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no store found in {cwd} or its parents, and no home directory for the default store; \
         choose one with --dir or SRL_DIR"
    )]
    NoStore { cwd: PathBuf },

    #[error(
        "{root} is not a store (it has no srl.toml); create one with `srl init` or choose another with --dir or SRL_DIR"
    )]
    NotAStore { root: PathBuf },

    #[error(
        "{root} is already a store (it has srl.toml); use it as it is, or choose another directory"
    )]
    StoreExists { root: PathBuf },

    #[error(
        "cannot access {path}: {source}; check that it exists and that you may read and write it"
    )]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wraps an I/O error with the path it happened on.
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
