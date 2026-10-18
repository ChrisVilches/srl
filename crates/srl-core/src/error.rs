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
}

pub type Result<T> = std::result::Result<T, Error>;
