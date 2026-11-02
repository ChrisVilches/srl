//! The commands that change items: `srl id`, `srl accept`, `srl add` and
//! `srl edit --stdin` (see `docs/notes.md`, "How srl writes to notes").
//!
//! This module decides what changes, and checks every change before
//! anything is written: item objects valid, IDs unique in the store, kinds
//! accepting the fields, `see` naming a collection of the store. Making a
//! change is the business of the item's source, which is given each item
//! as it was read, and refuses to change an item that is no longer as it
//! was (see [`Source::write`](crate::source::Source::write)).

use std::collections::HashSet;

use crate::app::catalog::check_new;
use crate::domain::note::object::{self, Object};
use crate::domain::note::{Entries, Origin};
use crate::domain::random;
use crate::source::{Change, Refusal};
use crate::{Catalog, Error, Item, ItemId, Result};

/// One change to an item: which item, where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub id: ItemId,
    /// The address of the item's collection.
    pub address: String,
    pub line: usize,
}

/// What `srl add` did, or would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub ids: Vec<ItemId>,
    /// The new items, as they appear in their collection.
    pub text: String,
}

/// `srl id`: gives an ID to every item without one. `notes` must have no
/// problem other than items without an ID.
pub fn assign_ids(notes: &Catalog, seed: u64, dry_run: bool) -> Result<Vec<Written>> {
    refuse_problems(notes, true)?;
    let mut fresh = fresh_ids(notes, HashSet::new(), seed);
    let given = notes
        .unidentified()
        .iter()
        .map(|origin| {
            let id = fresh.next().expect("random IDs should never run out");
            (origin, id)
        })
        .collect::<Vec<_>>();
    let changes = given
        .iter()
        .map(|(origin, id)| {
            let after = Object {
                entries: Entries {
                    id: Some(id.clone()),
                    ..origin.read.entries.clone()
                },
                fields: origin.read.fields.clone(),
            };
            (origin.source, replacement(origin, after))
        })
        .collect::<Vec<_>>();
    write(notes, &changes, dry_run).map_err(unexpected)?;
    Ok(given
        .into_iter()
        .map(|(origin, id)| Written {
            id,
            address: origin.address.clone(),
            line: origin.line,
        })
        .collect())
}

/// `srl accept`: removes `draft` from `items` that have it.
pub fn accept(notes: &Catalog, items: &[&Item], dry_run: bool) -> Result<Vec<Written>> {
    refuse_problems(notes, false)?;
    let drafts = items
        .iter()
        .filter(|item| item.is_draft())
        .collect::<Vec<_>>();
    let changes = drafts
        .iter()
        .map(|item| {
            let origin = &item.origin;
            let after = Object {
                entries: Entries {
                    draft: false,
                    ..origin.read.entries.clone()
                },
                fields: origin.read.fields.clone(),
            };
            (origin.source, replacement(origin, after))
        })
        .collect::<Vec<_>>();
    write(notes, &changes, dry_run).map_err(unexpected)?;
    Ok(drafts
        .into_iter()
        .map(|item| Written {
            id: item.id().clone(),
            address: item.address().to_owned(),
            line: item.line(),
        })
        .collect())
}

