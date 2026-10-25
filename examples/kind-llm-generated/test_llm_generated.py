"""Tests of the llm-generated kind. Run with `python3 -m unittest` in this
folder. They never call the API: a made-up response stands in for it.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from llm_generated import KindError, make_card, groups, make, read_response, validate

TOPIC = "Ohm's law in circuit analysis: when it's used and its edge cases"


def item(topic=TOPIC, suffix=False, **fields):
    made = {"id": "q4n8w2", "fields": {"PROMPT": topic, **fields}}
    if suffix:
        made.update(suffix=None, context=None, day="2026-10-27")
    return made


def response(text, status="completed"):
    return {
        "status": status,
        "error": None,
        "incomplete_details": None,
        "output": [
            {"type": "reasoning", "summary": []},
            {"type": "message", "content": [{"type": "output_text", "text": text}]},
        ],
    }


class Validate(unittest.TestCase):
    def test_a_prompt_alone_is_valid(self):
        self.assertEqual(validate(item()), {"errors": []})

    def test_invalid_items_are_errors_about_their_fields(self):
        for fields, about in [
            ({}, ["PROMPT"]),
            ({"Q": TOPIC}, ["Q", "PROMPT"]),
            ({"PROMPT": TOPIC, "A": "x"}, ["A"]),
            ({"PROMPT": "  "}, ["PROMPT"]),
        ]:
            with self.subTest(fields=fields):
                errors = validate({"id": "q4n8w2", "fields": fields})["errors"]
                self.assertEqual([error["field"] for error in errors], about)


class Groups(unittest.TestCase):
    def test_one_card_with_the_item_id(self):
        self.assertEqual(groups(item()), {"groups": [[None]]})


class MakeCard(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.log_file = Path(folder.name) / "llm_generated.log"

    def logged(self):
        if not self.log_file.exists():
            return []
        return [json.loads(line) for line in self.log_file.read_text("utf-8").splitlines()]

    def test_the_card_is_what_the_model_wrote_for_the_prompt(self):
        asked = []

        def ask(text):
            asked.append(text)
            return response('{"prompt": "What is V when I = 2 A and R = 3 Ω?", "answer": "6 V"}')

        self.assertEqual(
            make_card(item(suffix=True), ask, self.log_file),
            {"prompt": "What is V when I = 2 A and R = 3 Ω?", "answer": "6 V"},
        )
        self.assertEqual(asked, [TOPIC])

    def test_the_note_is_sent_after_the_prompt(self):
        asked = []

        def ask(text):
            asked.append(text)
            return response('{"prompt": "Q?", "answer": "A."}')

        given = item(suffix=True)
        given["context"] = "# Ohm's law\n\nV = IR.\n"
        make_card(given, ask, self.log_file)
        self.assertTrue(asked[0].startswith(TOPIC + "\n\n"))
        self.assertTrue(asked[0].endswith("# Ohm's law\n\nV = IR.\n"))

    def test_a_card_that_cant_be_made_is_an_error_answer(self):
        def ask(text):
            raise KindError("the OpenAI API answered 500 Internal Server Error: {}")

        made = make(item(suffix=True), ask, self.log_file)
        self.assertEqual(list(made), ["error"])
        self.assertIn("500", made["error"])

    def test_each_call_is_a_line_of_the_log(self):
        def ask(text):
            made = response('{"prompt": "Q?", "answer": "A."}')
            made.update(id="resp_1", usage={"input_tokens": 10, "output_tokens": 5, "total_tokens": 15})
            return made

        make_card(item(suffix=True), ask, self.log_file)
        make_card(item(suffix=True), ask, self.log_file)
        entries = self.logged()
        self.assertEqual(len(entries), 2)
        entry = entries[0]
        self.assertEqual(entry["item"], "q4n8w2")
        self.assertEqual(entry["day"], "2026-10-27")
        self.assertEqual(entry["input"], TOPIC)
        self.assertEqual((entry["question"], entry["answer"]), ("Q?", "A."))
        self.assertEqual(entry["usage"]["total_tokens"], 15)
        self.assertEqual(entry["response_id"], "resp_1")
        self.assertIsNone(entry["error"])
        self.assertIsInstance(entry["seconds"], float)
        self.assertRegex(entry["t"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d[+-]\d\d:\d\d$")

    def test_a_failed_call_is_logged_with_its_error(self):
        def ask(text):
            raise KindError("the OpenAI API answered 500 Internal Server Error: {}")

        with self.assertRaises(KindError):
            make_card(item(suffix=True), ask, self.log_file)
        [entry] = self.logged()
        self.assertIn("500", entry["error"])
        self.assertIsNone(entry["question"])

    def test_a_response_that_cant_be_read_is_logged_with_its_usage(self):
        def ask(text):
            made = response("not json")
            made["usage"] = {"total_tokens": 7}
            return made

        with self.assertRaises(KindError):
            make_card(item(suffix=True), ask, self.log_file)
        [entry] = self.logged()
        self.assertIn("not JSON", entry["error"])
        self.assertEqual(entry["usage"], {"total_tokens": 7})


class ReadResponse(unittest.TestCase):
    def test_the_structured_output(self):
        self.assertEqual(
            read_response(response('{"prompt": "Q?", "answer": "A."}')),
            {"prompt": "Q?", "answer": "A."},
        )

    def test_failures_are_errors(self):
        refusal = response("")
        refusal["output"][1]["content"] = [{"type": "refusal", "refusal": "no"}]
        failures = [
            response('{"prompt": "Q?", "answer": "A."}', status="incomplete"),
            response("not json"),
            response('{"prompt": "Q?"}'),
            refusal,
            {"status": "completed", "output": []},
            {"status": "failed", "error": {"code": "server_error", "message": "boom"}},
        ]
        for failure in failures:
            with self.subTest(failure=failure), self.assertRaises(KindError):
                read_response(failure)


class AsSrlRunsIt(unittest.TestCase):
    """Runs the program with JSONL on standard input, as srl does."""

    def run_call(self, call, given, env=None):
        return subprocess.run(
            [sys.executable, str(Path(__file__).with_name("llm_generated.py")), call],
            input="".join(json.dumps(line) + "\n" for line in given),
            capture_output=True,
            text=True,
            encoding="utf-8",
            env=env,
        )

    def answers(self, run):
        self.assertEqual(run.returncode, 0, run.stderr)
        return [json.loads(line) for line in run.stdout.splitlines()]

    def test_each_input_line_gets_one_line_in_order(self):
        invalid = {"id": "q4n8w2", "fields": {}}
        answers = self.answers(self.run_call("validate", [item(), invalid]))
        self.assertEqual([len(answer["errors"]) for answer in answers], [0, 1])
        answers = self.answers(self.run_call("groups", [item(), item()]))
        self.assertEqual(answers, [{"groups": [[None]]}] * 2)

    def test_describe_names_the_field_that_validate_checks(self):
        [described] = self.answers(self.run_call("describe", [{}]))
        self.assertEqual([field["name"] for field in described["fields"]], ["PROMPT"])

    def test_make_without_a_key_gives_errors_and_logs_nothing(self):
        with tempfile.TemporaryDirectory() as folder:
            env = {key: value for key, value in os.environ.items() if key != "OPENAI_API_KEY"}
            # The temporary folder, on every platform.
            env.update(TMPDIR=folder, TEMP=folder, TMP=folder)
            answers = self.answers(self.run_call("make", [item(suffix=True)] * 2, env))
            self.assertEqual(len(answers), 2)
            self.assertIn("OPENAI_API_KEY", answers[0]["error"])
            self.assertEqual(os.listdir(folder), [])
