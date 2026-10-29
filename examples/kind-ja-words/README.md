# A kind program: words of a Japanese sentence

`ja_words.py` is the `ja-words` kind: a lazy way to study words in
context. You write a sentence and put braces around the words to study:

````markdown
```srl id:v81k3z kind:ja-words
TEXT: 東京{都}は、日本の関東{地方}に{位置}する{都}
```
````

Each distinct word in braces is a card. The card shows the whole sentence
with that word highlighted, not hidden: you say the word to yourself, look
at the answer, and rate yourself. The answer is the same for every card of
the sentence: MeCab's analysis of it, one line per token, with its reading
in hiragana, its part of speech and its base form. Since nothing is
hidden, the card has no `prompt_revealed`: the sentence stays as it is
when the answer is shown (see [docs/cards.md](../../docs/cards.md),
"Format").

It is a proof of concept of a kind program with several cards and no
siblings (see [docs/cards.md](../../docs/cards.md), "Kind programs").

## Requirements

- Python 3, with nothing but its standard library.
- MeCab and its IPA dictionary, run as the program `mecab` found on PATH.
  On Arch Linux: `yay -S mecab mecab-ipadic`.

Tried on Arch Linux only.

## Cards

- **One card per distinct word.** A word in braces more than once, like
  都 above, gives one card, with every braced occurrence highlighted:
  `東京==都==は、日本の関東地方に位置する==都==`. The same word
  without braces is not highlighted, so 都 in 京都 is left alone.
- **Highlights that touch get a space between them.** `{都}{都}` would
  give `==都====都==`, which most Markdown renderers don't show as two
  highlights, so the card shows `==都== ==都==`: a space the sentence
  doesn't have, but each word stays visible on its own.
- **No siblings.** Each card is a group of its own, so on the same day
  you can see every word of a sentence.
- **The card suffix is the word's UTF-8 bytes in hex**: 都 gives
  `v81k3z.e983bd`, 位置 gives `v81k3z.e4bd8de7bdae`. It comes from the
  word alone, so adding, removing or moving words doesn't give one card's
  history to another, and it can be read back with
  `echo e983bd | xxd -r -p`. Changing a word makes a new card. This rule
  must never change in a new version of `ja_words.py`: every card's
  history is kept under its suffix (see
  [docs/cards.md](../../docs/cards.md), "Kind programs").
- **A word is taken as it reads, not as it was typed.** Spaces at either
  end, full-width ones included, aren't part of it: `{ 都}` is the card
  of 都, and the space stays in the sentence, outside the highlight. The
  suffix is made from the word in Unicode NFC, so が typed as か followed
  by a combining ゛, as some copied text has it, is the same card as が.
- **Braces only go around a word.** An empty `{}`, a brace without its
  pair, braces inside braces, and a `TEXT` without any `{word}` are
  errors about `TEXT`, which `validate` gives; there is no way to write a
  literal brace. `TEXT` is the only field: any other is an error about
  that field. `describe` says so, for `srl kinds` and editors.

MeCab's IPA dictionary is Japanese only, and it reads a word in context
as best it can: 都 at the end of the sentence above comes out as と, not
みやこ.

## Running it

Copy `ja_words.py` into your store, for example to `kinds/ja_words.py`,
and name it in the store's `.srl/config.toml`:

```toml
[kinds]
ja-words = ["python3", "kinds/ja_words.py"]
```

The path is relative to the store's folder; an absolute path works too,
so you can also point at this folder instead of copying. Items with
`kind:ja-words` then have their cards:

```sh
srl cards v81k3z     # the three cards of the sentence above
srl next             # study as usual
srl next kind:ja-words   # study only ja-words items
```

To see the cards as a frontend shows them, with the words highlighted,
open the Browse tab of `srl-web` (see
[web-frontend/](../web-frontend/README.md)) with the selector
`kind:ja-words`, and choose a card.

Every command that reads items runs `ja_words.py validate`, then
`ja_words.py groups`, once each for the `ja-words` items of the store
whose answers srl hasn't kept: new or changed items. After changing
`ja_words.py`, run `srl check` so that srl asks it again about every
item.
`make`, which runs MeCab, only runs for the cards about to be shown or
printed.

By hand, one JSON object per line in, one per line out:

```sh
echo '{"id": "v81k3z", "fields": {"TEXT": "東京{都}は、日本の関東{地方}に{位置}する{都}"}}' \
  | python3 ja_words.py validate
# {"errors": []}

echo '{"id": "v81k3z", "fields": {"TEXT": "東京{都}は、日本の関東{地方}に{位置}する{都}"}}' \
  | python3 ja_words.py groups
# {"groups": [["e983bd"], ["e59cb0e696b9"], ["e4bd8de7bdae"]]}

echo '{"id": "v81k3z", "fields": {"TEXT": "東京{都}は、日本の関東{地方}に{位置}する{都}"}, "suffix": "e983bd", "context": null, "context_format": null, "day": "2026-10-27"}' \
  | python3 ja_words.py make

echo '{}' | python3 ja_words.py describe
```

An invalid item is an answer, its `errors`, and so is a card that can't
be made, `{"error": …}` (MeCab missing, or a suffix of no word). Only a
call that can't run at all, such as input that isn't JSON, exits with a
status other than 0, with a traceback on standard error.

## Tests

The tests use Python's own `unittest`, so there is nothing to install.
Run them from this folder, because they import `ja_words.py` from it:

```sh
cd examples/kind-ja-words
python3 -m unittest            # all tests, a dot per test
python3 -m unittest -v         # each test by name
python3 -m unittest test_ja_words.Groups                               # one class
python3 -m unittest test_ja_words.Groups.test_a_repeated_word_gives_one_card  # one test
```

They are in `test_ja_words.py`, in these classes:

- `Describe`: it names `TEXT`.
- `Validate`: a valid sentence, every invalid `TEXT` as an error about
  `TEXT`, and every other field as an error about it.
- `Groups`: one card per distinct word, a repeated word giving one card,
  every card in a group of its own, the hex suffix and that it doesn't
  depend on where the word is, and spaces around a word and combining
  marks not making another card.
- `MakeCard`: which occurrences are highlighted, spaces staying outside the
  highlight, a space between highlights that touch, the card having no
  `prompt_revealed`, the answer being the analysis
  of the whole sentence, and how each token becomes a line. They give the
  program a made-up MeCab output, so they don't need MeCab.
- `Calls`: runs `ja_words.py` as `srl` would, with JSONL on standard
  input, and checks one answer per line, in order, and, with `make`, the
  real analysis of a sentence and an error for a card that can't be made.
  That last test is skipped, and reported as skipped, when `mecab` isn't
  on PATH.

To try a change by hand, run the calls of "Running it" above with your own
sentence. Running the tests leaves a `__pycache__` folder here, which git
ignores and which can be deleted.
