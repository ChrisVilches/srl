#!/usr/bin/env python3
"""The llm-generated kind: one card whose question an LLM writes anew
each time it is shown.

A proof of concept of a kind program whose content differs every time
(see docs/cards.md, "Kind programs"). Name it in `[kinds]` of the
store's .srl/config.toml, or call it by hand (see README.md).

Calls OpenAI's Responses API over HTTP with Python's standard library;
no Python packages. Needs the API key in OPENAI_API_KEY. Meant for
Linux, macOS and Windows; tried on Linux only.

    python3 llm_generated.py ids  < item.json
    python3 llm_generated.py card < item-with-suffix.json
"""

import json
import os
import sys
import urllib.error
import urllib.request

FIELD = "PROMPT"
MODEL = "gpt-6-luna"
DEFAULT_BASE_URL = "https://api.openai.com/v1"

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
    """A reason to fail the call, printed on standard error."""


def topic_of(item):
    """The item's PROMPT, checked."""
    fields = item["fields"]
    if FIELD not in fields:
        raise KindError(f"an llm-generated item needs the field {FIELD}")
    others = sorted(key for key in fields if key != FIELD)
    if others:
        raise KindError(f"an llm-generated item has only the field {FIELD}, not {', '.join(others)}")
    if not fields[FIELD].strip():
        raise KindError(f"{FIELD} is empty; say what the question should be about")
    return fields[FIELD]


def ids(item):
    """The output of `ids`: one card, with the item's ID. Its content
    changes every time, but a new ID each time would never be due again,
    so the ID stays the same and the card is scheduled as any other."""
    topic_of(item)
    return {"groups": [{"cards": [None]}]}


def card(item, ask=None):
    """The output of `card`: a question and answer the LLM writes for the
    item's PROMPT, with the item's note as background. There is no
    `prompt_revealed`: nothing is hidden. `ask` stands in for the API in
    tests."""
    made = (ask or responses)(request_text(topic_of(item), item.get("context")))
    return {"prompt": made["prompt"], "answer": made["answer"]}


def request_text(topic, context):
    """What the model is asked: the topic, then the note, if there is one."""
    if not context:
        return topic
    return f"{topic}\n\nThe note this card is in, as background:\n\n{context}"


def responses(topic):
    """The {"prompt", "answer"} the Responses API gives for the request.
    Every failure is a KindError with all the API said, since nothing can
    be done about it here."""
    key = os.environ.get("OPENAI_API_KEY")
    if not key:
        raise KindError("OPENAI_API_KEY is not set; set it to your OpenAI API key")
    base = os.environ.get("OPENAI_BASE_URL", DEFAULT_BASE_URL).rstrip("/")
    body = {
        "model": MODEL,
        "instructions": INSTRUCTIONS,
        "input": topic,
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
    return parse(response)


def parse(response):
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
    calls = {"ids": ids, "card": card}
    if len(argv) != 2 or argv[1] not in calls:
        print("usage: llm_generated.py ids|card < ITEM.json", file=sys.stderr)
        return 2
    try:
        output = calls[argv[1]](json.load(sys.stdin))
    except KindError as error:
        print(error, file=sys.stderr)
        return 1
    json.dump(output, sys.stdout, ensure_ascii=False)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
