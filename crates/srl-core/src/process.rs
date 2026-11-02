//! Running the programs a store names, kind programs and source programs
//! alike: one process per call, its input on standard input, its answer on
//! standard output (see `docs/cards.md`, "How srl calls them", and
//! `docs/sources.md`, "Source programs").

use std::io::{self, Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How often a program with a time limit is checked on.
const POLL: Duration = Duration::from_millis(10);

/// Why a call of a program gave no answer.
#[derive(Debug)]
pub(crate) enum Failure {
    /// It couldn't be started.
    CannotStart(io::Error),
    /// Talking to it failed.
    Io(io::Error),
    /// It ran out of time, and was stopped.
    TimedOut,
    /// It exited with a status other than 0, with what it printed on
    /// standard error.
    Status { status: ExitStatus, stderr: String },
    /// What it printed is not UTF-8.
    NotUtf8(std::string::FromUtf8Error),
}

impl Failure {
    /// The reason, for a program described as `program` (such as "the
    /// program of kind conj"), when it doesn't depend on the call.
    pub(crate) fn reason(&self, program: &str) -> String {
        match self {
            Self::CannotStart(error) => format!("cannot run {program} ({error})"),
            Self::Io(error) => format!("running {program} failed: {error}"),
            Self::TimedOut => format!("{program} took too long, so it was stopped"),
            Self::Status { status, stderr } => {
                if stderr.trim().is_empty() {
                    format!("{program} failed ({status}) and gave no reason")
                } else {
                    format!("{program} failed: {}", stderr.trim())
                }
            }
            Self::NotUtf8(error) => format!("{program} printed output that is not UTF-8 ({error})"),
        }
    }
}

/// The file of `program`, as a store's configuration names it: a path with
/// a folder is relative to the store's folder `root`, unless it is
/// absolute; a bare name is found as the platform finds programs. Errors
/// are plain reasons.
pub(crate) fn find(root: &Path, program: &str) -> Result<PathBuf, String> {
    let path = Path::new(program);
    if path.is_absolute() || path.components().count() > 1 {
        return Ok(root.join(path));
    }
    which::which(program).map_err(|error| error.to_string())
}

/// Runs `program` with `args` and then `call`, in the folder `root`, with
/// `input` on its standard input, and gives what it printed on standard
/// output. It is stopped after `timeout`, if there is one.
pub(crate) fn run(
    root: &Path,
    program: &Path,
    args: &[String],
    call: &str,
    input: String,
    timeout: Option<Duration>,
) -> Result<String, Failure> {
    let mut child = Command::new(program)
        .args(args)
        .arg(call)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Failure::CannotStart)?;
    let stdin = child
        .stdin
        .take()
        .expect("a program's stdin should be piped");
    let writer = thread::spawn(move || write_input(stdin, &input));
    let stdout = reader(
        child
            .stdout
            .take()
            .expect("a program's stdout should be piped"),
    );
    let stderr = reader(
        child
            .stderr
            .take()
            .expect("a program's stderr should be piped"),
    );
    let Some(status) = wait(&mut child, timeout).map_err(Failure::Io)? else {
        return Err(Failure::TimedOut);
    };
    let joined = |handle: JoinHandle<io::Result<Vec<u8>>>| {
        handle
            .join()
            .expect("reading a program's output should not panic")
    };
    let stdout = joined(stdout).map_err(Failure::Io)?;
    let stderr = joined(stderr).map_err(Failure::Io)?;
    writer
        .join()
        .expect("writing a program's input should not panic")
        .map_err(Failure::Io)?;
    if !status.success() {
        return Err(Failure::Status {
            status,
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    String::from_utf8(stdout).map_err(Failure::NotUtf8)
}

/// Writes `input` to a program's standard input, and closes it.
fn write_input(mut stdin: ChildStdin, input: &str) -> io::Result<()> {
    match stdin.write_all(input.as_bytes()) {
        // A program may finish without reading its input; whether it
        // succeeded is up to its exit status.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

/// Reads all of `pipe` on its own thread, so that a program never waits on
/// a full pipe.
fn reader(mut pipe: impl Read + Send + 'static) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

/// Waits for `child` to finish, for at most `timeout` if there is one.
/// `None` if it ran out of time, in which case it is stopped.
fn wait(child: &mut Child, timeout: Option<Duration>) -> io::Result<Option<ExitStatus>> {
    let Some(timeout) = timeout else {
        return child.wait().map(Some);
    };
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() >= timeout {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        thread::sleep(POLL);
    }
}
