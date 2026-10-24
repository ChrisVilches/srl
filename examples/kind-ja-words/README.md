# A kind program: words of a Japanese sentence

`ja_words.py` is the `ja-words` kind: a lazy way to study words in
context. You write a sentence and put braces around the words to study:

```markdown
<!-- srl id:v81k3z kind:ja-words -->
TEXT: 東京{都}は、日本の関東{地方}に{位置}する{都}
```

Each distinct word in braces is a card. The card shows the whole sentence
with that word highlighted, not hidden: you say the word to yourself, look
at the answer, and rate yourself. The answer is the same for every card of
the sentence: MeCab's analysis of it, one line per token, with its reading
in hiragana, its part of speech and its base form.

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
- **No siblings.** Each card is a group of its own, so on the same day
  you can see every word of a sentence.
- **The card suffix is the word's UTF-8 bytes in hex**: 都 gives
  `v81k3z-e983bd`, 位置 gives `v81k3z-e4bd8de7bdae`. It comes from the
  word alone, so adding, removing or moving words doesn't give one card's
  history to another, and it can be read back with
  `echo e983bd | xxd -r -p`. Changing a word makes a new card.
- **Braces only go around a word.** An empty `{}`, a brace without its
  pair, braces inside braces, and a `TEXT` without any `{word}` are
  errors; there is no way to write a literal brace. `TEXT` is the only
  field.

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
```

Every command that reads items runs `ja_words.py ids` once for each
`ja-words` item, so a store with many of them is slower. `card`, which
runs MeCab, only runs for a card about to be shown.

By hand:

```sh
echo '{"id": "v81k3z", "fields": {"TEXT": "東京{都}は、日本の関東{地方}に{位置}する{都}"}}' \
  | python3 ja_words.py ids
# {"groups": [{"cards": ["e983bd"]}, {"cards": ["e59cb0e696b9"]}, {"cards": ["e4bd8de7bdae"]}]}

echo '{"id": "v81k3z", "fields": {"TEXT": "東京{都}は、日本の関東{地方}に{位置}する{都}"},
       "suffix": "e983bd", "context": null, "day": "2026-10-27"}' \
  | python3 ja_words.py card
```

An error exits with status 1 and its reason on standard error.

## Tests

The tests use Python's own `unittest`, so there is nothing to install.
Run them from this folder, because they import `ja_words.py` from it:

```sh
cd examples/kind-ja-words
python3 -m unittest            # all tests, a dot per test
python3 -m unittest -v         # each test by name
python3 -m unittest test_ja_words.Ids                                  # one class
python3 -m unittest test_ja_words.Ids.test_a_repeated_word_gives_one_card  # one test
```

They are in `test_ja_words.py`, in three classes:

- `Ids`: one card per distinct word, a repeated word giving one card,
  every card in a group of its own, the hex suffix and that it doesn't
  depend on where the word is, and every invalid `TEXT` or field.
- `Card`: which occurrences are highlighted, the answer being the analysis
  of the whole sentence, and how each token becomes a line. They give the
  program a made-up MeCab output, so they don't need MeCab.
- `WithMecab`: runs `ja_words.py` as `srl` would, with JSON on standard
  input, and checks its output, its exit status and the real analysis of
  a sentence. Skipped, and reported as skipped, when `mecab` isn't on
  PATH.

To try a change by hand, run the calls of "Running it" above with your own
sentence. Running the tests leaves a `__pycache__` folder here, which git
ignores and which can be deleted.
