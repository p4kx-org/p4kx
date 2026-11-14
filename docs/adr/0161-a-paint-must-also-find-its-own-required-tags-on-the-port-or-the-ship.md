# A paint must also find its own required tags on the port or the ship, and the catalog becomes version 2

Decided for issue #434, a follow-up of #399 and PR #428. Applies ADR-0054 to the paint rule of
`docs/catalog.md`.

## Context

A paint fitted a ship when one paint port of the ship required only tags the paint carries. The
paint's own `AttachDef.RequiredTags` were never read, so a paint made for one ship fitted every
ship that shares the port.

## Decision

- A paint also fits a ship only when every tag of the paint's `RequiredTags` is offered by the
  same port: its `portTags`, its `requiredTags` (the Omega port requires `Paint_Omega` after the
  Modification while it lists `Paint_Mustang` as port tag), or the `AttachDef.Tags` of the ship's
  Record. A required tag is met by an offered tag of the same name or by an offered tag that lies
  below it in the tag tree; names compare ignoring case. A paint without `RequiredTags` is
  unaffected.
- Measured on LIVE (4.7): 1,064 paints, 1,039 assigned, 25 unassigned before and after. The
  check removes 40 ship assignments of four paints and no paint loses its last ship. All 40 are
  false positives: the F7A Mk II desert paint on ten F7C variants, the C8X Pisces Expedition paint
  on eight C8 and C8R variants, and two Cutlass Black paints on the eleven Red, Blue and Steel
  variants each. Without `requiredTags` among the offered tags the Omega paint would lose the Omega.
- **`schemaVersion` becomes 2.** The owner decided that the check is a breaking change: it changes
  which ships an existing kind lists (ADR-0054: a changed selection of an existing kind). A
  Basetool that reads version 1 data would otherwise silently mix assignments of two rules, so the
  import must be told. No field changed its name, type or meaning; a v1 reader of the new shape
  fails only on the version check, as ADR-0054 intends.

## Consequences

- The ships are grouped by their paint ports and their Record tags, so a ship with other tags is
  tested on its own.
- The Idris-M still fits the paints of the Idris-P: those paints require no tag of their own.
- The Basetool import must accept version 2 before it reads a P4KX catalog of this version.
