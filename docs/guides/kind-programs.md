# Writing a kind program

This is a guide, not part of the specification (see
[overview.md](../overview.md), "Specification, guides and examples"). A
kind program is any program that answers the four calls of
[cards.md](../cards.md), "Kind programs". This guide is about keeping one
fast when it needs something slow: a large dictionary, a database, a
morphological analyzer, a web service.

## Where the time goes

Every call is a new process: `srl` starts the program, writes its input
lines, and waits for it to end. Nothing the program loads survives to the
next call. So what matters is how often each call runs (see
[cards.md](../cards.md), "How srl calls them"):

| Call | Runs | Lines per call |
|---|---|---|
| `validate`, `groups` | On any command that reads items, but only for items whose answer isn't kept in the index cache: new or edited items, `srl add`, `srl check`, or after deleting `.srl/cache/` | As many as there are such items |
| `make` | `srl next`, once per card shown; `srl cards`, once per command | One for `srl next`; one per card printed for `srl cards` |
| `describe` | `srl kinds` only | One |

Two things follow:

- **`make` is the call you study with.** Every card a frontend shows is
  one `srl next`, so one process, so one start of your program and of
  whatever it loads. A second of loading is a second of waiting on every
  card of the kind.
- **`validate` and `groups` are usually free**, because their answers are
  kept. But they run on the first command after you add or edit items,
  and on every item at `srl check`, and they run for commands that never
  show a card, such as `srl find` or `srl stats`. Anything slow they do
  makes those commands slow.

A card of another kind never starts your program, so whatever it loads
costs nothing to the rest of the store.

## Load only what the call needs

**Load heavy resources inside `make`, not when the program starts.**
`validate` and `groups` work on the item's fields alone (see
[cards.md](../cards.md), "Calls"), and almost never need the dictionary
that `make` looks words up in. A program that loads it at the top runs it
for every call, including those that never use it.

The same holds for slow imports: a large library imported at the top of a
Python script is imported by `validate` too. Import it in the function
that uses it.

[examples/kind-ja-words/](../../examples/kind-ja-words/) does this: its
`validate` and `groups` only parse the `{word}` braces of the sentence,
and only `make` runs MeCab.

## Load once per process, not once per line

A `make` call from `srl cards` may hold many lines. Load the resource the
first time a line needs it, and keep it for the rest of the call, instead
of loading it again for each line. In Python:

```python
import functools

@functools.cache
def dictionary():
    # Read the dictionary here; this runs at most once per process.
    ...

def make(item):
    entries = dictionary()
    ...
```

`validate`, `groups` and `describe` never call `dictionary()`, so they
never pay for it.

Running an external program for each line, as `kind-ja-words` runs
`mecab` once per card, is fine when that program starts fast: MeCab maps
its compiled dictionary into memory instead of reading it.

## Make loading itself cheap

Loading once per process still means once per card shown with `srl
next`. When that is too slow, make the resource cheaper to open rather
than to read:

- **Query instead of reading everything.** A dictionary in SQLite, with
  an index on the headword, opens at once and answers only the words of
  the card. A 10 MB text file parsed on every card is read whole to look
  up a handful of words.
- **Convert once, ahead of time.** Turn a text format into one that loads
  fast (SQLite, a compiled dictionary, a file the program can map into
  memory) with a separate script, and have the kind program open the
  result. Keep the conversion out of the kind program: it isn't one of
  the four calls.
- **Use a tool that is already fast to start**, as `kind-ja-words` uses
  MeCab.

Then measure the call that matters, with one line, as `srl next` makes
it:

```sh
time python3 kinds/my_kind.py make < one-card.jsonl
```

The input line is the item plus `suffix`, `see`, `context` and `day` (see
[cards.md](../cards.md), "Calls"); `srl show ID --json` gives most of
it.

## Slow services

A `make` that calls a web service, such as an LLM, waits for it on every
card. `srl` stops a `make` call after `kind_programs.card_timeout` seconds
per line (see [store.md](../store.md), "Configuration"), and the card
fails. Ways to keep it bearable:

- **Never call the service from `validate` or `groups`.** They must be
  deterministic anyway, and they run for commands that show no card.
- **Cache answers yourself**, by what makes them the same: the item's ID,
  its suffix and the study day, for example, so the card is made once a
  day and shown again at once.

[examples/kind-llm-generated/](../../examples/kind-llm-generated/) calls
an LLM in `make` only, once per card shown.

## After changing the program

`srl` keeps the answers of `validate` and `groups` until the program's
command in `[kinds]` changes, so a faster program that gives the same
answers needs nothing more. A program that answers differently needs
`srl check`, or deleting `.srl/cache/` (see [cards.md](../cards.md),
"How srl calls them"), and must still give the same card suffixes for
the same fields.

## A program that is slow to start anyway

When none of this is enough, the specification has no other way to run a
kind program yet: a batch per call is the only one. A server that `srl`
keeps running between calls is an idea, not specified (see
[ideas.md](../ideas.md), "Kind programs").
