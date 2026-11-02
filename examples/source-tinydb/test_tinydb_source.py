"""Tests of the TinyDB source. Run in this folder with
`uv run --with tinydb python -m unittest`.
"""

import tempfile
import unittest
from pathlib import Path

from tinydb_source import collections, context, database, read, write

ITEM = {"id": "cat001", "Q": "猫", "A": "cat"}


class Source(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.db = database({"database": str(Path(self.folder.name) / "vocab" / "vocab.json")})

    def tearDown(self):
        self.db.close()
        self.folder.cleanup()

    def change(self, op, dry_run=False, **rest):
        return {"op": op, "dry_run": dry_run, "collection": "words", **rest}

    def test_an_added_item_is_read_with_its_table_and_document_id(self):
        self.assertEqual(write(self.db, [self.change("add", item=ITEM)]), [{"ok": True}])
        self.assertEqual(collections(self.db, [{}]), [{"collection": "words", "file": None}])
        self.assertEqual(
            read(self.db, [{}]),
            [{"collection": "words", "handle": "1", "line": None, "written": None, "item": ITEM}],
        )

    def test_a_dry_run_previews_and_writes_nothing(self):
        answers = write(self.db, [self.change("add", dry_run=True, item=ITEM)])
        self.assertEqual(answers, [{"preview": '{"id": "cat001", "Q": "猫", "A": "cat"}'}])
        self.assertEqual(read(self.db, [{}]), [])

    def test_a_stale_replace_refuses_every_change(self):
        write(self.db, [self.change("add", item=ITEM)])
        after = {**ITEM, "A": "a cat"}
        stale = [self.change("replace", handle="1", before={**ITEM, "A": "dog"}, after=after),
                 self.change("add", item={"Q": "犬"})]
        self.assertTrue(all("error" in answer for answer in write(self.db, stale)))
        self.assertEqual(len(read(self.db, [{}])), 1)
        current = [self.change("replace", handle="1", before=ITEM, after=after)]
        self.assertEqual(write(self.db, current), [{"ok": True}])
        self.assertEqual(read(self.db, [{}])[0]["item"], after)

    def test_a_table_s_context_is_its_context_document(self):
        self.db.table("words").insert({"context": "Animals."})
        request = {"collection": "words", "mode": "prose", "own": True}
        self.assertEqual(context(self.db, [request, {**request, "collection": "none"}]),
                         [{"context": "Animals."}, {"context": None}])


if __name__ == "__main__":
    unittest.main()
