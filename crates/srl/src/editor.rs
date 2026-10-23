//! Opening a note in the person's editor: `srl edit ID` (see
//! `docs/cli.md`, "Editing in the editor").

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

/// Opens `file` in the editor and waits for it. A note is the person's own
/// file, so it is edited in place and not checked afterwards: the next
/// command that reads items reports any error in it.
pub(crate) fn open(file: &Path) -> anyhow::Result<()> {
    let (program, args) = editor()?;
    open_with(file, &program, &args)
}

/// Like [`open`], with the editor `program` and its `args` given.
fn open_with(file: &Path, program: &Path, args: &[String]) -> anyhow::Result<()> {
    let status = Command::new(program)
        .args(args)
        .arg(file)
        .status()
        .with_context(|| format!("cannot start the editor `{}`", program.display()))?;
    if !status.success() {
        bail!("the editor exited with {status}");
    }
    Ok(())
}

/// The editor when neither `VISUAL` nor `EDITOR` is set, like git's.
///
// TODO(platform): Notepad is the one editor every Windows has, but this
// hasn't been run on Windows. Windows 11's Notepad may open the file in a
// tab of a Notepad window that is already open and return at once, which
// is harmless: `srl edit` doesn't check the note afterwards. See
// docs/cli.md, "Editing in the editor".
#[cfg(windows)]
const DEFAULT_EDITOR: &str = "notepad";

/// The editor when neither `VISUAL` nor `EDITOR` is set, like git's.
#[cfg(not(windows))]
const DEFAULT_EDITOR: &str = "vi";

/// The editor: `VISUAL`, else `EDITOR`, else [`DEFAULT_EDITOR`], as a
/// program found on `PATH` (or given as a path) and its arguments (see
/// [`split`]).
fn editor() -> anyhow::Result<(PathBuf, Vec<String>)> {
    let value = chosen(["VISUAL", "EDITOR"].map(|name| (name, env::var(name))))?;
    let (name, args) = split(&value)?;
    let program =
        which::which(&name).with_context(|| format!("the editor `{name}` is not on PATH"))?;
    Ok((program, args))
}

/// The first of the variables that is set and not empty, else
/// [`DEFAULT_EDITOR`]. A variable that is not valid Unicode is an error.
fn chosen<const N: usize>(
    variables: [(&str, Result<String, env::VarError>); N],
) -> anyhow::Result<String> {
    for (name, value) in variables {
        match value {
            Ok(value) if value.trim().is_empty() => {}
            Ok(value) => return Ok(value),
            Err(env::VarError::NotPresent) => {}
            Err(env::VarError::NotUnicode(_)) => {
                bail!("{name} is not valid Unicode; set it again")
            }
        }
    }
    Ok(DEFAULT_EDITOR.to_owned())
}

/// Splits an editor setting into a program and its arguments: at spaces,
/// except that a setting starting with `"` names the program up to the
/// next `"`, so that a path with spaces can come with arguments
/// (`"C:\Program Files\Microsoft VS Code\bin\code.cmd" --wait`).
fn split(value: &str) -> anyhow::Result<(String, Vec<String>)> {
    let value = value.trim();
    let (name, rest) = match value.strip_prefix('"') {
        Some(quoted) => quoted.split_once('"').with_context(|| {
            format!(
                "the editor setting `{value}` has a `\"` that is never closed; fix VISUAL or EDITOR"
            )
        })?,
        None => value.split_once(char::is_whitespace).unwrap_or((value, "")),
    };
    if name.is_empty() {
        bail!("the editor setting `{value}` names no program; fix VISUAL or EDITOR");
    }
    let args = rest.split_whitespace().map(str::to_owned).collect();
    Ok((name.to_owned(), args))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

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

    #[test]
    fn the_editor_works_on_the_note_itself() {
        let tmp = TempDir::new().unwrap();
        let note = tmp.path().join("a.md");
        fs::write(&note, "Prose.\n").unwrap();
        let program = editor_program(tmp.path(), r#"echo 'More.' >> "$1""#, "echo More.>> %1");
        open_with(&note, &program, &[]).unwrap();
        assert!(fs::read_to_string(&note).unwrap().contains("More."));
    }

    #[test]
    fn a_failing_editor_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let note = tmp.path().join("a.md");
        let program = editor_program(tmp.path(), "exit 3", "exit /b 3");
        assert!(open_with(&note, &program, &[]).is_err());
    }

    fn words(program: &str, args: &[&str]) -> (String, Vec<String>) {
        (
            program.to_owned(),
            args.iter().map(|arg| (*arg).to_owned()).collect(),
        )
    }

    #[test]
    fn an_editor_setting_is_split_at_spaces() {
        assert_eq!(split("vim").unwrap(), words("vim", &[]));
        assert_eq!(split(" code  --wait ").unwrap(), words("code", &["--wait"]));
    }

    #[test]
    fn a_quoted_program_may_contain_spaces() {
        assert_eq!(
            split(r#""C:\Program Files\Microsoft VS Code\bin\code.cmd" --wait"#).unwrap(),
            words(
                r"C:\Program Files\Microsoft VS Code\bin\code.cmd",
                &["--wait"]
            )
        );
        assert_eq!(
            split(r#""/opt/my editor/ed""#).unwrap(),
            words("/opt/my editor/ed", &[])
        );
        assert!(split(r#""/opt/my editor/ed --wait"#).is_err());
        assert!(split(r#""" --wait"#).is_err());
    }

    #[test]
    fn visual_comes_before_editor_and_the_platform_default_comes_last() {
        let unset = || Err(env::VarError::NotPresent);
        let set = |value: &str| Ok(value.to_owned());
        assert_eq!(
            chosen([("VISUAL", set("vim")), ("EDITOR", set("nano"))]).unwrap(),
            "vim"
        );
        assert_eq!(
            chosen([("VISUAL", set("  ")), ("EDITOR", set("nano"))]).unwrap(),
            "nano"
        );
        assert_eq!(
            chosen([("VISUAL", unset()), ("EDITOR", unset())]).unwrap(),
            DEFAULT_EDITOR
        );
    }
}
