"""srl-tui: a terminal frontend for srl, with Rich.

It is a starting point: copy it, rename it, change it. On PATH as
`srl-tui`, it runs as `srl tui [SELECTOR]`.

It runs the frontend loop of docs/frontends.md: `srl next --json` for the
card and its context, `srl review --stdin` for the rating. Keys act at
once, without Enter: space or Enter shows the answer, 1-4 rate, e edits
the note, s suspends the card, q quits.
"""

import json
import shutil
import subprocess
import sys

from markdown_it import MarkdownIt
from rich.console import Console, Group
from rich.markdown import Markdown
from rich.padding import Padding
from rich.panel import Panel
from rich.rule import Rule
from rich.table import Table
from rich.text import Text
from rich.theme import Theme

# How highlights and headings look; any of Rich's markdown.* styles can be
# changed here.
THEME = Theme({"markdown.mark": "bold yellow", "markdown.h1": "bold magenta"})

RATINGS = [("again", "red"), ("hard", "yellow"), ("good", "green"), ("easy", "blue")]

console = Console(theme=THEME, highlight=False)


class SrlError(Exception):
    """srl failed; its message has been printed on standard error."""


def srl(*args, stdin=None):
    """Runs srl and returns its output and exit code; any code but 0 and 1 is an error."""
    result = subprocess.run(["srl", *args], input=stdin, capture_output=True, text=True)
    if result.returncode not in (0, 1):
        sys.stderr.write(result.stderr)
        raise SrlError
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


def read_key():
    """Reads one key press, without waiting for Enter."""
    if sys.platform == "win32":
        import msvcrt

        return msvcrt.getwch()
    import termios
    import tty

    fd = sys.stdin.fileno()
    old = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        return sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old)


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

    def show(self, card, answered):
        console.clear()
        console.print(self.header(card))
        body = [CardMarkdown(card["prompt"])]
        if answered:
            body += [Padding(Rule(style="dim"), (1, 0)), CardMarkdown(card["answer"])]
        console.print(Panel(Group(*body), border_style="green" if answered else "cyan", padding=(1, 2)))
        if answered:
            if card["context"] is None:
                console.print(Padding(Text("no context", "dim"), (0, 2)))
            else:
                title = Text(card["see"] or card["path"], "dim")
                console.print(Panel(CardMarkdown(card["context"]), title=title, title_align="left",
                                    border_style="dim", padding=(0, 2)))
            keys = Text("  ")
            for number, (name, color) in enumerate(RATINGS, 1):
                keys.append(f" {number} {name} ", f"bold black on {color}").append(" ")
            keys.append("   e edit  s suspend  q quit", "dim")
        else:
            keys = Text("  space answer   e edit  s suspend  q quit", "dim")
        console.print()
        console.print(keys)

    def run(self):
        """Studies until no card is left (returns True) or q is pressed."""
        while True:
            out, code = srl("next", *self.selector, "--json")
            if code == 1:
                return True
            card = json.loads(out)
            answered = False
            while True:
                self.show(card, answered)
                key = wait_for("esq1234" if answered else " \nesq")
                if key == "q":
                    return False
                if key == "e":
                    subprocess.run(["srl", "edit", card["id"]])
                    break  # the note may have changed: get the card again
                if key == "s":
                    srl("suspend", f"id:{card['id']}")
                    break
                if answered:
                    self.review(card, int(key))
                    break
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
        finished = session.run()
    except SrlError:
        sys.exit(2)
    console.clear()
    console.print(Text("Nothing left to study." if finished else "Stopped.", "bold"))
    console.print(session.stats())
