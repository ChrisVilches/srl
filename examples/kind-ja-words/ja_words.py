#!/usr/bin/env python3
"""The ja-words kind: one card per {word} of a Japanese sentence.

A proof of concept of a kind program with several cards and no siblings
(see docs/cards.md, "Kind programs"). Name it in `[kinds]` of the
store's .srl/config.toml, or call it by hand (see README.md).

Requires MeCab and its IPA dictionary as programs on PATH; no Python
packages. Tried on Arch Linux only, where they are installed with
`yay -S mecab mecab-ipadic`.

    python3 ja_words.py ids  < item.json
    python3 ja_words.py card < item-with-suffix.json
"""

import json
import subprocess
import sys

FIELD = "TEXT"


class KindError(Exception):
    """A reason to fail the call, printed on standard error."""


def pieces(text):
    """The text split into plain strings and ("word", WORD) pairs, one per
    {WORD}. Any other brace is an error: there is no escaping."""
    result = []
    plain = ""
    word = None
    for char in text:
        if word is None:
            if char == "{":
                word = ""
            elif char == "}":
                raise KindError("a `}` without its `{`; braces only go around a word")
            else:
                plain += char
        elif char == "{":
            raise KindError("a `{` inside `{…}`; braces only go around a word")
        elif char == "}":
            if not word.strip():
                raise KindError("an empty `{}`; write the word inside it")
            if plain:
                result.append(plain)
                plain = ""
            result.append(("word", word))
            word = None
        else:
            word += char
    if word is not None:
        raise KindError("a `{` without its `}`")
    if plain:
        result.append(plain)
    return result


def text_of(item):
    """The pieces of the item's TEXT, checked."""
    fields = item["fields"]
    if FIELD not in fields:
        raise KindError(f"a ja-words item needs the field {FIELD}")
    others = sorted(key for key in fields if key != FIELD)
    if others:
        raise KindError(f"a ja-words item has only the field {FIELD}, not {', '.join(others)}")
    result = pieces(fields[FIELD])
    if not any(isinstance(piece, tuple) for piece in result):
        raise KindError(f"{FIELD} has no `{{word}}` to make a card of")
    return result


def suffix(word):
    """The card suffix of a word: its UTF-8 bytes in hex, so 都 is e983bd."""
    return word.encode("utf-8").hex()


def ids(item):
    """The output of `ids`: one card per distinct word, in the order they
    first appear, each in a group of its own."""
    suffixes = []
    for piece in text_of(item):
        if isinstance(piece, tuple) and suffix(piece[1]) not in suffixes:
            suffixes.append(suffix(piece[1]))
    return {"groups": [{"cards": [card]} for card in suffixes]}


def card(item, analyze=None):
    """The output of `card`: the sentence with every {…} of the card's
    word highlighted, and the analysis of the whole sentence as the
    answer. `analyze` stands in for MeCab in tests."""
    found = text_of(item)
    words = [piece[1] for piece in found if isinstance(piece, tuple)]
    chosen = next((word for word in words if suffix(word) == item["suffix"]), None)
    if chosen is None:
        raise KindError(f"no `{{word}}` in {FIELD} has the suffix {item['suffix']}")
    # NOTE: the same word twice in a row, `{都}{都}`, gives `==都====都==`,
    # which most Markdown renderers don't show as two highlights. Left as is
    # for this proof of concept. Ways to fix it:
    # - Separate the marks while building the string: when a highlight ends
    #   right where the next begins, put a space between them, giving
    #   `==都== ==都==`. Hacky, and it adds a space that isn't in the
    #   sentence, but it works.
    # - Always wrap the word in a marker that isolates it, such as
    #   `==[都]==`. A hardcoded decoration on every card, not only on the
    #   ones that need it; and `==[…]==` is also how a qa cloze shows its
    #   blank, so it could read as a hidden word.
    # - Merge highlights in a row into one, `==都都==`: adds nothing to the
    #   sentence, but no longer shows where one word ends.
    prompt = "".join(
        piece if not isinstance(piece, tuple)
        else f"=={piece[1]}==" if piece[1] == chosen
        else piece[1]
        for piece in found
    )
    plain = "".join(piece if not isinstance(piece, tuple) else piece[1] for piece in found)
    return {
        "prompt": prompt,
        "answer": answer(plain, analyze or mecab),
    }


def mecab(text):
    """MeCab's output for the text."""
    try:
        run = subprocess.run(
            ["mecab"], input=text, capture_output=True, text=True, encoding="utf-8"
        )
    except FileNotFoundError:
        raise KindError("mecab was not found; install mecab and mecab-ipadic") from None
    if run.returncode != 0:
        raise KindError(f"mecab failed: {run.stderr.strip()}")
    return run.stdout


def answer(text, analyze):
    """One Markdown list line per token of the text: the word, its reading
    in hiragana, its part of speech, and its base form if it differs."""
    lines = []
    for line in analyze(text).splitlines():
        if line == "EOS" or "\t" not in line:
            continue
        surface, features = line.split("\t", 1)
        # IPA dictionary: part of speech, three subdivisions, conjugation
        # type and form, base form, reading, pronunciation. Unknown words
        # stop after the conjugation form.
        features = features.split(",")
        parts = [part for part in features[:2] if part != "*"]
        reading = hiragana(features[7]) if len(features) > 7 else None
        base = features[6] if len(features) > 6 and features[6] != "*" else surface
        entry = f"- {surface}"
        if reading and reading != surface:
            entry += f" — {reading}"
        entry += f" ({', '.join(parts)})"
        if base != surface:
            entry += f", from {base}"
        lines.append(entry)
    return "\n".join(lines)


def hiragana(katakana):
    """The katakana of a reading as hiragana; ー and the rest unchanged."""
    return "".join(
        chr(ord(char) - 0x60) if "ァ" <= char <= "ヶ" else char for char in katakana
    )


def main(argv):
    calls = {"ids": ids, "card": card}
    if len(argv) != 2 or argv[1] not in calls:
        print("usage: ja_words.py ids|card < ITEM.json", file=sys.stderr)
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
