//! Editing an atom in the person's editor: `srl edit ID`, and `e` in
//! `srl study` (see `docs/cli.md`, "Editing in `$EDITOR`").

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use srl_core::{Atoms, Error, Id, Store};

/// What an edit did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edited {
    Changed,
    Unchanged,
}

/// Opens atom `id` in the editor, and replaces the atom's file with the
/// result, checked like `srl edit ID --stdin`. The editor works on a
/// temporary copy, so the atom file never holds a half-finished edit. An
/// edit that is not a valid atom is kept in its temporary file, and the
/// error says where.
pub(crate) fn edit(store: &Store, atoms: &Atoms, id: &Id) -> anyhow::Result<Edited> {
    let (program, args) = editor()?;
    edit_with(store, atoms, id, &program, &args)
}

/// Like [`edit`], with the editor `program` and its `args` given.
fn edit_with(
    store: &Store,
    atoms: &Atoms,
    id: &Id,
    program: &Path,
    args: &[String],
) -> anyhow::Result<Edited> {
    let atom = atoms
        .get(id)
        .ok_or_else(|| Error::UnknownId { id: id.clone() })?;
    let mut copy = tempfile::Builder::new()
        .prefix(&format!("srl-{id}-"))
        .suffix(".md")
        .tempfile()
        .context("cannot create a temporary file to edit the atom in")?;
    copy.write_all(atom.text().as_bytes())
        .context("cannot write the atom to a temporary file")?;
    let path = copy.path().to_path_buf();
    let status = Command::new(program)
        .args(args)
        .arg(&path)
        .status()
        .with_context(|| format!("cannot start the editor `{}`", program.display()))?;
    if !status.success() {
        bail!("the editor exited with {status}; nothing was changed");
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("cannot read the edited file {}", path.display()))?;
    if text == atom.text() {
        return Ok(Edited::Unchanged);
    }
    if let Err(error) = srl_core::replace(store, atoms, id, &text, false) {
        let kept = copy
            .keep()
            .context("cannot keep the temporary file with your edit")?
            .1;
        bail!("{error}; your edit is kept in {}", kept.display());
    }
    Ok(Edited::Changed)
}

/// The editor: `VISUAL`, else `EDITOR`, split at spaces into a program,
/// found on `PATH`, and its arguments (`code --wait`).
///
// TODO(platform): `EDITOR` and `VISUAL` are usually set on Unix but rarely
// on Windows, where this fails and says to set `EDITOR`. Decide whether to
// fall back to a platform default there (such as `notepad`). Values are
// split at spaces on every platform, so an editor whose path contains a
// space can't be named with its arguments (see docs/cli.md).
fn editor() -> anyhow::Result<(PathBuf, Vec<String>)> {
    let set = ["VISUAL", "EDITOR"]
        .iter()
        .find_map(|name| match env::var(name) {
            Ok(value) if value.trim().is_empty() => None,
            Ok(value) => Some(Ok(value)),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => Some(Err(anyhow::anyhow!(
                "{name} is not valid Unicode; set it again"
            ))),
        });
    let value = set.context(
        "no editor is set; set the EDITOR environment variable (for example EDITOR=vim)",
    )??;
    let mut words = value.split_whitespace().map(str::to_owned);
    let name = words
        .next()
        .context("the editor variable is empty; set EDITOR")?;
    let program =
        which::which(&name).with_context(|| format!("the editor `{name}` is not on PATH"))?;
    Ok((program, words.collect()))
}

#[cfg(test)]
mod tests {
    use srl_core::{Content, Lookup};
    use tempfile::TempDir;

    use super::*;

    /// A store with one atom, and its ID.
    fn store() -> (TempDir, Store, Id) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        let contents = Content::from_json_lines(r#"{"title": "x"}"#).unwrap();
        let added = srl_core::add(
            &store,
            &Atoms::load(&store).unwrap(),
            contents,
            false,
            0,
            false,
        )
        .unwrap();
        let id = added[0].id.clone();
        (tmp, store, id)
    }

    /// An editor program in `dir` that runs `unix` (a POSIX shell script) or
    /// `windows` (a batch file), with the file to edit as its argument.
    #[cfg(unix)]
    fn editor_program(dir: &Path, unix: &str, _windows: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("editor");
        fs::write(&path, format!("#!/bin/sh\n{unix}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(windows)]
    fn editor_program(dir: &Path, _unix: &str, windows: &str) -> PathBuf {
        let path = dir.join("editor.cmd");
        fs::write(&path, format!("@echo off\r\n{windows}\r\n")).unwrap();
        path
    }

    fn file(store: &Store) -> String {
        let atoms = Atoms::load(store).unwrap();
        atoms.atoms[0].text().to_owned()
    }

    #[test]
    fn an_edit_replaces_the_atom_file() {
        let (tmp, store, id) = store();
        let program = editor_program(tmp.path(), r#"echo 'More.' >> "$1""#, "echo More.>> %1");
        let atoms = Atoms::load(&store).unwrap();
        let edited = edit_with(&store, &atoms, &id, &program, &[]).unwrap();
        assert_eq!(edited, Edited::Changed);
        assert!(file(&store).ends_with("More.\n") || file(&store).ends_with("More.\r\n"));
    }

    #[test]
    fn closing_without_changes_changes_nothing() {
        let (tmp, store, id) = store();
        let before = file(&store);
        let program = editor_program(tmp.path(), "true", "exit /b 0");
        let atoms = Atoms::load(&store).unwrap();
        assert_eq!(
            edit_with(&store, &atoms, &id, &program, &[]).unwrap(),
            Edited::Unchanged
        );
        assert_eq!(file(&store), before);
    }

    #[test]
    fn an_invalid_edit_is_kept_and_the_atom_left_alone() {
        let (tmp, store, id) = store();
        let before = file(&store);
        let program = editor_program(
            tmp.path(),
            r#"echo 'not an atom' > "$1""#,
            "echo not an atom> %1",
        );
        let atoms = Atoms::load(&store).unwrap();
        let error = edit_with(&store, &atoms, &id, &program, &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("your edit is kept in"), "{error}");
        let kept = error.rsplit("kept in ").next().unwrap();
        assert!(fs::read_to_string(kept).unwrap().contains("not an atom"));
        fs::remove_file(kept).unwrap();
        assert_eq!(file(&store), before);
    }

    #[test]
    fn a_failing_editor_changes_nothing() {
        let (tmp, store, id) = store();
        let before = file(&store);
        let program = editor_program(
            tmp.path(),
            r#"echo 'More.' >> "$1"; exit 3"#,
            "echo More.>> %1\r\nexit /b 3",
        );
        let atoms = Atoms::load(&store).unwrap();
        assert!(edit_with(&store, &atoms, &id, &program, &[]).is_err());
        assert_eq!(file(&store), before);
    }
}
