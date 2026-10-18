# AGENTS.md

Instructions for coding agents working in this repository.

## Specifications

The specifications for `srl` are in [docs/](docs/). Start with
[docs/overview.md](docs/overview.md), which explains the main ideas and lists
the other documents. Read the relevant documents before changing anything.

(`docs/agents.md` is part of the specification: it describes how an LLM agent
uses `srl`. It is not instructions for working on this repository.)

## Rules

### Commit after every meaningful unit of work

After each meaningful unit of work, make a git commit. Do not add Claude, or
any other agent, as a co-author: no `Co-Authored-By` trailer and no other
attribution lines in the commit message.

### Keep docs/ in sync with the specifications

Whenever the specifications change, update the documents in `docs/` in the
same piece of work so they describe the new specifications.

The documents are the source of truth for the specifications at all times.
They are not a record of the initial planning to be left behind as the
project moves on. If the code, a decision made in conversation, or anything
else disagrees with `docs/`, either `docs/` is updated or the other thing is
wrong.
