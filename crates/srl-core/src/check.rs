//! `srl check`: validating atoms, drills and cards, and assigning missing
//! IDs (see `docs/atoms.md`, "Validation").

use std::collections::HashSet;
use std::fs;
use std::time::Duration;

use jiff::civil::Date;

use crate::add::{self, random_ids};
use crate::{Atom, Atoms, Drills, Error, Id, Result, Store, atom, drill, frontmatter};

/// An atom that `srl check --fix` gave an ID (or would, with `--dry-run`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub id: Id,
    /// Relative to `atoms/`, with `/` between folders.
    pub path: String,
}

/// What `srl check` found.
#[derive(Debug)]
pub struct Report {
    /// The atoms that got an ID, with `fix`.
    pub fixed: Vec<Fixed>,
    /// Every problem left, in the order: paths over the length limit,
    /// unreadable atom files, then each atom's facets, drills and cards, by
    /// path.
    pub problems: Vec<Error>,
}

/// Checks every atom of the store, runs every drill that applies to it, and
/// checks the cards (see `docs/atoms.md`, "Validation").
///
/// With `fix`, atoms without an `id` first get one, made from the seed in
/// `fix`; with `dry_run`, nothing is written. Atoms are read again after
/// fixing, so the report describes the store as it is now.
pub fn check(
    store: &Store,
    drills: &Drills,
    day: Date,
    timeout: Duration,
    fix: Option<u64>,
    dry_run: bool,
) -> Result<Report> {
    let atoms = Atoms::load(store)?;
    let fixed = match fix {
        Some(seed) => assign_ids(store, &atoms, seed, dry_run)?,
        None => Vec::new(),
    };
    let atoms = if fixed.is_empty() || dry_run {
        atoms
    } else {
        Atoms::load(store)?
    };
    let card_problems = atoms
        .atoms
        .iter()
        .flat_map(|atom| atom_problems(atom, &atoms, drills, day, timeout))
        .collect::<Vec<_>>();
    Ok(Report {
        fixed,
        problems: long_paths(store)?
            .into_iter()
            .chain(atoms.problems)
            .chain(card_problems)
            .collect(),
    })
}

/// The atom files whose full path is longer than new files may be (see
/// `docs/atoms.md`, "Creating atoms"), as problems.
fn long_paths(store: &Store) -> Result<Vec<Error>> {
    let room = add::path_room(store)?;
    Ok(store
        .atom_paths()?
        .into_iter()
        .filter(|path| add::units(path) > room.units)
        .map(|path| Error::PathOverLimit {
            path,
            limit: add::MAX_PATH,
        })
        .collect())
}

/// The problems of one atom's facets, drills and cards.
fn atom_problems(
    atom: &Atom,
    atoms: &Atoms,
    drills: &Drills,
    day: Date,
    timeout: Duration,
) -> Vec<Error> {
    if atom.content().facets().is_empty() {
        return vec![Error::NoFacets {
            id: atom.id().clone(),
        }];
    }
    let (_, missing) = drills.applicable(atom);
    let missing = missing.into_iter().map(|facet| {
        let (id, facet) = (atom.id().clone(), facet.to_owned());
        if drill::is_drill_name(&facet) {
            Error::NoDrill { id, facet }
        } else {
            Error::NotADrillName { id, facet }
        }
    });
    let from_drills = drills
        .cards(atom, atoms, day, timeout)
        .into_iter()
        .flat_map(|made| {
            if made.cards.is_empty() && made.problems.is_empty() {
                vec![Error::NoCards {
                    drill: made.drill,
                    id: atom.id().clone(),
                }]
            } else {
                made.problems
            }
        });
    missing.chain(from_drills).collect()
}

