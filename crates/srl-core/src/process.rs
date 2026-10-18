//! Running a program (a drill, the external grader) with an optional time
//! limit.

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How often a running program is checked for having finished.
const POLL: Duration = Duration::from_millis(5);

/// Runs `program` with `args`, writes `input` to its standard input, and
/// returns its standard output. Its standard error goes to ours, so its own
/// messages reach the person running `srl`.
///
/// It fails if the program can't be started, takes longer than `timeout`
/// (then it is killed), exits with a status other than 0, or writes output
/// that is not UTF-8. Errors are plain reasons.
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    input: &[u8],
    timeout: Option<Duration>,
) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("cannot start {} ({error})", program.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .expect("the child's stdin should be piped");
    let mut stdout = child
        .stdout
        .take()
        .expect("the child's stdout should be piped");
    let input = input.to_vec();
    // Writing and reading happen on their own threads, so that neither a
    // program that doesn't read its input nor one that writes a lot can
    // block the other side. Dropping `stdin` at the end closes it.
    let writer = thread::spawn(move || match stdin.write_all(&input) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    });
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).map(|_| output)
    });
    let deadline = timeout.map(|timeout| Instant::now() + timeout);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot wait for it ({error})"))?
        {
            break status;
        }
        if let Some((deadline, timeout)) = deadline.zip(timeout)
            && Instant::now() >= deadline
        {
            child
                .kill()
                .map_err(|error| format!("it took too long, and cannot be stopped ({error})"))?;
            child
                .wait()
                .map_err(|error| format!("it took too long, and cannot be waited for ({error})"))?;
            return Err(format!("it took longer than {} seconds", timeout.as_secs()));
        }
        thread::sleep(POLL);
    };
    writer
        .join()
        .expect("the thread writing to a program should not panic")
        .map_err(|error| format!("cannot write its input ({error})"))?;
    let output = reader
        .join()
        .expect("the thread reading from a program should not panic")
        .map_err(|error| format!("cannot read its output ({error})"))?;
    if !status.success() {
        return Err(match status.code() {
            Some(code) => format!("it exited with status {code}"),
            // Only on Unix: a process killed by a signal has no exit code.
            None => "it was stopped by a signal".to_owned(),
        });
    }
    String::from_utf8(output).map_err(|error| format!("its output is not valid UTF-8 ({error})"))
}
