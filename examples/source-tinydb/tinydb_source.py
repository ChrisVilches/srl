#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["tinydb>=4.8"]
# ///
"""A source program that keeps items in a TinyDB database.

A proof of concept of source programs (see docs/sources.md, "Source
programs"): items kept in a small document database, one JSON file,
instead of in notes. Name it in `[sources]` of the store's
.srl/config.toml (see README.md), or call it by hand:

    echo '{"config": {"database": "vocab.json"}}' | uv run --script tinydb_source.py read

Needs uv, which installs TinyDB for it. Meant for Linux, macOS and
Windows, but tried on Linux only.

- Each table of the database is a collection, and `srl add` may create
  any new one.
- Each item is a document `{"item": {...}}` holding its item object, and
  its handle is TinyDB's document ID.
- A table's context is the text of its one document `{"context": "..."}`,
  if it has one, shown in modes `prose` and `full` alike: it is prose,
  never items.

Every call reads one JSON object per line up to the end of standard
input, and prints its answer, one JSON object per line.
"""

import json
import sys
from pathlib import Path

from tinydb import TinyDB, where


class SourceError(Exception):
    """A call that fails as a whole: the reason goes to standard error."""


def database(config):
    """The database the source's settings name, relative to the store."""
    name = config.get("database")
    if not isinstance(name, str) or not name:
        raise SourceError("the setting `database` is missing; give the database's file, such as vocab/vocab.json")
    Path(name).parent.mkdir(parents=True, exist_ok=True)
    return TinyDB(name, ensure_ascii=False)


def describe(_db, inputs):
    return [{"description": "Items kept in a TinyDB database, one table per collection.", "creates": ["*"]} for _ in inputs]


def collections(db, _inputs):
    return [{"collection": name, "file": None} for name in sorted(db.tables())]


def read(db, _inputs):
    lines = []
    for name in sorted(db.tables()):
        for document in db.table(name).all():
            if "item" not in document:
                continue
            item = {"collection": name, "handle": str(document.doc_id), "line": None, "written": None, "item": document["item"]}
            lines.append(item)
    return lines


def context(db, inputs):
    answers = []
    for request in inputs:
        found = db.table(request["collection"]).search(where("context").exists())
        answers.append({"context": found[0]["context"] if found else None})
    return answers


def write(db, inputs):
    """Every change, or none: a change that can't be made refuses them all."""
    answers = []
    for change in inputs:
        table = db.table(change["collection"])
        if change["op"] == "replace":
            document = table.get(doc_id=int(change["handle"]))
            if document is None or document.get("item") != change["before"]:
                return [{"error": "the item changed in the database since srl read it; run the command again"} for _ in inputs]
        answers.append({"preview": json.dumps(change["item"], ensure_ascii=False)} if change["op"] == "add" and change["dry_run"] else {"ok": True})
    if not inputs or inputs[0]["dry_run"]:
        return answers
    for change in inputs:
        table = db.table(change["collection"])
        if change["op"] == "add":
            table.insert({"item": change["item"]})
        else:
            table.update({"item": change["after"]}, doc_ids=[int(change["handle"])])
    return answers


CALLS = {"describe": describe, "collections": collections, "read": read, "context": context, "write": write}


def main():
    call = sys.argv[-1] if len(sys.argv) > 1 else ""
    if call not in CALLS:
        print(f"usage: tinydb_source.py {'|'.join(CALLS)}", file=sys.stderr)
        return 2
    inputs = [json.loads(line) for line in sys.stdin if line.strip()]
    try:
        db = database(inputs[0]["config"] if inputs else {})
        for answer in CALLS[call](db, inputs):
            print(json.dumps(answer, ensure_ascii=False))
    except SourceError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
