"""Tests of the ja-words kind. Run with `python3 -m unittest` in this folder.

The tests of the answer need MeCab and are skipped without it.
"""

import json
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

from ja_words import KindError, card, ids

SENTENCE = "東京{都}は、日本の関東{地方}に{位置}する{都}"


def item(text, suffix=None, **fields):
    made = {"id": "v81k3z", "fields": {"TEXT": text, **fields}}
    if suffix is not None:
        made.update(suffix=suffix, context=None, day="2026-10-27")
    return made


class Ids(unittest.TestCase):
    def test_each_word_gives_a_card_in_a_group_of_its_own(self):
        self.assertEqual(
            ids(item("{東京}の{地方}")),
            {"groups": [{"cards": ["e69db1e4baac"]}, {"cards": ["e59cb0e696b9"]}]},
        )

    def test_a_repeated_word_gives_one_card(self):
        groups = ids(item(SENTENCE))["groups"]
        self.assertEqual(
            groups,
            [{"cards": ["e983bd"]}, {"cards": ["e59cb0e696b9"]}, {"cards": ["e4bd8de7bdae"]}],
        )

    def test_the_suffix_is_the_hex_of_the_utf8_of_the_word(self):
        self.assertEqual(ids(item("{都}")), {"groups": [{"cards": ["e983bd"]}]})
        self.assertEqual(bytes.fromhex("e983bd").decode("utf-8"), "都")

    def test_a_suffix_does_not_depend_on_where_the_word_is(self):
        self.assertEqual(
            ids(item("{都}"))["groups"][0],
            ids(item("{地方}と{都}"))["groups"][1],
        )

    def test_invalid_items_are_errors(self):
        for text in ["no words", "{}", "{ }", "{都", "都}", "{{都}}", "{都{}}"]:
            with self.subTest(text=text), self.assertRaises(KindError):
                ids(item(text))
        with self.assertRaises(KindError):
            ids({"id": "v81k3z", "fields": {"Q": "{都}"}})
        with self.assertRaises(KindError):
            ids(item("{都}", A="capital"))


class Card(unittest.TestCase):
    def analyze(self, text):
        return "EOS\n"

    def test_every_braced_occurrence_of_the_word_is_highlighted(self):
        made = card(item(SENTENCE, "e983bd"), self.analyze)
        self.assertEqual(made["prompt"], "東京==都==は、日本の関東地方に位置する==都==")
        self.assertNotIn("prompt_revealed", made)

    def test_only_the_cards_word_is_highlighted(self):
        made = card(item(SENTENCE, "e4bd8de7bdae"), self.analyze)
        self.assertEqual(made["prompt"], "東京都は、日本の関東地方に==位置==する都")

    def test_an_unbraced_occurrence_is_not_highlighted(self):
        made = card(item("京都は{都}", "e983bd"), self.analyze)
        self.assertEqual(made["prompt"], "京都は==都==")

    def test_the_answer_is_the_analysis_of_the_whole_sentence(self):
        seen = []
        card(item(SENTENCE, "e983bd"), lambda text: seen.append(text) or "EOS\n")
        card(item(SENTENCE, "e59cb0e696b9"), lambda text: seen.append(text) or "EOS\n")
        self.assertEqual(seen, ["東京都は、日本の関東地方に位置する都"] * 2)

    def test_a_token_gives_its_reading_part_of_speech_and_base_form(self):
        analysis = (
            "位置\t名詞,サ変接続,*,*,*,*,位置,イチ,イチ\n"
            "し\t動詞,自立,*,*,サ変・スル,連用形,する,シ,シ\n"
            "、\t記号,読点,*,*,*,*,、,、,、\n"
            "ほげ\t名詞,一般,*,*,*,*,*\n"
            "EOS\n"
        )
        made = card(item("{位置}", "e4bd8de7bdae"), lambda text: analysis)
        self.assertEqual(
            made["answer"],
            "- 位置 — いち (名詞, サ変接続)\n"
            "- し (動詞, 自立), from する\n"
            "- 、 (記号, 読点)\n"
            "- ほげ (名詞, 一般)",
        )

    def test_an_unknown_suffix_is_an_error(self):
        with self.assertRaises(KindError):
            card(item(SENTENCE, "e69db1"), self.analyze)


@unittest.skipIf(shutil.which("mecab") is None, "mecab is not installed")
class WithMecab(unittest.TestCase):
    def run_call(self, call, given):
        script = Path(__file__).with_name("ja_words.py")
        return subprocess.run(
            [sys.executable, str(script), call],
            input=json.dumps(given),
            capture_output=True,
            text=True,
            encoding="utf-8",
        )

    def test_card_analyzes_the_sentence_with_mecab(self):
        run = self.run_call("card", item(SENTENCE, "e983bd"))
        self.assertEqual(run.returncode, 0, run.stderr)
        answer = json.loads(run.stdout)["answer"].splitlines()
        self.assertEqual(answer[0], "- 東京 — とうきょう (名詞, 固有名詞)")
        self.assertIn("- 位置 — いち (名詞, サ変接続)", answer)
        self.assertEqual(len(answer), 12)

    def test_ids_prints_json_and_errors_go_to_standard_error(self):
        run = self.run_call("ids", item("{都}"))
        self.assertEqual((run.returncode, json.loads(run.stdout)), (0, {"groups": [{"cards": ["e983bd"]}]}))
        run = self.run_call("ids", item("{都"))
        self.assertEqual((run.returncode, run.stdout), (1, ""))
        self.assertIn("without its", run.stderr)


if __name__ == "__main__":
    unittest.main()
