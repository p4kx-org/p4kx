# The Entity Assembly hides the parts at the ports its worn clothing hides, by port name and wearer

Decided for issue #943. It applies ADR-0279 (worn clothing hides the ports its Record names) to the
Entity Assembly of ADR-0228. Evidence: the Loadout tree of
`NPC_Archetypes-Male-Human-MissionGivers-Forge` on LIVE, and the real-Archive test
`crates/p4kx-assembly/tests/real_archive/real_npc_hidden_parts.rs`.

## The ports of the tree

The Armour knows the port every piece sits at (`Piece::at_port`). The tree of an Entity has those
ports already: a part sits at a port of its parent, and the clothing of a character hangs below the
port of its body (`Body_ItemPort/Clothing_Torso_0`, `Body_ItemPort/Armor_Undersuit`). The names a
`HiddenParts` list holds are the names of those ports, so no mapping is needed.

**Decision.** The port of a part is the last name of its port path, compared by name only, ignoring
ASCII case, as in ADR-0279. Two limits keep a tree that holds more than one character, or items
that are no clothing, from being hidden by accident:

- **Types.** Only an Entity whose item type is armour or clothing (`Char_Armor_`, `Char_Clothing_`)
  hides, and only a character item (any `Char_` type: clothing, armour, hair and the other head
  parts) is hidden. A weapon or any other Entity at a port named like a clothing port stays. The
  root and a landing gear sit at no port and are never hidden.
- **Wearer.** A hider affects only the parts of its own wearer: the wearer of a part is the nearest
  body (`Char_Body`) above it, else the root. The hair port sits on the head below the body, so a
  hat still hides it; the hat of one character cannot hide the hair of another, as the crew of a
  ship holds several. The rule is not narrowed to siblings.

## Rules

`Assembly::build` marks a part hidden by the rules of ADR-0279, with the function the Armour uses
(`hide_rule.rs`):

- every Entity hides by its Record, whether it is hidden itself or not, and an Entity never hides
  itself; of several hiders the report names the one with the smallest part name;
- a hidden part is kept with no model, no materials and no zones, as a part on an `invisible`
  port is, so it covers no zone of the body and the body shows where it covered it;
- a part below a hidden part is hidden too, a landing gear of a hidden part with it;
- a hidden part is listed as `part_hidden` with the port and the part that hides it, and counts in
  `Assembly::hidden_parts`.

**Unresolved attachments.** The item type of an attachment that cannot be resolved is unknown, so
only the wearer limit applies to it: it is matched by its port name among the hiders of the wearer
of the Entity whose Loadout holds it, and it inherits the hider of that Entity when the Entity is
hidden. It is then reported as `part_hidden` instead of `unresolved`: it would have no part in
either case, and the garment that hides its port is the reason that matters.

## Consequences

`entity glb`, `entity package`, the hologram export and the GUI's Assembly views leave out the
garments another garment hides, and the body shows where a hidden garment covered it. Ships and
every other Entity are unchanged: only clothing has `HiddenParts`. A garment covered only in part
by another is still drawn whole (ADR-0279).