/// `srl add ADDRESS`: appends the items of `input`, one item object per
/// line, to the collection at `address`, or creates it. An item without an
/// `id` gets a new one.
pub fn add(notes: &Catalog, address: &str, input: &str, seed: u64, dry_run: bool) -> Result<Added> {
    refuse_problems(notes, false)?;
    check_address(address)?;
    let (source, collection) = match notes.located(address) {
        Some(located) => (located.source, located.collection.clone()),
        None => notes
            .sources()
            .for_new(address)
            .map_err(|reason| Error::InvalidNotePath {
                path: address.to_owned(),
                reason,
            })?,
    };
    let lines = read_input(notes, input)?;
    let mut given = HashSet::new();
    for (line, object) in &lines {
        let Some(id) = &object.entries.id else {
            continue;
        };
        let reason = if notes.item(id).is_some() {
            format!("the ID {id} is already used by an item; leave `id` out to get a new one")
        } else if !given.insert(id.clone()) {
            format!("the ID {id} is given twice")
        } else {
            continue;
        };
        return Err(Error::InvalidLine {
            line: *line,
            reason,
        });
    }
    let mut fresh = fresh_ids(notes, given, seed);
    let new = lines
        .iter()
        .map(|(_, object)| {
            let id = object
                .entries
                .id
                .clone()
                .or_else(|| fresh.next())
                .expect("random IDs should never run out");
            Object {
                entries: Entries {
                    id: Some(id),
                    ..object.entries.clone()
                },
                fields: object.fields.clone(),
            }
        })
        .collect::<Vec<_>>();
    check_lines(&new, &lines, notes)?;
    let changes = new
        .iter()
        .map(|item| {
            (
                source,
                Change::Add {
                    collection: collection.clone(),
                    item: item.clone(),
                },
            )
        })
        .collect::<Vec<_>>();
    let text = write(notes, &changes, dry_run).map_err(|refusal| at_line(refusal, &lines))?;
    let ids = new.into_iter().filter_map(|item| item.entries.id).collect();
    Ok(Added { ids, text })
}

/// `srl edit --stdin`: replaces, for each line of `input`, one item object
/// per line, the item its `id` names. Every line is checked, and every
/// change made and verified, before anything is written. Gives the IDs of
/// the items, in the order of `input`.
pub fn edit(notes: &Catalog, input: &str, dry_run: bool) -> Result<Vec<ItemId>> {
    refuse_problems(notes, false)?;
    let lines = read_input(notes, input)?;
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for (line, object) in &lines {
        let invalid = |reason: String| Error::InvalidLine {
            line: *line,
            reason,
        };
        let id = object.entries.id.as_ref().ok_or_else(|| {
            invalid("it has no `id`; give the ID of the item it replaces".to_owned())
        })?;
        let item = notes
            .item(id)
            .ok_or_else(|| invalid(format!("no item has the ID {id}")))?;
        if !seen.insert(id) {
            return Err(invalid(format!("the item {id} is given twice")));
        }
        targets.push(item);
    }
    let new = lines
        .iter()
        .map(|(_, object)| object.clone())
        .collect::<Vec<_>>();
    check_lines(&new, &lines, notes)?;
    let changes = targets
        .iter()
        .zip(&new)
        .map(|(item, object)| {
            (
                item.origin.source,
                replacement(&item.origin, object.clone()),
            )
        })
        .collect::<Vec<_>>();
    write(notes, &changes, dry_run).map_err(|refusal| at_line(refusal, &lines))?;
    Ok(new
        .into_iter()
        .filter_map(|object| object.entries.id)
        .collect())
}

/// The change that replaces the item read at `origin` with `after`.
fn replacement(origin: &Origin, after: Object) -> Change {
    Change::Replace {
        collection: origin.collection.clone(),
        handle: origin.handle.clone(),
        before: origin.read.clone(),
        after,
    }
}

/// Makes `changes`, each to the source it comes with, one source after
/// the other; gives the text of the items added. A refused change is named
/// by its place in `changes`.
fn write(
    notes: &Catalog,
    changes: &[(usize, Change)],
    dry_run: bool,
) -> std::result::Result<String, Refusal> {
    let mut text = String::new();
    for (index, mounted) in notes.sources().all().iter().enumerate() {
        let (places, mine): (Vec<_>, Vec<_>) = changes
            .iter()
            .enumerate()
            .filter(|(_, (source, _))| *source == index)
            .map(|(at, (_, change))| (at, change.clone()))
            .unzip();
        if mine.is_empty() {
            continue;
        }
        let written = mounted
            .source
            .write(&mine, dry_run)
            .map_err(|refusal| match refusal {
                Refusal::Change { at, reason } => Refusal::Change {
                    at: places.get(at).copied().unwrap_or(at),
                    reason,
                },
                Refusal::Error(error) => Refusal::Error(error),
            })?;
        text.push_str(&written);
    }
    Ok(text)
}

