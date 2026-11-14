# Ship base data reports partial number reads and the supplier of each Loadout port

Decided for issue #442. Extends ADR-0050 and ADR-0061.

The review of the ship base data (#316) left four rules open: what the hull rule reproduces,
which numbers are read only in part, where each Loadout port comes from, and which characters
count as invisible format characters.

## Decision

- **Hull rule.** The hull sum leaves out parts holding an item port flagged `uneditable` or
  `invisible`, and sums `damageMax` over all remaining parts instead of taking the root part. It
  reproduces no reference tool: it is read from the game data, where those flags mark the item
  ports of thrusters, intakes and other hardware. The sum over the parts rather than the root
  part follows from the vehicle implementation, which states hit points per part and no total.
  Whether a reference tool agrees is open; the real-Archive anchors in
  `docs/catalog.md` are the only comparison.
- **Partial reads.** The prefix rule of ADR-0050 stays, so `12,5x` is still 12 and `1,5` still
  1. A remainder after the number other than white space, `d`, `f` or `kg` (ignoring case) is a
  partial read. The sums list it next to `unreadable` as `partial` with the part, the attribute,
  the raw text and the number read, when the value enters a sum or a threshold; a left-out part,
  a non-positive `damageMax` and an unused `detachRatio` are not listed. Mass and hull each have
  the list for their own attributes. It is additive within version 1 (ADR-0054).
- **Supplier per Loadout port.** Each `LoadoutPort` of `p4kx ships` has `suppliedBy`: the
  Record or XML Loadout file whose entry or port definition supplied the attachment. A Loadout
  entry gives its Record (field `Components/SEntityComponentDefaultLoadoutParams/loadout`) or the
  XML file it was read from, an included file counting as its own file (`kind` `loadoutXml`); a
  `port_default` port gives the Record that defines the port (field
  `Components/SItemPortContainerComponentParams/Ports`); a port nothing names gives `null`.
  `loadoutSource` names the XML file when the ship's Loadout is an XML Loadout. `p4kx-assembly`
  carries the supplier as `PortNode::origin` and `LoadoutOrigin`.
- **Format characters.** `is_format_character` covers the whole of the general category Cf of
  Unicode 16. It lives in `p4kx-foundation` with the shared number parser (ADR-0145).

## Consequences

- A consumer can tell which file to open to see why a port holds an item (#393).
- The ship's Record tree is built once per ship: `LoadoutResolver::resolve_tree` takes the tree a
  caller already holds.
