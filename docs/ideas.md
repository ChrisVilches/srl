# Ideas

Changes to `srl` that are not part of the specification yet. They are
recorded here so they are not lost, not as promises. Each would change
`srl`'s data, commands or contracts; ideas for things built on top of
`srl` as it is are in [guides/ideas.md](guides/ideas.md). An idea becomes a
design decision to take, listed in [overview.md](overview.md), "Open
questions", once it is planned.

## Knowledge structure

- **Prerequisites.** Items could name the items they require, for example
  in a header entry. A new header entry changes the item syntax. Finding
  prerequisites in notes, by hand or by an agent, is then work on top.
- **Partial credit for prerequisites.** A correct answer about a card also
  counts, partially, for the cards it requires. Needs a scheduling rule.

## Writing notes

- **Blank space before added items** (low priority). `srl add` puts a
  blank line between a note and the items it appends, and keeps every
  byte already in the note. A note that holds only blank lines, such as
  an "empty" note an editor saved as one newline, is not empty to it, so
  the items start after blank lines; a note that ends in several blank
  lines keeps them all, plus one. Either rule would fix it: a note with
  nothing but blank space counts as empty and the items replace it, or
  trailing blank lines are trimmed so that exactly one blank line comes
  before the items. The second covers both cases, but rewrites blank
  space at the end of the note (see [notes.md](notes.md), "How srl writes
  to notes").

## Stores

- **Export and import between stores.** Move or copy notes from one store to
  another together with the review history of their cards, so that their
  memory state comes along. It needs rules for ID clashes and for merging
  logs. Copying a note by hand moves only the content.
