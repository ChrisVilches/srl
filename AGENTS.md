# AGENTS.md

Instructions for coding agents working in this repository.

## Specifications

The specifications for `srl` are the documents directly in [docs/](docs/).
Start with [docs/overview.md](docs/overview.md), which explains the main
ideas and lists the other documents. Read the relevant documents before
changing anything.

`docs/guides/` is not part of the specification: it holds guides to using
`srl` and building on it. Which content goes where is explained in
[docs/overview.md](docs/overview.md), "Specification, guides and examples";
follow it when you add or move content. (`docs/guides/agents.md` describes
how an LLM agent uses `srl`. It is not instructions for working on this
repository.)

## Rules

### Commit after every meaningful unit of work

After each meaningful unit of work, make a git commit. Do not add Claude, or
any other agent, as a co-author: no `Co-Authored-By` trailer and no other
attribution lines in the commit message.

### Keep the implementation platform-independent

`srl` must work the same on Linux, macOS and Windows. Anything that can't be
platform-independent yet gets a `TODO(platform):` comment above the code and
a note in `docs/`. See [docs/rust.md](docs/rust.md), "Platform independence".

### Keep examples/ out of the core

`examples/` holds things built on `srl` that are not part of it (see
[docs/overview.md](docs/overview.md), "Specification, guides and
examples"). Nothing outside it may
depend on it: deleting it must leave `srl` building, testing and running
exactly as before. The rules above for `srl`'s code don't apply there; an
example that works on some platforms only says so in a comment at its top.
When a change to the specifications affects an example, update the example
too.

To take screenshots of the example frontends, read
[examples/screenshots/SKILL.md](examples/screenshots/SKILL.md) first.

### Keep docs/ in sync with the specifications

Whenever the specifications change, update the documents in `docs/` in the
same piece of work so they describe the new specifications. Update the
guides in `docs/guides/` too when the change affects them, as with
examples. When a change fixes a finding of [docs/audit.md](docs/audit.md),
remove the finding in the same piece of work; when you notice a new
problem with the specification that you don't fix, add it there.

The documents are the source of truth for the specifications at all times.
They are not a record of the initial planning to be left behind as the
project moves on. If the code, a decision made in conversation, or anything
else disagrees with `docs/`, either `docs/` is updated or the other thing is
wrong.
