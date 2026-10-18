//! The index cache, `.cache/index.sqlite` (see `docs/store.md`, "The index
//! cache").
//!
//! It keeps what is slow to compute from the files: each atom file's parsed
//! front-matter, and the log replayed through FSRS. Every file is still read
//! on every use and compared with the cache by its BLAKE3 hash, so the cache can
//! only skip work, never change a result. When it can't be used, the files
//! are read directly and the problem is reported.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jiff::civil::Date;
use rusqlite::{Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{Map, Value};

use crate::atom::read_atom_file;
use crate::schedule::Replayer;
use crate::{Atom, Atoms, Config, Error, History, Id, Log, Memory, Result, Reviewed, Store, log};

/// The version of the tables below. Change it whenever they, or what they
/// mean, change: a cache of another version is rebuilt.
const SCHEMA_VERSION: i64 = 2;

/// The tables. Their layout is not a contract (see `docs/store.md`).
const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE atoms (path TEXT PRIMARY KEY, hash TEXT NOT NULL, id TEXT NOT NULL,
    fields TEXT NOT NULL);
CREATE TABLE log_files (name TEXT PRIMARY KEY, len INTEGER NOT NULL, hash TEXT NOT NULL,
    lines INTEGER NOT NULL);
CREATE TABLE log_problems (seq INTEGER PRIMARY KEY, name TEXT NOT NULL, line INTEGER NOT NULL,
    reason TEXT NOT NULL);
CREATE TABLE memories (atom TEXT PRIMARY KEY, reviews INTEGER NOT NULL, lapses INTEGER NOT NULL,
    stability REAL, difficulty REAL, first_review TEXT, last_review TEXT, due TEXT,
    last_used TEXT NOT NULL);
CREATE TABLE reviews (seq INTEGER PRIMARY KEY, atom TEXT NOT NULL, day TEXT NOT NULL,
    rating INTEGER NOT NULL, was_new INTEGER NOT NULL);
";

/// Every table, to drop them all before building them again.
const TABLES: [&str; 6] = [
    "meta",
    "atoms",
    "log_files",
    "log_problems",
    "memories",
    "reviews",
];

/// How long to wait for another `srl` that is updating the cache.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The cache's file name in `.cache/`.
const FILE: &str = "index.sqlite";

/// Why using the cache failed.
enum Failure {
    /// Reading the store's own files failed: the same error the commands
    /// give without a cache.
    Store(Error),
    /// The cache is damaged: it is deleted and built again.
    Corrupt(String),
    /// The cache can't be used (permissions, a lock held too long, …).
    Unusable(String),
}

impl From<rusqlite::Error> for Failure {
    /// Errors that say the cache can't be reached or written make it
    /// unusable; any other error means it isn't what `srl` wrote (damaged,
    /// or changed by something else), so it is rebuilt.
    fn from(error: rusqlite::Error) -> Self {
        let unusable = matches!(
            error.sqlite_error_code(),
            Some(
                ErrorCode::DatabaseBusy
                    | ErrorCode::DatabaseLocked
                    | ErrorCode::ReadOnly
                    | ErrorCode::PermissionDenied
                    | ErrorCode::CannotOpen
                    | ErrorCode::DiskFull
                    | ErrorCode::SystemIoFailure
                    | ErrorCode::OutOfMemory
                    | ErrorCode::NoLargeFileSupport
                    | ErrorCode::FileLockingProtocolFailed
            )
        );
        if unusable {
            Self::Unusable(error.to_string())
        } else {
            Self::Corrupt(error.to_string())
        }
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Store(error)
    }
}

/// A damaged cache: data in it that doesn't decode.
fn corrupt(what: &str) -> impl FnOnce(String) -> Failure {
    move |reason| Failure::Corrupt(format!("{what}: {reason}"))
}

/// The atoms and memories of a store, through the index cache.
///
/// Problems with the cache itself are collected, to be reported with
/// [`Index::problems`]; the results are the same either way.
#[derive(Debug)]
pub struct Index {
    store: Store,
    connection: Option<Connection>,
    /// Set once the cache turned out to be unusable: from then on the files
    /// are read directly.
    disabled: bool,
    /// For `srl reindex`: fail instead of reading the files directly.
    strict: bool,
    problems: Vec<Error>,
}

impl Index {
    /// The index of `store`. Nothing is opened until it is used.
    pub fn open(store: &Store) -> Self {
        Self {
            store: store.clone(),
            connection: None,
            disabled: false,
            strict: false,
            problems: Vec::new(),
        }
    }

    /// Deletes the cache and builds it again from the files (`srl
    /// reindex`). Returns the atoms, whose `problems` are those of the atom
    /// files, and the log lines that were skipped. Fails if the cache can't
    /// be written.
    pub fn rebuild(store: &Store, config: &Config) -> Result<(Atoms, Vec<Error>)> {
        let mut index = Self {
            strict: true,
            ..Self::open(store)
        };
        remove(&index.file()).map_err(|reason| index.reindex_error(reason))?;
        let atoms = index.atoms()?;
        let (_, problems) = index.memories(config)?;
        Ok((atoms, problems))
    }

    /// The store this index belongs to.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The problems with the cache since the last call: it was rebuilt, or
    /// can't be used.
    pub fn problems(&mut self) -> Vec<Error> {
        std::mem::take(&mut self.problems)
    }

    /// Every atom, as [`Atoms::load`] reads them.
    pub fn atoms(&mut self) -> Result<Atoms> {
        self.run(cached_atoms, Atoms::load)
    }

    /// Every atom's memory, as [`History::replay`] computes it from
    /// [`Log::load`], and the log lines that were skipped.
    pub fn memories(&mut self, config: &Config) -> Result<(HashMap<Id, Memory>, Vec<Error>)> {
        self.run(
            |transaction, store| {
                let (memories, problems) = refresh_log(transaction, store, config)?;
                Ok((memories, problems))
            },
            |store| {
                let (history, problems) = uncached_history(store, config)?;
                Ok((history.memories, problems))
            },
        )
    }

    /// Like [`Index::memories`], with every review of the log as well.
    pub fn history(&mut self, config: &Config) -> Result<(History, Vec<Error>)> {
        self.run(
            |transaction, store| {
                let (memories, problems) = refresh_log(transaction, store, config)?;
                let reviews = cached_reviews(transaction)?;
                Ok((History { memories, reviews }, problems))
            },
            |store| uncached_history(store, config),
        )
    }

    fn file(&self) -> PathBuf {
        self.store.cache_dir().join(FILE)
    }

    /// Runs `cached` in one transaction on the cache, or `uncached` if the
    /// cache can't be used. A damaged cache is deleted and built again
    /// once.
    fn run<T>(
        &mut self,
        cached: impl Fn(&Transaction<'_>, &Store) -> std::result::Result<T, Failure>,
        uncached: impl Fn(&Store) -> Result<T>,
    ) -> Result<T> {
        if self.disabled {
            return uncached(&self.store);
        }
        let failure = match self.attempt(&cached) {
            Ok(value) => return Ok(value),
            Err(Failure::Store(error)) => return Err(error),
            Err(Failure::Corrupt(reason)) => {
                self.connection = None;
                match remove(&self.file()) {
                    Ok(()) => {
                        self.problems.push(Error::CacheRebuilt {
                            path: self.file(),
                            reason,
                        });
                        match self.attempt(&cached) {
                            Ok(value) => return Ok(value),
                            Err(Failure::Store(error)) => return Err(error),
                            Err(Failure::Corrupt(reason) | Failure::Unusable(reason)) => reason,
                        }
                    }
                    Err(reason) => reason,
                }
            }
            Err(Failure::Unusable(reason)) => reason,
        };
        if self.strict {
            return Err(self.reindex_error(failure));
        }
        self.connection = None;
        self.disabled = true;
        self.problems.push(Error::CacheUnusable {
            path: self.file(),
            reason: failure,
        });
        uncached(&self.store)
    }

    fn attempt<T>(
        &mut self,
        cached: &impl Fn(&Transaction<'_>, &Store) -> std::result::Result<T, Failure>,
    ) -> std::result::Result<T, Failure> {
        let connection = match &mut self.connection {
            Some(connection) => connection,
            empty => empty.insert(connect(&self.store)?),
        };
        // One immediate transaction for the whole refresh: another `srl`
        // must not update the cache between reading it and writing it.
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        prepare(&transaction)?;
        let value = cached(&transaction, &self.store)?;
        transaction.commit()?;
        Ok(value)
    }

    fn reindex_error(&self, reason: String) -> Error {
        Error::Reindex {
            path: self.file(),
            reason,
        }
    }
}

/// Deletes the cache file and its journal, if they exist.
fn remove(file: &Path) -> std::result::Result<(), String> {
    let journal = file.with_file_name(format!("{FILE}-journal"));
    [file, journal.as_path()]
        .into_iter()
        .try_for_each(|path| match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("cannot delete {}: {error}", path.display())),
        })
}

/// Opens the cache, creating `.cache/` (with a `.gitignore` that ignores
/// everything in it) and the file if needed.
///
// TODO(platform): SQLite's file locking is unreliable on network file
// systems (NFS, SMB) on every platform, so two `srl` processes updating the
// same cache there could damage it. A damaged file is usually detected and
// rebuilt, but not always. See docs/store.md, "The index cache", and
// docs/rust.md, "Platform independence".
fn connect(store: &Store) -> std::result::Result<Connection, Failure> {
    let dir = store.cache_dir();
    let unusable = |error: io::Error| Failure::Unusable(format!("{}: {error}", dir.display()));
    fs::create_dir_all(&dir).map_err(unusable)?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(".gitignore"))
    {
        Ok(mut file) => io::Write::write_all(&mut file, b"*\n").map_err(unusable)?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(unusable(error)),
    }
    let connection = Connection::open(dir.join(FILE))?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    Ok(connection)
}

