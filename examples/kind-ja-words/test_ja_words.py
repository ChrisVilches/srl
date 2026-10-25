"""Tests of the ja-words kind. Run with `python3 -m unittest` in this folder.

The tests of the answer need MeCab and are skipped without it.
"""

import json
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

from ja_words import KindError, card, describe, groups, make, validate

SENTENCE = "東京{都}は、日本の関東{地方}に{位置}する{都}"


def item(text, suffix=None, **fields):
    made = {"id": "v81k3z", "fields": {"TEXT": text, **fields}}
    if suffix is not None:
        made.update(suffix=suffix, context=None, day="2026-10-27")
    return made


class Describe(unittest.TestCase):
    def test_it_names_the_field_that_validate_checks(self):
        self.assertEqual([field["name"] for field in describe({})["fields"]], ["TEXT"])


class Validate(unittest.TestCase):
    def test_a_sentence_with_words_is_valid(self):
        self.assertEqual(validate(item(SENTENCE)), {"errors": []})

    def test_invalid_text_is_an_error_about_text(self):
        for text in ["no words", "{}", "{ }", "{都", "都}", "{{都}}", "{都{}}"]:
            with self.subTest(text=text):
                errors = validate(item(text))["errors"]
                self.assertEqual([error["field"] for error in errors], ["TEXT"])

    def test_every_other_field_is_an_error_about_it(self):
        errors = validate({"id": "v81k3z", "fields": {"Q": "{都}", "A": "x"}})["errors"]
        self.assertEqual([error["field"] for error in errors], ["A", "Q", "TEXT"])
        errors = validate(item("{都}", A="capital"))["errors"]
        self.assertEqual([error["field"] for error in errors], ["A"])


class Groups(unittest.TestCase):
    def test_each_word_gives_a_card_in_a_group_of_its_own(self):
        self.assertEqual(
            groups(item("{東京}の{地方}")),
            {"groups": [["e69db1e4baac"], ["e59cb0e696b9"]]},
        )

    def test_a_repeated_word_gives_one_card(self):
        self.assertEqual(
            groups(item(SENTENCE))["groups"],
            [["e983bd"], ["e59cb0e696b9"], ["e4bd8de7bdae"]],
        )

    def test_the_suffix_is_the_hex_of_the_utf8_of_the_word(self):
        self.assertEqual(groups(item("{都}")), {"groups": [["e983bd"]]})
        self.assertEqual(bytes.fromhex("e983bd").decode("utf-8"), "都")

    def test_a_suffix_does_not_depend_on_where_the_word_is(self):
        self.assertEqual(
            groups(item("{都}"))["groups"][0],
            groups(item("{地方}と{都}"))["groups"][1],
        )

    def test_spaces_around_a_word_are_not_part_of_it(self):
        for text in ["{ 都}", "{都 }", "{　都　}"]:
            with self.subTest(text=text):
                self.assertEqual(groups(item(text)), {"groups": [["e983bd"]]})

    def test_a_word_with_combining_marks_is_the_same_card(self):
        self.assertEqual(groups(item("{が}{が}")), {"groups": [["e3818c"]]})


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

    def test_spaces_around_a_word_stay_outside_the_highlight(self):
        made = card(item("東京{ 都 }は", "e983bd"), self.analyze)
        self.assertEqual(made["prompt"], "東京 ==都== は")

    def test_highlights_that_touch_are_separated_by_a_space(self):
        made = card(item("{都}{都}は", "e983bd"), self.analyze)
        self.assertEqual(made["prompt"], "==都== ==都==は")

    def test_a_highlight_next_to_another_word_gets_no_space(self):
        made = card(item("{東京}{都}", "e983bd"), self.analyze)
        self.assertEqual(made["prompt"], "東京==都==")

    def test_both_forms_of_a_word_with_combining_marks_are_highlighted(self):
        made = card(item("{が}と{が}", "e3818c"), self.analyze)
        self.assertEqual(made["prompt"], "==が==と==が==")

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
        self.assertIn("error", make(item(SENTENCE, "e69db1"), self.analyze))


class Calls(unittest.TestCase):
    """Runs the program with JSONL on standard input, as srl does."""

    def run_call(self, call, given):
        script = Path(__file__).with_name("ja_words.py")
        return subprocess.run(
            [sys.executable, str(script), call],
            input="".join(json.dumps(line) + "\n" for line in given),
            capture_output=True,
            text=True,
            encoding="utf-8",
        )

    def test_each_input_line_gets_one_line_in_order(self):
        run = self.run_call("validate", [item("{都}"), item("{都"), item("{地方}")])
        self.assertEqual(run.returncode, 0, run.stderr)
        answers = [json.loads(line) for line in run.stdout.splitlines()]
        self.assertEqual([len(answer["errors"]) for answer in answers], [0, 1, 0])
        run = self.run_call("groups", [item("{都}"), item("{地方}")])
        self.assertEqual(
            [json.loads(line) for line in run.stdout.splitlines()],
            [{"groups": [["e983bd"]]}, {"groups": [["e59cb0e696b9"]]}],
        )

    def test_describe_answers_its_one_line(self):
        run = self.run_call("describe", [{}])
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(json.loads(run.stdout)["fields"][0]["name"], "TEXT")

    @unittest.skipIf(shutil.which("mecab") is None, "mecab is not installed")
    def test_make_analyzes_the_sentence_with_mecab(self):
        run = self.run_call("make", [item(SENTENCE, "e983bd"), item(SENTENCE, "e69db1")])
        self.assertEqual(run.returncode, 0, run.stderr)
        made, failed = [json.loads(line) for line in run.stdout.splitlines()]
        answer = made["answer"].splitlines()
        self.assertEqual(answer[0], "- 東京 — とうきょう (名詞, 固有名詞)")
        self.assertIn("- 位置 — いち (名詞, サ変接続)", answer)
        self.assertEqual(len(answer), 12)
        self.assertIn("e69db1", failed["error"])


if __name__ == "__main__":
    unittest.main()
