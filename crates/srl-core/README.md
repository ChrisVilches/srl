# srl-core

The core library of [srl](https://crates.io/crates/srl), spaced repetition
for the command line. **If you want to study, install `srl` instead:**

```sh
cargo install srl
```

`srl-core` holds every rule of `srl`: the store, its sources of items
(the built-in drivers, of srl blocks in Markdown notes and of JSONL
notes, and source programs), kinds, cards and cloze deletions, the review
log, FSRS scheduling and selectors. It also runs the kind and source
programs a store names in its configuration: kind programs turn items
into cards, and source programs keep items their own way.
The `srl` command line is a thin layer on top of it.

The library has no CLI code and does no I/O behind your back: it never
prints, and it never reads environment variables, the current directory,
standard input or the clock on its own. Everything comes in as an
argument.

Its API is not stable yet, and may change in any release before 1.0. To
build on `srl` today, the supported interface is the command line: every
command has a non-interactive form with JSON output.

See the [repository](https://github.com/ChrisVilches/srl) for the full
specification.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
