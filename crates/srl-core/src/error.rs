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
        "{root} is not a store (it has no .srl/config.toml); create one with `srl init` or \
         choose another with --dir or SRL_DIR"
    )]
    NotAStore { root: PathBuf },

    #[error(
        "{root} is already a store (it has .srl/); use it as it is, or choose another directory"
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

    #[error("`{text}` is not an item ID; item IDs are 1 to 32 characters a-z and 0-9")]
    InvalidItemId { text: String },

    #[error(
        "`{text}` is not a card ID; a card ID is an item ID, optionally followed by `-` and \
         a suffix of a-z and 0-9 (x7k2m9, k3m9q1.2)"
    )]
    InvalidCardId { text: String },

    /// A problem in a source, at an item of a collection, or in the
    /// collection when `spot` is `None`.
    #[error("{}: {reason}", at(collection, spot.as_ref()))]
    At {
        collection: String,
        spot: Option<Spot>,
        reason: String,
    },

    #[error("{collection}: {reason}")]
    Collection { collection: String, reason: String },

    #[error("{collection}: item {position} has no ID; run `srl id` to give it one")]
    MissingId { collection: String, position: usize },

    #[error(
        "{count} problem(s) in the store's sources, listed above; fix them first, since srl \
         reads every item strictly"
    )]
    ItemsInvalid { count: usize },

    #[error("no item or card has the ID {id}; list cards with `srl find`")]
    UnknownId { id: String },

    #[error("there is no card {id}; reviews are about one card (list them with `srl cards`)")]
    UnknownCard { id: String },

    #[error(
        "cannot make the card {card} (fix what failed, or set the card aside with `srl suspend \
         {card}`): {reason}"
    )]
    CardFailed { card: String, reason: String },

    #[error("{reason}; no item of kind {kind} can be read until the program works")]
    KindFailed { kind: String, reason: String },

    #[error("the source {name} failed: {reason} (see docs/sources.md, \"Source programs\")")]
    SourceFailed { name: String, reason: String },

    #[error("cannot describe the kind {kind}: {reason}")]
    DescribeFailed { kind: String, reason: String },

    #[error("invalid selector term `{term}`: {reason}")]
    BadSelector { term: String, reason: String },

    #[error("line {line} of standard input is not a valid item: {reason}; nothing was written")]
    InvalidLine { line: usize, reason: String },

    #[error("cannot add to {collection}: {reason}; nothing was added")]
    CannotAdd { collection: String, reason: String },

    #[error(
        "{path} changed while srl was working on it, so it was not written; run the command \
         again"
    )]
    ChangedMeanwhile { path: String },

    #[error("cannot change {path}: {reason}; it was left as it was")]
    CannotWrite { path: String, reason: String },

    #[error("{} is not a valid configuration: {reason}; fix it (see docs/store.md)", file.display())]
    InvalidConfig { file: PathBuf, reason: String },

    #[error(
        "skipped line {line} of {}: {reason}; the log must not be edited by hand, \
         so restore the line from a backup or version control",
        file.display()
    )]
    InvalidLogLine {
        file: PathBuf,
        line: usize,
        reason: String,
    },

    #[error(
        "another srl has been writing the log for over {} seconds ({} is locked), so nothing was \
         recorded; try again, and if it keeps happening, look for a stuck srl process",
        timeout.as_secs(),
        path.display()
    )]
    LogLocked {
        path: PathBuf,
        timeout: std::time::Duration,
    },

    #[error("cannot write the event: {reason}; nothing more was written")]
    InvalidEvent { reason: String },

    #[error("the review is not valid: {reason}; nothing was recorded")]
    InvalidReview { reason: String },

    #[error("{id} has reviews but no card; its history stays in the log")]
    Orphan { id: String },

    #[error("cannot schedule with FSRS: {reason}; check `[schedule]` in .srl/config.toml")]
    Fsrs { reason: String },

    #[error(
        "the index cache {} was damaged ({reason}), so it was rebuilt from the log and the kind programs; nothing was lost",
        path.display()
    )]
    CacheRebuilt { path: PathBuf, reason: String },

    #[error(
        "cannot use the index cache {}: {reason}; reading the log and asking kind programs directly instead, which is slower \
         (check that you may write to the store's .srl/cache folder, or delete it)",
        path.display()
    )]
    CacheUnusable { path: PathBuf, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wraps an I/O error with the path it happened on.
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }

    /// The collection a problem in a source is about, and the position of its
    /// item, 0 for a collection as a whole (see `docs/items.md`,
    /// "Errors").
    pub(crate) fn place(&self) -> Option<(&str, usize)> {
        if let Self::Collection { collection, .. } = self {
            return Some((collection.as_str(), 0));
        }
        if let Self::At {
            collection, spot, ..
        } = self
        {
            return Some((
                collection.as_str(),
                spot.as_ref().map_or(0, |spot| spot.position),
            ));
        }
        if let Self::MissingId {
            collection,
            position,
        } = self
        {
            return Some((collection.as_str(), *position));
        }
        None
    }
}

/// Which item of a collection a problem is about: its position among the
/// items its source read there, counting from 1, and its ID, if it has
/// one, which names it to people (see `docs/items.md`, "Errors").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub position: usize,
    pub id: Option<String>,
}

impl std::fmt::Display for Spot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.id {
            Some(id) => write!(f, "item {id}"),
            None => write!(f, "item {}", self.position),
        }
    }
}

/// Where something is, for people: `COLLECTION: item ID`, or `COLLECTION` alone
/// for a collection as a whole.
pub(crate) fn at(collection: &str, spot: Option<&Spot>) -> String {
    match spot {
        Some(spot) => format!("{collection}: {spot}"),
        None => collection.to_owned(),
    }
}
