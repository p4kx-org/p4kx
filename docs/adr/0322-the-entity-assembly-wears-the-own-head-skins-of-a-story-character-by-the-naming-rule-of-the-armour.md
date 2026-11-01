# The Entity Assembly wears the own head skins of a story character by the naming rule of the Armour

Decided for issue #875. It applies the rule of ADR-0109 (the own skins of a story character's head,
`<prefix>_<part>.skin` in the folder of the head's material library) to the Entity Assembly of
ADR-0228. Evidence: `entity glb` and `armour glb` of
`NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart` on LIVE, and the real-Archive test
`crates/p4kx-assembly/tests/real_archive/real_entity_head_skins.rs`.

## The gap

The Armour of a Loadout (`LoadoutResolver::npc_head`) put the head and the Entities on the head's
ports through `Piece::bespoke_in`; the Entity Assembly did not. For Eckhart the Armour drew
`eckhart_t2_head.skin`, `eckhart_t2_teeth.skin` and `eckhart_t2_eyes.skin`, the Entity Assembly the
generic Protos skins (`protos_human_male_face_t1_pu_*`) that the Records name.

## Decision

One implementation serves both paths (`bespoke.rs`): `Bespoke::of_head` finds the folder and the
prefix from the head Entity's class name and the material library of its geometry, and
`Bespoke::skin_for` names the own skin for a generic one.

- **Where.** After the body is known, the resolver (`EntityNode::wear_own_head_skins`) walks the
  character's `Char_Head` Entity and the Entities on the head's own ports. A part whose chosen
  geometry is a generic head skin takes the own skin; the generic geometry stays in `fallback`.
  What the port flags or the body chose before (ADR-0109, sub-geometry by flag) is the generic
  skin the rule starts from, so the shared `PortFlags` of the head keep choosing among the
  sub-geometries as before.
- **Archive check.** The resolver does not read the Archive. The Assembly build reads the own
  skin; one the Archive lacks gives way to the generic skin without an issue, as in the Armour.
  Any other missing sub-geometry is still listed as `sub_geometry_missing`.
- **Material library.** A part with an own skin whose Entity names a material library that the
  Archive lacks takes the library of the head and lists a `material_library` issue that does not
  skip the part, as in the Armour. A part that fell back to the generic skin keeps its library.

## Consequences

- `entity glb` and `armour glb` draw the same head skins for a story character.
- A head whose library names no folder, a tree without a head or without a named body, and parts
  whose skin is no generic head skin (hair, brows, lashes) are unchanged.
- A male generic NPC taking teeth and eyes from the female head folder (#1000) is not touched: the
  rule only replaces a generic head skin of a head that has a folder of its own.