/// Creates the tables, or drops and creates them again if they belong to
/// another schema version or another version of `srl`.
fn prepare(transaction: &Transaction<'_>) -> std::result::Result<(), Failure> {
    let schema = transaction.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    let srl = if schema == SCHEMA_VERSION {
        meta(transaction, "srl")?
    } else {
        None
    };
    if srl.as_deref() == Some(env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    for table in TABLES {
        transaction.execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
    }
    transaction.execute_batch(SCHEMA)?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    set_meta(transaction, "srl", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

fn meta(transaction: &Transaction<'_>, key: &str) -> std::result::Result<Option<String>, Failure> {
    Ok(transaction
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}

fn set_meta(transaction: &Transaction<'_>, key: &str, value: &str) -> rusqlite::Result<()> {
    transaction
        .execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            [key, value],
        )
        .map(|_| ())
}

/// Runs one statement, prepared once per connection: a refresh can insert
/// many rows with the same statement.
fn execute(
    transaction: &Transaction<'_>,
    sql: &str,
    params: impl rusqlite::Params,
) -> rusqlite::Result<()> {
    transaction.prepare_cached(sql)?.execute(params).map(|_| ())
}

/// The BLAKE3 hash of `bytes`, in hex. Not SHA-256: without the CPU's SHA
/// instructions it is several times slower, and every file is hashed on
/// every use.
fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// A cached atom: the hash of its file, its ID and its fields.
struct CachedAtom {
    hash: String,
    id: String,
    fields: String,
}

/// Reads every atom file, and parses only those whose text isn't cached.
fn cached_atoms(
    transaction: &Transaction<'_>,
    store: &Store,
) -> std::result::Result<Atoms, Failure> {
    let cached = transaction
        .prepare("SELECT path, hash, id, fields FROM atoms")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CachedAtom {
                    hash: row.get(1)?,
                    id: row.get(2)?,
                    fields: row.get(3)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    let mut parsed = Vec::new();
    let mut kept = HashSet::new();
    for path in store.atom_paths()? {
        let text = match read_atom_file(store, &path) {
            Ok(text) => text,
            Err(error) => {
                parsed.push(Err(error));
                continue;
            }
        };
        let hash = hash(text.as_bytes());
        let atom = match cached.get(&path) {
            Some(atom) if atom.hash == hash => Ok(cached_atom(path.clone(), text, atom)?),
            _ => {
                let atom = Atom::parse(path.clone(), text);
                if let Ok(atom) = &atom {
                    let fields = Value::Object(atom.fields()).to_string();
                    execute(
                        transaction,
                        "INSERT OR REPLACE INTO atoms (path, hash, id, fields) \
                         VALUES (?1, ?2, ?3, ?4)",
                        [&path, &hash, atom.id().as_str(), &fields],
                    )?;
                }
                atom
            }
        };
        if atom.is_ok() {
            kept.insert(path);
        }
        parsed.push(atom);
    }
    for path in cached.keys().filter(|path| !kept.contains(*path)) {
        execute(transaction, "DELETE FROM atoms WHERE path = ?1", [path])?;
    }
    Ok(Atoms::from_parsed(parsed))
}

fn cached_atom(
    path: String,
    text: String,
    cached: &CachedAtom,
) -> std::result::Result<Atom, Failure> {
    let id = Id::parse(&cached.id).map_err(|error| corrupt("an atom's ID")(error.to_string()))?;
    let fields = serde_json::from_str::<Map<String, Value>>(&cached.fields)
        .map_err(|error| corrupt("an atom's fields")(error.to_string()))?;
    Atom::from_fields(path, text, id, fields).map_err(corrupt("an atom"))
}

/// A log file as the cache last read it.
struct CachedFile {
    name: String,
    /// How many bytes were read.
    len: usize,
    /// The hash of those bytes.
    hash: String,
    /// How many lines they hold.
    lines: usize,
}

/// A log file as it is now.
struct LogFile {
    name: String,
    path: PathBuf,
    text: String,
}

/// Brings the replayed log up to date, and returns every atom's memory and
/// the log lines that were skipped.
fn refresh_log(
    transaction: &Transaction<'_>,
    store: &Store,
    config: &Config,
) -> std::result::Result<(HashMap<Id, Memory>, Vec<Error>), Failure> {
    let files = log::files(store)?
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    Failure::Unusable(format!("the log file name {} is not UTF-8", path.display()))
                })?
                .to_owned();
            let text = fs::read_to_string(&path).map_err(Error::io(&path))?;
            Ok(LogFile { name, path, text })
        })
        .collect::<std::result::Result<Vec<_>, Failure>>()?;
    let cached = cached_files(transaction)?;
    let settings = settings(config);
    let incremental = meta(transaction, "settings")?.as_deref() == Some(settings.as_str())
        && only_appended(&cached, &files);
    let (mut memories, mut position) = if incremental {
        let entries = meta(transaction, "entries")?
            .ok_or_else(|| Failure::Corrupt("the number of log entries is missing".to_owned()))?
            .parse::<usize>()
            .map_err(|error| corrupt("the number of log entries")(error.to_string()))?;
        (cached_memories(transaction)?, entries)
    } else {
        for table in ["memories", "reviews", "log_problems", "log_files"] {
            transaction.execute(&format!("DELETE FROM {table}"), [])?;
        }
        (HashMap::new(), 0)
    };
    let replayer = Replayer::new(config)?;
    let mut changed = HashSet::new();
    for file in &files {
        let previous = cached
            .iter()
            .find(|cached| incremental && cached.name == file.name);
        let (start, lines_before) = previous.map_or((0, 0), |cached| (cached.len, cached.lines));
        if previous.is_some_and(|cached| cached.len == file.text.len()) {
            continue;
        }
        let new = file.text.get(start..).ok_or_else(|| {
            Failure::Corrupt(format!("the cached length of log/{} is wrong", file.name))
        })?;
        let mut log = Log {
            entries: Vec::new(),
            problems: Vec::new(),
        };
        log.read(&file.path, new, lines_before, &config.schedule);
        for entry in &log.entries {
            let atom = entry.event.atom();
            let memory = memories.entry(atom.clone()).or_default();
            let (next, reviewed) = replayer.apply(memory, entry, position)?;
            *memory = next;
            if let Some(reviewed) = reviewed {
                insert_review(transaction, &reviewed)?;
            }
            changed.insert(atom.clone());
            position += 1;
        }
        for problem in log.problems {
            let Error::InvalidLogLine { line, reason, .. } = &problem else {
                return Err(Failure::Store(problem));
            };
            execute(
                transaction,
                "INSERT INTO log_problems (name, line, reason) VALUES (?1, ?2, ?3)",
                (&file.name, to_i64(*line), reason),
            )?;
        }
        execute(
            transaction,
            "INSERT OR REPLACE INTO log_files (name, len, hash, lines) VALUES (?1, ?2, ?3, ?4)",
            (
                &file.name,
                to_i64(file.text.len()),
                hash(file.text.as_bytes()),
                to_i64(file.text.lines().count()),
            ),
        )?;
    }
    for id in &changed {
        let memory = memories
            .get(id)
            .expect("a changed atom should have a memory");
        insert_memory(transaction, id, memory)?;
    }
    set_meta(transaction, "settings", &settings)?;
    set_meta(transaction, "entries", &position.to_string())?;
    let problems = cached_problems(transaction, store)?;
    Ok((memories, problems))
}

