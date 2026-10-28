"""srl-tui: a terminal frontend for srl, with Rich.

Meant for Linux, macOS and Windows, but tried on Linux only. All of it is
portable except reading one key without Enter (read_key): termios on
Linux and macOS, msvcrt on Windows, a branch that has never run.

It is a starting point: copy it, rename it, change it.

It runs the frontend loop of docs/frontends.md: `srl next --json --context
CONTEXT` for the card and its context, `srl review --stdin` for the rating.
Keys act at once, without Enter: space or Enter shows the answer, 1-4 rate,
j/k or the arrows scroll the context, Ctrl-D/Ctrl-U by half of it, s
suspends the card, q quits.
"""

import json
import os
import shutil
import subprocess
import sys

from markdown_it import MarkdownIt
from rich.console import Console, Group
from rich.markdown import Markdown
from rich.padding import Padding
from rich.panel import Panel
from rich.rule import Rule
from rich.segment import Segment
from rich.table import Table
from rich.text import Text
from rich.theme import Theme

# How highlights and headings look; any of Rich's markdown.* styles can be
# changed here.
THEME = Theme({"markdown.mark": "bold yellow", "markdown.h1": "bold magenta"})

RATINGS = [("again", "red"), ("hard", "yellow"), ("good", "green"), ("easy", "blue")]

# How much of the note to show after the answer: "prose" (the note without its
# items), "full" (with them) or "none" (see docs/notes.md, "Context").
CONTEXT = "prose"

console = Console(theme=THEME, highlight=False)


class SrlError(Exception):
    """srl failed; the message is what it printed on standard error."""


def srl(*args, stdin=None):
    """Runs srl and returns its output and exit code; any code but 0 and 1 is an error."""
    result = subprocess.run(["srl", *args], input=stdin, capture_output=True, text=True)
    if result.returncode not in (0, 1):
        raise SrlError(result.stderr)
    return result.stdout, result.returncode


def highlight(md):
    """==highlight== (docs/cards.md): each `==` is paired with the next one.

    Plugins that treat `==` like `*` nest the marks in text without spaces,
    such as `==[...]==を==[...]==`, so the pairs are found in order.
    """

    def rule(state, silent):
        start = state.pos
        if not state.src.startswith("==", start):
            return False
        end = state.src.find("==", start + 2)
        if end == -1 or end == start + 2 or end + 2 > state.posMax:
            return False
        if not silent:
            pos_max = state.posMax
            state.push("mark_open", "mark", 1)
            state.pos, state.posMax = start + 2, end
            state.md.inline.tokenize(state)
            state.push("mark_close", "mark", -1)
            state.posMax = pos_max
        state.pos = end + 2
        return True

    md.inline.ruler.before("emphasis", "srl_highlight", rule)


class CardMarkdown(Markdown):
    """Rich's Markdown, with ==highlights== in the style markdown.mark."""

    inlines = Markdown.inlines | {"mark"}
    parser = MarkdownIt().enable("strikethrough").enable("table").use(highlight)

    def __init__(self, markup):
        super().__init__(markup)
        self.parsed = self.parser.parse(markup)  # Rich has no way to give it a parser


class Lines:
    """Lines already rendered by console.render_lines, printed as they are."""

    def __init__(self, lines):
        self.lines = lines

    def __rich_console__(self, console, options):
        for line in self.lines:
            yield from line
            yield Segment.line()


ARROWS = {"\x1b[A": "up", "\x1b[B": "down"}
CTRL_C, CTRL_D, CTRL_U = "\x03", "\x04", "\x15"

pending = []  # keys read together, such as a held key repeating, not handled yet


def read_key():
    """Reads one key press, without waiting for Enter; the arrows are "up" and "down"."""
    if sys.platform == "win32":
        import msvcrt

        key = msvcrt.getwch()
        if key in ("\x00", "\xe0"):
            return {"H": "up", "P": "down"}.get(msvcrt.getwch(), "")
        return key
    if not pending:
        import termios
        import tty

        fd = sys.stdin.fileno()
        old = termios.tcgetattr(fd)
        try:
            tty.setraw(fd, termios.TCSANOW)  # not the default TCSAFLUSH, which drops keys typed ahead
            data = os.read(fd, 64).decode(errors="replace")
        finally:
            termios.tcsetattr(fd, termios.TCSADRAIN, old)
        while data:
            key = next((arrow for arrow in ARROWS if data.startswith(arrow)), data[0])
            pending.append(ARROWS.get(key, key))
            data = data[len(key):]
    return pending.pop(0)


def wait_for(keys):
    """Waits until one of the keys is pressed; Enter gives "\\n", Ctrl-C gives q."""
    while True:
        key = read_key()
        if key == CTRL_C:
            return "q"
        if key == "\r":
            key = "\n"
        if key in keys:
            return key


# The keys each side of the card takes, and the end of the bar that lists them.
QUESTION_KEYS = {" ", "\n", "s", "q"}
ANSWER_KEYS = {"1", "2", "3", "4", "j", "k", "up", "down", CTRL_D, CTRL_U, "s", "q"}
OTHER_KEYS = "   s suspend  q quit"


