//! Changing atoms that exist: their status, or their whole file (see
//! `docs/atoms.md`, "Status" and "Identity and editing").
//!
//! Every change is prepared and checked for every atom before the first file
//! is written, so a command either changes everything it should or nothing.

use std::fs;

use crate::atom;
use crate::{Atom, Atoms, Error, Id, Result, Selector, Status, Store, frontmatter};

/// Sets the status of every atom that matches `selector` and has status
/// `from` to `to`, and returns their IDs, ordered by path. Matching atoms
/// with another status are left alone. With `dry_run`, nothing is written.
///
/// Only the `status` line of each file changes (see `frontmatter`).
pub fn change_status(
    store: &Store,
    atoms: &Atoms,
    selector: &Selector,
    from: Status,
    to: Status,
    dry_run: bool,
) -> Result<Vec<Id>> {
    let prepared = atoms
        .select(selector)
        .into_iter()
        .filter(|atom| atom.content().status() == from)
        .map(|atom| with_status(atom, to).map(|text| (atom, text)))
        .collect::<Result<Vec<_>>>()?;
    if !dry_run {
        for (atom, text) in &prepared {
            overwrite(store, atom, text)?;
        }
    }
    Ok(prepared
        .into_iter()
        .map(|(atom, _)| atom.id().clone())
        .collect())
}

/// The text of `atom`'s file with its status set to `status`. Checks that it
/// reads back as the same atom with only the status changed.
fn with_status(atom: &Atom, status: Status) -> Result<String> {
    let cannot = |reason: String| Error::CannotEdit {
        path: atom.path().to_owned(),
        reason,
    };
    let text = frontmatter::set(atom.text(), "status", status.as_str()).map_err(cannot)?;
    let expected = (
        Some(atom.id().clone()),
        atom.content().clone().with_status(status),
    );
    if atom::read(&text).map_err(cannot)? == expected {
        Ok(text)
    } else {
        Err(cannot(
            "it would not read back with only its status changed".to_owned(),
        ))
    }
}

/// Replaces the file of atom `id` with `text`, which must be a valid atom
/// with the same `id`. The file keeps its path, whatever its new tags and
/// cue. With `dry_run`, the text is only checked.
pub fn replace(store: &Store, atoms: &Atoms, id: &Id, text: &str, dry_run: bool) -> Result<()> {
    let atom = atoms
        .get(id)
        .ok_or_else(|| Error::UnknownId { id: id.clone() })?;
    let invalid = |reason: String| Error::InvalidEdit {
        id: id.clone(),
        reason,
    };
    match atom::read(text).map_err(invalid)?.0 {
        Some(new) if &new == id => {}
        Some(new) => {
            return Err(invalid(format!(
                "it changes the `id` to {new}; keep `id: {id}`"
            )));
        }
        None => return Err(invalid(format!("it has no `id`; keep `id: {id}`"))),
    }
    if !dry_run {
        overwrite(store, atom, text)?;
    }
    Ok(())
}