/// Whether the log files only grew since the cache read them, so that
/// replaying their new lines gives the same as replaying everything: every
/// file the cache read still starts with the bytes it read, which end with
/// a newline unless nothing was added, and the new bytes are all in the
/// last file it read or in files that sort after it.
fn only_appended(cached: &[CachedFile], files: &[LogFile]) -> bool {
    let unchanged_prefixes = cached.iter().all(|cached| {
        files
            .iter()
            .find(|file| file.name == cached.name)
            .is_some_and(|file| {
                let bytes = file.text.as_bytes();
                bytes.get(..cached.len).is_some_and(|prefix| {
                    hash(prefix) == cached.hash
                        && (bytes.len() == cached.len || prefix.last() == Some(&b'\n'))
                })
            })
    });
    let last = cached.iter().map(|cached| cached.name.as_str()).max();
    let grown_in_order = files
        .iter()
        .filter(|file| {
            cached
                .iter()
                .find(|cached| cached.name == file.name)
                .is_none_or(|cached| cached.len < file.text.len())
        })
        .all(|file| last.is_none_or(|last| file.name.as_str() >= last));
    unchanged_prefixes && grown_in_order
}

/// The settings the replay depends on, as text to compare.
fn settings(config: &Config) -> String {
    let schedule = &config.schedule;
    serde_json::json!({
        "day_starts_at": schedule.day_starts_at,
        "desired_retention": schedule.desired_retention,
        "weights": schedule.weights,
    })
    .to_string()
}

