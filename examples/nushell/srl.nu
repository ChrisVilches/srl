# srl.nu: a Nushell wrapper for srl that turns its JSON into Nushell data.
# Load it with `use srl.nu`, and `srl` is this command. Meant for every
# platform Nushell runs on, but tried on Linux only (Nushell 0.116).

# Commands whose --json prints one object; every other prints a list.
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

# Run srl. With --json, give its output as Nushell data: a table for
# commands that print a list, a record for `next` and `stats`. A list or
# record piped in is written to srl as JSONL, for `add`, `edit` and
# `review` with --stdin.
export def --wrapped main [...args: string] {
    let input = $in
    let stdin = if ($input | describe) =~ '^(list|table|record)' {
        ($input | each { to json --raw } | str join "\n") + "\n"
    } else {
        $input
    }

    # Options are what comes before `--`; after it, everything is a selector.
    let options = $args | take until {|a| $a == '--' }
    if '--json' not-in $options { return ($stdin | ^srl ...$args) }

    let r = $stdin | ^srl ...$args | complete
    if ($r.stderr | is-not-empty) { print --stderr --no-newline $r.stderr }
    # Exit 1 means nothing found: an empty result, not an error.
    if $r.exit_code > 1 {
        error make --unspanned { msg: $"srl exited with ($r.exit_code)" }
    }

    let out = $r.stdout | from json --objects
    if (subcommand $options) in $SINGLE { $out | get 0? } else { $out }
}