/// The error of a refusal of a change made from `lines`, named by its
/// place.
fn at_line(refusal: Refusal, lines: &[(usize, Object)]) -> Error {
    match refusal {
        Refusal::Change { at, reason } => Error::InvalidLine {
            line: lines.get(at).map_or(0, |(line, _)| *line),
            reason,
        },
        Refusal::Error(error) => error,
    }
}

/// The error of a refusal of a change that can't be refused, which can
/// only be a bug in `srl`.
fn unexpected(refusal: Refusal) -> Error {
    match refusal {
        Refusal::Change { reason, .. } => Error::CannotWrite {
            path: "an item".to_owned(),
            reason: format!("{reason} (a bug in srl; please report it)"),
        },
        Refusal::Error(error) => error,
    }
}

/// The item objects of `input`, one per line, each with its line, counting
/// from 1; blank lines are skipped. A `see` must name a collection of
/// `notes`.
fn read_input(notes: &Catalog, input: &str) -> Result<Vec<(usize, Object)>> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            item_of(notes, line)
                .map(|object| (index + 1, object))
                .map_err(|reason| Error::InvalidLine {
                    line: index + 1,
                    reason,
                })
        })
        .collect()
}

/// New item IDs from `seed`, each used by no item of `notes`, not in
/// `taken`, and given only once.
fn fresh_ids(
    notes: &Catalog,
    mut taken: HashSet<ItemId>,
    seed: u64,
) -> impl Iterator<Item = ItemId> {
    taken.extend(notes.items().iter().map(|item| item.id().clone()));
    random::numbers(seed)
        .map(ItemId::from_number)
        .filter(move |id| taken.insert(id.clone()))
}

/// Fails if `notes` has problems, except items without an ID when
/// `missing_ids` allows them.
fn refuse_problems(notes: &Catalog, missing_ids: bool) -> Result<()> {
    let count = notes
        .problems()
        .iter()
        .filter(|problem| !(missing_ids && matches!(problem, Error::MissingId { .. })))
        .count();
    if count == 0 {
        Ok(())
    } else {
        Err(Error::NotesInvalid { count })
    }
}

/// Checks the `new` items of the `lines` of standard input, as they would
/// be read in a note, all at once, so that each kind program is asked once
/// about all of them (see [`check_new`]). The first problem is an error
/// about its line, unless it is a kind program failing as a whole, which is
/// no line's fault.
fn check_lines(new: &[Object], lines: &[(usize, Object)], notes: &Catalog) -> Result<()> {
    let Some(problem) = check_new(new, notes.programs()).into_iter().next() else {
        return Ok(());
    };
    let Some((_, at)) = problem.place() else {
        return Err(problem);
    };
    let line = lines.get(at - 1).map_or(0, |(line, _)| *line);
    let reason = if let Error::Note { reason, .. } = &problem {
        reason.clone()
    } else {
        problem.to_string()
    };
    Err(Error::InvalidLine { line, reason })
}

/// Checks that `address` can name a collection (see `docs/cli.md`,
/// "Adding items"): a path relative to the store's folder.
fn check_address(address: &str) -> Result<()> {
    let invalid = |reason: &str| Error::InvalidNotePath {
        path: address.to_owned(),
        reason: reason.to_owned(),
    };
    if address.starts_with('/') || address.contains('\\') || address.contains(':') {
        return Err(invalid(
            "give a path relative to the store's folder, with / between folders",
        ));
    }
    if address
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
    {
        return Err(invalid(
            "the path has an empty part, `.`, `..`, or a folder starting with `.`, which is not \
             read for notes",
        ));
    }
    Ok(())
}