fn cached_files(transaction: &Transaction<'_>) -> std::result::Result<Vec<CachedFile>, Failure> {
    transaction
        .prepare("SELECT name, len, hash, lines FROM log_files ORDER BY name")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .map(|row| {
            let (name, len, hash, lines) = row?;
            Ok(CachedFile {
                name,
                len: to_usize(len)?,
                hash,
                lines: to_usize(lines)?,
            })
        })
        .collect()
}

fn cached_problems(
    transaction: &Transaction<'_>,
    store: &Store,
) -> std::result::Result<Vec<Error>, Failure> {
    let dir = store.log_dir();
    transaction
        .prepare("SELECT name, line, reason FROM log_problems ORDER BY name, line")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .map(|row| {
            let (name, line, reason) = row?;
            Ok(Error::InvalidLogLine {
                file: dir.join(name),
                line: to_usize(line)?,
                reason,
            })
        })
        .collect()
}

fn insert_review(transaction: &Transaction<'_>, reviewed: &Reviewed) -> rusqlite::Result<()> {
    execute(
        transaction,
        "INSERT INTO reviews (atom, day, rating, was_new) VALUES (?1, ?2, ?3, ?4)",
        (
            reviewed.atom.as_str(),
            reviewed.day.to_string(),
            reviewed.rating,
            reviewed.was_new,
        ),
    )
}

