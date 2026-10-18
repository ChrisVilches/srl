//! The only place that writes to standard output and standard error.

use std::path::PathBuf;

/// Prints one path per line.
#[expect(clippy::print_stdout, reason = "this module is where output happens")]
pub(crate) fn paths(paths: &[PathBuf]) {
    for path in paths {
        println!("{}", path.display());
    }
}

/// Prints an error as one plain message on standard error.
#[expect(clippy::print_stderr, reason = "this module is where output happens")]
pub(crate) fn error(error: &anyhow::Error) {
    eprintln!("srl: {error:#}");
}
