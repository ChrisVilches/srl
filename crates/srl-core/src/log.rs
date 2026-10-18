//! The review log: reading and appending events (see `docs/log.md`).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use jiff::Zoned;
use jiff::civil::{Date, DateTime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Schedule;
use crate::{Atom, Error, Grading, Id, Result, Store};

/// The number of hex characters of an atom file's SHA-256 in `hash`.
const HASH_LEN: usize = 12;

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    Review(Review),
    Reset(Reset),
}

/// An atom was rated (see `docs/log.md`, "`review`").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// The time, ISO 8601 with offset.
    pub t: String,
    pub atom: Id,
    /// 1–4, as stored (after the `choice` rule).
    pub rating: u8,
    /// The card used, or `None` for a review without a card.
    pub card: Option<CardUsed>,
    pub response: Option<Vec<String>>,
    pub by: By,
    /// `self`, `exact`, `choice` or `cmd:<command>`.
    pub grader: String,
    /// The start of the SHA-256 of the atom file at review time, in hex.
    pub hash: String,
}

/// Which card a review used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardUsed {
    pub drill: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub grading: String,
}

/// The atom's memory starts over (see `docs/log.md`, "`reset`").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reset {
    pub t: String,
    pub atom: Id,
}

/// Who ran a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    Human,
    Agent,
}

impl Event {
    pub fn atom(&self) -> &Id {
        match self {
            Self::Review(review) => &review.atom,
            Self::Reset(reset) => &reset.atom,
        }
    }

    pub fn t(&self) -> &str {
        match self {
            Self::Review(review) => &review.t,
            Self::Reset(reset) => &reset.t,
        }
    }

    /// The local date and time of the event, as written (its own offset).
    fn local(&self) -> std::result::Result<DateTime, String> {
        let t = self.t();
        t.parse::<jiff::Timestamp>()
            .map_err(|error| format!("its time `{t}` is not ISO 8601 with an offset ({error})"))?;
        t.parse::<DateTime>()
            .map_err(|error| format!("its time `{t}` is not ISO 8601 ({error})"))
    }

    /// Checks what serde can't: the time and the rating.
    fn check(&self) -> std::result::Result<DateTime, String> {
        if let Self::Review(review) = self
            && !(1..=4).contains(&review.rating)
        {
            return Err(format!("its rating is {}, not 1 to 4", review.rating));
        }
        self.local()
    }
}

impl CardUsed {
    pub(crate) fn of(drill: &str, reference: &str, grading: &Grading) -> Self {
        Self {
            drill: drill.to_owned(),
            reference: reference.to_owned(),
            grading: grading.name().to_owned(),
        }
    }
}

/// The time of an event that happens `now`, to the second, with its offset:
/// `2026-10-26T09:12:31+09:00`.
pub(crate) fn time(now: &Zoned) -> String {
    now.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// The `hash` of an atom: the start of the SHA-256 of its file, in hex.
pub(crate) fn hash(atom: &Atom) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    Sha256::digest(atom.text().as_bytes())
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .take(HASH_LEN)
        .map(|digit| {
            let digit = HEX
                .get(usize::from(digit))
                .expect("a half byte should index HEX");
            char::from(*digit)
        })
        .collect()
}

/// One event of the log, and the study day it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub event: Event,
    pub day: Date,
}

/// Every event of the log, in file order, and the lines that could not be
/// read.
#[derive(Debug)]
pub struct Log {
    /// The events: files by name (so by month), lines in order.
    pub entries: Vec<Entry>,
    /// One error per line that was skipped.
    pub problems: Vec<Error>,
}

impl Log {
    /// Reads every `log/*.jsonl` file. A line that is not a valid event is
    /// skipped and reported in `problems`. Study days come from each event's
    /// own time and offset, and `schedule`'s `day_starts_at`.
    pub fn load(store: &Store, schedule: &Schedule) -> Result<Self> {
        let mut log = Self {
            entries: Vec::new(),
            problems: Vec::new(),
        };
        for file in files(store)? {
            let text = fs::read_to_string(&file).map_err(Error::io(&file))?;
            log.read(&file, &text, 0, schedule);
        }
        Ok(log)
    }

    /// Adds the events of `text`, which comes from `file` and starts after
    /// line `lines_before`, and the lines that are not valid events.
    pub(crate) fn read(
        &mut self,
        file: &Path,
        text: &str,
        lines_before: usize,
        schedule: &Schedule,
    ) {
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match parse_line(line, schedule) {
                Ok(entry) => self.entries.push(entry),
                Err(reason) => self.problems.push(Error::InvalidLogLine {
                    file: file.to_path_buf(),
                    line: lines_before + index + 1,
                    reason,
                }),
            }
        }
    }

    /// The events of the atoms in `ids`, in log order.
    pub fn of<'a>(&'a self, ids: &'a [&Id]) -> impl Iterator<Item = &'a Entry> {
        self.entries
            .iter()
            .filter(move |entry| ids.contains(&entry.event.atom()))
    }
}

/// Every `log/*.jsonl` file, sorted by name (so by month).
pub(crate) fn files(store: &Store) -> Result<Vec<PathBuf>> {
    let dir = store.log_dir();
    let mut files = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()).map_err(Error::io(&dir)))
            .collect::<Result<Vec<_>>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(Error::io(&dir)(error)),
    };
    files.retain(|file| {
        file.extension()
            .is_some_and(|extension| extension == "jsonl")
    });
    files.sort();
    Ok(files)
}

