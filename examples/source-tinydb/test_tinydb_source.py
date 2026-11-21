"""Tests of the TinyDB source. Run in this folder with
`uv run --with tinydb python -m unittest`.
"""

import tempfile
import unittest
from pathlib import Path

from tinydb_source import Source, SourceError, accepts, collections, describe, read, write

ITEM = {"id": "cat001", "Q": "猫", "A": "cat"}


class Tests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.source = Source({"database": str(Path(self.folder.name) / "vocab" / "vocab.json")})

    def create(self, *names):
        write(self.source, [{**self.change("create"), "collection": name} for name in names])

    def tearDown(self):
        self.source.close()
        self.folder.cleanup()

    def change(self, op, dry_run=False, **rest):
        return {"op": op, "dry_run": dry_run, "collection": "words", **rest}

    def test_a_created_table_is_listed_without_items_and_only_once(self):
        self.assertEqual(write(self.source, [self.change("create", dry_run=True)]), [{"ok": True}])
        self.assertEqual(collections(self.source, [{}]), [])
        self.assertEqual(write(self.source, [self.change("create")]), [{"ok": True}])
        self.assertEqual(collections(self.source, [{}]), [{"collection": "words"}])
        self.assertEqual(read(self.source, [{}]), [])
        self.assertEqual(
            write(self.source, [self.change("create")]),
            [{"error": "the table words exists already"}],
        )

    def test_an_item_is_added_only_to_a_table_that_exists(self):
        refused = write(self.source, [self.change("add", item=ITEM)])
        self.assertEqual(refused, [{"error": "no table is named words; srl create makes one"}])
        self.assertEqual(collections(self.source, [{}]), [])

    def test_an_added_item_is_read_with_its_table(self):
        self.create("words")
        self.assertEqual(write(self.source, [self.change("add", item=ITEM)]), [{"ok": True}])
        self.assertEqual(collections(self.source, [{}]), [{"collection": "words"}])
        self.assertEqual(
            read(self.source, [{}]),
            [{"collection": "words", "item": ITEM}],
        )

    def test_describe_gives_the_database_and_accepts_a_name_without_dot_or_slash(self):
        database = str(Path(self.folder.name) / "vocab" / "vocab.json")
        self.assertEqual(describe(self.source, [{}])[0]["settings"], {"database": database})
        refused = {"error": "a table's name has no `.` or `/`"}
        self.assertEqual(
            accepts(self.source, [{"collection": "words"}, {"collection": "a/b"}, {"collection": "words.jsonl"}]),
            [{"ok": True}, refused, refused],
        )

    def test_a_change_finds_its_item_in_its_own_table(self):
        animals = {"id": "dog001", "Q": "犬"}
        self.create("words", "animals")
        write(self.source, [self.change("add", item=ITEM), {**self.change("add", item=animals), "collection": "animals"}])
        after = {**animals, "A": "dog"}
        replace = {**self.change("replace", before=animals, after=after), "collection": "animals"}
        self.assertEqual(write(self.source, [replace]), [{"ok": True}])
        self.assertEqual([line["item"] for line in read(self.source, [{}])], [after, ITEM])

    def test_a_dry_run_previews_and_writes_nothing(self):
        self.create("words")
        answers = write(self.source, [self.change("add", dry_run=True, item=ITEM)])
        self.assertEqual(answers, [{"preview": '{"id": "cat001", "Q": "猫", "A": "cat"}'}])
        self.assertEqual(read(self.source, [{}]), [])

    def test_a_stale_replace_refuses_every_change(self):
        self.create("words")
        write(self.source, [self.change("add", item=ITEM)])
        after = {**ITEM, "A": "a cat"}
        stale = [self.change("replace", before={**ITEM, "A": "dog"}, after=after),
                 self.change("add", item={"id": "dog001", "Q": "犬"})]
        self.assertTrue(all("error" in answer for answer in write(self.source, stale)))
        self.assertEqual(len(read(self.source, [{}])), 1)
        current = [self.change("replace", before=ITEM, after=after)]
        self.assertEqual(write(self.source, current), [{"ok": True}])
        self.assertEqual(read(self.source, [{}])[0]["item"], after)

    def test_remove_takes_the_item_out_only_if_it_is_as_read(self):
        self.create("words")
        write(self.source, [self.change("add", item=ITEM)])
        stale = [self.change("remove", before={**ITEM, "A": "dog"})]
        self.assertTrue(all("error" in answer for answer in write(self.source, stale)))
        self.assertEqual(len(read(self.source, [{}])), 1)
        current = [self.change("remove", before=ITEM)]
        self.assertEqual(write(self.source, current), [{"ok": True}])
        self.assertEqual(read(self.source, [{}]), [])

    def test_settings_are_checked(self):
        database = str(Path(self.folder.name) / "other.json")
        for config in [{}, {"database": 1}, {"database": database, "folder": "db"}]:
            with self.assertRaises(SourceError):
                Source(config)


if __name__ == "__main__":
    unittest.main()