/// Writes `text` over the file of `atom`.
fn overwrite(store: &Store, atom: &Atom, text: &str) -> Result<()> {
    let file = store.atom_file(atom.path());
    fs::write(&file, text).map_err(Error::io(file))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::{Lookup, NewAtom, add};

    /// A store with the given atom files, and its atoms.
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

    fn id(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    fn file(store: &Store, path: &str) -> String {
        fs::read_to_string(store.atom_file(path)).unwrap()
    }

    const DRAFT: &str =
        "---\nid: aaaaa1\ncue: a  # first\nstatus: draft # by an agent\ntags: [x]\n---\nbody\n";
    const ACTIVE: &str = "---\nid: aaaaa2\ncue: b\ntags: [x]\n---\n";
    const SUSPENDED: &str = "---\nid: aaaaa3\ncue: c\nstatus: suspended\n---\n";

    fn three() -> (TempDir, Store) {
        store(&[("a.md", DRAFT), ("b.md", ACTIVE), ("c.md", SUSPENDED)])
    }

    #[test]
    fn only_atoms_in_the_source_status_change() {
        let (_tmp, store) = three();
        let atoms = Atoms::load(&store).unwrap();
        let all = Selector::parse("").unwrap();
        let changed =
            change_status(&store, &atoms, &all, Status::Draft, Status::Active, false).unwrap();
        assert_eq!(changed, [id("aaaaa1")]);
        assert_eq!(file(&store, "a.md"), DRAFT.replace("draft", "active"));
        assert_eq!(file(&store, "b.md"), ACTIVE);
        assert_eq!(file(&store, "c.md"), SUSPENDED);
    }

    #[test]
    fn a_missing_status_line_is_added() {
        let (_tmp, store) = three();
        let atoms = Atoms::load(&store).unwrap();
        let selector = Selector::parse("tag:x").unwrap();
        let changed = change_status(
            &store,
            &atoms,
            &selector,
            Status::Active,
            Status::Suspended,
            false,
        )
        .unwrap();
        assert_eq!(changed, [id("aaaaa2")]);
        assert_eq!(
            file(&store, "b.md"),
            "---\nstatus: suspended\nid: aaaaa2\ncue: b\ntags: [x]\n---\n"
        );
    }

    #[test]
    fn dry_run_changes_no_status() {
        let (_tmp, store) = three();
        let atoms = Atoms::load(&store).unwrap();
        let all = Selector::parse("").unwrap();
        let changed = change_status(
            &store,
            &atoms,
            &all,
            Status::Suspended,
            Status::Active,
            true,
        )
        .unwrap();
        assert_eq!(changed, [id("aaaaa3")]);
        assert_eq!(file(&store, "c.md"), SUSPENDED);
    }

    #[test]
    fn one_file_that_cannot_be_edited_changes_nothing() {
        let multi_line = "---\nid: aaaaa4\ncue: d\nstatus: >-\n  draft\n---\n";
        let (_tmp, store) = store(&[("a.md", DRAFT), ("d.md", multi_line)]);
        let atoms = Atoms::load(&store).unwrap();
        let all = Selector::parse("").unwrap();
        let result = change_status(&store, &atoms, &all, Status::Draft, Status::Active, false);
        assert!(matches!(result, Err(Error::CannotEdit { .. })));
        assert_eq!(file(&store, "a.md"), DRAFT);
    }

    #[test]
    fn replace_keeps_the_path_and_takes_the_text_as_is() {
        let (_tmp, store) = three();
        let atoms = Atoms::load(&store).unwrap();
        let text = "---\nid: aaaaa2\ncue: b, rewritten\ntags: [elsewhere]\n---\r\nnew body";
        replace(&store, &atoms, &id("aaaaa2"), text, false).unwrap();
        assert_eq!(file(&store, "b.md"), text);
    }

    #[test]
    fn replace_refuses_a_changed_or_missing_id_and_invalid_atoms() {
        let (_tmp, store) = three();
        let atoms = Atoms::load(&store).unwrap();
        for text in [
            "---\nid: zzzzzz\ncue: b\n---\n",
            "---\ncue: b\n---\n",
            "---\nid: aaaaa2\n---\n",
            "no front-matter\n",
        ] {
            assert!(
                matches!(
                    replace(&store, &atoms, &id("aaaaa2"), text, false),
                    Err(Error::InvalidEdit { .. })
                ),
                "accepted:\n{text}"
            );
        }
        assert_eq!(file(&store, "b.md"), ACTIVE);
    }

    #[test]
    fn replace_needs_a_known_atom_and_dry_run_writes_nothing() {
        let (_tmp, store) = three();
        let atoms = Atoms::load(&store).unwrap();
        assert!(matches!(
            replace(&store, &atoms, &id("zzzzzz"), ACTIVE, false),
            Err(Error::UnknownId { .. })
        ));
        let text = ACTIVE.replace("cue: b", "cue: B");
        replace(&store, &atoms, &id("aaaaa2"), &text, true).unwrap();
        assert_eq!(file(&store, "b.md"), ACTIVE);
    }

    #[test]
    fn copied_drafts_can_be_accepted() {
        let (_tmp, store) = store(&[]);
        let new = NewAtom::from_file("in.md".into(), "---\ncue: x # c\n---\n".to_owned()).unwrap();
        let added = add(
            &store,
            &Atoms::load(&store).unwrap(),
            vec![new],
            true,
            0,
            false,
        )
        .unwrap();
        let atoms = Atoms::load(&store).unwrap();
        let all = Selector::parse("").unwrap();
        change_status(&store, &atoms, &all, Status::Draft, Status::Active, false).unwrap();
        assert_eq!(
            file(&store, &added[0].path),
            format!(
                "---\nid: {}\nstatus: active\ncue: x # c\n---\n",
                added[0].id
            )
        );
    }
}
