# An agent that teaches srl

A system prompt and a few skills that make an LLM agent a guide to `srl`.
Start an agent with them, in a folder where you want to study, and ask it
anything: what spaced repetition is, how to write a question, why a card
didn't show up today. It can create a store for you, write example items
from your notes or from a topic you name, and give you the commands to
study them, or quiz you in the chat and record your ratings.

```
agents/
  system-prompt.md              what the agent always knows: the ideas, the
                                common commands, the rules it follows
  skills/
    setting-up-a-store/         stores, configuration, git and syncing
    writing-items/              the full item syntax, cloze, srl add and edit
    studying/                   study sessions, ratings, which card comes next
    selectors/                  the selector language and shell quoting
    fixing-problems/            errors, srl check and lint, lost history
    reviewing-progress/         srl stats, srl log, leeches
```

The system prompt covers the most common use on its own. Each skill is a
folder with a `SKILL.md` whose front-matter (`name` and `description`) says
when it is needed, so an agent that lists skills loads one only when a
question goes further.

## What the agent needs

- `srl` on its `PATH`.
- A shell tool, to run `srl` commands. Without one it can still explain,
  but you run the commands.
- A way to take a system prompt and skills in this format: a folder per
  skill, holding a `SKILL.md` with `name` and `description` in its
  front-matter.

## Starting one

Start the agent in your store (or in the folder that will become one), so
that its commands find it.

With [agi](https://blog.chrisvilches.com/custom-ai-agent/), the system
prompt is a `--sys` file and the skills folder a `--resources` folder:

```sh
agi --sys path/to/examples/agents/system-prompt.md \
    --resources path/to/examples/agents/skills \
    "I'm new to srl. Where do I start?"
```

`agi-repl`, from agi's own examples, takes the same flags and keeps the
conversation going.

With Claude Code, append the system prompt, and copy (or link) the skills
into the store's `.claude/skills/`, where Claude Code looks for project
skills. `srl` never reads folders whose name starts with `.`:

```sh
mkdir -p .claude/skills
cp -r path/to/examples/agents/skills/* .claude/skills/
claude --append-system-prompt "$(cat path/to/examples/agents/system-prompt.md)"
```

Other agents work the same way: give them the system prompt as their
instructions, and the skills however they take skills. One that has no
skills can be given the `SKILL.md` files to read when needed.

## Things to ask

- "I know nothing about srl. Set up a store here and show me how a card
  works."
- "Write 10 questions about `rust/ownership.md`."
- "Make me 5 example cards about the capitals of South America, with cloze."
- "How do I study only my grammar notes?"
- "Quiz me on what's due, and judge my answers."
- "`srl find` says `notes/a.md:12: ...`. What's wrong?"
- "Which cards do I keep forgetting, and how could they be better?"

## What it will and won't do

The prompt has the agent follow the rules in `docs/guides/agents.md`:

- Items it writes are drafts, in notes of their own, with `see:` pointing
  at the note they are about. You read them and `srl accept` the ones you
  want.
- It changes your notes or your schedule (`srl accept`, `srl edit`,
  `srl reset`, `srl suspend`, `srl move`) only when you ask.
- Reviews it records are marked `--by agent`.
- It never writes to `.srl/`.

These are instructions, not guarantees: an agent with a shell can do
anything the shell allows. For an agent you don't fully trust, give it
tools that each run one `srl` command instead of a shell, as
`docs/guides/agents.md` describes, and drop the instructions about the shell.
