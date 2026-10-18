# Cards

A **card** is one ready-to-show question about one or more atoms. Every card,
whatever drill produced it, has the same format. That fixed format is what
lets any drill work with any frontend: drills never think about display, and
frontends never think about where cards come from.

## Format

```json
{"atoms": ["a7f3k2"],
 "drill": "cloze",
 "ref": "cloze:0:1",
 "prompt": "嵐を==[1]==、漁師たちは海へ出た。",
 "answer": "嵐を==ものともせず==、漁師たちは海へ出た。"}
```

| Field | Meaning |
|---|---|
| `atoms` | IDs of the atoms the rating applies to. Usually one. |
| `drill` | Name of the drill that produced the card. |
| `ref` | Stable identity of the card within its atom, prefixed with the drill name: `cloze:0:1`, `conj:te`, `cards:0`. |
| `prompt` | The question. |
| `answer` | The answer. |

Every field is required. You always rate a card yourself (see
[scheduling.md](scheduling.md), "Ratings"): a frontend shows the prompt, then
the answer, and records the rating you give.

A card carries no explanation. After the answer, a frontend shows the body of
the card's atoms, which it gets with `srl show ID` (see
[frontends.md](frontends.md)). Every card of an atom is explained by the same
body.

## Rules

1. **The prompt never contains the answer.** A card must be safe to display
   as-is, by the simplest frontend, in a log viewer or with `jq`.
2. **Cards describe meaning, frontends decide appearance.** A card can say
   "this part is important"; it never says "this part is yellow".
3. **The format is extended for everyone or not at all.** If a drill needs
   something the format cannot express, the format gets a new field, and
   frontends learn it. No drill invents private fields.
4. **`srl` validates every card** coming out of a drill or a session, and
   drops and reports each invalid one. A card is valid when:
   - every field is present, with the right type, and there is no field the
     format doesn't have;
   - `atoms` is not empty, and every ID in it is an atom of the store;
   - `ref` is `DRILL` or starts with `DRILL:` followed by something, where
     `DRILL` is the card's `drill`;
   - `prompt` is not empty (`answer` may be).

## Text formatting

The text fields `prompt` and `answer` are **CommonMark Markdown plus one
extension: `==highlight==`.**

- `==…==` marks the important part: the blank in a cloze prompt, the filled-in
  span in its answer.
- Each frontend decides how to render it: bold yellow in a terminal, `<mark>`
  with CSS in a browser, or the raw `==…==` in a plain frontend.
- Printing the raw text must always be an acceptable rendering. A frontend may
  render less than Markdown offers, but never needs more.

The usual way to write a blank is `==[1]==`, `==[2]==`, … in order, but
frontends do not parse prompts, so this is a convention for readers, not a
rule.

> **TODO (design):** cards can't carry media (audio, images) or furigana
> yet; how to add them without breaking "raw text is always acceptable" is
> undecided (see [overview.md](overview.md), "Open questions").

## Card identity

A card is identified by its atom and its `ref`. `srl next` uses this to rotate
through an atom's cards, and review events record it.

Built-in drills derive refs from positions in a facet (`cloze:0:1` is the
first sentence's group 1). If you reorder the facet, refs move to other cards.
This only affects which card is shown next and how old log entries read; it
never affects scheduling, because scheduling belongs to the atom.

## Where cards come from

| Source | Stored? |
|---|---|
| A drill run by `srl next` | No |
| `sessions/<name>.jsonl`, studied with `srl study --session` | Yes, as a prepared list |

Cards in sessions use exactly the same format, and are validated when written
(`srl session put`).