class Session:
    def __init__(self, selector):
        self.selector = selector  # the arguments given to srl next
        self.reviewed = self.new = self.again = 0

    def counts(self):
        return Text.assemble(
            (str(self.reviewed), "bold"), " reviewed · ",
            (str(self.new), "bold cyan"), " new · ",
            (str(self.again), "bold red"), " again",
        )

    def header(self, card):
        left = Text.assemble(("srl", "bold"), "  ", (" ".join(self.selector) or "every card", "dim"))
        right = Text.assemble((card["id"], "dim"), "  ")
        # qa is the default kind; any other is a program, such as one that asks an LLM
        right.append(card["kind"] + "  ", "dim" if card["kind"] == "qa" else "bold magenta")
        if card["state"] == "new":
            right.append("new  ", "bold cyan")
        right.append_text(self.counts())
        grid = Table.grid(expand=True)
        grid.add_column()
        grid.add_column(justify="right")
        grid.add_row(left, right)
        return grid

    def context(self, card, height, scroll):
        """The context panel, at most height lines tall, the scroll it shows,
        and how many lines of the context it shows.

        A context too long for the panel shows the lines from scroll on,
        so that the card above it never leaves the screen.
        """
        if card["context"] is None:
            return Padding(Text("no context", "dim"), (0, 2)), 0, 0
        width = console.width - 6  # the borders and the padding
        lines = console.render_lines(CardMarkdown(card["context"]), console.options.update_width(width))
        shown = max(height - 2, 3)  # the borders take 2 lines
        scroll = max(0, min(scroll, len(lines) - shown))
        subtitle = None
        if len(lines) > shown:
            subtitle = Text(f"{scroll + 1}–{scroll + shown} of {len(lines)}  j/k scroll", "dim")
        panel = Panel(Lines(lines[scroll:scroll + shown]), title=Text(card["see"] or card["path"], "dim"),
                      title_align="left", subtitle=subtitle, subtitle_align="right",
                      border_style="dim", padding=(0, 2))
        return panel, scroll, shown

    def show(self, card, answered, scroll):
        """Draws the screen for the card, and returns the scroll of its context
        and how many lines of it are shown."""
        revealed = card["prompt_revealed"] or card["prompt"]
        body = [CardMarkdown(revealed if answered else card["prompt"])]
        if answered and card["answer"] is not None:
            body += [Padding(Rule(style="dim"), (1, 0)), CardMarkdown(card["answer"])]
        panel = Panel(Group(*body), border_style="green" if answered else "cyan", padding=(1, 2))
        screen = [self.header(card), panel]
        shown = 0
        if answered:
            # What is left of the screen once the header (1 line), the card,
            # and the blank line and the keys at the bottom (2) are drawn.
            height = console.height - 1 - len(console.render_lines(panel)) - 2
            context, scroll, shown = self.context(card, height, scroll)
            screen.append(context)
            keys = Text("  ")
            for number, (name, color) in enumerate(RATINGS, 1):
                keys.append(f" {number} {name} ", f"bold black on {color}").append(" ")
            keys.append(OTHER_KEYS, "dim")
        else:
            keys = Text("  space answer" + OTHER_KEYS, "dim")
        screen.append(Text())
        # The frame is written at once, between the begin and the end of a
        # synchronized update, so the terminal never shows it cleared and
        # half drawn; terminals without synchronized updates ignore them.
        sync = not console.legacy_windows
        if sync:
            console.file.write("\x1b[?2026h")
        with console:  # Rich writes the buffer when the block ends
            console.clear()
            console.print(Group(*screen))
            console.print(keys, end="")  # no newline: on the last line, it would scroll the screen
        if sync:
            console.file.write("\x1b[?2026l")
            console.file.flush()
        return scroll, shown

    def run(self):
        """Studies until no card is left (returns True) or q is pressed."""
        while True:
            out, code = srl("next", "--json", "--context", CONTEXT, "--", *self.selector)
            if code == 1:
                return True
            card = json.loads(out)
            answered = False
            scroll = 0
            while True:
                scroll, shown = self.show(card, answered, scroll)
                key = wait_for(ANSWER_KEYS if answered else QUESTION_KEYS)
                if key == "q":
                    return False
                if key in (" ", "\n"):
                    answered = True
                elif key in ("1", "2", "3", "4"):
                    self.review(card, int(key))
                    break
                elif key in ("j", "down"):
                    scroll += 1
                elif key in ("k", "up"):
                    scroll -= 1
                # Half of the lines shown, as in Vim: the lines that were at
                # the bottom are then at the top, so none is skipped.
                elif key == CTRL_D:
                    scroll += shown // 2
                elif key == CTRL_U:
                    scroll -= shown // 2
                elif key == "s":
                    srl("suspend", f"id:{card['id']}")
                    break

    def review(self, card, rating):
        srl("review", "--stdin", stdin=json.dumps({"card": card["id"], "rating": rating}))
        self.reviewed += 1
        self.new += card["state"] == "new"
        self.again += rating == 1


USAGE = "usage: srl-tui [SELECTOR]...\n"


def selector(args):
    """The selector terms in the arguments, the only thing srl-tui takes.

    A term may start with one `-` (`-in:n5` is a negation), so only an
    argument starting with `--` is an option, and --help is the only one.
    The terms reach srl after `--`, so none of them is read as an option.
    """
    for arg in args:
        if arg == "--help":
            sys.stdout.write(USAGE + "Studies the cards that match SELECTOR (every card without one).\n")
            sys.exit(0)
        if arg.startswith("--"):
            sys.stderr.write(f"srl-tui: unknown option {arg}; it takes only a selector\n{USAGE}")
            sys.exit(2)
    return args


def main():
    terms = selector(sys.argv[1:])
    if not sys.stdin.isatty():
        sys.exit("srl-tui: needs a terminal; in scripts, use srl next and srl review")
    if shutil.which("srl") is None:
        sys.exit("srl-tui: srl is not on PATH; install it first")
    session = Session(terms)
    try:
        # The alternate screen: the session fills the terminal, and leaves
        # it as it was when it ends.
        with console.screen():
            finished = session.run()
    except SrlError as error:
        sys.stderr.write(str(error))
        sys.exit(2)
    console.print(Text("Nothing left to study." if finished else "Stopped.", "bold"))
    console.print(session.counts())
