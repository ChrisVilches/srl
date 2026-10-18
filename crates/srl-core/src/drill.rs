//! Drills: finding them, running them, and checking the cards they write
//! (see `docs/drills.md`).

mod builtin;

use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jiff::civil::Date;
use serde::Serialize;

use crate::card::{DrillDefaults, RawCard};
use crate::{Atom, Atoms, Card, Error, Result, Store, atom, process, yaml};

pub(crate) use builtin::Builtin;

/// The file name of the drill `NAME` is `srl-drill-NAME`.
const PREFIX: &str = "srl-drill-";

/// One drill, and where it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drill {
    name: String,
    source: Source,
}

/// What runs a drill.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    Builtin(Builtin),
    Program(PathBuf),
}

impl Drill {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The program that runs the drill, or `None` if it is built in.
    pub fn path(&self) -> Option<&Path> {
        match &self.source {
            Source::Builtin(_) => None,
            Source::Program(path) => Some(path),
        }
    }

    /// The first line of the drill's help: built in, or the first non-empty
    /// line of `srl-drill-NAME --help`, without a leading Markdown `#`.
    pub fn summary(&self, timeout: Duration) -> Result<String> {
        match &self.source {
            Source::Builtin(builtin) => Ok(builtin.summary().to_owned()),
            Source::Program(path) => {
                let help = process::run(path, &["--help"], b"", timeout).map_err(|reason| {
                    Error::DrillHelp {
                        drill: self.name.clone(),
                        reason,
                    }
                })?;
                Ok(help
                    .lines()
                    .map(|line| line.trim_start_matches('#').trim())
                    .find(|line| !line.is_empty())
                    .unwrap_or_default()
                    .to_owned())
            }
        }
    }

    /// Runs the drill for `atom` and returns the cards it wrote, before they
    /// are checked: one result per card, so that a bad card doesn't take the
    /// others with it. Errors are plain reasons; the outer one means the
    /// drill failed.
    fn run(
        &self,
        atom: &Atom,
        run: &Run<'_>,
    ) -> std::result::Result<Vec<std::result::Result<RawCard, String>>, String> {
        match &self.source {
            Source::Builtin(builtin) => Ok(builtin.cards(atom)?.into_iter().map(Ok).collect()),
            Source::Program(path) => {
                let input = input(atom, run.day)?;
                let output = process::run(path, &[], input.as_bytes(), run.timeout)?;
                Ok(output
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| !line.trim().is_empty())
                    .map(|(index, line)| {
                        RawCard::parse(line)
                            .map_err(|reason| format!("on line {}, {reason}", index + 1))
                    })
                    .collect())
            }
        }
    }
}

impl Serialize for Drill {
    /// `{"name": NAME, "path": PATH}`, with `path` `null` for a built-in drill.
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Written<'a> {
            name: &'a str,
            path: Option<&'a Path>,
        }
        Written {
            name: &self.name,
            path: self.path(),
        }
        .serialize(serializer)
    }
}

/// The input of a drill program: the atom as JSON (see `docs/drills.md`,
/// "The drill protocol"). Errors are plain reasons.
fn input(atom: &Atom, day: Date) -> std::result::Result<String, String> {
    let (front, _) = atom::split_front_matter(atom.text())
        .ok_or_else(|| "its file has no front-matter".to_owned())?;
    let frontmatter = yaml::parse_mapping(front)?;
    let value = serde_json::json!({
        "id": atom.id(),
        "path": format!("atoms/{}", atom.path()),
        "frontmatter": frontmatter,
        "body": atom.content().body(),
        "day": day.to_string(),
    });
    Ok(value.to_string())
}

/// Whether `name` can name a drill: lowercase ASCII letters, digits, `-` and
/// `_`. Other facet names have no drill.
pub(crate) fn is_drill_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
}

/// Every drill there is: the store's, those on `PATH`, and the built-in ones.
#[derive(Debug, Clone)]
pub struct Drills {
    by_name: BTreeMap<String, Drill>,
}

/// Where to look for drill programs, gathered by the caller.
#[derive(Debug, Clone, Default)]
pub struct Search {
    /// The folders of `PATH`, in order.
    pub path: Vec<PathBuf>,
    /// The current directory, against which relative `PATH` entries are
    /// read.
    pub cwd: PathBuf,
}

impl Drills {
    /// Finds every drill. A drill named `NAME` is the first of: an executable
    /// `drills/srl-drill-NAME` in the store, an executable `srl-drill-NAME` on
    /// `PATH`, and a built-in drill (see `docs/drills.md`, "Finding drills").
    ///
    /// A folder that can't be searched is reported in the second value, and
    /// the search carries on.
    pub fn find(store: &Store, search: &Search) -> (Self, Vec<Error>) {
        let folders = std::iter::once(store.drills_dir()).chain(search.path.iter().cloned());
        let mut by_name = BTreeMap::new();
        let mut problems = Vec::new();
        for folder in folders {
            match programs(&folder, &search.cwd) {
                Ok(found) => {
                    for (name, path) in found {
                        by_name.entry(name.clone()).or_insert(Drill {
                            name,
                            source: Source::Program(path),
                        });
                    }
                }
                Err(problem) => problems.push(problem),
            }
        }
        for builtin in Builtin::ALL {
            by_name.entry(builtin.name().to_owned()).or_insert(Drill {
                name: builtin.name().to_owned(),
                source: Source::Builtin(builtin),
            });
        }
        (Self { by_name }, problems)
    }