/// Gives each atom file without an `id` a new one, by adding an `id` line.
/// Every file is prepared and checked before the first one is written.
fn assign_ids(store: &Store, atoms: &Atoms, seed: u64, dry_run: bool) -> Result<Vec<Fixed>> {
    let mut used = atoms
        .atoms
        .iter()
        .map(|atom| atom.id().clone())
        .chain(atoms.problems.iter().filter_map(|problem| {
            if let Error::DuplicateId { id, .. } = problem {
                Some(id.clone())
            } else {
                None
            }
        }))
        .collect::<HashSet<_>>();
    let mut ids = random_ids(seed);
    let prepared = atoms
        .problems
        .iter()
        .filter_map(|problem| {
            if let Error::MissingId { path } = problem {
                Some(path)
            } else {
                None
            }
        })
        .map(|path| {
            let id = ids
                .by_ref()
                .find(|id| !used.contains(id))
                .expect("the sequence of IDs should be endless");
            used.insert(id.clone());
            let text = with_id(store, path, &id)?;
            Ok((
                Fixed {
                    id,
                    path: path.clone(),
                },
                text,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    if !dry_run {
        for (fixed, text) in &prepared {
            let file = store.atom_file(&fixed.path);
            fs::write(&file, text).map_err(Error::io(file))?;
        }
    }
    Ok(prepared.into_iter().map(|(fixed, _)| fixed).collect())
}

/// The text of the atom file at `path` with `id` added. Checks that it reads
/// back as the same atom with that ID.
fn with_id(store: &Store, path: &str, id: &Id) -> Result<String> {
    let cannot = |reason: String| Error::CannotEdit {
        path: path.to_owned(),
        reason,
    };
    let file = store.atom_file(path);
    let text = fs::read_to_string(&file).map_err(Error::io(&file))?;
    let (_, content) = atom::read(&text).map_err(cannot)?;
    let new = frontmatter::set(&text, "id", id.as_str()).map_err(cannot)?;
    if atom::read(&new).map_err(cannot)? == (Some(id.clone()), content) {
        Ok(new)
    } else {
        Err(cannot(
            "it would not read back with only its `id` added".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::{Lookup, Search};

    fn store(files: &[(&str, &str)]) -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (path, text) in files {
            fs::write(store.atom_file(path), text).unwrap();
        }
        (tmp, store)
    }

    fn run(store: &Store, fix: Option<u64>, dry_run: bool) -> Report {
        let (drills, problems) = Drills::find(store, &Search::default());
        assert!(problems.is_empty());
        let day = Date::constant(2026, 10, 26);
        check(store, &drills, day, Duration::from_secs(10), fix, dry_run).unwrap()
    }

    #[test]
    fn a_clean_store_has_no_problems() {
        let (_tmp, store) = store(&[(
            "a.md",
            "---\nid: aaaaa1\ntitle: x\ncloze: ['{{a}} b']\n---\n",
        )]);
        let report = run(&store, None, false);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
    }

    #[test]
    fn facets_without_drills_and_without_cards_are_reported() {
        let (_tmp, store) = store(&[(
            "a.md",
            "---\nid: aaaaa1\ntitle: x\nconj: {verb: 食べる}\nmy facet: 1\ncards: []\ncloze: [no span]\n---\n",
        )]);
        let problems = run(&store, None, false).problems;
        assert!(
            matches!(&problems[..], [
            Error::NoDrill { facet: conj, .. },
            Error::NotADrillName { .. },
            Error::NoCards { drill: cards, .. },
            Error::DrillFailed { drill: cloze, .. },
        ] if conj == "conj" && cards == "cards" && cloze == "cloze"),
            "{problems:?}"
        );
    }

    #[test]
    fn atoms_without_facets_are_reported() {
        let (_tmp, store) = store(&[("a.md", "---\nid: aaaaa1\ntitle: x\n---\nNotes.\n")]);
        let problems = run(&store, None, false).problems;
        assert!(
            matches!(&problems[..], [Error::NoFacets { .. }]),
            "{problems:?}"
        );
    }

    #[test]
    fn fix_assigns_missing_ids_and_keeps_the_rest_of_the_file() {
        let text = "---\ntitle: x  # a comment\ncards: [{q: a, a: b}]\n---\nbody\n";
        let (_tmp, store) = store(&[
            ("a.md", text),
            (
                "b.md",
                "---\nid: aaaaa1\ntitle: y\ncards: [{q: a, a: b}]\n---\n",
            ),
        ]);
        let dry = run(&store, Some(0), true);
        assert_eq!(dry.fixed.len(), 1);
        assert!(matches!(&dry.problems[..], [Error::MissingId { .. }]));
        assert_eq!(fs::read_to_string(store.atom_file("a.md")).unwrap(), text);

        let report = run(&store, Some(0), false);
        assert_eq!(report.fixed, dry.fixed);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(
            fs::read_to_string(store.atom_file("a.md")).unwrap(),
            format!(
                "---\nid: {}\ntitle: x  # a comment\ncards: [{{q: a, a: b}}]\n---\nbody\n",
                report.fixed[0].id
            )
        );
    }

    #[test]
    fn paths_longer_than_new_files_may_be_are_reported() {
        let atom = "---\nid: aaaaa1\ntitle: x\ncards: [{q: a, a: b}]\n---\n";
        let (_tmp, store) = store(&[(
            "a.md",
            "---\nid: aaaaa2\ntitle: y\ncards: [{q: a, a: b}]\n---\n",
        )]);
        let long = format!(
            "{}/{}/{}.md",
            "x".repeat(100),
            "y".repeat(100),
            "z".repeat(60)
        );
        fs::create_dir_all(store.atom_file(&long).parent().unwrap()).unwrap();
        fs::write(store.atom_file(&long), atom).unwrap();
        let problems = run(&store, None, false).problems;
        assert!(
            matches!(&problems[..], [Error::PathOverLimit { path, .. }] if *path == long),
            "{problems:?}"
        );
    }
}
