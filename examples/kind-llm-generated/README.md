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
- **`ids` never calls the LLM.** It only checks that `PROMPT` is the
  item's only field and isn't empty. Every command that reads items calls
  it, so `srl find`, `srl check`, selectors and scheduling cost nothing.
- **`card` calls the LLM**, and srl calls it only for a card that is about to
  be shown: the one `srl next` (and so every frontend) gives, and each
  card `srl cards` prints. No question is written that nobody sees.
- **The question and answer come from one call**, with structured output
  (a JSON schema of `prompt` and `answer`), so they always match.
- **No `prompt_revealed`, no clozes.** Nothing is hidden, and the
  instructions tell the model not to write cloze deletions or `==…==`.
- **The instructions are fixed**, in `INSTRUCTIONS` at the top of the
  program: write one question on the topic and its answer, a different angle
  each time. The model gets `PROMPT`, then the item's note (its context in
  mode `prose`) as background.

## Errors

Every failure makes `card` exit with status 1 and the whole reason on
standard error: no `OPENAI_API_KEY`, a server that can't be reached, an
error from the API (with the JSON it answered), an incomplete response, a
refusal, or output that isn't the expected JSON. Nothing is retried and
no card is made up. `srl cards` then fails with that reason; `srl next`
prints it and goes on to the next card (see
[docs/cards.md](../../docs/cards.md), "Kind programs"). A call is stopped
after `kind_programs.card_timeout` seconds (60 by default). A call usually
takes a few seconds.

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

By hand:

```sh
echo '{"id": "q4n8w2", "fields": {"PROMPT": "Ohm'\''s law"}}' \
  | python3 llm_generated.py ids
# {"groups": [{"cards": [null]}]}

echo '{"id": "q4n8w2", "fields": {"PROMPT": "Ohm'\''s law"},
       "suffix": null, "context": null, "day": "2026-10-27"}' \
  | python3 llm_generated.py card
```

## Tests

The tests use Python's own `unittest` and never call the API. Run them from
this folder:

```sh
cd examples/kind-llm-generated
python3 -m unittest -v
```

They are in `test_llm_generated.py`:

- `Ids`: one card with the item's ID, and every invalid item.
- `Card`: the card is what the model wrote, the note is sent after
  `PROMPT`, and an invalid item never reaches the API. A function stands
  in for the API.
- `Parse`: reading the structured output from a Responses API response,
  and every failure in one.
- `AsSrlRunsIt`: runs the program with JSON on standard input, as srl
  does: `ids`, and `card` failing loudly without `OPENAI_API_KEY`.