fn cached_reviews(transaction: &Transaction<'_>) -> std::result::Result<Vec<Reviewed>, Failure> {
    transaction
        .prepare("SELECT atom, day, rating, was_new FROM reviews ORDER BY seq")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u8>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })?
        .map(|row| {
            let (atom, day, rating, was_new) = row?;
            Ok(Reviewed {
                atom: to_id(&atom)?,
                day: to_date(&day)?,
                rating,
                was_new,
            })
        })
        .collect()
}

fn insert_memory(
    transaction: &Transaction<'_>,
    id: &Id,
    memory: &Memory,
) -> std::result::Result<(), Failure> {
    let last_used = serde_json::to_string(&memory.last_used)
        .map_err(|error| Failure::Unusable(error.to_string()))?;
    execute(
        transaction,
        "INSERT OR REPLACE INTO memories (atom, reviews, lapses, stability, difficulty, \
         first_review, last_review, due, last_used) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        (
            id.as_str(),
            to_i64(memory.reviews),
            memory.lapses,
            memory.state.map(|state| f64::from(state.stability)),
            memory.state.map(|state| f64::from(state.difficulty)),
            memory.first_review.map(|day| day.to_string()),
            memory.last_review.map(|day| day.to_string()),
            memory.due.map(|day| day.to_string()),
            last_used,
        ),
    )?;
    Ok(())
}

/// One row of `memories`, as read.
type MemoryRow = (
    String,
    i64,
    u32,
    Option<f64>,
    Option<f64>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

fn cached_memories(
    transaction: &Transaction<'_>,
) -> std::result::Result<HashMap<Id, Memory>, Failure> {
    transaction
        .prepare(
            "SELECT atom, reviews, lapses, stability, difficulty, first_review, last_review, \
             due, last_used FROM memories",
        )?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })?
        .map(|row: rusqlite::Result<MemoryRow>| {
            let (atom, reviews, lapses, stability, difficulty, first, last, due, last_used) = row?;
            let state = match (stability, difficulty) {
                (Some(stability), Some(difficulty)) => Some(fsrs::MemoryState {
                    stability: to_f32(stability)?,
                    difficulty: to_f32(difficulty)?,
                }),
                (None, None) => None,
                (Some(_), None) | (None, Some(_)) => {
                    return Err(Failure::Corrupt(format!(
                        "atom {atom} has only half a memory state"
                    )));
                }
            };
            let memory = Memory {
                reviews: to_usize(reviews)?,
                lapses,
                state,
                first_review: first.as_deref().map(to_date).transpose()?,
                last_review: last.as_deref().map(to_date).transpose()?,
                due: due.as_deref().map(to_date).transpose()?,
                last_used: serde_json::from_str(&last_used)
                    .map_err(|error| corrupt("a memory's cards")(error.to_string()))?,
            };
            Ok((to_id(&atom)?, memory))
        })
        .collect()
}

/// Reads the files directly, as if there were no cache.
fn uncached_history(store: &Store, config: &Config) -> Result<(History, Vec<Error>)> {
    let log = Log::load(store, &config.schedule)?;
    Ok((History::replay(&log, config)?, log.problems))
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).expect("a count of bytes or lines should fit in an i64")
}

