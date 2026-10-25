"""Tests of the llm-generated kind. Run with `python3 -m unittest` in this
folder. They never call the API: a made-up response stands in for it.
"""

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

from llm_generated import KindError, card, ids, parse

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


class Ids(unittest.TestCase):
    def test_one_card_with_the_item_id(self):
        self.assertEqual(ids(item()), {"groups": [{"cards": [None]}]})

    def test_invalid_items_are_errors(self):
        for fields in [{}, {"Q": TOPIC}, {"PROMPT": TOPIC, "A": "x"}, {"PROMPT": "  "}]:
            with self.subTest(fields=fields), self.assertRaises(KindError):
                ids({"id": "q4n8w2", "fields": fields})


class Card(unittest.TestCase):
    def test_the_card_is_what_the_model_wrote_for_the_prompt(self):
        asked = []

        def ask(topic):
            asked.append(topic)
            return {"prompt": "What is V when I = 2 A and R = 3 Ω?", "answer": "6 V"}

        self.assertEqual(
            card(item(suffix=True), ask),
            {"prompt": "What is V when I = 2 A and R = 3 Ω?", "answer": "6 V"},
        )
        self.assertEqual(asked, [TOPIC])

    def test_the_note_is_sent_after_the_prompt(self):
        asked = []

        def ask(topic):
            asked.append(topic)
            return {"prompt": "Q?", "answer": "A."}

        given = item(suffix=True)
        given["context"] = "# Ohm's law\n\nV = IR.\n"
        card(given, ask)
        self.assertTrue(asked[0].startswith(TOPIC + "\n\n"))
        self.assertTrue(asked[0].endswith("# Ohm's law\n\nV = IR.\n"))

    def test_an_invalid_item_never_asks(self):
        def ask(topic):
            raise AssertionError("asked")

        with self.assertRaises(KindError):
            card({"id": "q4n8w2", "fields": {}, "suffix": None}, ask)


class Parse(unittest.TestCase):
    def test_the_structured_output(self):
        self.assertEqual(
            parse(response('{"prompt": "Q?", "answer": "A."}')),
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
                parse(failure)


class AsSrlRunsIt(unittest.TestCase):
    """Runs the program with JSON on standard input, as srl does."""

    def run_call(self, call, given, env=None):
        return subprocess.run(
            [sys.executable, str(Path(__file__).with_name("llm_generated.py")), call],
            input=json.dumps(given),
            capture_output=True,
            text=True,
            encoding="utf-8",
            env=env,
        )

    def test_ids(self):
        run = self.run_call("ids", item())
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(json.loads(run.stdout), {"groups": [{"cards": [None]}]})

    def test_card_without_a_key_fails_loudly(self):
        env = {key: value for key, value in os.environ.items() if key != "OPENAI_API_KEY"}
        run = self.run_call("card", item(suffix=True), env)
        self.assertEqual(run.returncode, 1)
        self.assertEqual(run.stdout, "")
        self.assertIn("OPENAI_API_KEY", run.stderr)
