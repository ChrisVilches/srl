//! The commands that change items: `srl accept`, `srl add`, `srl edit
//! --stdin` and `srl move` (see `docs/items.md`, "How srl changes
//! items"), and `srl create`, which makes a collection for them.
//!
//! This module decides what changes, and checks every change before
//! anything is written: item objects valid, IDs unique in the store, kinds
//! accepting the fields, `see` naming a file of study material. Making a
//! change is the business of the item's source, which is given each item
//! as it was read, and refuses to change an item that is no longer as it
//! was (see [`Source::write`](crate::source::Source::write)).

use std::collections::HashSet;

use crate::app::catalog::check_new;
use crate::domain::item::object::{self, Object};
use crate::domain::item::{Entries, Origin};
use crate::domain::random;
use crate::source::{Change, Refusal};
use crate::{Catalog, Error, Item, ItemId, Result};

/// What `srl add` did, or would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub ids: Vec<ItemId>,
    /// The new items, as they appear in their collection.
    pub text: String,
}

/// What `srl create` did, or would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    pub collection: String,
    /// The name of the source it is in.
    pub source: String,
}

/// `srl create COLLECTION`: creates `collection`, without items, in the
/// one source that accepts its name (see `docs/sources.md`,
/// "Collections").
pub fn create(catalog: &Catalog, collection: &str, dry_run: bool) -> Result<Created> {
    refuse_problems(catalog)?;
    check_collection(collection)?;
    if let Some(located) = catalog.located(collection) {
        let name = catalog
            .sources()
            .all()
            .get(located.source)
            .map_or("", |named| named.name.as_str());
        return Err(Error::CannotCreate {
            collection: collection.to_owned(),
            reason: format!("it exists already, in the source {name}"),
        });
    }
    let source = catalog.sources().for_new(collection)?;
    let change = Change::Create {
        collection: collection.to_owned(),
    };
    write(catalog, &[(source, change)], dry_run).map_err(|refusal| match refusal {
        Refusal::Change { reason, .. } => Error::CannotCreate {
            collection: collection.to_owned(),
            reason,
        },
        Refusal::Error(error) => error,
    })?;
    Ok(Created {
        collection: collection.to_owned(),
        source: catalog
            .sources()
            .all()
            .get(source)
            .map(|named| named.name.clone())
            .unwrap_or_default(),
    })
}

/// `srl accept`: removes `draft` from `items` that have it. Gives the IDs
/// of those items, in the order of `items`.
pub fn accept(catalog: &Catalog, items: &[&Item], dry_run: bool) -> Result<Vec<ItemId>> {
    refuse_problems(catalog)?;
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
    write(catalog, &changes, dry_run).map_err(unexpected)?;
    Ok(drafts.into_iter().map(|item| item.id().clone()).collect())
}

