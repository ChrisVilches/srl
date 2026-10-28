"""srl-tui: a terminal frontend for srl, with Rich.

Meant for Linux, macOS and Windows, but tried on Linux only. All of it is
portable except reading one key without Enter (read_key): termios on
Linux and macOS, msvcrt on Windows, a branch that has never run.

It is a starting point: copy it, rename it, change it.

It runs the frontend loop of docs/frontends.md: `srl next --json --context
CONTEXT` for the card and its context, `srl review --stdin` for the rating.
Keys act at once, without Enter: space or Enter shows the answer, 1-4 rate,
j/k or the arrows scroll the panel in focus, Ctrl-D/Ctrl-U by half of it,
g/G to its top and bottom, Tab moves the focus between the card and the
context, s suspends the card, q quits.
"""

import json
import os
import shutil
import subprocess
import sys
import time

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
    """Runs srl and returns its output and exit code; any code but 0 and 1 is an error.

    srl can take a while, as when srl next runs a kind program that asks an
    LLM: past a moment, the screen shows a spinner and the seconds taken.
    """
    pipe = subprocess.PIPE
    process = subprocess.Popen(["srl", *args], stdin=pipe, stdout=pipe, stderr=pipe, text=True)
    start = time.monotonic()
    try:
        out, err = process.communicate(stdin, timeout=0.3)  # most calls end here, without a spinner
    except subprocess.TimeoutExpired:
        console.clear()
        with console.status("Loading…") as status:
            while True:
                try:
                    out, err = process.communicate(timeout=0.1)
                    break
                except subprocess.TimeoutExpired:
                    status.update(f"Loading… {time.monotonic() - start:.1f} s")
    if process.returncode not in (0, 1):
        raise SrlError(err)
    return out, process.returncode


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
SCROLL_KEYS = {"j", "k", "up", "down", CTRL_D, CTRL_U, "g", "G"}
QUESTION_KEYS = {" ", "\n", "s", "q"} | SCROLL_KEYS
ANSWER_KEYS = {"1", "2", "3", "4", "\t", "s", "q"} | SCROLL_KEYS
OTHER_KEYS = "   s suspend  q quit"


def scrolled(lines, height, scroll, focused, **panel):
    """A panel at most height lines tall showing the lines from scroll on,
    the scroll it shows, and how many lines it shows.

    panel holds the arguments of Panel; height counts its borders and its
    padding. A panel that can't show every line says which ones it shows,
    and how to scroll it: j/k when it has the focus, Tab to give it the focus.
    """
    shown = max(height - 2 - 2 * panel["padding"][0], 1)  # the borders take 2 lines
    scroll = max(0, min(scroll, len(lines) - shown))
    if len(lines) > shown:
        hint = "j/k scroll" if focused else "tab to focus"
        panel["subtitle"] = Text(f"{scroll + 1}–{scroll + shown} of {len(lines)}  {hint}", "dim")
    return Panel(Lines(lines[scroll:scroll + shown]), subtitle_align="right", **panel), scroll, shown


