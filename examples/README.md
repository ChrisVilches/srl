# Examples

What is under here is built on `srl` and is not part of it. Each example is
something useful to have around `srl`: a way to learn it, a convenience, a
starting point for your own tools. They are kept in the repository because a
short working example often says more than a page of documentation.

`srl` does not depend on any of it: delete this folder and `srl` builds,
tests and runs exactly as it did. Nothing here is built, tested or installed
with `srl`, and the rules for `srl`'s own code don't apply here. An example
may be a program, a script, or just files to point another program at. It
may work on some platforms only, in which case it says so in a comment at
its top.

Read an example before you use it, then copy it and change it to suit you.

| Path | What it is |
|---|---|
| [`agents/`](agents/) | A system prompt and skills that turn an LLM agent into a guide to `srl`: it answers questions, writes example items and walks you through study sessions. |
| [`python-frontend/`](python-frontend/) | `srl tui`, a study frontend for the terminal in Python with Rich: the card, the answer and the context rendered as Markdown on one screen, with keys without Enter. A uv project of its own. |
| [`web-frontend/`](web-frontend/) | `srl web`, the web viewer: a local server over the CLI and a React page that renders cards and context as Markdown. An npm package of its own. |
