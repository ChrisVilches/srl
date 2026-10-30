# A Nushell wrapper

`srl.nu` is a Nushell module with one command, `srl`, that runs `srl` and
gives its data as Nushell data, without `--json` or `| from json --objects`:

```nu
srl find in:grammar | where state == due | get id
```

It is a convenience for people who use `srl` from Nushell. Nothing in it is
part of `srl`. Tried with Nushell 0.116 on Linux.

## Loading it

```nu
use path/to/srl.nu
```

Put that line in your `config.nu` to always have it. The wrapper calls the
real `srl` as `^srl`, so `srl` must be on your PATH.

## What it does

- **The commands that read data give Nushell data.** These are the commands
  with `--json` (see [docs/cli.md](../../docs/cli.md), "Conventions"). The
  wrapper adds `--json` to them and parses what `srl` prints. Commands that
  print a list (`find`, `show`, `cards`, `notes`, `kinds`, `log`) give a
  table. `next` and `stats` give a record. Writing `--json` yourself
  changes nothing.
- **`--raw` gives `srl`'s text instead**, for one command:
  `srl find in:grammar --raw`. The wrapper removes it before calling `srl`.
- **"Nothing found" is an empty result, not an error.** `srl` exits with 1
  when nothing matches. Nushell would show that as a failed command. The
  wrapper gives an empty list instead, or `null` for `next` and `stats`.
- **Errors are still errors.** Exit code 2 becomes a Nushell error after
  `srl`'s message has been printed, so `try` and `catch` work.
- **Lists and records piped in are written as JSONL**, one object per line.
  That is the input of `srl add --stdin` and `srl edit --stdin`, so you can
  pipe tables straight in. `srl review --stdin` takes a single review, so
  pipe it a record.
- **Everything else runs as it is.** Other commands (`check`, `add`,
  `review`, `help` and the rest), `--raw`, and `--help` or `-h` on any
  command pass their arguments, output and exit codes through unchanged. So
  `srl find in:grammar --raw` still fails in Nushell when nothing matches.

The wrapper finds the subcommand by skipping options and the value of
`--dir`, so `srl --dir ~/cards next` gives a record too. A `--raw` or
`--json` after `--` is a selector term, as it is for `srl` (see
[docs/selectors.md](../../docs/selectors.md)), and the wrapper leaves it
there.

## Examples

All of these use the wrapper, not the bare `srl`.

### Looking at cards

```nu
# Due cards about grammar, as a table
srl find in:grammar state:due

# How many cards each note has, largest first
srl find | group-by path --to-table
  | each {|g| {path: $g.path, cards: ($g.items | length)} }
  | sort-by cards --reverse

# How many cards are in each state
srl stats | get states

# Cards that have been due for more than a week
srl find state:due
  | where ($it.due | into datetime) < ((date now) - 1wk)
```

### Studying

`srl next` gives a record, so its fields can be read directly:

```nu
let card = srl next in:kanji
if $card == null { print "Nothing left for today" } else {
    print $card.prompt
    input "Press Enter for the answer "
    print $card.answer
    let rating = input "Rating (1-4): " | into int
    {card: $card.id, rating: $rating} | srl review --stdin
}
```

### The log

```nu
# Reviews per day over the last two weeks
srl log
  | where type == review
  | update t { into datetime }
  | where t > ((date now) - 2wk)
  | insert day { $in.t | format date '%Y-%m-%d' }
  | group-by day --to-table
  | each {|g| {day: $g.day, reviews: ($g.items | length)} }

# Cards you have answered Again on most often
srl log
  | where type == review and rating == 1
  | group-by card --to-table
  | each {|g| {card: $g.card, agains: ($g.items | length)} }
  | sort-by agains --reverse
  | first 10
```

### Editing and adding items

`srl show` gives items as `srl edit --stdin` takes them, so an edit
is a table transformation:

```nu
# Add a full stop to every answer of an item that lacks one
srl show k3m9q1
  | update A {|i| if ($i.A | str ends-with '.') { $i.A } else { $i.A + '.' } }
  | srl edit --stdin
```

Add `--dry-run` to see what would change first.

New items can come from any table, for example a CSV file with `Q` and `A`
columns:

```nu
open words.csv
  | select Q A
  | insert draft true
  | srl add generated/words.md --stdin
```

### Text and errors

```nu
# srl's text, as it prints it without the wrapper
srl find in:grammar --raw

try { srl show nosuch } catch { print "no such item" }
```

## Adapting it

The module is about 55 lines. It names the commands it gives data for in
two lists at its top: `DATA`, the commands with `--json`, and `SINGLE`,
those whose `--json` prints one object. They are copied from
[docs/cli.md](../../docs/cli.md), so when `srl` gets a new command with
`--json`, add its name to `DATA`, and to `SINGLE` if it prints one object.
Until then the wrapper runs that command as it is.
