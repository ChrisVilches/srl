# srl

Spaced repetition for the command line. You write questions inside your
Markdown notes, and `srl` schedules them for review with
[FSRS](https://github.com/open-spaced-repetition/fsrs4anki/wiki/ABC-of-FSRS),
the modern scheduling algorithm that Anki also offers.

```markdown
# 〜をものともせず

Undeterred by (adversity), braving. Admiring tone; not used about oneself.

<!-- srl id:x7k2m9 -->
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.

<!-- srl id:p4n8d2 -->
Q: 嵐を{{c1::ものともせず}}、漁師たちは海へ出た。
```

That's a complete note with two cards in it. Rendered, the `<!-- srl -->`
lines disappear and the note reads as ordinary questions and answers. The
explanation above them is shown as context after you answer.

## Why srl

- **Questions live next to what they ask about.** No separate deck to keep
  in sync with your notes: a card is two lines in the file you're already
  writing. After the answer, you see the note it came from.
- **Notes stay notes.** A note with questions in it is still plain Markdown
  for every other program: your editor, your viewer, GitHub, Obsidian. `srl`
  doesn't care which one you use.
- **Everything is plain text.** Questions are lines in your notes, and
  reviews are lines in append-only JSONL files. You can `cat` and `grep`
  all of it, edit it in any editor, and version it with git. The SQLite
  index is only a cache and can be deleted at any time.
- **Edit freely.** A card's identity is its ID, so you can reword a question
  or rename and move its note without losing its schedule.
- **Your folders are your topics.** `srl next in:grammar` studies every note
  under a `grammar` folder. There are no tags to maintain, and selectors
  such as `in:rust -state:new` or `state:leech` work the same in every
  command.
- **The CLI is the API.** Every command has a non-interactive form with JSON
  output, so frontends, shell scripts and LLM agents all use the same
  commands you do. A study loop takes a few lines of shell.
- **Predictable.** One simple rule per decision, no hidden state, and errors
  that say what to do about them. A broken note stops `srl` loudly instead
  of being silently skipped.
- **No LLM inside.** `srl` never calls one. An agent can write questions or
  tutor you through the CLI, but nothing depends on it.

## Install

```sh
cargo install srl
```

`srl` works on Linux, macOS and Windows.

## Quick start

```sh
srl init ~/notes          # make a folder of notes into a store
cd ~/notes
$EDITOR rust/ownership.md # write an item: <!-- srl -->, then Q: and A: lines
srl id                    # give new items their IDs
srl next                  # the next card to study (frontends use srl next --json)
srl review ID --rating 3  # rate it: 1 Again, 2 Hard, 3 Good, 4 Easy
srl stats                 # what's new, due, young, mature
```

Cloze deletions use Anki's syntax (`{{c1::text}}` and `{{c1::text::hint}}`).
Multi-line fields, code blocks and drafts are supported too.

## Frontends

`srl` has no frontend built in: it decides what to study, and a frontend
shows it. Any program that calls `srl next --json` and `srl review` is one.
The repository has two to start from:

- `srl-web`: a local web viewer that renders cards and context as Markdown,
  with a card browser to see how any card looks.
- `srl-tui`: a terminal frontend in Python with Rich.

This shell loop is a frontend too:

```sh
while card=$(srl next in:grammar --json); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r '.prompt_revealed // .prompt, .answer // empty, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  jq -n --argjson card "$card" --argjson rating "$r" '{card: $card.id, rating: $rating}' | srl review --stdin
done
```

## Documentation

The full specification is in
[docs/](https://github.com/ChrisVilches/srl/tree/main/docs). Start with
[overview.md](https://github.com/ChrisVilches/srl/blob/main/docs/overview.md).
Guides to using `srl` and building on it, such as writing a frontend or
operating `srl` with an agent, are in
[docs/guides/](https://github.com/ChrisVilches/srl/tree/main/docs/guides).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