def split(height, card, context):
    """The heights of the card and the context panels, which would take card
    and context lines, in a space height lines tall.

    Both get what they take when it fits. Otherwise each gets half of the
    space, and one that takes less than half leaves the rest to the other.
    """
    if card + context <= height:
        return card, context
    half = height // 2
    if card <= half:
        return card, height - card
    if context <= height - half:
        return height - context, context
    return half, height - half


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

    def show(self, card, answered, scroll, focus):
        """Draws the screen for the card, with the focus ("card" or "context")
        on one of its panels. Clamps the scroll of each panel to its lines,
        and returns how many lines each one shows.

        The card and its context scroll each inside its own panel, so that
        neither of them pushes the other off the screen.
        """
        options = console.options.update_width(console.width - 6)  # the borders and the padding
        revealed = card["prompt_revealed"] or card["prompt"]
        body = [CardMarkdown(revealed if answered else card["prompt"])]
        if answered and card["answer"] is not None:
            body += [Padding(Rule(style="dim"), (1, 0)), CardMarkdown(card["answer"])]
        card_lines = console.render_lines(Group(*body), options)
        # What is left of the screen once the header (1 line), and the blank
        # line and the keys at the bottom (2) are drawn. The card's panel
        # takes 4 lines more than its text, the context's 2, and "no
        # context" 1.
        height = console.height - 1 - 2
        with_context = answered and card["context"] is not None
        card_height = min(height - (answered and not with_context), len(card_lines) + 4)
        if with_context:
            context_lines = console.render_lines(CardMarkdown(card["context"]), options)
            card_height, context_height = split(height, len(card_lines) + 4, len(context_lines) + 2)
        # The panel in focus has the brighter border.
        color = "green" if answered else "cyan"
        panel, scroll["card"], card_shown = scrolled(
            card_lines, card_height, scroll["card"], focus == "card",
            border_style=color if focus == "card" else f"dim {color}", padding=(1, 2))
        screen = [self.header(card), panel]
        shown = {"card": card_shown, "context": 0}
        if with_context:
            context, scroll["context"], shown["context"] = scrolled(
                context_lines, context_height, scroll["context"], focus == "context",
                title=Text(card["see"] or card["path"], "dim"), title_align="left",
                border_style="white" if focus == "context" else "dim", padding=(0, 2))
            screen.append(context)
        elif answered:
            screen.append(Padding(Text("no context", "dim"), (0, 2)))
        if answered:
            keys = Text("  ")
            for number, (name, color) in enumerate(RATINGS, 1):
                keys.append(f" {number} {name} ", f"bold black on {color}").append(" ")
            if with_context:
                keys.append("  tab focus", "dim")
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
            # No newline: on the last line, it would scroll the screen. Cropped
            # to one line, which the layout above counts on.
            console.print(keys, end="", no_wrap=True, overflow="crop")
        if sync:
            console.file.write("\x1b[?2026l")
            console.file.flush()
        return shown

    def run(self):
        """Studies until no card is left (returns True) or q is pressed."""
        while True:
            out, code = srl("next", "--json", "--context", CONTEXT, "--", *self.selector)
            if code == 1:
                return True
            card = json.loads(out)
            answered = False
            scroll = {"card": 0, "context": 0}  # show clamps them to the lines there are
            focus = "card"
            while True:
                shown = self.show(card, answered, scroll, focus)
                key = wait_for(ANSWER_KEYS if answered else QUESTION_KEYS)
                if key == "q":
                    return False
                if key in (" ", "\n"):
                    # The card starts again from its top, now with the
                    # answer, and the focus goes to the context, if any.
                    answered = True
                    scroll["card"] = 0
                    focus = "card" if card["context"] is None else "context"
                elif key in ("1", "2", "3", "4"):
                    self.review(card, int(key))
                    break
                elif key == "\t" and card["context"] is not None:
                    focus = "card" if focus == "context" else "context"
                elif key in ("j", "down"):
                    scroll[focus] += 1
                elif key in ("k", "up"):
                    scroll[focus] -= 1
                # Half of the lines shown, as in Vim: the lines that were at
                # the bottom are then at the top, so none is skipped.
                elif key == CTRL_D:
                    scroll[focus] += shown[focus] // 2
                elif key == CTRL_U:
                    scroll[focus] -= shown[focus] // 2
                elif key == "g":
                    scroll[focus] = 0
                elif key == "G":
                    scroll[focus] = sys.maxsize
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
    except KeyboardInterrupt:  # Ctrl-C while srl runs, when keys aren't read one by one
        finished = False
    except SrlError as error:
        sys.stderr.write(str(error))
        sys.exit(2)
    console.print(Text("Nothing left to study." if finished else "Stopped.", "bold"))
    console.print(session.counts())
