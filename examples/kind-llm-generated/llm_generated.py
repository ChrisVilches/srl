#!/usr/bin/env python3
"""The llm-generated kind: one card whose question an LLM writes anew
each time it is shown.

A proof of concept of a kind program whose content differs every time
(see docs/cards.md, "Kind programs"). Name it in `[kinds]` of the
store's .srl/config.toml, or call it by hand (see README.md).

Calls OpenAI's Responses API over HTTP with Python's standard library;
no Python packages. Needs the API key in OPENAI_API_KEY. Logs every
call to llm_generated.log in the temporary folder. Meant for Linux,
macOS and Windows; tried on Linux only.

Every call reads one JSON object per line up to the end of standard
input, and prints one line per input line, in order:

    python3 llm_generated.py validate < items.jsonl
    python3 llm_generated.py groups   < items.jsonl
    python3 llm_generated.py make     < items-with-suffixes.jsonl
    echo '{}' | python3 llm_generated.py describe
"""

import json
import os
import sys
import tempfile
import time
import urllib.error
import urllib.request
from datetime import datetime
from pathlib import Path

FIELD = "PROMPT"
MODEL = "gpt-6-luna"
DEFAULT_BASE_URL = "https://api.openai.com/v1"
# Every API call, one JSON line each (see README.md, "The log").
LOG_FILE = Path(tempfile.gettempdir()) / f"{Path(__file__).stem}.log"

INSTRUCTIONS = """\
You write one flashcard for a spaced repetition system. The user says what
the card should be about, and may add the note the card is in as
background. Write a single question on that topic and its answer.

- The question must make sense on its own and must not contain or give
  away the answer.
- Ask one thing. Prefer questions that test understanding over trivia.
- The same topic is asked about many times, so pick a different angle each
  time: a definition, an application, a calculation, an edge case, a
  comparison, a common mistake.
- The answer is short and complete: what a good student would say, with a
  brief explanation when it helps.
- Write CommonMark Markdown. No cloze deletions, no blanks to fill in, no
  `{{c1::…}}`, no `==…==`.
"""

SCHEMA = {
    "type": "object",
    "properties": {
        "prompt": {"type": "string", "description": "The question."},
        "answer": {"type": "string", "description": "The answer to the question."},
    },
    "required": ["prompt", "answer"],
    "additionalProperties": False,
}


class KindError(Exception):
    """Why a card can't be made."""


def errors_of(item):
    """Why the item is invalid, as `validate` gives it: one error per
    other field, and one about PROMPT."""
    fields = item["fields"]
    errors = [
        {"field": key, "message": f"an llm-generated item has only the field {FIELD}; remove {key}"}
        for key in sorted(fields)
        if key != FIELD
    ]
    if FIELD not in fields:
        errors.append({"field": FIELD, "message": f"an llm-generated item needs the field {FIELD}"})
    elif not fields[FIELD].strip():
        errors.append({"field": FIELD, "message": f"{FIELD} is empty; say what the question should be about"})
    return errors


def describe(_input):
    """The output of `describe`: what the kind is for, and its field."""
    return {
        "description": "One card, whose question and answer an LLM writes anew each time.",
        "fields": [
            {
                "name": FIELD,
                "description": "What the question should be about, as you would tell a "
                "tutor. The item's note is sent too, as background.",
            }
        ],
    }


def validate(item):
    """The output of `validate`: the item's errors, none if it is valid.
    It never calls the LLM."""
    return {"errors": errors_of(item)}


def groups(_item):
    """The output of `groups`: one card, with the item's ID. Its content
    changes every time, but a new ID each time would never be due again,
    so the ID stays the same and the card is scheduled as any other."""
    return {"groups": [[None]]}


def make(item, ask=None, log_file=LOG_FILE):
    """The output of `make`: the card, or why it can't be made."""
    try:
        return make_card(item, ask, log_file)
    except KindError as error:
        return {"error": str(error)}


