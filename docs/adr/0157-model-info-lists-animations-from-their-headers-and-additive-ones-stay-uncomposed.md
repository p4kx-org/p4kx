# `model info` lists animations from their headers, and additive ones stay uncomposed

Decided for issue #386. Follows up ADR-0046 and ADR-0151.

`p4kx model info` read every `.caf` and `.dba` in the model's folders and decoded every key,
only to print names, durations and counts. #386 asked to list from the headers and to say whether
additive animations can be composed onto a base.

## Decision

- **Header listing.** `AnimationListing` (`p4kx-extraction`) is what `AnimationSet` is for
  `AnimationChoice::All`, with `AnimationHeader` in place of `Animation`: name, path, frame rate,
  end frame, additive flag, the controller IDs of the tracks that have keys, and the number of
  keys they declare. `AnimationHeader::read_caf` and `DbaHeaders::read` read the record, the block
  headers and the controller entries and decode no key. They apply the same limits as the
  decoders (frame rate, signatures, bounds, the key budgets), so a file that fails there fails
  in both. `p4kx model info` (text and `--json`) uses it; the MCP tools and the GUI keep their
  own reads. The selection (folders, `--anim-from`, additive ones listed as unread) is the one
  code path of `AnimationSet`, generic over what a file loads to.
- **What the listing does not notice.** An animation whose keys the decoder refuses (an unknown
  key format) is listed from its header and skipped only when it is decoded for an export. The
  duration is the end frame over the frame rate; the full read takes the later of the end frame
  and the last key, and in the 70,058 animations of LIVE both are equal
  (`every_header_of_the_archive_agrees_with_the_decoded_animation`).
- **Where the cost was.** Before the change a call of `AnimationSet::from_archive` took 105 to
  120 ms on LIVE for every model, also for a folder without an animation, and 7 files or 52 files
  made no difference: the folder lookup normalised and compared the path of every Entry of the
  Archive on one thread. It now compares the folder in place, case-insensitively and without
  allocating, over the Entries in parallel, and takes 4 to 7 ms. The decoding of a folder's files
  is about 1 ms; the header listing saves that and the memory of the keys, which only matters for
  a database of thousands of animations named with `--anim-from`.
- **Checker.** The testkit's GLB checker compares the `min` and `max` of the key times with the
  first and last time (the claim of ADR-0046), not only that they exist.
- **Additive animations stay uncomposed.** ADR-0046 still holds: they are written only when
  named, with the note that the pose needs a base. Composing them needs two facts the data does
  not give. Which pose the offsets apply to (the bind pose, or the matching frame of a base
  animation) and the order of the multiplication (offset times base, or base times offset) are
  engine behaviour; the keys of the 1,475 additive `.caf` files and 1,905 database animations
  lie near the identity (docs/model-animations.md) whichever convention is right, so no
  measurement on the Archive tells the conventions apart, and no field of the records read so far
  names a base. Writing a guess would pose a skeleton wrongly without any sign of it. This is
  reopened if a base is named by a source (an animation graph or `.chrparams` entry) and a
  convention is confirmed against the game.

## Scoped installed source check

LIVE `13220821` and PTU `13260598` each supplied the original controller and
Mannequin animation database at uniquely admitted physical entries. The LIVE
payloads matched the retained CRC-verified copies. The PTU extraction verified
the CRC of both current entries and read 185,457 decoded bytes. The Archive stamps
and manifest digests were stable across the checks.

The animation database was byte-identical across Builds. Both controllers named
the same fragment-definition selector and scope. The matching scope carried two
alternative fragments, each with one direct animation layer, one named clip and one
blend declaration. A direct base clip remains unidentified in the inspected
declarations. Binding the scope to a physical rig and its owning DBA or CAF, and
discriminating the composition order in an observed pose, remain open under #668.

The retained original payloads also contain a separate fragment outside that
single-layer scope. Re-decoding the byte-identical databases found 64 options
per Build. Exactly one option declared two direct animation layers with
different named clips and two blend declarations. Both original controllers
resolve the fragment to a declared scope. Its layer declarations provide no
additive flag or explicit base selector. The scope and context definitions
supply no physical rig or clip path. The option identifies a source candidate
for binding to a rig and original clip headers before comparing a game pose.

On LIVE 13220821 two original Mannequin Preview setups bind the same controller
and root ADB to two physical rig entries across two of the relevant contexts.
The verified root ADB has one original SubADB link to the candidate database
and explicitly admits the selected fragment. This establishes the authored
editor setup for those contexts. Gameplay can bind the contexts separately.

The adjacent CHRPARAMS files do not explicitly name either candidate clip or
include another CHRPARAMS. An exact basename lookup under their CAF masks
found no loose entry and no matching path field in the verified 0x902 CAF
image. The exact physical clip binding in the authored track databases and
the base and multiplication order remain unresolved.

## Consequences

- `AnimationInfo` has public fields and is `#[non_exhaustive]`; `ModelInfo::of_listing` takes the
  listing, `ModelInfo::of` the set.
- The error chain of an unreadable animation file names the file once
  (`cannot read the animation file`, then the cause).
