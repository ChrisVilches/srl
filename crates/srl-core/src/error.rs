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

    #[error(
        "cannot add the atom: the folder {} and the folder of its first tag are so long that no \
         file name fits within {limit} characters, the most Windows allows; move the store to \
         a shorter path, or give the atom a shorter first tag",
        dir.display()
    )]
    PathTooLong { dir: PathBuf, limit: usize },

    #[error("line {line} of standard input is not a valid atom: {reason}; nothing was added")]
    InvalidLine { line: usize, reason: String },

    #[error("{} is not a valid configuration: {reason}; fix it (see docs/store.md)", file.display())]
    InvalidConfig { file: PathBuf, reason: String },

    #[error("cannot look for drills in {}: {reason}", folder.display())]
    DrillSearch { folder: PathBuf, reason: String },

    #[error("`srl-drill-{drill} --help` failed: {reason}; fix the drill program")]
    DrillHelp { drill: String, reason: String },

    #[error(
        "drill `{drill}` failed for atom {id}: {reason}; its cards were skipped \
         (fix the facet, or the drill program)"
    )]
    DrillFailed {
        drill: String,
        id: crate::Id,
        reason: String,
    },

    #[error("drill `{drill}` wrote an invalid card for atom {id}: {reason}; the card was dropped")]
    InvalidCard {
        drill: String,
        id: crate::Id,
        reason: String,
    },

    #[error(
        "atom {id} has the facet `{facet}`, but there is no drill `{facet}`; \
         install srl-drill-{facet} or remove the facet (`srl drills` lists the drills)"
    )]
    NoDrill { id: crate::Id, facet: String },

    #[error(
        "atom {id} has the facet `{facet}`, which can't have a drill: drill names are lowercase \
         letters, digits, - and _; rename or remove the facet"
    )]
    NotADrillName { id: crate::Id, facet: String },

    #[error("drill `{drill}` writes no cards for atom {id}; check the facet (see `srl drills`)")]
    NoCards { drill: String, id: crate::Id },

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

    #[error("the review on standard input is not valid: {reason}; nothing was recorded")]
    InvalidReview { reason: String },

    #[error("the grading request on standard input is not valid: {reason}")]
    InvalidGradeRequest { reason: String },

    #[error(
        "the grader `{command}` failed: {reason}; falling back to self-grading \
         (fix the grader, or `[grading] cmd` in srl.toml)"
    )]
    GraderFailed { command: String, reason: String },

    #[error("cannot schedule with FSRS: {reason}; check `[schedule]` in srl.toml")]
    Fsrs { reason: String },

    #[error(
        "the index cache {} was damaged ({reason}), so it was rebuilt from the files; nothing was lost",
        path.display()
    )]
    CacheRebuilt { path: PathBuf, reason: String },

    #[error(
        "cannot use the index cache {}: {reason}; reading the files directly instead, which is slower \
         (check that you may write to the store's .cache folder, or delete it)",
        path.display()
    )]
    CacheUnusable { path: PathBuf, reason: String },

    #[error(
        "cannot build the index cache {}: {reason}; check that you may write to the store's .cache folder",
        path.display()
    )]
    Reindex { path: PathBuf, reason: String },

    #[error("there is no drill `{name}`; `srl drills` lists the drills")]
    UnknownDrill { name: String },

    #[error(
        "`{name}` can't name a session: use lowercase letters, digits, - and _ (as in n1-food)"
    )]
    InvalidSessionName { name: String },

    #[error("line {line} of standard input is not a valid card: {reason}; nothing was saved")]
    InvalidSessionLine { line: usize, reason: String },

    #[error("there are no cards for session `{name}` on standard input; nothing was saved")]
    EmptySession { name: String },

    #[error("there is no session `{name}`; `srl session list` lists the sessions")]
    UnknownSession { name: String },

    #[error(
        "skipped line {line} of session `{name}`: {reason}; save the session again with `srl session put`"
    )]
    InvalidSessionCard {
        name: String,
        line: usize,
        reason: String,
    },

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