def make_card(item, ask=None, log_file=LOG_FILE):
    """A question and answer the LLM writes for the PROMPT of an item
    `validate` accepted, with the item's note as background. There is no
    `prompt_revealed`: nothing is hidden. Every call to the API, failed or
    not, is a line of `log_file`. `ask` stands in for the API in tests: it
    gets the request's text and gives the response."""
    text = request_text(item["fields"][FIELD], item.get("context"))
    if ask is None:
        key = api_key()
        ask = lambda text: responses(text, key)
    entry = {
        "t": datetime.now().astimezone().isoformat(timespec="seconds"),
        "item": item["id"],
        "day": item.get("day"),
        "model": MODEL,
        "input": text,
        "question": None,
        "answer": None,
        "usage": None,
        "response_id": None,
        "seconds": None,
        "error": None,
    }
    started = time.monotonic()
    try:
        response = ask(text)
        entry["usage"] = response.get("usage")
        entry["response_id"] = response.get("id")
        made = read_response(response)
        entry["question"] = made["prompt"]
        entry["answer"] = made["answer"]
    except KindError as error:
        entry["error"] = str(error)
        raise
    finally:
        entry["seconds"] = round(time.monotonic() - started, 3)
        log(log_file, entry)
    return {"prompt": made["prompt"], "answer": made["answer"]}


def log(log_file, entry):
    """Appends the entry to the log as one JSON line."""
    try:
        with open(log_file, "a", encoding="utf-8") as file:
            file.write(json.dumps(entry, ensure_ascii=False) + "\n")
    except OSError as error:
        raise KindError(f"cannot write the log {log_file}: {error}") from None


def request_text(topic, context):
    """What the model is asked: the topic, then the note, if there is one."""
    if not context:
        return topic
    return f"{topic}\n\nThe note this card is in, as background:\n\n{context}"


def api_key():
    """The OpenAI API key, from OPENAI_API_KEY."""
    key = os.environ.get("OPENAI_API_KEY")
    if not key:
        raise KindError("OPENAI_API_KEY is not set; set it to your OpenAI API key")
    return key


def responses(text, key):
    """The response of the Responses API to the request. Every failure is
    a KindError with all the API said, since nothing can be done about it
    here."""
    base = os.environ.get("OPENAI_BASE_URL", DEFAULT_BASE_URL).rstrip("/")
    body = {
        "model": MODEL,
        "instructions": INSTRUCTIONS,
        "input": text,
        "text": {"format": {"type": "json_schema", "name": "card", "schema": SCHEMA, "strict": True}},
    }
    request = urllib.request.Request(
        f"{base}/responses",
        data=json.dumps(body).encode("utf-8"),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
        method="POST",
    )
    # No timeout here: srl stops the call after kind_programs.card_timeout.
    try:
        with urllib.request.urlopen(request) as reply:
            response = json.load(reply)
    except urllib.error.HTTPError as error:
        details = error.read().decode("utf-8", errors="replace")
        raise KindError(f"the OpenAI API answered {error.code} {error.reason}: {details}") from None
    except urllib.error.URLError as error:
        raise KindError(f"the OpenAI API could not be reached at {base}: {error.reason}") from None
    return response


def read_response(response):
    """The {"prompt", "answer"} in a Responses API response."""
    if response.get("error"):
        raise KindError(f"the OpenAI API failed: {json.dumps(response['error'])}")
    if response.get("status") != "completed":
        raise KindError(f"the OpenAI response is {response.get('status')}: {json.dumps(response.get('incomplete_details'))}")
    for output in response.get("output", []):
        if output.get("type") != "message":
            continue
        for content in output.get("content", []):
            if content.get("type") == "refusal":
                raise KindError(f"the model refused: {content.get('refusal')}")
            if content.get("type") == "output_text":
                try:
                    made = json.loads(content["text"])
                except json.JSONDecodeError as error:
                    raise KindError(f"the model's output is not JSON ({error}): {content['text']}") from None
                if not isinstance(made.get("prompt"), str) or not isinstance(made.get("answer"), str):
                    raise KindError(f"the model's output has no prompt and answer: {content['text']}")
                return made
    raise KindError(f"the OpenAI response has no text: {json.dumps(response)}")


def main(argv):
    """Answers the call in `argv`: one line of JSON per line of standard
    input, in order (see docs/cards.md, "Kind programs"). Anything that
    goes wrong outside the answers, such as a line that isn't JSON, fails
    the whole call with a traceback on standard error."""
    calls = {"describe": describe, "validate": validate, "groups": groups, "make": make}
    if len(argv) != 2 or argv[1] not in calls:
        print("usage: llm_generated.py describe|validate|groups|make < INPUT.jsonl", file=sys.stderr)
        return 2
    # srl writes and reads UTF-8, whatever the platform's default.
    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    for line in sys.stdin:
        print(json.dumps(calls[argv[1]](json.loads(line)), ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
