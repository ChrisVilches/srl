# srl.nu: a Nushell wrapper for srl that gives its data as Nushell data.
# Load it with `use srl.nu`, and `srl` is this command. Meant for every
# platform Nushell runs on, but tried on Linux only (Nushell 0.116).

# Commands that have --json (docs/cli.md, "Conventions"). The wrapper adds
# --json to them; any other command runs as it is.
const DATA = [find show cards notes kinds next stats log]
# Of those, the ones whose --json prints one object rather than a list.
const SINGLE = [next stats]

# The subcommand of srl's arguments: the first one that isn't an option
# or the value of --dir.
def subcommand [args: list<string>] {
    mut skip = false
    for a in $args {
        if $skip { $skip = false; continue }
        if $a == '--dir' { $skip = true; continue }
        if not ($a | str starts-with '-') { return $a }
    }
    null
}

# Run srl. The commands that read data (find, show, cards, notes, kinds,
# next, stats, log) give Nushell data: a table, or a record for `next` and
# `stats`; --raw gives srl's text instead. A list or record piped in is
# written to srl as JSONL, for `add`, `edit` and `review` with --stdin.
export def --wrapped main [...args: string] {
    let input = $in
    let stdin = if ($input | describe) =~ '^(list|table|record)' {
        ($input | each { to json --raw } | str join "\n") + "\n"
    } else {
        $input
    }

    # Options are what comes before `--`; after it, everything is a selector.
    let split = $args | enumerate | where item == '--' | get 0?.index | default ($args | length)
    let options = $args | first $split
    let rest = $args | skip $split
    let raw = '--raw' in $options
    let help = '--help' in $options or '-h' in $options
    let options = $options | where $it not-in ['--raw' '--json']
    let command = subcommand $options

    if $raw or $help or $command not-in $DATA {
        return ($stdin | ^srl ...$options ...$rest)
    }

    let r = $stdin | ^srl ...$options --json ...$rest | complete
    if ($r.stderr | is-not-empty) { print --stderr --no-newline $r.stderr }
    # Exit 1 means nothing found: an empty result, not an error.
    if $r.exit_code > 1 {
        error make --unspanned { msg: $"srl exited with ($r.exit_code)" }
    }

    let out = $r.stdout | from json --objects
    if $command in $SINGLE { $out | get 0? } else { $out }
}
