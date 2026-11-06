"""Tests of the TinyDB source. Run in this folder with
`uv run --with tinydb python -m unittest`.
"""

import tempfile
import unittest
from pathlib import Path

from tinydb_source import Source, SourceError, collections, describe, read, write

ITEM = {"id": "cat001", "Q": "猫", "A": "cat"}


class Tests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.source = Source({"database": str(Path(self.folder.name) / "vocab" / "vocab.json")})

    def tearDown(self):
        self.source.close()
        self.folder.cleanup()

    def change(self, op, dry_run=False, **rest):
        return {"op": op, "dry_run": dry_run, "collection": "words", **rest}

    def test_an_added_item_is_read_with_its_table_and_document_id(self):
        self.assertEqual(write(self.source, [self.change("add", item=ITEM)]), [{"ok": True}])
        self.assertEqual(collections(self.source, [{}]), [{"collection": "words"}])
        self.assertEqual(
            read(self.source, [{}]),
            [{"collection": "words", "handle": "1", "written": None, "item": ITEM}],
        )
        self.assertEqual(describe(self.source, [{}])[0]["creates"], ["*"])

    def test_a_dry_run_previews_and_writes_nothing(self):
        answers = write(self.source, [self.change("add", dry_run=True, item=ITEM)])
        self.assertEqual(answers, [{"preview": '{"id": "cat001", "Q": "猫", "A": "cat"}'}])
        self.assertEqual(read(self.source, [{}]), [])

    def test_a_stale_replace_refuses_every_change(self):
        write(self.source, [self.change("add", item=ITEM)])
        after = {**ITEM, "A": "a cat"}
        stale = [self.change("replace", handle="1", before={**ITEM, "A": "dog"}, after=after),
                 self.change("add", item={"Q": "犬"})]
        self.assertTrue(all("error" in answer for answer in write(self.source, stale)))
        self.assertEqual(len(read(self.source, [{}])), 1)
        current = [self.change("replace", handle="1", before=ITEM, after=after)]
        self.assertEqual(write(self.source, current), [{"ok": True}])
        self.assertEqual(read(self.source, [{}])[0]["item"], after)

    def test_settings_are_checked(self):
        database = str(Path(self.folder.name) / "other.json")
        for config in [{}, {"database": 1}, {"database": database, "folder": "db"}]:
            with self.assertRaises(SourceError):
                Source(config)


if __name__ == "__main__":
    unittest.main()
