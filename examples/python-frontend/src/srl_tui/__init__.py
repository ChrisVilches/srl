"""srl-tui: a terminal frontend for srl, with Rich.

Meant for Linux, macOS and Windows, but tried on Linux only. All of it is
portable except reading one key without Enter (read_key): termios on
Linux and macOS, msvcrt on Windows, a branch that has never run.

It is a starting point: copy it, rename it, change it. On PATH as
`srl-tui`, it runs as `srl tui [SELECTOR]`.

It runs the frontend loop of docs/frontends.md: `srl next --json` for the
card and its context, `srl review --stdin` for the rating. Keys act at
once, without Enter: space or Enter shows the answer, 1-4 rate, j/k or the
arrows scroll the context, e edits the note, s suspends the card, q quits.
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
        self.parsed = self.parser.parse(markup)


class Lines:
    """Lines already rendered by console.render_lines, printed as they are."""

    def __init__(self, lines):
        self.lines = lines

    def __rich_console__(self, console, options):
        for line in self.lines:
            yield from line
            yield Segment.line()


ARROWS = {"\x1b[A": "up", "\x1b[B": "down"}

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
            arrow = next((a for a in ARROWS if data.startswith(a)), None)
            key = arrow or data[0]
            pending.append(ARROWS.get(key, key))
            data = data[len(key):]
    return pending.pop(0)


def wait_for(keys):
    """Waits until one of the keys is pressed; Ctrl-C and Ctrl-D give q."""
    while True:
        key = read_key()
        if key in ("\x03", "\x04"):
            return "q"
        if key == "\r":
            key = "\n"
        if key in keys:
            return key


class Session:
    def __init__(self, selector):
        self.selector = selector  # the arguments given to srl next
        self.reviewed = self.new = self.again = 0

    def stats(self):
        return Text.assemble(
            (str(self.reviewed), "bold"), " reviewed · ",
            (str(self.new), "bold cyan"), " new · ",
            (str(self.again), "bold red"), " again",
        )

    def header(self, card):
        left = Text.assemble(("srl", "bold"), "  ", (" ".join(self.selector) or "every card", "dim"))
        right = Text.assemble((card["id"], "dim"), "  ")
        if card["state"] == "new":
            right.append("new  ", "bold cyan")
        right.append_text(self.stats())
        grid = Table.grid(expand=True)
        grid.add_column()
        grid.add_column(justify="right")
        grid.add_row(left, right)
        return grid

    def context(self, card, height, scroll):
        """The context panel, at most height lines tall, and the scroll it shows.

        A context too long for the panel shows the lines from scroll on,
        so that the card above it never leaves the screen.
        """
        if card["context"] is None:
            return Padding(Text("no context", "dim"), (0, 2)), 0
        width = console.width - 6  # the borders and the padding
        lines = console.render_lines(CardMarkdown(card["context"]), console.options.update_width(width))
        shown = max(height - 2, 3)
        scroll = max(0, min(scroll, len(lines) - shown))
        subtitle = None
        if len(lines) > shown:
            end = min(scroll + shown, len(lines))
            subtitle = Text(f"{scroll + 1}–{end} of {len(lines)}  j/k scroll", "dim")
        panel = Panel(Lines(lines[scroll:scroll + shown]), title=Text(card["see"] or card["path"], "dim"),
                      title_align="left", subtitle=subtitle, subtitle_align="right",
                      border_style="dim", padding=(0, 2))
        return panel, scroll

    def show(self, card, answered, scroll):
        """Draws the screen for the card, and returns the scroll of its context."""
        header = self.header(card)
        body = [CardMarkdown(card["prompt"])]
        if answered:
            body += [Padding(Rule(style="dim"), (1, 0)), CardMarkdown(card["answer"])]
        panel = Panel(Group(*body), border_style="green" if answered else "cyan", padding=(1, 2))
        if answered:
            keys = Text("  ")
            for number, (name, color) in enumerate(RATINGS, 1):
                keys.append(f" {number} {name} ", f"bold black on {color}").append(" ")
            keys.append("   e edit  s suspend  q quit", "dim")
            # What is left of the screen, under the header, the card, and
            # the blank line and the keys at the bottom.
            height = console.height - 1 - len(console.render_lines(panel)) - 2
            context, scroll = self.context(card, height, scroll)
            screen = Group(header, panel, context, Text())
        else:
            keys = Text("  space answer   e edit  s suspend  q quit", "dim")
            screen = Group(header, panel, Text())
        console.clear()
        console.print(screen)
        console.print(keys, end="")  # no newline: on the last line, it would scroll the screen
        return scroll

    def run(self):
        """Studies until no card is left (returns True) or q is pressed."""
        while True:
            out, code = srl("next", *self.selector, "--json")
            if code == 1:
                return True
            card = json.loads(out)
            answered = False
            scroll = 0
            while True:
                scroll = self.show(card, answered, scroll)
                if answered:
                    key = wait_for({"e", "s", "q", "1", "2", "3", "4", "j", "k", "up", "down"})
                else:
                    key = wait_for({" ", "\n", "e", "s", "q"})
                if key == "q":
                    return False
                if key in ("j", "down"):
                    scroll += 1
                elif key in ("k", "up"):
                    scroll -= 1
                elif key == "e":
                    subprocess.run(["srl", "edit", card["id"]])
                    break  # the note may have changed: get the card again
                elif key == "s":
                    srl("suspend", f"id:{card['id']}")
                    break
                elif answered:
                    self.review(card, int(key))
                    break
                else:
                    answered = True

    def review(self, card, rating):
        srl("review", "--stdin", stdin=json.dumps({"card": card["id"], "rating": rating}))
        self.reviewed += 1
        self.new += card["state"] == "new"
        self.again += rating == 1


def main():
    if not sys.stdin.isatty():
        sys.exit("srl-tui: needs a terminal; in scripts, use srl next and srl review")
    if shutil.which("srl") is None:
        sys.exit("srl-tui: srl is not on PATH; install it first")
    session = Session(sys.argv[1:])
    try:
        # The alternate screen: the session fills the terminal, and leaves
        # it as it was when it ends.
        with console.screen():
            finished = session.run()
    except SrlError as error:
        sys.stderr.write(str(error))
        sys.exit(2)
    console.print(Text("Nothing left to study." if finished else "Stopped.", "bold"))
    console.print(session.stats())