fn parse_line(line: &str, schedule: &Schedule) -> std::result::Result<Entry, String> {
    let event = serde_json::from_str::<Event>(line)
        .map_err(|error| format!("it is not an event ({error})"))?;
    let local = event.check()?;
    Ok(Entry {
        day: schedule.study_day(local),
        event,
    })
}

/// Appends `events` to the log, each to the file of its month
/// (`log/YYYY-MM.jsonl`), each as one line written in one call.
///
// TODO(platform): Each event is one `write` to a file opened in append mode.
// On local Linux and macOS file systems the kernel makes such a write atomic
// with respect to other appends, so two frontends recording reviews at the
// same time can't interleave their lines. Windows gives no such guarantee,
// and neither do network file systems (NFS, SMB) anywhere: there, two
// concurrent `srl review` runs could mix their bytes into one corrupt line,
// which every later command would report and skip, losing both reviews.
// The likely fix is a lock file (for example `log/.lock`, created with
// `create_new` and removed after the append) around every append, on every
// platform. See docs/log.md, "Files", and docs/rust.md, "Platform
// independence".
pub(crate) fn append(store: &Store, events: &[Event]) -> Result<()> {
    let dir = store.log_dir();
    fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
    for event in events {
        let local = event
            .local()
            .map_err(|reason| Error::InvalidEvent { reason })?;
        let file = dir.join(format!("{:04}-{:02}.jsonl", local.year(), local.month()));
        let line = format!(
            "{}\n",
            serde_json::to_string(event).map_err(|error| Error::InvalidEvent {
                reason: error.to_string()
            })?
        );
        append_line(&file, &line)?;
    }
    Ok(())
}

fn append_line(file: &Path, line: &str) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .and_then(|mut opened| opened.write_all(line.as_bytes()))
        .map_err(Error::io(file))
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

    fn review(t: &str, rating: u8) -> Event {
        Event::Review(Review {
            t: t.to_owned(),
            atom: Id::parse("a7f3k2").unwrap(),
            rating,
            card: Some(CardUsed {
                drill: "cloze".to_owned(),
                reference: "cloze:0:1".to_owned(),
                grading: "exact".to_owned(),
            }),
            response: Some(vec!["ものともせず".to_owned()]),
            by: By::Human,
            grader: "exact".to_owned(),
            hash: "3fa91c0d22be".to_owned(),
        })
    }

    #[test]
    fn a_review_is_written_in_the_documented_format() {
        let line = serde_json::to_value(review("2026-10-26T09:12:31+09:00", 3)).unwrap();
        assert_eq!(
            line,
            serde_json::json!({"type": "review", "t": "2026-10-26T09:12:31+09:00", "atom": "a7f3k2",
                "rating": 3, "card": {"drill": "cloze", "ref": "cloze:0:1", "grading": "exact"},
                "response": ["ものともせず"], "by": "human", "grader": "exact", "hash": "3fa91c0d22be"})
        );
    }

    #[test]
    fn the_hash_is_the_start_of_the_files_sha256() {
        let text = "---\nid: a7f3k2\ntitle: x\n---\n";
        let atom = Atom::parse("a.md".to_owned(), text.to_owned()).unwrap();
        // `printf -- '---\nid: a7f3k2\ntitle: x\n---\n' | sha256sum | cut -c1-12`
        assert_eq!(hash(&atom), "596de0cd75dd");
    }

    #[test]
    fn appended_events_read_back_by_month_with_their_study_day() {
        let (_tmp, store) = store();
        let reset = Event::Reset(Reset {
            t: "2026-11-01T03:00:00+09:00".to_owned(),
            atom: Id::parse("a7f3k2").unwrap(),
        });
        let events = [review("2026-10-26T09:12:31+09:00", 3), reset];
        append(&store, &events).unwrap();
        assert!(store.log_dir().join("2026-10.jsonl").is_file());
        assert!(store.log_dir().join("2026-11.jsonl").is_file());
        let log = Log::load(&store, &Schedule::default()).unwrap();
        assert!(log.problems.is_empty(), "{:?}", log.problems);
        let read = log
            .entries
            .iter()
            .map(|entry| entry.event.clone())
            .collect::<Vec<_>>();
        assert_eq!(read, events);
        // 03:00 is before `day_starts_at` (4), so it belongs to the day before.
        assert_eq!(log.entries[1].day, jiff::civil::date(2026, 10, 31));
    }

    #[test]
    fn bad_lines_are_skipped_and_reported() {
        let (_tmp, store) = store();
        append(&store, &[review("2026-10-26T09:12:31+09:00", 3)]).unwrap();
        let file = store.log_dir().join("2026-10.jsonl");
        let good = fs::read_to_string(&file).unwrap();
        let bad = [
            "not json".to_owned(),
            good.replace("\"rating\":3", "\"rating\":5"),
            good.replace("+09:00", ""),
            good.replace("\"by\":\"human\"", "\"by\":\"robot\""),
            good.replace("\"hash\"", "\"extra\":1,\"hash\""),
        ];
        fs::write(&file, format!("{good}{}\n", bad.join("\n"))).unwrap();
        let log = Log::load(&store, &Schedule::default()).unwrap();
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.problems.len(), bad.len(), "{:?}", log.problems);
    }
}
