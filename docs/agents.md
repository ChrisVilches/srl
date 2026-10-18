# Agents

`srl` contains no LLM and does not depend on one. An agent relates to a store
the way it relates to a folder of notes: it can open it, read it, add to it and
analyze it, but the store is complete without it.

This document describes how an agent fits around `srl`, using
[agi](https://blog.chrisvilches.com/custom-ai-agent/) as the example. Any other
agent that can run programs works the same way. Nothing here is part of `srl`
itself.

## How an LLM can touch `srl`

There are exactly three ways, and in all of them the core only sees programs
and JSON:

| Way | Direction | Example |
|---|---|---|
| **An agent runs CLI commands** | Agent → `srl` | Adds draft atoms from an article; prepares a session; tutors you through `srl next` and `srl review`. |
| **A drill calls an LLM** | `srl` → drill program → LLM | `srl-drill-llm` writes application questions for atoms with an `llm:` facet. |
| **The external grader calls an LLM** | `srl` → grader command → LLM | `srl-grade-agi` judges sentences you wrote for `cmd` cards. |

## Rules for agents

1. **Agents create drafts.** Atoms written by an agent use `srl add --draft`.
   They enter your study queue only after `srl accept`.
2. **Reviews go through `srl review`.** An agent never writes to the log, and
   marks its reviews with `--by agent` (or `"by": "agent"`).
3. **What an agent writes is either a draft, an edit you can see in the atom
   file, or a disposable cache** (`generated/`, `sessions/`). Nothing it does is
   hidden.

These rules are enforced by giving the agent only the tools it needs, not by
`srl`.

## Tools

The safest setup gives the agent **no general tools** (no shell, no file
writing) and only small tools that each run one `srl` command. Then the agent
cannot do anything `srl` would not accept, and even a weak model is safe to
use. For example:

| Tool | Runs |
|---|---|
| `srs_find`, `srs_show`, `srs_tags`, `srs_drills`, `srs_log`, `srs_stats` | Reading |
| `srs_add` | `srl add --draft --stdin` (always draft) |
| `srs_edit` | `srl edit ID --stdin` (only if you want agents to edit atoms) |
| `srs_next`, `srs_grade`, `srs_review` | Tutoring (`srs_review` always passes `--by agent`) |
| `srs_session_put`, `srs_generated_put` | Preparing sessions and caches |
| `read_file` | Reading source material (notes, articles) |

There is deliberately no `srs_accept` tool. Which tools a given run gets is a
choice of the script that starts the agent.

## Instructions

Most of what the agent needs to know fits in a system prompt, which avoids a
round trip to load a skill:

| Layer | Contents |
|---|---|
| Tool descriptions | What each tool does and its arguments. |
| Base prompt | Atoms, facets, drafts, ratings 1–4, the selector language, the rules above. |
| Role prompt | One workflow: ingest, prepare, tutor, enrich, or analyze. |
| Personal context | Your goals, where your notes are, your languages. |
| Skills (rare) | Long procedures used occasionally, such as conventions for Japanese atoms. |

Drill documentation comes from `srl drills` and `srl-drill-NAME --help`, so the
prompts don't need to change when you add a drill.

With agi, a run combines these with repeated `--sys` flags and `--resources`
folders, usually inside a small wrapper script per role:

```sh
agi --sys base.md --sys tutor.md --sys me.md --resources srl-agent/tutor "…"
```

## Workflows

### Ingest: any material becomes draft atoms

```sh
srl-agent ingest "read ~/notes/Japanese/Tobira.md chapter 3 and turn it into atoms"
srl-agent ingest "turn https://… into atoms, at most 8"
srl-agent ingest "atoms for the 20 most common N1 grammar points about contrast"
```

The agent reads the material, runs `srl tags` to reuse existing tags, runs
`srl find` to avoid duplicates, picks facets from `srl drills`, and sends
JSONL to `srl add --draft --stdin`. It records where the material came from in
`source`.

### Prepare: a request in natural language becomes a session

"Test me on JLPT N1 grammar using food kanji, 15 questions":

1. Map the words to real tags with `srl tags`: `jlpt/n1`, `kanji/food`.
2. `srl find tag:jlpt/n1 state:due,new --json` for the atoms to cover.
3. Write cards that use the grammar with food vocabulary, in the card format.
4. `srl session put n1-food`.

You then study it with `srl play n1-food`, with or without an LLM.

### Tutor: an interactive session

With agi, each of your replies is one turn in a saved session (for example
through `agi-repl`). The agent calls `srl next`, asks, judges your answer, and
records `srl review --by agent`. It can probe a mistake with follow-up
questions, and propose a new draft atom for a misconception it found.

### Enrich: more variety, studied offline

A batch job (for example nightly) gives due atoms more material:

- more sentences in `cloze:`, more items in `cards:`, more `confusable:` links,
  through `srl edit`;
- or ready-made cards in `generated/`, through `srl generated put`.

Studying then needs no LLM at all.

### Analyze

The agent reads `srl stats`, `srl find state:leech` and `srl log` to report weak
areas, and proposes rewrites of leeches as edits or new drafts.

## The grader script

The `cmd` grader can be a few lines around agi:

```sh
#!/bin/sh
# srl-grade-agi: reads {card, response, atom} on stdin, prints {correct, rating, feedback}
exec agi --rm --sys ~/srl-agent/prompts/grader.md \
  --schema 'correct:bool:"Is the answer acceptable?", rating:int(1..4):"FSRS rating", feedback:str:"One or two sentences"'
```

`correct` comes back as a single boolean here; the grader prompt (or a small
`jq` step) wraps it in a list to match the protocol in
[grading.md](grading.md).
