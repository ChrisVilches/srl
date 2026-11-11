#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["tinydb>=4.8"]
# ///
"""A source program that keeps items in a TinyDB database.

A proof of concept of source programs (see docs/sources.md, "Source
programs"): items kept in a small document database, one JSON file,
instead of in files of the store. Name it in `[sources]` of the store's
.srl/config.toml (see README.md), or call it by hand:

    echo '{"config": {"database": "vocab.json"}}' | uv run --script tinydb_source.py read

Needs uv, which installs TinyDB for it. Meant for Linux, macOS and
Windows, but tried on Linux only.

- Each table of the database is a collection, named by the table: the
  table `words` is the collection `words`. `srl add` may create any new
  one whose name has no `/`.
- Each item is a document `{"item": {...}}` holding its item object, and
  its handle is its table and TinyDB's document ID, `words/17`: TinyDB
  numbers the documents of each table from 1, so the ID alone would name
  items of two tables alike.

Every call reads one JSON object per line up to the end of standard
input, and prints its answer, one JSON object per line.
"""

import json
import sys
from pathlib import Path

from tinydb import TinyDB

# The settings this source takes, in `[sources]` of .srl/config.toml.
SETTINGS = {"database"}


class SourceError(Exception):
    """A call that fails as a whole: the reason goes to standard error."""


class Source:
    """The database the source's settings name, relative to the store."""

    def __init__(self, config):
        unknown = sorted(set(config) - SETTINGS)
        if unknown:
            raise SourceError(f"unknown setting `{unknown[0]}`; the only setting is `database`")
        name = config.get("database")
        if not isinstance(name, str) or not name:
            raise SourceError("the setting `database` is missing; give the database's file, such as vocab/vocab.json")
        Path(name).parent.mkdir(parents=True, exist_ok=True)
        self.name = name
        self.db = TinyDB(name, ensure_ascii=False)

    def close(self):
        self.db.close()


def describe(source, inputs):
    return [{"description": "Items kept in a TinyDB database, one table per collection.",
             "settings": {"database": source.name}} for _ in inputs]


def accepts(_source, inputs):
    """`srl add` may create any table whose name has no `/`."""
    return [{"error": "a table's name has no `/`"} if "/" in change["collection"] else {"ok": True}
            for change in inputs]


def handle(table, doc_id):
    return f"{table}/{doc_id}"


def doc_id(handle):
    """The document ID of HANDLE, as `read` gave it: a table's name may
    hold `/`, its document ID never does."""
    return int(handle.rsplit("/", 1)[1])


def collections(source, _inputs):
    return [{"collection": name} for name in sorted(source.db.tables())]


def read(source, _inputs):
    lines = []
    for name in sorted(source.db.tables()):
        for document in source.db.table(name).all():
            if "item" not in document:
                continue
            lines.append({"collection": name, "handle": handle(name, document.doc_id),
                          "item": document["item"]})
    return lines


def write(source, inputs):
    """Every change, or none: a change that can't be made refuses them all."""
    answers = []
    for change in inputs:
        table = source.db.table(change["collection"])
        if change["op"] == "replace":
            document = table.get(doc_id=doc_id(change["handle"]))
            if document is None or document.get("item") != change["before"]:
                return [{"error": "the item changed in the database since srl read it; run the command again"} for _ in inputs]
        answers.append({"preview": json.dumps(change["item"], ensure_ascii=False)} if change["op"] == "add" and change["dry_run"] else {"ok": True})
    if not inputs or inputs[0]["dry_run"]:
        return answers
    for change in inputs:
        table = source.db.table(change["collection"])
        if change["op"] == "add":
            table.insert({"item": change["item"]})
        else:
            table.update({"item": change["after"]}, doc_ids=[doc_id(change["handle"])])
    return answers


CALLS = {"describe": describe, "accepts": accepts, "collections": collections, "read": read, "write": write}


def main():
    call = sys.argv[-1] if len(sys.argv) > 1 else ""
    if call not in CALLS:
        print(f"usage: tinydb_source.py {'|'.join(CALLS)}", file=sys.stderr)
        return 2
    inputs = [json.loads(line) for line in sys.stdin if line.strip()]
    try:
        source = Source(inputs[0]["config"] if inputs else {})
        for answer in CALLS[call](source, inputs):
            print(json.dumps(answer, ensure_ascii=False))
    except SourceError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
