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
| [`kind-ja-words/`](kind-ja-words/) | `ja_words.py`, a kind program in Python: one card per `{word}` of a Japanese sentence, answered with MeCab's analysis. A proof of concept of kind programs (`docs/cards.md`, "Kind programs"); tried on Arch Linux and Ubuntu; needs a UTF-8 MeCab dictionary. |
| [`kind-llm-generated/`](kind-llm-generated/) | `llm_generated.py`, a kind program in Python: one card whose question and answer an LLM (OpenAI's Responses API) writes anew each time it is shown. Meant for Linux, macOS and Windows but tried on Linux only. |
| [`nushell/`](nushell/) | `srl.nu`, a Nushell module whose `srl` command gives the data commands' output as tables and records without `--json` (`--raw` for text), turns "nothing found" into an empty result, and writes piped tables to `--stdin` as JSONL. Meant for every platform Nushell runs on, but tried on Linux only. |
| [`python-frontend/`](python-frontend/) | `srl-tui`, a study frontend for the terminal in Python with Rich: the card, the answer and the context rendered as Markdown on one screen, with keys without Enter. A uv project of its own, meant for Linux, macOS and Windows but tried on Linux only. |
| [`screenshots/`](screenshots/) | Screenshots of `srl-tui` and `srl-web` studying a demo store, with the store, the scripts that take them again, and `SKILL.md`, instructions for an agent asked to take new ones. Tried on Linux only. |
| [`web-frontend/`](web-frontend/) | `srl-web`, the web viewer: a local server over the CLI and a React page that renders cards and context as Markdown, with a browser that shows any card as studying would, and a form that adds items. An npm package of its own, meant for Linux, macOS and Windows but tried on Linux only. |
