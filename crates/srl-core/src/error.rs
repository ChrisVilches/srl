use std::path::PathBuf;

/// Everything that can go wrong in the core.
///
/// Each message is one plain sentence that says what to do about it, because
/// the CLI prints it to standard error as is (see `docs/cli.md`).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no store found in {cwd} or its parents, and no data directory for the default store; \
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

    #[error("the file name {path} is not valid UTF-8; rename it")]
    NonUtf8Path { path: PathBuf },

    #[error("`{text}` is not an atom ID; IDs are six characters a-z and 0-9")]
    InvalidId { text: String },

    #[error("skipped atoms/{path}: {reason}; fix the file")]
    InvalidAtom { path: String, reason: String },

    #[error("skipped atoms/{path}: it has no `id`; run `srl check --fix` to assign one")]
    MissingId { path: String },

    #[error(
        "skipped atoms/{path}: its ID {id} is already used by atoms/{first}; \
         give one of them a new ID by deleting its `id` line and running `srl check --fix`"
    )]
    DuplicateId {
        id: crate::Id,
        path: String,
        first: String,
    },

    #[error("no atom has the ID {id}; list atoms with `srl find`")]
    UnknownId { id: crate::Id },

    #[error("invalid selector term `{term}`: {reason}")]
    BadSelector { term: String, reason: String },

    #[error("cannot add the atom: {reason}")]
    InvalidNewAtom { reason: String },

    #[error("line {line} of standard input is not a valid atom: {reason}; nothing was added")]
    InvalidLine { line: usize, reason: String },

    #[error("cannot change atoms/{path}: {reason}; nothing was changed")]
    CannotEdit { path: String, reason: String },

    #[error("the new text of atom {id} is not valid: {reason}; nothing was changed")]
    InvalidEdit { id: crate::Id, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wraps an I/O error with the path it happened on.
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