fn to_usize(value: i64) -> std::result::Result<usize, Failure> {
    usize::try_from(value).map_err(|error| corrupt("a count")(error.to_string()))
}

/// An `f32` that was stored as an `f64`, so converts back exactly.
fn to_f32(value: f64) -> std::result::Result<f32, Failure> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value was an f32, and the check below catches any other"
    )]
    let narrow = value as f32;
    if f64::from(narrow).to_bits() == value.to_bits() {
        Ok(narrow)
    } else {
        Err(Failure::Corrupt(format!("{value} is not a stored f32")))
    }
}

fn to_date(text: &str) -> std::result::Result<Date, Failure> {
    text.parse::<Date>()
        .map_err(|error| corrupt("a day")(error.to_string()))
}

fn to_id(text: &str) -> std::result::Result<Id, Failure> {
    Id::parse(text).map_err(|error| corrupt("an atom ID")(error.to_string()))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::Lookup;

    fn store() -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        (tmp, store)
    }

    fn write(store: &Store, path: &str, text: &str) {
        let file = store.root().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn append(store: &Store, path: &str, text: &str) {
        let file = store.root().join(path);
        let old = fs::read_to_string(&file).unwrap_or_default();
        fs::write(file, format!("{old}{text}")).unwrap();
    }

    fn atom(id: &str, title: &str) -> String {
        format!(
            "---\nid: {id}\ntitle: {title}\ntags: [a, b]\nn: 0.1\ncloze:\n  - x {{{{y}}}}\n---\nBody of {title}.\n"
        )
    }

    fn review(t: &str, atom: &str, rating: u8, reference: &str) -> String {
        format!(
            "{{\"type\":\"review\",\"t\":\"{t}\",\"atom\":\"{atom}\",\"rating\":{rating},\
             \"card\":{{\"drill\":\"cloze\",\"ref\":\"{reference}\"}},\
             \"response\":null,\"by\":\"human\",\"hash\":\"000000000000\"}}\n"
        )
    }

    fn reset(t: &str, atom: &str) -> String {
        format!("{{\"type\":\"reset\",\"t\":\"{t}\",\"atom\":\"{atom}\"}}\n")
    }

    fn strings(errors: &[Error]) -> Vec<String> {
        errors.iter().map(ToString::to_string).collect()
    }

    /// Reads the store through `index` and directly, and checks that both
    /// give the same atoms, memories, reviews and problems.
    fn assert_same(index: &mut Index, store: &Store) {
        let config = Config::load(store).unwrap();
        let cached = index.atoms().unwrap();
        let (history, problems) = index.history(&config).unwrap();
        let (memories, memory_problems) = index.memories(&config).unwrap();
        assert!(index.problems().is_empty(), "{:?}", index.problems());
        let atoms = Atoms::load(store).unwrap();
        let log = Log::load(store, &config.schedule).unwrap();
        let replayed = History::replay(&log, &config).unwrap();
        assert_eq!(
            serde_json::to_value(&cached.atoms).unwrap(),
            serde_json::to_value(&atoms.atoms).unwrap()
        );
        let texts = |atoms: &Atoms| {
            atoms
                .atoms
                .iter()
                .map(|atom| atom.text().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(&cached), texts(&atoms));
        assert_eq!(strings(&cached.problems), strings(&atoms.problems));
        assert_eq!(history, replayed);
        assert_eq!(memories, replayed.memories);
        assert_eq!(strings(&problems), strings(&log.problems));
        assert_eq!(strings(&memory_problems), strings(&log.problems));
    }

    #[test]
    fn the_cache_gives_the_same_results_as_the_files_through_every_change() {
        let (_tmp, store) = store();
        let mut index = Index::open(&store);
        assert_same(&mut index, &store);
        write(&store, "atoms/one.md", &atom("aaaaa1", "one"));
        write(&store, "atoms/sub/two.md", &atom("aaaaa2", "two"));
        write(
            &store,
            "log/2026-09.jsonl",
            &review("2026-09-01T10:00:00+09:00", "aaaaa1", 3, "cloze:0:1"),
        );
        assert_same(&mut index, &store);
        // Appending to the last file, and starting a new month.
        append(
            &store,
            "log/2026-09.jsonl",
            &review("2026-09-05T10:00:00+09:00", "aaaaa1", 1, "cloze:0:1"),
        );
        write(
            &store,
            "log/2026-10.jsonl",
            &review("2026-10-01T10:00:00+09:00", "aaaaa2", 4, "cloze:0:1"),
        );
        assert_same(&mut index, &store);
        // An edit that keeps the size, a new atom, a removed one, a bad one.
        write(&store, "atoms/one.md", &atom("aaaaa1", "uno"));
        write(&store, "atoms/three.md", &atom("aaaaa3", "three"));
        fs::remove_file(store.root().join("atoms/sub/two.md")).unwrap();
        write(&store, "atoms/bad.md", "no front-matter\n");
        write(&store, "atoms/dup.md", &atom("aaaaa1", "dup"));
        assert_same(&mut index, &store);
        // Bad lines, blank lines, resets, a line without a newline yet.
        append(&store, "log/2026-10.jsonl", "not json\n\n");
        append(
            &store,
            "log/2026-10.jsonl",
            &reset("2026-10-02T10:00:00+09:00", "aaaaa2"),
        );
        append(
            &store,
            "log/2026-10.jsonl",
            r#"{"type":"review","t":"2026-10-03T10"#,
        );
        assert_same(&mut index, &store);
        append(&store, "log/2026-10.jsonl", "\n");
        append(
            &store,
            "log/2026-10.jsonl",
            &review("2026-10-04T03:00:00+09:00", "aaaaa3", 3, "cloze:0:1"),
        );
        assert_same(&mut index, &store);
        // A file that sorts before the last one, and a line inserted in the
        // middle of an old file.
        write(
            &store,
            "log/2026-08.jsonl",
            &review("2026-08-30T10:00:00+09:00", "aaaaa1", 2, "cloze:0:1"),
        );
        assert_same(&mut index, &store);
        let file = store.root().join("log/2026-09.jsonl");
        let text = fs::read_to_string(&file).unwrap();
        let (first, rest) = text.split_once('\n').unwrap();
        fs::write(
            &file,
            format!(
                "{first}\n{}{rest}",
                review("2026-09-02T10:00:00+09:00", "aaaaa1", 4, "x")
            ),
        )
        .unwrap();
        assert_same(&mut index, &store);
        // A truncated file, and a removed one.
        fs::write(&file, first).unwrap();
        fs::remove_file(store.root().join("log/2026-08.jsonl")).unwrap();
        assert_same(&mut index, &store);
        // Settings the replay depends on.
        write(&store, "srl.toml", "[schedule]\nday_starts_at = 11\n");
        assert_same(&mut index, &store);
        write(
            &store,
            "srl.toml",
            "[schedule]\nday_starts_at = 11\ndesired_retention = 0.8\n",
        );
        assert_same(&mut index, &store);
        // A new index on the same cache.
        assert_same(&mut Index::open(&store), &store);
    }

    #[test]
    fn appended_lines_are_replayed_on_top_of_the_cache() {
        let (_tmp, store) = store();
        let config = Config::default();
        write(&store, "atoms/one.md", &atom("aaaaa1", "one"));
        write(
            &store,
            "log/2026-09.jsonl",
            &review("2026-09-01T10:00:00+09:00", "aaaaa1", 3, "cloze:0:1"),
        );
        Index::open(&store).memories(&config).unwrap();
        // A cached memory that differs from the log: only a full replay
        // would correct it, so it shows whether the log was replayed again.
        let connection = Connection::open(store.cache_dir().join(FILE)).unwrap();
        connection
            .execute("UPDATE memories SET lapses = 42", [])
            .unwrap();
        drop(connection);
        append(
            &store,
            "log/2026-09.jsonl",
            &review("2026-09-05T10:00:00+09:00", "aaaaa1", 3, "cloze:0:1"),
        );
        let (memories, _) = Index::open(&store).memories(&config).unwrap();
        assert_eq!(memories[&Id::parse("aaaaa1").unwrap()].lapses, 42);
        assert_eq!(memories[&Id::parse("aaaaa1").unwrap()].reviews, 2);
        // srl reindex starts over.
        Index::rebuild(&store, &config).unwrap();
        let (memories, _) = Index::open(&store).memories(&config).unwrap();
        assert_eq!(memories[&Id::parse("aaaaa1").unwrap()].lapses, 0);
    }

    #[test]
    fn a_damaged_cache_is_rebuilt_and_reported() {
        let (_tmp, store) = store();
        write(&store, "atoms/one.md", &atom("aaaaa1", "one"));
        write(
            &store,
            "log/2026-09.jsonl",
            &review("2026-09-01T10:00:00+09:00", "aaaaa1", 3, "cloze:0:1"),
        );
        Index::open(&store).atoms().unwrap();
        fs::write(
            store.cache_dir().join(FILE),
            "this is not a database, but long enough to look like one at first glance",
        )
        .unwrap();
        let mut index = Index::open(&store);
        index.atoms().unwrap();
        let problems = index.problems();
        assert!(
            matches!(problems.as_slice(), [Error::CacheRebuilt { .. }]),
            "{problems:?}"
        );
        assert_same(&mut index, &store);
        // Data that doesn't decode counts as damage too.
        let connection = Connection::open(store.cache_dir().join(FILE)).unwrap();
        connection
            .execute("UPDATE atoms SET fields = 'nonsense'", [])
            .unwrap();
        drop(connection);
        let mut index = Index::open(&store);
        index.atoms().unwrap();
        assert!(matches!(
            index.problems().as_slice(),
            [Error::CacheRebuilt { .. }]
        ));
        assert_same(&mut index, &store);
    }

    #[test]
    fn an_unusable_cache_is_reported_and_the_files_are_read_directly() {
        let (_tmp, store) = store();
        write(&store, "atoms/one.md", &atom("aaaaa1", "one"));
        write(&store, ".cache", "a file where the folder should be");
        let mut index = Index::open(&store);
        let atoms = index.atoms().unwrap();
        assert_eq!(atoms.atoms.len(), 1);
        assert!(matches!(
            index.problems().as_slice(),
            [Error::CacheUnusable { .. }]
        ));
        // Reported once: the files are read directly from then on.
        index.memories(&Config::default()).unwrap();
        assert!(index.problems().is_empty());
        assert!(matches!(
            Index::rebuild(&store, &Config::default()),
            Err(Error::Reindex { .. })
        ));
    }

    #[test]
    fn the_cache_folder_ignores_itself_in_git() {
        let (_tmp, store) = store();
        Index::open(&store).atoms().unwrap();
        assert_eq!(
            fs::read_to_string(store.cache_dir().join(".gitignore")).unwrap(),
            "*\n"
        );
        assert!(store.cache_dir().join(FILE).is_file());
    }

    #[test]
    fn only_growth_at_the_end_of_the_log_is_appended() {
        let cached = |name: &str, text: &str| CachedFile {
            name: name.to_owned(),
            len: text.len(),
            hash: hash(text.as_bytes()),
            lines: text.lines().count(),
        };
        let file = |name: &str, text: &str| LogFile {
            name: name.to_owned(),
            path: PathBuf::from(name),
            text: text.to_owned(),
        };
        let old = [cached("a", "1\n"), cached("b", "2\n")];
        assert!(only_appended(&old, &[file("a", "1\n"), file("b", "2\n")]));
        assert!(only_appended(
            &old,
            &[file("a", "1\n"), file("b", "2\n3\n")]
        ));
        assert!(only_appended(
            &old,
            &[file("a", "1\n"), file("b", "2\n"), file("c", "3\n")]
        ));
        assert!(!only_appended(
            &old,
            &[file("a", "1\n4\n"), file("b", "2\n")]
        ));
        assert!(!only_appended(
            &old,
            &[file("0", "0\n"), file("a", "1\n"), file("b", "2\n")]
        ));
        assert!(!only_appended(
            &old,
            &[file("a", "1\n"), file("b", "9\n3\n")]
        ));
        assert!(!only_appended(&old, &[file("b", "2\n")]));
        let partial = [cached("a", "1")];
        assert!(only_appended(&partial, &[file("a", "1")]));
        assert!(!only_appended(&partial, &[file("a", "1\n")]));
    }

    #[test]
    fn processes_can_share_the_cache_while_the_log_grows() {
        let (_tmp, store) = store();
        write(&store, "atoms/one.md", &atom("aaaaa1", "one"));
        write(&store, "atoms/two.md", &atom("aaaaa2", "two"));
        let event = |atom: &str| {
            let line = review("2026-09-01T10:00:00+09:00", atom, 3, "cloze:0:1");
            serde_json::from_str::<crate::Event>(&line).unwrap()
        };
        let writers = ["aaaaa1", "aaaaa2"].map(|atom| {
            let store = store.clone();
            let event = event(atom);
            std::thread::spawn(move || {
                for _ in 0..40 {
                    log::append(&store, std::slice::from_ref(&event)).unwrap();
                }
            })
        });
        let readers = (0..4)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        let mut index = Index::open(&store);
                        index.atoms().unwrap();
                        let (_, problems) = index.memories(&Config::default()).unwrap();
                        assert!(problems.is_empty(), "{problems:?}");
                        assert!(index.problems().is_empty(), "{:?}", index.problems());
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in writers.into_iter().chain(readers) {
            thread.join().unwrap();
        }
        let mut index = Index::open(&store);
        assert_same(&mut index, &store);
        let (memories, _) = index.memories(&Config::default()).unwrap();
        assert_eq!(memories[&Id::parse("aaaaa1").unwrap()].reviews, 40);
    }
}
