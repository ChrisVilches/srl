//! Sessions: prepared lists of cards in `sessions/NAME.jsonl` (see
//! `docs/cli.md`, "Sessions").

use std::fs;
use std::io;

use serde::Serialize;

use crate::{Atoms, Card, Error, Result, Store, drill};

/// A session and how many cards it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionInfo {
    pub name: String,
    pub cards: usize,
}

/// Checks that `name` can name a session: lowercase letters, digits, `-`
/// and `_`, as drill names, because it becomes a file name.
fn check_name(name: &str) -> Result<()> {
    if drill::is_drill_name(name) {
        Ok(())
    } else {
        Err(Error::InvalidSessionName {
            name: name.to_owned(),
        })
    }
}

/// Saves the cards in `text` (one JSON card per line) as session `name`,
/// replacing a session of that name, and returns how many cards it holds.
/// Every card is checked first: if any line is not a valid card, or there
/// are none, nothing is written. With `dry_run`, nothing is written.
pub fn put_session(
    store: &Store,
    atoms: &Atoms,
    name: &str,
    text: &str,
    dry_run: bool,
) -> Result<usize> {
    check_name(name)?;
    let cards = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            parse_card(line, atoms).map_err(|reason| Error::InvalidSessionLine {
                line: index + 1,
                reason,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if cards.is_empty() {
        return Err(Error::EmptySession {
            name: name.to_owned(),
        });
    }
    let lines = cards
        .iter()
        .map(|card| {
            serde_json::to_string(card)
                .map(|line| format!("{line}\n"))
                .map_err(|error| Error::InvalidSessionLine {
                    line: 0,
                    reason: error.to_string(),
                })
        })
        .collect::<Result<String>>()?;
    if !dry_run {
        let dir = store.sessions_dir();
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let file = store.session_file(name);
        fs::write(&file, lines).map_err(Error::io(file))?;
    }
    Ok(cards.len())
}

fn parse_card(line: &str, atoms: &Atoms) -> std::result::Result<Card, String> {
    let value = serde_json::from_str(line).map_err(|error| format!("it is not JSON ({error})"))?;
    Card::from_value(value, atoms)
}

/// Every session, by name, with its number of cards (its non-blank lines).
pub fn list_sessions(store: &Store) -> Result<Vec<SessionInfo>> {
    let dir = store.sessions_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(Error::io(&dir)(error)),
    };
    let mut sessions = entries
        .map(|entry| {
            let path = entry.map_err(Error::io(&dir))?.path();
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|_| {
                    path.extension()
                        .is_some_and(|extension| extension == "jsonl")
                })
                .map(str::to_owned);
            match name {
                Some(name) => {
                    let text = fs::read_to_string(&path).map_err(Error::io(&path))?;
                    let cards = text.lines().filter(|line| !line.trim().is_empty()).count();
                    Ok(Some(SessionInfo { name, cards }))
                }
                None => Ok(None),
            }
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sessions)
}

/// Removes session `name`, which must exist. With `dry_run`, only checks.
pub fn remove_session(store: &Store, name: &str, dry_run: bool) -> Result<()> {
    check_name(name)?;
    let file = store.session_file(name);
    if !file.is_file() {
        return Err(Error::UnknownSession {
            name: name.to_owned(),
        });
    }
    if !dry_run {
        fs::remove_file(&file).map_err(Error::io(file))?;
    }
    Ok(())
}

/// The cards of session `name`, in file order, checked against the atoms
/// as they are now. A card that is no longer valid (its atom was deleted,
/// say) is left out and reported in the second value.
pub fn load_session(store: &Store, atoms: &Atoms, name: &str) -> Result<(Vec<Card>, Vec<Error>)> {
    check_name(name)?;
    let file = store.session_file(name);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(Error::UnknownSession {
                name: name.to_owned(),
            });
        }
        Err(error) => return Err(Error::io(file)(error)),
    };
    let (cards, problems): (Vec<_>, Vec<_>) = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            parse_card(line, atoms).map_err(|reason| Error::InvalidSessionCard {
                name: name.to_owned(),
                line: index + 1,
                reason,
            })
        })
        .partition(std::result::Result::is_ok);
    Ok((
        cards.into_iter().flatten().collect(),
        problems
            .into_iter()
            .filter_map(std::result::Result::err)
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::Lookup;

    fn store() -> (TempDir, Store, Atoms) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        fs::write(store.atom_file("a.md"), "---\nid: aaaaa1\ntitle: x\n---\n").unwrap();
        let atoms = Atoms::load(&store).unwrap();
        (tmp, store, atoms)
    }

    const CARD: &str =
        r#"{"atoms": ["aaaaa1"], "drill": "llm", "ref": "llm:1", "prompt": "q", "answer": "a"}"#;

    #[test]
    fn a_session_is_saved_listed_loaded_and_removed() {
        let (_tmp, store, atoms) = store();
        assert_eq!(
            put_session(
                &store,
                &atoms,
                "n1-food",
                &format!("{CARD}\n\n{CARD}\n"),
                false
            )
            .unwrap(),
            2
        );
        assert_eq!(
            list_sessions(&store).unwrap(),
            [SessionInfo {
                name: "n1-food".to_owned(),
                cards: 2
            }]
        );
        let (cards, problems) = load_session(&store, &atoms, "n1-food").unwrap();
        assert_eq!(cards.len(), 2);
        assert!(problems.is_empty());
        remove_session(&store, "n1-food", false).unwrap();
        assert!(list_sessions(&store).unwrap().is_empty());
        assert!(matches!(
            remove_session(&store, "n1-food", false),
            Err(Error::UnknownSession { .. })
        ));
    }

    #[test]
    fn one_bad_card_or_none_saves_nothing() {
        let (_tmp, store, atoms) = store();
        let bad = CARD.replace("aaaaa1", "zzzzzz");
        assert!(matches!(
            put_session(&store, &atoms, "s", &format!("{CARD}\n{bad}\n"), false),
            Err(Error::InvalidSessionLine { line: 2, .. })
        ));
        assert!(matches!(
            put_session(&store, &atoms, "s", "\n", false),
            Err(Error::EmptySession { .. })
        ));
        assert!(put_session(&store, &atoms, "s", CARD, true).is_ok());
        assert!(list_sessions(&store).unwrap().is_empty());
    }

    #[test]
    fn session_names_are_file_safe() {
        let (_tmp, store, atoms) = store();
        for name in ["../x", "My Session", "", "a/b"] {
            assert!(
                matches!(
                    put_session(&store, &atoms, name, CARD, false),
                    Err(Error::InvalidSessionName { .. })
                ),
                "accepted {name}"
            );
        }
    }

    #[test]
    fn cards_that_became_invalid_are_left_out_and_reported() {
        let (_tmp, store, atoms) = store();
        put_session(&store, &atoms, "s", CARD, false).unwrap();
        let empty = Atoms {
            atoms: Vec::new(),
            problems: Vec::new(),
        };
        let (cards, problems) = load_session(&store, &empty, "s").unwrap();
        assert!(cards.is_empty());
        assert!(matches!(
            &problems[..],
            [Error::InvalidSessionCard { line: 1, .. }]
        ));
    }
}