    /// Every drill, by name.
    pub fn all(&self) -> impl Iterator<Item = &Drill> {
        self.by_name.values()
    }

    pub fn get(&self, name: &str) -> Option<&Drill> {
        self.by_name.get(name)
    }

    /// The drills that apply to `atom`, by name: the drill of each facet (see
    /// `docs/drills.md`, rules 1 and 2). Also returns the facets that have no
    /// drill.
    pub fn applicable<'a>(&'a self, atom: &'a Atom) -> (Vec<&'a Drill>, Vec<&'a str>) {
        let facets = atom.content().facets().keys().map(String::as_str);
        let mut drills = facets
            .clone()
            .filter_map(|name| self.get(name))
            .collect::<Vec<_>>();
        drills.sort_by(|one, other| one.name().cmp(other.name()));
        let missing = facets.filter(|name| self.get(name).is_none()).collect();
        (drills, missing)
    }

    /// The cards of every drill that applies to `atom`, by drill name, one
    /// entry per drill. A drill that fails, and each invalid card, is
    /// reported in that drill's entry; the other cards are kept.
    pub fn cards(
        &self,
        atom: &Atom,
        atoms: &Atoms,
        day: Date,
        timeout: Duration,
    ) -> Vec<DrillCards> {
        self.cards_of(atom, atoms, day, timeout, None)
    }

    /// Like [`Drills::cards`], but with `only`, runs only the drill of that
    /// name, if it applies.
    pub fn cards_of(
        &self,
        atom: &Atom,
        atoms: &Atoms,
        day: Date,
        timeout: Duration,
        only: Option<&str>,
    ) -> Vec<DrillCards> {
        let run = Run {
            atoms,
            day,
            timeout,
        };
        self.applicable(atom)
            .0
            .into_iter()
            .filter(|drill| only.is_none_or(|name| drill.name() == name))
            .map(|drill| drill_cards(drill, atom, &run))
            .collect()
    }
}

/// What one drill made for one atom.
#[derive(Debug)]
pub struct DrillCards {
    pub drill: String,
    /// The valid cards, in the order the drill wrote them.
    pub cards: Vec<Card>,
    /// The drill's failure, or one error per invalid card.
    pub problems: Vec<Error>,
}

/// What a drill run needs besides the atom.
#[derive(Debug, Clone, Copy)]
struct Run<'a> {
    atoms: &'a Atoms,
    day: Date,
    timeout: Duration,
}

fn drill_cards(drill: &Drill, atom: &Atom, run: &Run<'_>) -> DrillCards {
    let raw = match drill.run(atom, run) {
        Ok(raw) => raw,
        Err(reason) => {
            return DrillCards {
                drill: drill.name.clone(),
                cards: Vec::new(),
                problems: vec![Error::DrillFailed {
                    drill: drill.name.clone(),
                    id: atom.id().clone(),
                    reason,
                }],
            };
        }
    };
    let defaults = DrillDefaults {
        drill: &drill.name,
        prefix: matches!(drill.source, Source::Program(_)),
        atom,
    };
    let mut seen = HashSet::new();
    let mut cards = Vec::new();
    let mut problems = Vec::new();
    for card in raw {
        let checked = card
            .and_then(|card| Card::from_drill(card, defaults, run.atoms))
            .and_then(|card| {
                if seen.insert(card.reference().to_owned()) {
                    Ok(card)
                } else {
                    Err(format!(
                        "its ref `{}` is used by an earlier card",
                        card.reference()
                    ))
                }
            });
        match checked {
            Ok(card) => cards.push(card),
            Err(reason) => problems.push(Error::InvalidCard {
                drill: drill.name.clone(),
                id: atom.id().clone(),
                reason,
            }),
        }
    }
    DrillCards {
        drill: drill.name.clone(),
        cards,
        problems,
    }
}

/// The drill programs in `folder`: each file `srl-drill-NAME` (plus an
/// extension from `PATHEXT` on Windows) that the platform can run, by name.
/// A folder that doesn't exist has none.
fn programs(folder: &Path, cwd: &Path) -> Result<Vec<(String, PathBuf)>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(Error::io(folder)(error)),
    };
    let names = entries
        .map(|entry| {
            let path = entry.map_err(Error::io(folder))?.path();
            Ok(path
                .file_stem()
                .and_then(OsStr::to_str)
                .and_then(|stem| stem.strip_prefix(PREFIX))
                .filter(|name| is_drill_name(name))
                .map(str::to_owned))
        })
        .collect::<Result<Vec<_>>>()?;
    let unique = names.into_iter().flatten().collect::<HashSet<_>>();
    // `which` decides what the platform can run: the execute permission on
    // Unix, an extension listed in PATHEXT on Windows.
    let found = unique
        .into_iter()
        .map(
            |name| match which::which_in(format!("{PREFIX}{name}"), Some(folder), cwd) {
                Ok(path) => Ok(Some((name, path))),
                Err(which::Error::CannotFindBinaryPath) => Ok(None),
                Err(
                    error @ (which::Error::CannotGetCurrentDirAndPathListEmpty
                    | which::Error::CannotCanonicalize),
                ) => Err(Error::DrillSearch {
                    folder: folder.to_path_buf(),
                    reason: error.to_string(),
                }),
            },
        )
        .collect::<Result<Vec<_>>>()?;
    Ok(found.into_iter().flatten().collect())
}
