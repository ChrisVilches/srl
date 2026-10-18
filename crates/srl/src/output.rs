//! The only place that writes to standard output and standard error.

/// Prints an error as one plain message on standard error.
#[expect(clippy::print_stderr, reason = "this module is where output happens")]
pub(crate) fn error(error: &anyhow::Error) {
    eprintln!("srl: {error:#}");
}