/// One line of `srl add --stdin` or `srl edit --stdin`: an item object
/// whose `see`, if any, names a collection of `notes`. Errors are plain
/// reasons.
fn item_of(notes: &Catalog, line: &str) -> std::result::Result<Object, String> {
    let object = object::read(line)?;
    if let Some(see) = &object.entries.see
        && notes.located(see).is_none()
    {
        return Err(format!(
            "`see` {see} doesn't name a note of the store; give a path relative to the \
             store's folder"
        ));
    }
    Ok(object)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;
    use crate::domain::note::is_id_text as is_id;
    use crate::{Config, Index, Lookup, Store};

    fn store(files: &[(&str, &str)]) -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (path, text) in files {
            let file = store.note_file(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
        (tmp, store)
    }

    fn notes(store: &Store) -> Catalog {
        Catalog::load(store, &Config::default(), &mut Index::open(store)).unwrap()
    }

    fn notes_with(store: &Store, config: &Config) -> Catalog {
        Catalog::load(store, config, &mut Index::open(store)).unwrap()
    }

    fn read(store: &Store, path: &str) -> String {
        fs::read_to_string(store.note_file(path)).unwrap()
    }

    #[test]
    fn ids_are_added_to_headers_without_one_and_nothing_else_changes() {
        let text = "\u{feff}```srl\r\nQ: a\r\nA: b\r\n```\r\n\r\nProse.\r\n~~~  srl\r\n@  draft\r\nQ: c\r\nA: d\r\n@ id:keep\r\nQ: e\r\nA: f\r\n~~~";
        let (_tmp, store) = store(&[("a.md", text)]);
        let written = assign_ids(&notes(&store), 7, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!((written[0].line, written[1].line), (1, 8));
        assert!(written.iter().all(|done| is_id(done.id.as_str())));
        let (one, two) = (&written[0].id, &written[1].id);
        assert_eq!(
            read(&store, "a.md"),
            format!(
                "\u{feff}```srl id:{one}\r\nQ: a\r\nA: b\r\n```\r\n\r\nProse.\r\n~~~  srl\r\n@ id:{two}  draft\r\nQ: c\r\nA: d\r\n@ id:keep\r\nQ: e\r\nA: f\r\n~~~"
            )
        );
        assert!(notes(&store).problems().is_empty());
        assert_eq!(
            assign_ids(&notes(&store), 7, false).unwrap(),
            [] as [Written; 0]
        );
    }

    #[test]
    fn ids_are_not_added_while_other_problems_remain() {
        let (_tmp, store) =
            store(&[("a.md", "```srl\nQ: a\nA: b\n```\n```srl darft\nQ: c\n```\n")]);
        let before = read(&store, "a.md");
        assert!(matches!(
            assign_ids(&notes(&store), 7, false),
            Err(Error::NotesInvalid { count: 1 })
        ));
        assert_eq!(read(&store, "a.md"), before);
    }

    #[test]
    fn dry_runs_write_nothing() {
        let (_tmp, store) = store(&[("a.md", "```srl\nQ: a\nA: b\n```\n")]);
        let before = read(&store, "a.md");
        assert_eq!(assign_ids(&notes(&store), 7, true).unwrap().len(), 1);
        assert_eq!(read(&store, "a.md"), before);
    }

    #[test]
    fn accept_removes_draft_only() {
        let text = "```srl draft id:aaa1\nQ: a\nA: b\n```\n\n```srl\n@ id:aaa2 draft see:a.md\nQ: c\nA: d\n\n@ id:aaa3 draft\nQ: e\nA: f\n```\n";
        let (_tmp, store) = store(&[("a.md", text)]);
        let notes = notes(&store);
        let chosen = notes.items().iter().take(2).collect::<Vec<_>>();
        let written = accept(&notes, &chosen, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            read(&store, "a.md"),
            "```srl id:aaa1\nQ: a\nA: b\n```\n\n```srl\n@ id:aaa2 see:a.md\nQ: c\nA: d\n\n@ id:aaa3 draft\nQ: e\nA: f\n```\n"
        );
    }

    #[test]
    fn add_appends_items_after_a_blank_line_or_creates_the_note() {
        let (_tmp, store) = store(&[("a.md", "Prose.\r\n"), ("my notes/b c.md", "")]);
        let input = "{\"Q\": \"Two\\nlines\\n\\nand more\", \"see\": \"my notes/b c.md\", \"A\": \"x\", \"draft\": true}\n\n{\"kind\": \"qa\", \"Q\": \"{{c1::y}}\\n```\\n````rust\", \"draft\": false}\n";
        let added = add(&notes(&store), "a.md", input, 3, false).unwrap();
        let [one, two] = added.ids.as_slice() else {
            panic!("expected two IDs");
        };
        let expected = format!(
            "```srl id:{one} see:\"my notes/b c.md\" draft\nQ: Two\n  lines\n\n  and more\nA: x\n```\n\n`````srl id:{two}\nQ: {{{{c1::y}}}}\n  ```\n  ````rust\n`````\n"
        );
        assert_eq!(added.text, expected);
        assert_eq!(
            read(&store, "a.md"),
            format!("Prose.\r\n\r\n{}", expected.replace('\n', "\r\n"))
        );
        let created = add(
            &notes(&store),
            "new/b.md",
            "{\"Q\": \"q\", \"A\": \"a\"}",
            4,
            false,
        )
        .unwrap();
        assert_eq!(read(&store, "new/b.md"), created.text);
        // The temporary files were in `.srl/tmp/`, and are gone.
        let names = |dir: &Path| {
            fs::read_dir(dir)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&store.root().join("new")), ["b.md"]);
        assert_eq!(names(&store.tmp_dir()), [] as [&str; 0]);
    }

    #[test]
    fn add_checks_every_line_first() {
        let (_tmp, store) = store(&[("a.md", "Prose.\n")]);
        for input in [
            "{\"Q\": \"q\"}",
            "{\"q\": \"q\", \"A\": \"a\"}",
            "{\"Q\": 1, \"A\": \"a\"}",
            "[1]",
            "{}",
            "{\"see\": \"a.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": \"nowhere.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": 1}",
            "{\"Q\": \"q\", \"A\": \"a\", \"kind\": \"nope\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"kind\": \"\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"draft\": \"yes\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"ABC\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"abc\"}\n{\"Q\": \"r\", \"A\": \"a\", \"id\": \"abc\"}",
            "{\"Q\": \"q\", \"A\": \"a\"}\nnot json",
        ] {
            assert!(
                add(&notes(&store), "a.md", input, 3, false).is_err(),
                "accepted {input}"
            );
        }
        for path in [
            "a.txt",
            "/a.md",
            "../a.md",
            ".hidden/a.md",
            "a//b.md",
            "a\\b.md",
        ] {
            assert!(
                add(
                    &notes(&store),
                    path,
                    "{\"Q\": \"q\", \"A\": \"a\"}",
                    3,
                    false
                )
                .is_err(),
                "accepted {path}"
            );
        }
        assert_eq!(read(&store, "a.md"), "Prose.\n");
    }

    #[test]
    fn add_takes_any_path_include_matches() {
        let (_tmp, store) = store(&[]);
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["**/*.txt"]);
        let item = "{\"Q\": \"q\", \"A\": \"a\"}";
        assert!(add(&notes_with(&store, &config), "a.md", item, 3, false).is_err());
        add(&notes_with(&store, &config), "new/a.txt", item, 3, false).unwrap();
        assert!(read(&store, "new/a.txt").contains("Q: q"));
    }

    #[test]
    fn add_writes_json_lines_to_a_jsonl_note_and_keeps_given_ids() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["**/*.md", "**/*.jsonl"]);
        let (_tmp, store) = store(&[("a.jsonl", "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}")]);
        let notes = Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        let input = "{\"id\": \"mine\", \"Q\": \"r\", \"A\": \"b\", \"kind\": \"qa\", \"see\": \"a.jsonl\"}\n{\"draft\": true, \"Q\": \"{{c1::s}}\"}\n";
        let added = add(&notes, "a.jsonl", input, 3, false).unwrap();
        assert_eq!(added.ids[0].as_str(), "mine");
        let two = &added.ids[1];
        assert_eq!(
            read(&store, "a.jsonl"),
            format!(
                "{{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}}\n{{\"id\":\"mine\",\"see\":\"a.jsonl\",\"Q\":\"r\",\"A\":\"b\"}}\n{{\"id\":\"{two}\",\"draft\":true,\"Q\":\"{{{{c1::s}}}}\"}}\n"
            )
        );
        let notes = Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        assert!(notes.problems().is_empty(), "{:?}", notes.problems());
        let taken = "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}";
        assert!(add(&notes, "a.jsonl", taken, 3, false).is_err());
    }

    #[test]
    fn ids_and_accept_write_json_lines_again() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["*.jsonl"]);
        let text = "{\"Q\": \"q\", \"A\": \"a\", \"draft\": true}\r\n\r\n{\"id\": \"keep\", \"Q\": \"r\", \"A\": \"b\"}\r\n";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let load = || Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        let written = assign_ids(&load(), 7, false).unwrap();
        let id = &written[0].id;
        let notes = load();
        let drafts = notes.items().iter().collect::<Vec<_>>();
        accept(&notes, &drafts, false).unwrap();
        assert_eq!(
            read(&store, "a.jsonl"),
            format!(
                "{{\"id\":\"{id}\",\"Q\":\"q\",\"A\":\"a\"}}\r\n\r\n{{\"id\": \"keep\", \"Q\": \"r\", \"A\": \"b\"}}\r\n"
            )
        );
    }

    #[test]
    fn edit_replaces_items_in_any_syntax_and_keeps_the_rest() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["*.md", "*.jsonl"]);
        let text = "Before.\n```srl id:aaa1\nQ: old\nA: old\n```\n\n````srl\n@ id:aaa2\nQ: old\nA: old\n\n@ id:aaa3\nQ: old\nA: old\n````\nAfter.\n";
        let lines = "{\"id\": \"bbb1\", \"Q\": \"old\", \"A\": \"old\"}\n";
        let (_tmp, store) = store(&[("a.md", text), ("b.jsonl", lines)]);
        let notes = Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        let input = "{\"id\": \"aaa2\", \"Q\": \"new\\n```\\ncode\\n```\", \"A\": \"a\", \"address\": \"x\", \"cards\": []}\n{\"id\": \"aaa1\", \"draft\": true, \"Q\": \"new\", \"A\": \"new\\n\\nmore\"}\n{\"id\": \"bbb1\", \"Q\": \"new\", \"A\": \"a\", \"see\": \"a.md\"}\n";
        let ids = edit(&notes, input, false).unwrap();
        let ids = ids.iter().map(ItemId::as_str).collect::<Vec<_>>();
        assert_eq!(ids, ["aaa2", "aaa1", "bbb1"]);
        assert_eq!(
            read(&store, "a.md"),
            "Before.\n```srl id:aaa1 draft\nQ: new\nA: new\n\n  more\n```\n\n````srl\n@ id:aaa2\nQ: new\n  ```\n  code\n  ```\nA: a\n\n@ id:aaa3\nQ: old\nA: old\n````\nAfter.\n"
        );
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\":\"bbb1\",\"see\":\"a.md\",\"Q\":\"new\",\"A\":\"a\"}\n"
        );
        let notes = Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        let closing = "{\"id\": \"aaa3\", \"Q\": \"q\\n````\"}";
        assert!(edit(&notes, closing, false).is_err());
    }

    #[test]
    fn several_edits_in_one_block_or_one_jsonl_note_each_find_their_item() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["*.md", "*.jsonl"]);
        let text = "```srl\n@ id:aaa1\nQ: one\nA: a\n\n@ id:aaa2\nQ: two\nA: a\n\n@ id:aaa3\nQ: three\nA: a\n```\n";
        let lines = "{\"id\": \"bbb1\", \"Q\": \"one\", \"A\": \"a\"}\n{\"id\": \"bbb2\", \"Q\": \"two\", \"A\": \"a\"}\n{\"id\": \"bbb3\", \"Q\": \"three\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("a.md", text), ("b.jsonl", lines)]);
        let notes = Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        // The first items given grow by a line, so that a later edit made
        // from the top down would miss its item.
        let input = "{\"id\": \"aaa1\", \"Q\": \"one\\nmore\", \"A\": \"a\"}\n{\"id\": \"bbb1\", \"Q\": \"1\", \"A\": \"a\"}\n{\"id\": \"aaa3\", \"Q\": \"three\\nmore\", \"A\": \"a\"}\n{\"id\": \"bbb3\", \"Q\": \"3\", \"A\": \"a\"}\n";
        edit(&notes, input, false).unwrap();
        assert_eq!(
            read(&store, "a.md"),
            "```srl\n@ id:aaa1\nQ: one\n  more\nA: a\n\n@ id:aaa2\nQ: two\nA: a\n\n@ id:aaa3\nQ: three\n  more\nA: a\n```\n"
        );
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\":\"bbb1\",\"Q\":\"1\",\"A\":\"a\"}\n{\"id\": \"bbb2\", \"Q\": \"two\", \"A\": \"a\"}\n{\"id\":\"bbb3\",\"Q\":\"3\",\"A\":\"a\"}\n"
        );
    }

    #[test]
    fn writes_find_items_by_id_in_the_note_as_it_is_now() {
        let (_tmp, store) = store(&[
            ("a.md", "```srl id:aaa1\nQ: q\nA: a\n```\n"),
            ("b.md", "```srl id:bbb1\nQ: q\nA: a\n```\n"),
        ]);
        let notes = notes(&store);
        let input = "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}\n{\"id\": \"bbb1\", \"Q\": \"new\", \"A\": \"a\"}\n";
        // An item that moved since the notes were read is found where it
        // is now.
        fs::write(
            store.note_file("b.md"),
            "Prose.\n```srl id:bbb1\nQ: q\nA: a\n```\n",
        )
        .unwrap();
        edit(&notes, input, true).unwrap();
        // An item that is gone, or a note that has an error now, stops the
        // command, and no note is written.
        for changed in ["Prose.\n", "```srl id:bbb1\nQ: q\nA: a\n```\n```srl\n"] {
            fs::write(store.note_file("b.md"), changed).unwrap();
            assert!(matches!(
                edit(&notes, input, false),
                Err(Error::ChangedMeanwhile { .. })
            ));
            assert_eq!(read(&store, "a.md"), "```srl id:aaa1\nQ: q\nA: a\n```\n");
        }
        assert_eq!(edit(&notes, "\n\n", false).unwrap(), [] as [ItemId; 0]);
    }

    #[test]
    fn ids_go_to_the_items_without_one_by_their_position() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["*.jsonl"]);
        // Two items alike in everything, but where they are, are told apart
        // by their position.
        let (_tmp, store) = store(&[(
            "a.jsonl",
            "{\"Q\": \"q\", \"A\": \"a\"}\n{\"Q\": \"q\", \"A\": \"a\"}\n",
        )]);
        let load = || Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        let written = assign_ids(&load(), 7, false).unwrap();
        let lines = written.iter().map(|done| done.line).collect::<Vec<_>>();
        assert_eq!(lines, [1, 2]);
        assert_eq!(load().items().len(), 2);
        assert!(load().problems().is_empty());
    }

    #[test]
    fn add_to_a_jsonl_note_starts_a_new_line_and_keeps_its_line_endings() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["**/*.jsonl"]);
        let (_tmp, store) = store(&[
            ("a.jsonl", "{\"id\": \"old1\", \"Q\": \"q\", \"A\": \"a\"}"),
            (
                "b.jsonl",
                "{\"id\": \"old2\", \"Q\": \"q\", \"A\": \"a\"}\r\n",
            ),
        ]);
        let load = || Catalog::load(&store, &config, &mut Index::open(&store)).unwrap();
        let item = "{\"id\": \"new1\", \"Q\": \"r\", \"A\": \"a\"}\n{\"id\": \"new2\", \"Q\": \"s\", \"A\": \"a\"}";
        let added = add(&load(), "a.jsonl", item, 3, false).unwrap();
        assert_eq!(
            added.text,
            "{\"id\":\"new1\",\"Q\":\"r\",\"A\":\"a\"}\n{\"id\":\"new2\",\"Q\":\"s\",\"A\":\"a\"}\n"
        );
        assert_eq!(
            read(&store, "a.jsonl"),
            format!(
                "{{\"id\": \"old1\", \"Q\": \"q\", \"A\": \"a\"}}\n{}",
                added.text
            )
        );
        let item = "{\"id\": \"new3\", \"Q\": \"t\", \"A\": \"a\"}";
        add(&load(), "b.jsonl", item, 3, false).unwrap();
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\": \"old2\", \"Q\": \"q\", \"A\": \"a\"}\r\n{\"id\":\"new3\",\"Q\":\"t\",\"A\":\"a\"}\r\n"
        );
        let item = "{\"id\": \"new4\", \"Q\": \"u\", \"A\": \"a\"}";
        add(&load(), "c/new.jsonl", item, 3, false).unwrap();
        assert_eq!(
            read(&store, "c/new.jsonl"),
            "{\"id\":\"new4\",\"Q\":\"u\",\"A\":\"a\"}\n"
        );
        assert!(load().problems().is_empty(), "{:?}", load().problems());
    }

    #[test]
    fn edit_checks_every_line_first() {
        let (_tmp, store) = store(&[("a.md", "```srl id:aaa1\nQ: q\nA: a\n```\n")]);
        let notes = notes(&store);
        for input in [
            "{\"Q\": \"q\", \"A\": \"a\"}",
            "{\"id\": \"zzz9\", \"Q\": \"q\", \"A\": \"a\"}",
            "{\"id\": \"aaa1\", \"Q\": \"q\"}",
            "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa1\", \"Q\": \"r\", \"A\": \"a\"}",
            "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}\nnot json",
        ] {
            assert!(edit(&notes, input, false).is_err(), "accepted {input:?}");
        }
        assert_eq!(read(&store, "a.md"), "```srl id:aaa1\nQ: q\nA: a\n```\n");
        edit(
            &notes,
            "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}",
            true,
        )
        .unwrap();
        assert_eq!(read(&store, "a.md"), "```srl id:aaa1\nQ: q\nA: a\n```\n");
    }

    #[test]
    fn an_edit_that_only_removes_draft_or_changes_nothing_keeps_the_block() {
        let text = "````srl id:aaa1 draft\nQ: q\nA: a\n````\n\n````srl id:aaa2\nQ: r\nA: b\n````\n";
        let (_tmp, store) = store(&[("a.md", text)]);
        let input = "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa2\", \"Q\": \"r\", \"A\": \"b\"}\n";
        edit(&notes(&store), input, false).unwrap();
        assert_eq!(
            read(&store, "a.md"),
            "````srl id:aaa1\nQ: q\nA: a\n````\n\n````srl id:aaa2\nQ: r\nA: b\n````\n"
        );
    }
}
