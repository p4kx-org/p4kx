# TXTP files render a resolved Wwise event as text for vgmstream

Decided for issue #307, on top of ADR-0115 and ADR-0158 (the event resolution and the `HIRC`
decoding) and ADR-0250 (the object kinds). The format is that of vgmstream's `doc/TXTP.md`.

## Decision

- **Text only.** `p4kx wwise extract --txtp` writes `.txtp` text files next to the exported
  sounds. P4KX neither bundles nor runs vgmstream, and the syntax was read from its documentation;
  nothing of wwiser or vgmstream is copied.
- **No second parser.** `p4kx_assembly::audio::write_txtp` works on the variants of a resolved
  trigger (`SoundVariant::path`), not on the bank. Variants of one Wwise event come in depth-first
  order, so the tree of objects is rebuilt from their paths: a path step equal to the last open
  child is the same object. A Sound object is a leaf; a music track holds its sources as leaves.
- **One file per event** that leads to sounds, named after the event (`<event>.txtp`) by the
  rule of the sound files, which is the portable file-name rule of ADR-0248: the name is escaped
  with `escape_part` (`event` when it is longer than 228 bytes) and a name taken under the case
  fold of that rule gets `_2`, `_3`, …. The file lines name the exported files, which follow the
  same rule and lie in the same folder, so every path in a TXTP is a bare file name. The
  path-length limit of ADR-0248 is applied to every TXTP file (`check_path_length` on the
  absolute path): a file over the limit is listed in the report for its event and the run goes
  on. The sound export applies the same rule and limit (issue #879).
- **Layout.** File lines are indented by two spaces per nesting level, which the TXTP document
  allows before files and commands. A `group` line carries only the macro `#@layer-v`; the
  comment about the object, and for a choice how to pick another child, is the line above it.
- **How objects combine.**

  | Object | TXTP |
  |---|---|
  | Event (its actions), blend container, music segment, music track | `group = -L<n> #@layer-v` |
  | Sequence container, music playlist | `group = -S<n>` |
  | Random container, switch container, music switch | `group = -R<n>>1` |

  A group of one child is the child. A random or switch container is settled at run time in the
  game. A rendered group of several alternatives selects its first child. The header names
  this behavior with `# approximation: select-first-alternative`. The same code appears in the
  file's JSON `approximations` array after writing or verifying the retained bytes.
- **Loops come from the WEM.** The loop of the WEM (`smpl` chunk, issue #95) is written as
  `#I <start> <end>` in frames, the values the sound file carries as `smpl` or `LOOPSTART` too.
  `ExportedSound` gains `loop_points` and `prefetch_only` for this; the header of the WEM is
  parsed for the loop whatever the format written, so `--format raw` keeps it.
- **A broken link makes the file incomplete, not absent.** The events of a trigger that the walk
  broke on (a missing object, a path too deep, too many children) are written for what was
  reached, with the header comment `incomplete: <n> broken links`; `TxtpFile::breaks` and the
  JSON `breaks` of each file count them, and the CLI summary counts the incomplete files.
- **No guess where a file is missing.** An event with a sound that has no whole file (plug-in
  or external source, a WEM no soundbank holds, only the prefetched start of a streamed WEM) gets
  no TXTP and is listed with the reason in the report; the exit code is that of the sound export,
  which already lists such sounds as failed.
- **Names from the Archive stay in comments.** The header comment holds the event and trigger
  name with control characters replaced, so a name cannot start a TXTP line of its own.

## Consequences

- TXTP arranges the resolved sound files. Music clip positions, trims, transitions and authored
  container or playlist loop counts remain source metadata. A music event plays its tracks as
  layers in the generated TXTP.
- The `approximations` array describes the generated commands for each file. An empty array
  means the writer emitted none of the reported approximation codes. A `null` value means the
  retained bytes were unchecked or differed from the generated text.
- The segments of an `-S` group are decoded by vgmstream one after another: it takes the highest
  sample rate of them, and segments of different channel counts may need mixing (a mono segment
  next to a stereo one will not sound right without it), so a sequence of sounds with unequal
  format may play differently from the game.
- `ExportedSound` has two new public fields; constructing one outside the crate needs them.
- The JSON of `wwise extract` has an optional `txtp` object only with `--txtp`.

## Considered Options

- One TXTP per selectable child of a random container, as wwiser can write them: many files per
  event, and the choice is a run-time one; not done, the `>1` in the file is edited by hand.
- Re-reading the bank to walk the objects again for the TXTP: a second reader of the `HIRC`
  section; not done.
- Writing a silent `?.` entry for a source without a file: plays a rendition that differs from
  the game without saying so; not done.

The render tree groups sounds by the open visits of the resolved source
walk. Repeated authored edges contribute distinct visits in source order.
Separate physical holders of one logical Event contribute independent groups.