/// `srl add COLLECTION`: adds the items of `input`, one item object per
/// line, to the collection `collection`, which must exist. An item without
/// an `id` gets a new one.
pub fn add(
    catalog: &Catalog,
    collection: &str,
    input: &str,
    seed: u64,
    dry_run: bool,
) -> Result<Added> {
    refuse_problems(catalog)?;
    let located = catalog
        .located(collection)
        .ok_or_else(|| Error::CannotAdd {
            collection: collection.to_owned(),
            reason: "no collection has this name; create it with `srl create`, or give one \
                     `srl collections` lists"
                .to_owned(),
        })?;
    let (source, collection) = (located.source, located.collection.clone());
    let lines = read_input(catalog, input)?;
    let mut given = HashSet::new();
    for (line, object) in &lines {
        let Some(id) = &object.entries.id else {
            continue;
        };
        let reason = if catalog.item(id).is_some() {
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
    let mut fresh = fresh_ids(catalog, given, seed);
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
    check_lines(&new, &lines, catalog)?;
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
    let text = write(catalog, &changes, dry_run).map_err(|refusal| at_line(refusal, &lines))?;
    let ids = new.into_iter().filter_map(|item| item.entries.id).collect();
    Ok(Added { ids, text })
}

/// `srl edit --stdin`: replaces, for each line of `input`, one item object
/// per line, the item its `id` names. Every line is checked, and every
/// change made and verified, before anything is written. Gives the IDs of
/// the items, in the order of `input`.
pub fn edit(catalog: &Catalog, input: &str, dry_run: bool) -> Result<Vec<ItemId>> {
    refuse_problems(catalog)?;
    let lines = read_input(catalog, input)?;
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
        let item = catalog
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
    check_lines(&new, &lines, catalog)?;
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
    write(catalog, &changes, dry_run).map_err(|refusal| at_line(refusal, &lines))?;
    Ok(new
        .into_iter()
        .filter_map(|object| object.entries.id)
        .collect())
}

/// `srl move ID... COLLECTION`: moves the items `ids` to `collection`,
/// which must exist; an item already there is left as it is. Gives the IDs
/// of the items moved, in the order of `ids`.
///
/// The source of `collection` is written first, with the items added and
/// those removed from its own collections; then every other source, with
/// the items removed from it. So an item is never in neither place: if a
/// removal fails after the items were added, the error says so.
pub fn move_items(
    catalog: &Catalog,
    ids: &[ItemId],
    collection: &str,
    dry_run: bool,
) -> Result<Vec<ItemId>> {
    refuse_problems(catalog)?;
    let refuse = |reason: String| Error::CannotMove {
        collection: collection.to_owned(),
        reason,
    };
    let target = catalog.located(collection).ok_or_else(|| {
        refuse(
            "no collection has this name, and srl move moves items only to a collection that \
             exists; `srl collections` lists them"
                .to_owned(),
        )
    })?;
    let mut seen = HashSet::new();
    let mut moving = Vec::new();
    for id in ids {
        let item = catalog
            .item(id)
            .ok_or_else(|| refuse(format!("no item has the ID {id}")))?;
        if !seen.insert(id) {
            return Err(refuse(format!("the item {id} is given twice")));
        }
        if item.collection() != target.collection {
            moving.push(item);
        }
    }
    let removal = |item: &Item| {
        (
            item.origin.source,
            Change::Remove {
                collection: item.origin.collection.clone(),
                before: item.origin.read.clone(),
            },
        )
    };
    let (here, elsewhere): (Vec<&Item>, Vec<&Item>) = moving
        .iter()
        .partition(|item| item.origin.source == target.source);
    // Each change of the first write, with the item it is about.
    let (whose, first): (Vec<&Item>, Vec<_>) = moving
        .iter()
        .map(|item| {
            let add = Change::Add {
                collection: target.collection.clone(),
                item: item.origin.read.clone(),
            };
            (*item, (target.source, add))
        })
        .chain(here.iter().map(|item| (*item, removal(item))))
        .unzip();
    write(catalog, &first, dry_run).map_err(|refusal| match refusal {
        Refusal::Change { at, reason } => refuse(match whose.get(at) {
            Some(item) => format!("{}: {reason}", item.id()),
            None => reason,
        }),
        Refusal::Error(error) => error,
    })?;
    let rest = elsewhere
        .iter()
        .map(|item| removal(item))
        .collect::<Vec<_>>();
    write(catalog, &rest, dry_run).map_err(|refusal| {
        let reason = match refusal {
            Refusal::Change { reason, .. } => reason,
            Refusal::Error(error) => error.to_string(),
        };
        Error::MovedHalfway {
            collection: target.collection.clone(),
            ids: elsewhere
                .iter()
                .map(|item| item.id().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            reason,
        }
    })?;
    Ok(moving.into_iter().map(|item| item.id().clone()).collect())
}

/// The change that replaces the item read at `origin` with `after`.
fn replacement(origin: &Origin, after: Object) -> Change {
    Change::Replace {
        collection: origin.collection.clone(),
        before: origin.read.clone(),
        after,
    }
}

/// Makes `changes`, each to the source it comes with, one source after
/// the other; gives the text of the items added. A refused change is named
/// by its place in `changes`.
fn write(
    catalog: &Catalog,
    changes: &[(usize, Change)],
    dry_run: bool,
) -> std::result::Result<String, Refusal> {
    let mut text = String::new();
    for (index, mounted) in catalog.sources().all().iter().enumerate() {
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
/// from 1; blank lines are skipped. A `see` must name a file of study
/// material.
fn read_input(catalog: &Catalog, input: &str) -> Result<Vec<(usize, Object)>> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            item_of(catalog, line)
                .map(|object| (index + 1, object))
                .map_err(|reason| Error::InvalidLine {
                    line: index + 1,
                    reason,
                })
        })
        .collect()
}

/// New item IDs from `seed`, each used by no item of `catalog`, not in
/// `taken`, and given only once.
fn fresh_ids(
    catalog: &Catalog,
    mut taken: HashSet<ItemId>,
    seed: u64,
) -> impl Iterator<Item = ItemId> {
    taken.extend(catalog.items().iter().map(|item| item.id().clone()));
    random::numbers(seed)
        .map(ItemId::from_number)
        .filter(move |id| taken.insert(id.clone()))
}

/// Fails if `catalog` has problems.
fn refuse_problems(catalog: &Catalog) -> Result<()> {
    match catalog.problems().len() {
        0 => Ok(()),
        count => Err(Error::ItemsInvalid { count }),
    }
}

/// Checks the `new` items of the `lines` of standard input, as they would
/// be read from a source, all at once, so that each kind program is asked once
/// about all of them (see [`check_new`]). The first problem is an error
/// about its line, unless it is a kind program failing as a whole, which is
/// no line's fault.
fn check_lines(new: &[Object], lines: &[(usize, Object)], catalog: &Catalog) -> Result<()> {
    let Some(problem) = check_new(new, catalog.programs()).into_iter().next() else {
        return Ok(());
    };
    let Some((_, at)) = problem.place() else {
        return Err(problem);
    };
    let line = lines.get(at - 1).map_or(0, |(line, _)| *line);
    let reason = if let Error::At { reason, .. } = &problem {
        reason.clone()
    } else {
        problem.to_string()
    };
    Err(Error::InvalidLine { line, reason })
}

/// Checks that `collection` can name a collection (see `docs/sources.md`,
/// "Collections"): parts joined by `/`, none empty or starting with `.`.
// TODO(platform): Windows names devices `CON`, `PRN`, `AUX`, `NUL`, `COM1`
// to `COM9` and `LPT1` to `LPT9`, in any case and with any extension, so a
// collection with such a part is accepted here and then fails to be
// created on Windows only. See docs/formats.md and docs/audit.md, L11.
fn check_collection(collection: &str) -> Result<()> {
    let invalid = |reason: &str| Error::CannotCreate {
        collection: collection.to_owned(),
        reason: reason.to_owned(),
    };
    if collection.starts_with('/') || collection.contains('\\') || collection.contains(':') {
        return Err(invalid(
            "give a collection as `srl collections` lists it, with / between its parts",
        ));
    }
    if collection
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
    {
        return Err(invalid(
            "it has an empty part, `.`, `..`, or a part starting with `.`, which no \
             collection's name may have",
        ));
    }
    Ok(())
}

/// One line of `srl add --stdin` or `srl edit --stdin`: an item object
/// whose `see`, if any, names a file of study material. Errors are plain
/// reasons.
fn item_of(catalog: &Catalog, line: &str) -> std::result::Result<Object, String> {
    let object = object::read(line)?;
    if let Some(refusal) = object
        .entries
        .see
        .as_deref()
        .and_then(|see| catalog.refuses_see(see))
    {
        return Err(refusal);
    }
    Ok(object)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;
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
            let file = store.file(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
        (tmp, store)
    }

    fn catalog(store: &Store) -> Catalog {
        Catalog::load(store, &Config::default(), &mut Index::open(store)).unwrap()
    }

    fn catalog_with(store: &Store, config: &Config) -> Catalog {
        Catalog::load(store, config, &mut Index::open(store)).unwrap()
    }

    fn read(store: &Store, path: &str) -> String {
        fs::read_to_string(store.file(path)).unwrap()
    }

    #[test]
    fn an_item_without_an_id_stops_every_write() {
        let text = "{\"id\": \"aaa1\", \"draft\": true, \"Q\": \"a\", \"A\": \"b\"}\n{\"Q\": \"c\", \"A\": \"d\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let catalog = catalog(&store);
        let problems = catalog
            .problems()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(problems, ["a.jsonl: item 2: it has no ID; give it one"]);
        let drafts = catalog.items().iter().collect::<Vec<_>>();
        assert!(matches!(
            accept(&catalog, &drafts, false),
            Err(Error::ItemsInvalid { count: 1 })
        ));
        assert_eq!(read(&store, "a.jsonl"), text);
    }

    #[test]
    fn dry_runs_write_nothing() {
        let text = "{\"id\": \"aaa1\", \"draft\": true, \"Q\": \"a\", \"A\": \"b\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let catalog = catalog(&store);
        let drafts = catalog.items().iter().collect::<Vec<_>>();
        assert_eq!(accept(&catalog, &drafts, true).unwrap().len(), 1);
        assert_eq!(read(&store, "a.jsonl"), text);
    }

    #[test]
    fn accept_removes_draft_only() {
        let text = "{\"id\": \"aaa1\", \"draft\": true, \"Q\": \"a\", \"A\": \"b\"}\n{\"id\": \"aaa2\", \"draft\": true, \"see\": \"a.md\", \"Q\": \"c\", \"A\": \"d\"}\n{\"id\": \"aaa3\", \"draft\": true, \"Q\": \"e\", \"A\": \"f\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", text), ("a.md", "# A\n")]);
        let catalog = catalog(&store);
        let chosen = catalog.items().iter().take(2).collect::<Vec<_>>();
        let written = accept(&catalog, &chosen, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            read(&store, "a.jsonl"),
            "{\"id\":\"aaa1\",\"Q\":\"a\",\"A\":\"b\"}\n{\"id\":\"aaa2\",\"see\":\"a.md\",\"Q\":\"c\",\"A\":\"d\"}\n{\"id\": \"aaa3\", \"draft\": true, \"Q\": \"e\", \"A\": \"f\"}\n"
        );
    }

    #[test]
    fn add_appends_items_to_a_collection_that_exists() {
        let (_tmp, store) = store(&[
            ("a.jsonl", "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}"),
            ("my notes/b c.md", "Prose.\n"),
        ]);
        let input = "{\"id\": \"mine\", \"Q\": \"Two\\nlines\", \"see\": \"my notes/b c.md\", \"A\": \"x\", \"kind\": \"qa\"}\n\n{\"draft\": true, \"Q\": \"{{c1::y}}\"}\n";
        let added = add(&catalog(&store), "a.jsonl", input, 3, false).unwrap();
        let [one, two] = added.ids.as_slice() else {
            panic!("expected two IDs");
        };
        assert_eq!(one.as_str(), "mine");
        let expected = format!(
            "{{\"id\":\"mine\",\"see\":\"my notes/b c.md\",\"Q\":\"Two\\nlines\",\"A\":\"x\"}}\n{{\"id\":\"{two}\",\"draft\":true,\"Q\":\"{{{{c1::y}}}}\"}}\n"
        );
        assert_eq!(added.text, expected);
        assert_eq!(
            read(&store, "a.jsonl"),
            format!("{{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}}\n{expected}")
        );
        let item = "{\"Q\": \"q\", \"A\": \"a\"}";
        let error = add(&catalog(&store), "new/b.jsonl", item, 4, false).unwrap_err();
        assert!(error.to_string().contains("srl create"), "{error}");
        assert!(!store.file("new").exists());
        let created = create(&catalog(&store), "new/b.jsonl", false).unwrap();
        assert_eq!(
            created,
            Created {
                collection: "new/b.jsonl".to_owned(),
                source: "items".to_owned(),
            }
        );
        assert_eq!(read(&store, "new/b.jsonl"), "");
        let added = add(&catalog(&store), "new/b.jsonl", item, 4, false).unwrap();
        assert_eq!(read(&store, "new/b.jsonl"), added.text);
        // The temporary files were in `.srl/tmp/`, and are gone.
        let names = |dir: &Path| {
            fs::read_dir(dir)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&store.root().join("new")), ["b.jsonl"]);
        assert_eq!(names(&store.tmp_dir()), [] as [&str; 0]);
        let catalog = catalog(&store);
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        let taken = "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}";
        assert!(add(&catalog, "a.jsonl", taken, 3, false).is_err());
    }

    #[test]
    fn add_checks_every_line_first() {
        let text = "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", text), ("a.md", "# A\n"), ("data.json", "{}")]);
        for input in [
            "{\"Q\": \"q\"}",
            "{\"q\": \"q\", \"A\": \"a\"}",
            "{\"Q\": 1, \"A\": \"a\"}",
            "[1]",
            "{}",
            "{\"see\": \"a.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": \"nowhere.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": \"a.jsonl\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": \"data.json\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": \"../a.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": 1}",
            "{\"Q\": \"q\", \"A\": \"a\", \"kind\": \"nope\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"kind\": \"\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"draft\": \"yes\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"ABC\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"abc\"}\n{\"Q\": \"r\", \"A\": \"a\", \"id\": \"abc\"}",
            "{\"Q\": \"q\", \"A\": \"a\"}\nnot json",
        ] {
            assert!(
                add(&catalog(&store), "a.jsonl", input, 3, false).is_err(),
                "accepted {input}"
            );
        }
        assert_eq!(read(&store, "a.jsonl"), text);
        assert_eq!(read(&store, "a.md"), "# A\n");
    }

    #[test]
    fn create_refuses_names_no_collection_may_have_and_collections_that_exist() {
        let (_tmp, store) = store(&[("a.jsonl", ""), ("a.md", "# A\n")]);
        for path in [
            "a.jsonl",
            "a.md",
            "a.txt",
            "/a.jsonl",
            "../a.jsonl",
            ".hidden/a.jsonl",
            "a//b.jsonl",
            "a\\b.jsonl",
        ] {
            assert!(
                create(&catalog(&store), path, false).is_err(),
                "accepted {path}"
            );
        }
        let error = create(&catalog(&store), "a.jsonl", false).unwrap_err();
        assert_eq!(
            error.to_string(),
            "cannot create a.jsonl: it exists already, in the source items; nothing was created"
        );
        assert_eq!(read(&store, "a.md"), "# A\n");
    }

    #[test]
    fn create_takes_any_path_include_matches_in_a_format_srl_reads() {
        let (_tmp, store) = store(&[]);
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["cards/**"]);
        assert!(create(&catalog_with(&store, &config), "a.jsonl", false).is_err());
        let error = create(&catalog_with(&store, &config), "cards/a.md", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("extension"), "{error}");
        create(&catalog_with(&store, &config), "cards/new/a.jsonl", true).unwrap();
        assert!(!store.file("cards").exists());
        for (path, empty) in [
            ("cards/new/a.jsonl", ""),
            ("cards/b.json", "[]\n"),
            ("cards/c.xml", "<items>\n</items>\n"),
        ] {
            create(&catalog_with(&store, &config), path, false).unwrap();
            assert_eq!(read(&store, path), empty);
        }
        let catalog = catalog_with(&store, &config);
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        let item = "{\"Q\": \"q\", \"A\": \"a\"}";
        for path in ["cards/new/a.jsonl", "cards/b.json", "cards/c.xml"] {
            add(&catalog_with(&store, &config), path, item, 3, false).unwrap();
            assert!(read(&store, path).contains('q'), "{path}");
        }
    }

    #[test]
    fn see_names_study_material_as_context_chooses_it() {
        let (_tmp, store) = store(&[
            (
                "a.jsonl",
                "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n",
            ),
            ("notes/a.pdf", "%PDF"),
            ("notes/b.md", "# B\n"),
        ]);
        let mut config = Config::default();
        config.context.include = vec!["notes/**".to_owned()];
        config.context.exclude = vec!["**/*.md".to_owned()];
        let catalog = catalog_with(&store, &config);
        let see = |path: &str| {
            format!("{{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\", \"see\": \"{path}\"}}")
        };
        assert!(edit(&catalog, &see("notes/b.md"), true).is_err());
        assert!(edit(&catalog, &see("notes/c.pdf"), true).is_err());
        edit(&catalog, &see("notes/a.pdf"), false).unwrap();
        let catalog = catalog_with(&store, &config);
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        // A file that is not UTF-8 is no context to show.
        fs::write(store.file("notes/a.pdf"), [0xff, 0xfe, 0x00]).unwrap();
        let item = &catalog.items()[0];
        assert_eq!(catalog.context(item).unwrap(), None);
        fs::write(store.file("notes/a.pdf"), "\u{feff}Text.\n").unwrap();
        assert_eq!(catalog.context(item).unwrap().as_deref(), Some("Text.\n"));
        // An item whose file is gone, or no longer included, is a problem.
        fs::remove_file(store.file("notes/a.pdf")).unwrap();
        assert_eq!(catalog_with(&store, &config).problems().len(), 1);
    }

    #[test]
    fn accept_writes_json_lines_again_and_keeps_every_other_byte() {
        let text = "\u{feff}{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\", \"draft\": true}\r\n\r\n{\"id\": \"keep\", \"Q\": \"r\", \"A\": \"b\"}";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let catalog = catalog(&store);
        let drafts = catalog.items().iter().collect::<Vec<_>>();
        accept(&catalog, &drafts, false).unwrap();
        assert_eq!(
            read(&store, "a.jsonl"),
            "\u{feff}{\"id\":\"aaa1\",\"Q\":\"q\",\"A\":\"a\"}\r\n\r\n{\"id\": \"keep\", \"Q\": \"r\", \"A\": \"b\"}"
        );
    }

    #[test]
    fn edit_replaces_items_and_keeps_the_rest() {
        let lines = "{\"id\": \"aaa1\", \"Q\": \"old\", \"A\": \"old\"}\n{\"id\": \"aaa2\", \"Q\": \"old\", \"A\": \"old\"}\n";
        let (_tmp, store) = store(&[
            ("a.jsonl", lines),
            (
                "b.jsonl",
                "{\"id\": \"bbb1\", \"Q\": \"old\", \"A\": \"old\"}\n",
            ),
            ("a.md", "# A\n"),
        ]);
        let input = "{\"id\": \"aaa2\", \"Q\": \"new\\n```\\ncode\\n```\", \"A\": \"a\", \"collection\": \"x\", \"cards\": []}\n{\"id\": \"bbb1\", \"Q\": \"new\", \"A\": \"a\", \"see\": \"a.md\"}\n";
        let ids = edit(&catalog(&store), input, false).unwrap();
        let ids = ids.iter().map(ItemId::as_str).collect::<Vec<_>>();
        assert_eq!(ids, ["aaa2", "bbb1"]);
        assert_eq!(
            read(&store, "a.jsonl"),
            "{\"id\": \"aaa1\", \"Q\": \"old\", \"A\": \"old\"}\n{\"id\":\"aaa2\",\"Q\":\"new\\n```\\ncode\\n```\",\"A\":\"a\"}\n"
        );
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\":\"bbb1\",\"see\":\"a.md\",\"Q\":\"new\",\"A\":\"a\"}\n"
        );
    }

    #[test]
    fn several_edits_in_one_file_each_find_their_item() {
        let lines = "{\"id\": \"bbb1\", \"Q\": \"one\", \"A\": \"a\"}\n{\"id\": \"bbb2\", \"Q\": \"two\", \"A\": \"a\"}\n{\"id\": \"bbb3\", \"Q\": \"three\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("b.jsonl", lines)]);
        let input = "{\"id\": \"bbb3\", \"Q\": \"3\", \"A\": \"a\"}\n{\"id\": \"bbb1\", \"Q\": \"1\", \"A\": \"a\"}\n";
        edit(&catalog(&store), input, false).unwrap();
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\":\"bbb1\",\"Q\":\"1\",\"A\":\"a\"}\n{\"id\": \"bbb2\", \"Q\": \"two\", \"A\": \"a\"}\n{\"id\":\"bbb3\",\"Q\":\"3\",\"A\":\"a\"}\n"
        );
    }

    #[test]
    fn writes_find_items_by_id_in_the_file_as_it_is_now() {
        let a = "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n";
        let b = "{\"id\": \"bbb1\", \"Q\": \"q\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", a), ("b.jsonl", b)]);
        let catalog = catalog(&store);
        let input = "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}\n{\"id\": \"bbb1\", \"Q\": \"new\", \"A\": \"a\"}\n";
        // An item that moved since the files were read is found where it
        // is now.
        fs::write(
            store.file("b.jsonl"),
            format!("{{\"id\": \"bbb0\", \"Q\": \"q\", \"A\": \"a\"}}\n{b}"),
        )
        .unwrap();
        edit(&catalog, input, true).unwrap();
        // An item that is gone, or a file that has an error now, stops the
        // command, and no file is written.
        for changed in ["\n", &format!("{b}not json\n")] {
            fs::write(store.file("b.jsonl"), changed).unwrap();
            assert!(matches!(
                edit(&catalog, input, false),
                Err(Error::ChangedMeanwhile { .. })
            ));
            assert_eq!(read(&store, "a.jsonl"), a);
        }
        assert_eq!(edit(&catalog, "\n\n", false).unwrap(), [] as [ItemId; 0]);
    }

    #[test]
    fn add_to_a_jsonl_file_starts_a_new_line_and_keeps_its_line_endings() {
        let (_tmp, store) = store(&[
            ("a.jsonl", "{\"id\": \"old1\", \"Q\": \"q\", \"A\": \"a\"}"),
            (
                "b.jsonl",
                "{\"id\": \"old2\", \"Q\": \"q\", \"A\": \"a\"}\r\n",
            ),
        ]);
        let item = "{\"id\": \"new1\", \"Q\": \"r\", \"A\": \"a\"}\n{\"id\": \"new2\", \"Q\": \"s\", \"A\": \"a\"}";
        let added = add(&catalog(&store), "a.jsonl", item, 3, false).unwrap();
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
        add(&catalog(&store), "b.jsonl", item, 3, false).unwrap();
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\": \"old2\", \"Q\": \"q\", \"A\": \"a\"}\r\n{\"id\":\"new3\",\"Q\":\"t\",\"A\":\"a\"}\r\n"
        );
        let item = "{\"id\": \"new4\", \"Q\": \"u\", \"A\": \"a\"}";
        create(&catalog(&store), "c/new.jsonl", false).unwrap();
        add(&catalog(&store), "c/new.jsonl", item, 3, false).unwrap();
        assert_eq!(
            read(&store, "c/new.jsonl"),
            "{\"id\":\"new4\",\"Q\":\"u\",\"A\":\"a\"}\n"
        );
        assert!(
            catalog(&store).problems().is_empty(),
            "{:?}",
            catalog(&store).problems()
        );
    }

    #[test]
    fn edit_checks_every_line_first() {
        let text = "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let catalog = catalog(&store);
        for input in [
            "{\"Q\": \"q\", \"A\": \"a\"}",
            "{\"id\": \"zzz9\", \"Q\": \"q\", \"A\": \"a\"}",
            "{\"id\": \"aaa1\", \"Q\": \"q\"}",
            "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa1\", \"Q\": \"r\", \"A\": \"a\"}",
            "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}\nnot json",
        ] {
            assert!(edit(&catalog, input, false).is_err(), "accepted {input:?}");
        }
        assert_eq!(read(&store, "a.jsonl"), text);
        edit(
            &catalog,
            "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}",
            true,
        )
        .unwrap();
        assert_eq!(read(&store, "a.jsonl"), text);
    }

    #[test]
    fn an_edit_that_changes_nothing_keeps_the_line() {
        let text = "{\"id\": \"aaa1\",  \"Q\": \"q\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let input = "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n";
        edit(&catalog(&store), input, false).unwrap();
        assert_eq!(read(&store, "a.jsonl"), text);
    }

    #[test]
    fn accept_rewrites_only_the_elements_of_an_xml_collection() {
        let text = "<items>\n  <!-- Kept as written. -->\n  <item id=\"aaa1\" draft=\"true\">\n    <Q>\n      Two\n      lines\n    </Q>\n    <A>a</A>\n  </item>\n  <item id=\"keep\"><Q>q</Q><A>b</A></item>\n</items>\n";
        let (_tmp, store) = store(&[("a.xml", text)]);
        let catalog = catalog(&store);
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        let drafts = catalog.items().iter().collect::<Vec<_>>();
        assert_eq!(accept(&catalog, &drafts, false).unwrap().len(), 1);
        let accepted = read(&store, "a.xml");
        assert!(accepted.contains("<item id=\"aaa1\">\n"), "{accepted}");
        assert!(accepted.contains("<!-- Kept as written. -->"), "{accepted}");
        assert!(
            accepted.contains("<item id=\"keep\"><Q>q</Q><A>b</A></item>"),
            "{accepted}"
        );
    }

    #[test]
    fn a_json_collection_is_written_again_only_when_it_changes() {
        let mut config = Config::default();
        config.sources = crate::config::sources_of(&["**/*.json"]);
        let text = "\u{feff}[{\"id\": \"aaa1\", \"kind\": \"qa\", \"Q\": \"q\", \"A\": \"a\"},\r\n {\"id\": \"aaa2\", \"Q\": \"r\", \"A\": \"b\"}]";
        let (_tmp, store) = store(&[("a.json", text)]);
        let unchanged = "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}";
        edit(&catalog_with(&store, &config), unchanged, false).unwrap();
        assert_eq!(read(&store, "a.json"), text);
        let changed = "{\"id\": \"aaa2\", \"Q\": \"r\", \"A\": \"c\"}";
        edit(&catalog_with(&store, &config), changed, false).unwrap();
        assert_eq!(
            read(&store, "a.json"),
            "\u{feff}[\r\n  {\r\n    \"id\": \"aaa1\",\r\n    \"kind\": \"qa\",\r\n    \"Q\": \"q\",\r\n    \"A\": \"a\"\r\n  },\r\n  {\r\n    \"id\": \"aaa2\",\r\n    \"Q\": \"r\",\r\n    \"A\": \"c\"\r\n  }\r\n]\r\n"
        );
        let added = add(
            &catalog_with(&store, &config),
            "a.json",
            "{\"id\": \"aaa3\", \"Q\": \"s\", \"A\": \"c\"}",
            3,
            false,
        )
        .unwrap();
        assert_eq!(
            added.text,
            "{\n  \"id\": \"aaa3\",\n  \"Q\": \"s\",\n  \"A\": \"c\"\n}\n"
        );
        let catalog = catalog_with(&store, &config);
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        assert_eq!(catalog.items().len(), 3);
    }

    #[test]
    fn see_never_names_a_file_in_a_hidden_folder_or_one_context_excludes() {
        let (_tmp, store) = store(&[
            (
                "a.jsonl",
                "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n",
            ),
            (".hidden/a.md", "# Hidden\n"),
            ("drafts/b.md", "# Draft\n"),
            ("blank.md", " \n\n"),
        ]);
        let mut config = Config::default();
        config.context.exclude = vec!["drafts/**".to_owned()];
        let catalog = catalog_with(&store, &config);
        let see = |path: &str| {
            format!("{{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\", \"see\": \"{path}\"}}")
        };
        for path in [".hidden/a.md", "drafts/b.md", ".srl/config.toml"] {
            assert!(edit(&catalog, &see(path), true).is_err(), "accepted {path}");
        }
        // A note of nothing but blank space is a note, with no text to give.
        edit(&catalog, &see("blank.md"), false).unwrap();
        let catalog = catalog_with(&store, &config);
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        assert_eq!(catalog.context(&catalog.items()[0]).unwrap(), None);
        assert_eq!(
            crate::notes(&store, &config).unwrap(),
            ["blank.md"],
            "notes skip hidden folders and what [context] excludes"
        );
    }
}
