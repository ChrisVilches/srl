# source-tinydb

`tinydb_source.py`, a source program that keeps items in a
[TinyDB](https://tinydb.readthedocs.io/) database, a small document
database in one JSON file. A proof of concept of source programs (see
[docs/sources.md](../../docs/sources.md), "Source programs"): items that
live in a database instead of in files of the store, studied, shown and
changed with the same commands as any other item.

The built-in `filesystem` source reads JSON collections too (see
[docs/formats.md](../../docs/formats.md)), so this program keeps items
nowhere `srl` couldn't: it is here to show how a source program works,
and is a starting point for one that talks to a real database or a web
service.

Meant for Linux, macOS and Windows, but tried on Linux only. It needs
[uv](https://docs.astral.sh/uv/), which installs TinyDB for it from the
script's own header.

## Using it

Copy `tinydb_source.py` into your store, for example into `sources/`, and
name it in `.srl/config.toml`, beside the store's own collections:

```toml
[sources.items]
driver = "filesystem"

[sources.vocab]
program = ["uv", "run", "--quiet", "--script", "sources/tinydb_source.py"]
database = "vocab/vocab.json"
folder = "vocab"
```

Its settings, every key of the source but `program`, which `srl` passes
on as they are and the program checks, refusing any other:

- `database`: the database's file, relative to the store's folder,
  created on the first item added.
- `folder`: the folder its tables are collections under, by default
  `db`. The table `words` is the collection `vocab/words`, an address no
  file of the store has (see [docs/sources.md](../../docs/sources.md),
  "Addresses"). The `filesystem` source doesn't read the database file,
  a `.json` file, unless its `include` says so.

Then add items to a table, which is created if it doesn't exist:

```sh
srl add vocab/words --stdin <<'JSONL'
{"see": "japanese/animals.md", "Q": "What does 猫 mean?", "A": "Cat."}
JSONL
srl find source:vocab
srl sources
```

Items of the database may name a note with `see`, as any item may.

## How it keeps items

- **Each table is a collection**, named under `folder`, and `srl add`
  may create any new one there (`describe` gives `folder/*`).
- **Each item is a document `{"item": {...}}`**, holding its item object
  as `srl` writes one. Its handle is TinyDB's document ID. Other
  documents of a table are left alone.
- **A change that can't be made refuses every change of the command**:
  a `replace` whose item is no longer as `srl` read it fails the whole
  call, and nothing is written. `srl` then reads the database again to
  check that every change was made.
- **No cache.** TinyDB reads one local file, so a cache would save
  nothing.

## Calling it by hand

Each call reads JSON lines on standard input, each with `config`, the
source's settings:

```sh
echo '{"config": {"database": "vocab/vocab.json", "folder": "vocab"}}' | uv run --script tinydb_source.py read
```

The tests run with `uv run --with tinydb python -m unittest` in this
folder.
