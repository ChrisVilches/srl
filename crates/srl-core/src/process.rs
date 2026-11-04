//! Running the programs a store names, kind programs and source programs
//! alike: one process per call, its input on standard input, its answer on
//! standard output (see `docs/cards.md`, "How srl calls them", and
//! `docs/sources.md`, "Source programs").
//!
//! A program is stopped with every process it started, when it runs out of
//! time and when `srl` is stopped (see `docs/cards.md`, "Stopping a
//! program"): on Unix it leads a process group of its own, which
//! `stop_programs` stops; on Windows it is put in a job object that stops
//! its processes when it is closed, as it is when `srl` ends however it ends.

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
    let mut command = Command::new(program);
    command
        .args(args)
        .arg(call)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut running = Running::start(&mut command).map_err(Failure::CannotStart)?;
    let child = &mut running.child;
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
    let Some(status) = running.wait(timeout).map_err(Failure::Io)? else {
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

/// A program that was started, with what stops it and every process it
/// started: its process group on Unix, its job object on Windows.
struct Running {
    child: Child,
    #[cfg(windows)]
    job: win32job::Job,
}

impl Running {
    /// Starts `command` as the leader of a process group of its own (Unix),
    /// or in a job object of its own (Windows). On Unix no program starts
    /// once `srl` is being stopped.
    #[cfg(unix)]
    fn start(command: &mut Command) -> io::Result<Self> {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
        // Started and recorded under the lock, so that `stop_programs`
        // never misses a program.
        let mut groups = groups();
        if groups.stopping {
            return Err(io::Error::other("srl is being stopped"));
        }
        let child = command.spawn()?;
        groups.running.push(child.id());
        Ok(Self { child })
    }

    #[cfg(windows)]
    fn start(command: &mut Command) -> io::Result<Self> {
        use std::os::windows::io::AsRawHandle as _;
        let mut limits = win32job::ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&limits)?;
        let mut child = command.spawn()?;
        // TODO(platform): a process the program starts before it is put in
        // the job is not in it. Starting it suspended would close that
        // window, which std::process doesn't offer.
        if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
            child.kill()?;
            child.wait()?;
            return Err(error.into());
        }
        Ok(Self { child, job })
    }

    /// Waits for the program to finish, for at most `timeout` if there is
    /// one. `None` if it ran out of time, in which case it is stopped with
    /// every process it started.
    fn wait(&mut self, timeout: Option<Duration>) -> io::Result<Option<ExitStatus>> {
        let Some(timeout) = timeout else {
            return self
                .child
                .wait()
                .map(Some)
                .and_then(|status| self.ended(status));
        };
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait()? {
                return self.ended(Some(status));
            }
            if start.elapsed() >= timeout {
                self.stop()?;
                self.child.wait()?;
                return Ok(None);
            }
            thread::sleep(POLL);
        }
    }

    /// Stops the program and every process it started. It has not been
    /// waited for, so its process group can't be another's yet.
    #[cfg(unix)]
    fn stop(&mut self) -> io::Result<()> {
        kill_group(self.child.id())
    }

    #[cfg(windows)]
    fn stop(&mut self) -> io::Result<()> {
        self.child.kill()?;
        // Closing the job, once dropped, stops the rest.
        Ok(())
    }

    /// Once the program has exited on its own: what it left running is
    /// not stopped, so on Windows the job no longer stops its processes
    /// when it is closed.
    #[cfg(unix)]
    #[expect(clippy::unnecessary_wraps, reason = "the same as on Windows")]
    #[expect(clippy::unused_self, reason = "the same as on Windows")]
    fn ended<T>(&self, status: T) -> io::Result<T> {
        Ok(status)
    }

    #[cfg(windows)]
    fn ended<T>(&self, status: T) -> io::Result<T> {
        self.job
            .set_extended_limit_info(&win32job::ExtendedLimitInfo::new())?;
        Ok(status)
    }
}

#[cfg(unix)]
impl Drop for Running {
    fn drop(&mut self) {
        groups().running.retain(|&group| group != self.child.id());
    }
}

/// The process groups of the programs running now, by the ID of their
/// leader, and whether `srl` is being stopped.
#[cfg(unix)]
struct Groups {
    running: Vec<u32>,
    stopping: bool,
}

#[cfg(unix)]
static GROUPS: std::sync::Mutex<Groups> = std::sync::Mutex::new(Groups {
    running: Vec::new(),
    stopping: false,
});

#[cfg(unix)]
fn groups() -> std::sync::MutexGuard<'static, Groups> {
    // The list stays right if a thread panicked holding it: it is changed
    // in one step.
    GROUPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Stops every process of the process group `group`.
#[cfg(unix)]
fn kill_group(group: u32) -> io::Result<()> {
    let pid = i32::try_from(group)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| io::Error::other(format!("{group} is not a process ID")))?;
    Ok(rustix::process::kill_process_group(
        pid,
        rustix::process::Signal::KILL,
    )?)
}

/// Stops every program `srl` is running, with every process it started,
/// and starts no other: for `srl` to call before it ends on a signal such
/// as `SIGTERM`, which would otherwise leave them running. On Windows a
/// job object does this when `srl` ends.
// TODO(platform): `SIGKILL` can't be caught, so `srl` killed with it leaves
// its programs running. Linux could have them killed with their parent
// (`PR_SET_PDEATHSIG`), but only from code run in the new process before
// it starts the program, which needs `unsafe`; macOS has nothing like it.
#[cfg(unix)]
pub fn stop_programs() {
    let mut groups = groups();
    groups.stopping = true;
    for &group in &groups.running {
        // A group whose processes have all ended is gone already.
        let _gone = kill_group(group);
    }
}
