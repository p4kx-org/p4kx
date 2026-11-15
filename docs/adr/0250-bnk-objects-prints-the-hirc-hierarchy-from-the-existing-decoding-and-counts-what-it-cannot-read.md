# `bnk objects` prints the HIRC hierarchy from the existing decoding and counts what it cannot read

Decided for issue #860, on top of ADR-0115 and ADR-0158 (the `HIRC` decoding) and in the manner of
ADR-0133 (unsupported is not corrupt).

## Decision

- **One parser.** `p4kx bnk objects` prints the objects `p4kx-audio` already decodes. No second
  reader of the `HIRC` section exists. `p4kx-audio` gains only accessors on `HircObject`:
  `layout()`, `parent_id()` and `children()`, plus `HircKind::slug()`,
  `MusicPlaylist::item_depths()` and the enums `MusicPlayMode` and `MusicGroupKind` with
  `MusicPlaylistItem::mode()` and `MusicTrackSwitch::group_kind()`, so that other front-ends
  share the names of the codes. `has_decoder` (which types the reader decodes) is the one list
  behind the `Unsupported` and `Undecoded` split; a test builds every decoded type and checks
  both.
- **Four layouts, counted over the whole bank.** `HircLayout` tells how far an object is read:
  `Decoded`, `Undecoded` (a type with no decoder, e.g. actor-mixers, buses and states),
  `Unsupported` (a decoded type in a bank version whose layout is unknown, either
  `UnsupportedObjectLayout` or a body kept raw because of the version) and `Malformed` (the body
  does not hold its fields). The summary counts all four for the whole bank, whatever `--type`,
  `--id` and the page select, so a filtered view still shows what was not read. None of them fails
  the command: the exit code stays 0 for a readable bank.
- **Bounded output.** Objects come in bank order, `--limit` (1 to 10 000, default 100) at a time
  from `--offset`; the summary gives the next offset. The size of one object's listing is bounded
  by its body, which the parser already validated against the section; nothing is allocated from a
  declared size in the new code.
- **Playlist nesting is not validated, so the text caps it.** The bank states how many child
  items follow each playlist item and the parser does not check that the counts form a tree, so
  the depth of an item can be as large as the item count. `item_depths` keeps a stack no longer
  than the item list, the JSON gives the depth as a number, and the text draws at most 8 levels
  of indentation and marks deeper items with `+N`; the output stays linear in the bank size.
- **Parent 0 is no parent.** A node at the top has parent ID 0 in the bank; the output shows it
  as none (`-` in text, `null` in JSON), as it does for objects without a parent field.
- **A music track's children are its sources' media IDs**, not object IDs, because a source is
  not an object of the section.
- **The JSON shape** is documented in `docs/cli-json.md` and fixed by a snapshot test; the command
  has no JSON Schema file, like `bnk list`.

## Consequences

- The JSON gives the playlist loop fields as the bank stores them (`loops`, `loop_min`,
  `loop_max`). The text writes `loops endless` for 0, else `loops N`, and adds `(range a..b)`
  when either bound is not 0; it does not interpret how the fields combine.
- A decoded type that the reader reads in version 154 only (containers, music objects) is
  `unsupported` in a bank of another version, and every decoded type is in a bank above 154; such
  an object is listed with type, ID and status but no parent, children or detail.
- `Decoded` means the fields of the type were read. Containers and music objects must also use
  their whole body; a sound, action or event is not checked for bytes after its fields.

## Attenuation

Version 154 HIRC type 14 uses the height-spread flag followed by the cone flag.
The second flag's low bit controls the presence of five words. The source
layout then carries 19 signed selectors, ordered conversion curves and initial
RTPCs. Independent span checks against installed LIVE and PTU distinguish this
layout from the pinned StarBreaker layout, which starts at the cone flag.
[wwiser layout](https://github.com/bnnm/wwiser/blob/24eb88f3b2a99d54c1062d8438a7bd04be88335d/wwiser/parser/wparser.py),
[StarBreaker layout](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/attenuation.rs)

The slice reader uses the shared attenuation scanner for whole-body coverage.
ObjectView, CLI, MCP and Media replay the checked byte ranges through the
same graph-point and initial RTPC readers. They retain source bits, signed
selectors and ordered unknown codes. An incomplete supported body reports
its checked prefix and original remaining bytes. Other versions stay opaque.
[Attenuation data contract](../audio-attenuation.md)
