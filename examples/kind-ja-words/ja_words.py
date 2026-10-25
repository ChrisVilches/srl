#!/usr/bin/env python3
"""The ja-words kind: one card per {word} of a Japanese sentence.

A proof of concept of a kind program with several cards and no siblings
(see docs/cards.md, "Kind programs"). Name it in `[kinds]` of the
store's .srl/config.toml, or call it by hand (see README.md).

Requires MeCab and its IPA dictionary as programs on PATH; no Python
packages. Tried on Arch Linux only, where they are installed with
`yay -S mecab mecab-ipadic`.

Every call reads one JSON object per line up to the end of standard
input, and prints one line per input line, in order:

    python3 ja_words.py validate < items.jsonl
    python3 ja_words.py groups   < items.jsonl
    python3 ja_words.py make     < items-with-suffixes.jsonl
    echo '{}' | python3 ja_words.py describe
"""

import json
import subprocess
import sys
import unicodedata

FIELD = "TEXT"


class KindError(Exception):
    """Why an item is invalid, or a card can't be made."""


def pieces(text):
    """The text split into plain strings and ("word", WORD) pairs, one per
    {WORD}. Any other brace is an error: there is no escaping. Spaces at
    either end of a word stay in the sentence but are not part of the
    word, so `{ 都}` is the word 都 after a space."""
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
            stripped = word.strip()
            start = word.index(stripped)
            plain += word[:start]
            if plain:
                result.append(plain)
            result.append(("word", stripped))
            plain = word[start + len(stripped):]
            word = None
        else:
            word += char
    if word is not None:
        raise KindError("a `{` without its `}`")
    if plain:
        result.append(plain)
    return result


def errors_of(item):
    """Why the item is invalid, as `validate` gives it: one error per
    other field, and one about TEXT."""
    fields = item["fields"]
    errors = [
        {"field": key, "message": f"a ja-words item has only the field {FIELD}; remove {key}"}
        for key in sorted(fields)
        if key != FIELD
    ]
    if FIELD not in fields:
        errors.append({"field": FIELD, "message": f"a ja-words item needs the field {FIELD}"})
        return errors
    try:
        found = pieces(fields[FIELD])
    except KindError as error:
        errors.append({"field": FIELD, "message": str(error)})
        return errors
    if not any(isinstance(piece, tuple) for piece in found):
        errors.append({"field": FIELD, "message": f"{FIELD} has no `{{word}}` to make a card of"})
    return errors


def text_of(item):
    """The pieces of the TEXT of an item `validate` accepted."""
    return pieces(item["fields"][FIELD])


def suffix(word):
    """The card suffix of a word: the UTF-8 bytes of its NFC form in hex,
    so 都 is e983bd. NFC makes a word typed with combining marks, such as
    か followed by a combining ゛, the same card as the word typed with
    precomposed characters, が."""
    return unicodedata.normalize("NFC", word).encode("utf-8").hex()


def describe(_input):
    """The output of `describe`: what the kind is for, and its field."""
    return {
        "description": "One card per {word} of a Japanese sentence, the word highlighted.",
        "fields": [
            {
                "name": FIELD,
                "description": "A Japanese sentence, with braces around each word to study: "
                "東京{都}は、日本の関東{地方}にある。",
            }
        ],
    }


def validate(item):
    """The output of `validate`: the item's errors, none if it is valid."""
    return {"errors": errors_of(item)}


def groups(item):
    """The output of `groups`: one card per distinct word, in the order
    they first appear, each in a group of its own."""
    suffixes = []
    for piece in text_of(item):
        if isinstance(piece, tuple) and suffix(piece[1]) not in suffixes:
            suffixes.append(suffix(piece[1]))
    return {"groups": [[card] for card in suffixes]}


def make(item, analyze=None):
    """The output of `make`: the card, or why it can't be made."""
    try:
        return make_card(item, analyze or mecab)
    except KindError as error:
        return {"error": str(error)}


def make_card(item, analyze):
    """The card: the sentence with every {…} of the card's word
    highlighted, and the analysis of the whole sentence as the answer.
    Nothing is hidden, so there is no `prompt_revealed`: the sentence is
    shown as it is with the answer. `analyze` is MeCab, or what stands in
    for it in tests."""
    found = text_of(item)
    words = [piece[1] for piece in found if isinstance(piece, tuple)]
    if not any(suffix(word) == item["suffix"] for word in words):
        raise KindError(f"no `{{word}}` in {FIELD} has the suffix {item['suffix']}")
    # The same word twice in a row, `{都}{都}`, would give `==都====都==`,
    # which most Markdown renderers don't show as two highlights. A space
    # goes between highlights that touch, giving `==都== ==都==`: a
    # heuristic that adds a space the sentence doesn't have, but shows
    # where each word ends. Merging them, `==都都==`, would hide that.
    prompt = ""
    highlighted = False
    for piece in found:
        if isinstance(piece, tuple) and suffix(piece[1]) == item["suffix"]:
            prompt += (" " if highlighted else "") + f"=={piece[1]}=="
            highlighted = True
        else:
            prompt += piece if not isinstance(piece, tuple) else piece[1]
            highlighted = False
    plain = "".join(piece if not isinstance(piece, tuple) else piece[1] for piece in found)
    return {
        "prompt": prompt,
        "answer": answer(plain, analyze),
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
    """Answers the call in `argv`: one line of JSON per line of standard
    input, in order (see docs/cards.md, "Kind programs"). Anything that
    goes wrong outside the answers, such as a line that isn't JSON, fails
    the whole call with a traceback on standard error."""
    calls = {"describe": describe, "validate": validate, "groups": groups, "make": make}
    if len(argv) != 2 or argv[1] not in calls:
        print("usage: ja_words.py describe|validate|groups|make < INPUT.jsonl", file=sys.stderr)
        return 2
    # srl writes and reads UTF-8, whatever the platform's default.
    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    for line in sys.stdin:
        print(json.dumps(calls[argv[1]](json.loads(line)), ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
