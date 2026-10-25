# A kind program: questions written by an LLM

`llm_generated.py` is the `llm-generated` kind: you say what a question
should be about, and an LLM writes a new question and its answer every
time the card is shown.

```markdown
<!-- srl id:q4n8w2 kind:llm-generated -->
PROMPT: question about Ohm's law, related to circuits, when it's used, what edge cases there are, following a Circuit Analysis I course
```

It is the idea of [docs/guides/ideas.md](../../docs/guides/ideas.md),
"Questions written by an LLM", made to work: a proof of concept of a
kind program whose content differs every time (see
[docs/cards.md](../../docs/cards.md), "Kind programs").

## Requirements

- Python 3, with nothing but its standard library.
- An OpenAI API key in `OPENAI_API_KEY`. The model is `gpt-6-luna`,
  set in `MODEL` at the top of the program.
- Optional: `OPENAI_BASE_URL`, to send the request to another server
  that has the Responses API. The default is `https://api.openai.com/v1`,
  and the program posts to `$OPENAI_BASE_URL/responses`.

Meant for Linux, macOS and Windows; tried on Linux only.

## Cards

- **One card, with the item's ID** (`q4n8w2` above). Its content
  changes every time it is shown, so in theory each version is a new card.
  But a card ID that is never seen twice would never be due again, so the
  ID stays the same and the card is scheduled as usual. What you rate is
  how well you know the topic, asked a different way each time.
- **`validate` and `groups` never call the LLM.** `validate` only checks
  that `PROMPT` is the item's only field and isn't empty, and `groups`
  gives the one card. Every command that reads items calls them, so
  `srl find`, `srl check`, selectors and scheduling cost nothing.
  `describe`, for `srl kinds` and editors, names the same field.
- **`make` calls the LLM**, and srl calls it only for cards about to be
  shown: the one `srl next` (and so every frontend) gives, and each card
  `srl cards` prints. No question is written that nobody sees. A `make`
  call with several cards asks the LLM once per card.
- **The question and answer come from one call**, with structured output
  (a JSON schema of `prompt` and `answer`), so they always match.
- **No `prompt_revealed`, no clozes.** Nothing is hidden, and the
  instructions tell the model not to write cloze deletions or `==…==`.
- **The instructions are fixed**, in `INSTRUCTIONS` at the top of the
  program: write one question on the topic and its answer, a different angle
  each time. The model gets `PROMPT`, then the item's note (its context in
  mode `prose`) as background.

## Errors

Every failure to make a card is that card's answer, `{"error": …}`, with
the whole reason: no `OPENAI_API_KEY`, a server that can't be reached,
an error from the API (with the JSON it answered), an incomplete
response, a refusal, or output that isn't the expected JSON. Nothing is
retried and no card is made up. The program exits with a status other
than 0 only when it can't run at all, such as on input that isn't JSON. `srl cards` and `srl next` then fail with that
reason, and `srl next` keeps failing on this card until the cause is
fixed or the card is suspended (see [docs/cards.md](../../docs/cards.md),
"Kind programs"). A call is stopped
after `kind_programs.card_timeout` seconds (60 by default). A call usually
takes a few seconds.

## The log

Every call to the API is a line of `llm_generated.log` in the temporary
folder (Python's `tempfile.gettempdir()`: `/tmp` on Linux,
`%TEMP%` on Windows). The name is always the same, taken from the
program's file name, so a copy renamed `other.py` logs to `other.log`.
The log is for two things: checking that a question is only written when
a card is about to be shown, and counting calls and tokens to know the
cost.

Each line is one JSON object:

```json
{"t": "2026-10-27T22:15:47+09:00", "item": "q4n8w2", "day": "2026-10-27",
 "model": "gpt-6-luna", "input": "question about ohm law, …",
 "question": "In circuit analysis, when can you use …", "answer": "Use …",
 "usage": {"input_tokens": 277, "output_tokens": 269, "total_tokens": 546, …},
 "response_id": "resp_05d2…", "seconds": 4.666, "error": null}
```

| Key | Meaning |
|---|---|
| `t` | When the call started, local time with its offset. |
| `item` | The item's ID. |
| `day` | The study day srl gave. |
| `model` | The model asked. |
| `input` | What was sent: `PROMPT`, then the note. The fixed instructions are not repeated. |
| `question`, `answer` | The card the model wrote, or `null` if the call failed. |
| `usage` | The token counts the API returned, as it returned them, or `null` if it returned none. |
| `response_id` | The API's ID of the response, or `null`. |
| `seconds` | How long the call took. |
| `error` | Why the call failed, as `make` gives it in `{"error": …}`, or `null`. |

A failed call is logged too, since it may still cost something. What
fails before any request is sent is not logged: a
missing `OPENAI_API_KEY`. A call that srl stops after `card_timeout`
isn't logged either, because the program is killed before it can write. If
the log can't be written, the card fails with that reason.

Nothing deletes the log, and the temporary folder may be emptied when the
computer restarts. For example:

```sh
L=$(python3 -c 'import tempfile; print(tempfile.gettempdir())')/llm_generated.log
tail -f "$L"                                        # watch calls as they are made
jq -s 'length' "$L"                                 # calls
jq -s 'map(.usage.total_tokens // 0) | add' "$L"    # tokens
jq -c 'select(.error) | {t, item, error}' "$L"      # failures
```

## Running it

Copy `llm_generated.py` into your store, for example to
`kinds/llm_generated.py`, and name it in the store's `.srl/config.toml`:

```toml
[kinds]
llm-generated = ["python3", "kinds/llm_generated.py"]
```

The path is relative to the store's folder; an absolute path works too.
Then:

```sh
srl cards q4n8w2     # one question the LLM could ask (an API call)
srl next             # study as usual
srl next kind:llm-generated   # study only these items
```

The Browse tab of `srl-web` (see [web-frontend/](../web-frontend/README.md))
shows such a card as a frontend shows it, without studying it: choose a
card and it is made, with an API call, while the page counts the seconds.

By hand, one JSON object per line in, one per line out:

```sh
echo '{"id": "q4n8w2", "fields": {"PROMPT": "Ohm'\''s law"}}' \
  | python3 llm_generated.py validate
# {"errors": []}

echo '{"id": "q4n8w2", "fields": {"PROMPT": "Ohm'\''s law"}}' \
  | python3 llm_generated.py groups
# {"groups": [[null]]}

echo '{"id": "q4n8w2", "fields": {"PROMPT": "Ohm'\''s law"}, "suffix": null, "context": null, "day": "2026-10-27"}' \
  | python3 llm_generated.py make

echo '{}' | python3 llm_generated.py describe
```

## Tests

The tests use Python's own `unittest` and never call the API. Run them from
this folder:

```sh
cd examples/kind-llm-generated
python3 -m unittest -v
```

They are in `test_llm_generated.py`:

- `Validate`: a prompt alone is valid, and every invalid item gives errors
  about its fields.
- `Groups`: one card with the item's ID.
- `MakeCard`: the card is what the model wrote, the note is sent after
  `PROMPT`, a card that can't be made is an `{"error": …}` answer, and
  what each call, failed or not, writes to the log. A function stands in
  for the API, and the log is written to a temporary folder of each test.
- `ReadResponse`: reading the structured output from a Responses API
  response, and every failure in one.
- `AsSrlRunsIt`: runs the program with JSONL on standard input, as srl
  does: one answer per line, in order, for `validate`, `groups` and
  `describe`, and `make` without `OPENAI_API_KEY` giving errors and
  logging nothing.
