# Agents

`srl` contains no LLM and does not depend on one. An agent relates to a store
the way it relates to a folder of notes: it can open it, read it, add to it and
analyze it, but the store is complete without it.

This document describes how an agent fits around `srl`, using
[agi](https://blog.chrisvilches.com/custom-ai-agent/) as the example. Any other
agent that can run programs works the same way. This is a guide, not part of
the specification (see [overview.md](../overview.md), "Specification,
guides and examples"): nothing here is part of `srl` itself.

## How an LLM touches `srl`

In one way only: **an agent runs CLI commands and writes notes**, as a
person does. Everything an agent writes is an ordinary item in an ordinary
note. There is no separate kind of generated card: once accepted, an item
an agent wrote is scheduled like one you wrote.

`srl` never judges answers. When you want an LLM to judge a free-form answer,
the agent does it, outside `srl`, and records the rating it decided.

## Rules for agents

1. **Agents write drafts.** Items written by an agent are drafts
   (`"draft": true` in `srl add`'s input). They enter your study queue
   only after `srl accept`.
2. **Agents write new notes, not into yours.** Items an agent generates go
   into a note of their own, such as `generated/wo-mono-tomo-sezu.md`, with
   `see:` naming the note they ask about. They then show your note as their
   context, and your note stays as you wrote it. The one exception is
   `srl edit ID --stdin`, for an agent you allow to edit your items.
3. **Reviews go through `srl review`.** An agent never writes to the log, and
   marks its reviews with `--by agent` (or `"by": "agent"`).

These rules are enforced by giving the agent only the tools it needs, not by
`srl`.

## Tools

The safest setup gives the agent **no general tools** (no shell, no file
writing) and only small tools that each run one `srl` command. Then the agent
cannot do anything `srl` would not accept, and even a weak model is safe to
use. For example:

| Tool | Runs |
|---|---|
| `srs_find`, `srs_show`, `srs_cards`, `srs_log`, `srs_stats`, `srs_notes`, `srs_kinds` | Reading (`srs_notes` and `srs_kinds` tell which notes and which fields an item can have) |
| `srs_add` | `srl add NOTE --stdin` (always draft: the tool sets `"draft": true` on every line, for example with `jq -c '.draft = true'`) |
| `srs_edit` | `srl edit ID --stdin` (only if you want agents to edit items) |
| `srs_next`, `srs_review` | Tutoring (`srs_review` always passes `--by agent`) |
| `read_file` | Reading source material (notes, articles) |

There is deliberately no `srs_accept` tool. Which tools a given run gets is a
choice of the script that starts the agent.

## Instructions

Most of what the agent needs to know fits in a system prompt, which avoids a
round trip to load a skill:

| Layer | Contents |
|---|---|
| Tool descriptions | What each tool does and its arguments. |
| Base prompt | Notes, items, the `qa` kind and cloze deletions, drafts, ratings 1–4, the selector language, the rules above. |
| Role prompt | One workflow: generate, tutor, enrich, or analyze. |
| Personal context | Your goals, how your notes are arranged, your languages. |
| Skills (rare) | Long procedures used occasionally, such as conventions for Japanese items. |

With agi, a run combines these with repeated `--sys` flags and `--resources`
folders, usually inside a small wrapper script per role:

```sh
agi --sys base.md --sys tutor.md --sys me.md --resources srl-agent/tutor "…"
```

## Limitations of the item syntax for LLMs

Items are written in srl blocks, fenced code blocks whose language is `srl`
(see [notes.md](../notes.md), "The item syntax"). For a model, that is
mostly easy: fenced blocks are familiar, a block says where its items end,
a mistake inside a block is an error that names its line, and `srl show`
prints an item as one block that `srl edit ID --stdin` takes back. Three
things are harder, and the first can lose text without any error.

1. **Fences inside fields.** A field that holds a fenced code block needs
   its srl block to be opened with a longer fence, or with tildes, and
   models often get fence lengths wrong. With too short a fence, the
   code's closing fence closes the srl block early, and what follows is
   read as text of the note:
   - in a `qa` item without cloze deletions, the `A:` is cut off: `srl
     check` reports the item without an answer, and `srl lint` the `A:`
     line outside a block;
   - otherwise the item is cut short **without any error**: in a `Q` with
     cloze deletions, or in the fields of a kind program, which `srl lint`
     doesn't look at. Worse, the block's own closing fence, left over,
     opens an ordinary code block, which hides the srl blocks after it
     until the next fence closes it: their items silently disappear.
2. **Two forms of block.** A block holds one item, with its entries on the
   fence, or several, each starting with an `@` line, never both. A model
   that mixes them, for example by adding an `@` item to a block of one, gets an
   error, which says what to do; the rule is one more to get right.
3. **Fences inside other text.** An item shown inside a Markdown reply sits
   inside another fenced block, which needs a longer fence again. In a
   shell, backticks start command substitution, so an item given to
   `srl edit ID --stdin` through a heredoc needs a quoted delimiter
   (`<<'ITEM'`) to reach `srl` as written.

What keeps them small:

- **Write new items with `srl add`.** Its input is JSON, with no item
  syntax at all, and it picks a fence long enough for the fields.
- **Open blocks written by hand with `~~~srl`** when a field holds code:
  a tilde fence is never closed by backticks, so there is nothing to count.
- **Check after writing**, with `srl check` and `srl lint`, and compare
  what `srl show ID` prints with what was meant: a cut item shows there
  with fewer fields, or shorter ones. `srl find` tells whether every item
  written is there.
- **Give the agent tools instead of a shell** (see "Tools"): the tool
  passes standard input as it is, so there is no quoting to get wrong.

## A ready-made guide: `examples/agents/`

The repository's `examples/agents/` (see [overview.md](../overview.md),
"Specification, guides and examples") is a base prompt and a set of skills
for one role: a guide that teaches `srl` to someone who has never used it.
It answers questions, sets up a store, writes example items as drafts,
tells the person which frontend to run, or runs a study session in the
chat. The system prompt covers the common uses; each skill's front-matter
says when to load it for the details (the item syntax, studying,
selectors, fixing problems, progress). Its `README.md` shows how to start
it with agi and with other agents.

It assumes an agent with a shell, which is the simplest way to start, and
asks it in its instructions to follow the rules above. With a shell, those
are instructions and not guarantees; the tools-only setup above is the one
that enforces them.

## Workflows

### Generate: new questions about a note

```sh
srl-agent generate "10 application questions about japanese/grammar/wo-mono-tomo-sezu.md"
srl-agent generate "turn https://… into questions, at most 8"
```

The agent reads the note, runs `srl find` and `srl show` to see which items
already exist, and sends the new items as JSONL to:

```sh
srl add generated/wo-mono-tomo-sezu.md --stdin
```

Each line has the item's fields, `"draft": true`, and
`"see": "japanese/grammar/wo-mono-tomo-sezu.md"`, which makes your note the
context of the new item. Questions about several notes can go in one call,
each line with the note it asks about. You read the new
note, delete what you don't want, and run
`srl accept path:generated/wo-mono-tomo-sezu.md`.

A request such as "test me on N1 grammar using food vocabulary" is the same
workflow: the agent writes the questions as draft items, and you accept the
ones worth keeping.

### Tutor: an interactive session

With agi, each of your replies is one turn in a saved session (for example
through `agi-repl`). The agent acts as the frontend: it calls `srl next
--json`, which gives the card with its context, and asks. What happens next is
up to your instructions, for example:

- you answer in your own words, the agent judges the answer and records its
  rating, with your answer as `response`;
- the agent shows the answer and the context, you give the rating, and the
  agent records it, with no `response`.

Either way it records `srl review --by agent`. It can probe a mistake with
follow-up questions, and propose a new draft item for a misconception it
found.

A quiz that should not touch the schedule needs no `srl` at all: the agent
asks questions in the chat.

### Enrich: more variety, studied offline

A batch job (for example nightly) writes more draft items for notes whose
cards keep lapsing, or adds cloze deletions to existing items with
`srl edit ID --stdin`. Studying then needs no LLM at all.

Adding the first cloze deletion to an item changes its card ID (`x7k2m9`
becomes `x7k2m9-1`), which leaves the old card's history orphaned. The job
should run `srl check` afterwards and report the `srl move` commands it
suggests, for you to run (see [scheduling.md](../scheduling.md), "Moving
history").

### Analyze

The agent reads `srl stats`, `srl find state:leech` and `srl log` to report weak
areas, and proposes rewrites of leeches as edits or new drafts. Where reviews
kept a `response`, it can also look at what you actually answered.
