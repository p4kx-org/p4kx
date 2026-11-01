# The Entity Assembly wears the own head skins of a story character by the naming rule of the Armour

Decided for issue #875. It applies the rule of ADR-0109 (the own skins of a story character's head,
`<prefix>_<part>.skin` in the folder of the head's material library) to the Entity Assembly of
ADR-0228. Evidence: `entity glb` and `armour glb` of
`NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart` on LIVE, and the real-Archive test
`crates/p4kx-assembly/tests/real_archive/real_entity_head_skins.rs`.

## The gap

The Armour of a Loadout (`LoadoutResolver::npc_head`) gave the head and the Entities on the head's
ports their own skins; the Entity Assembly did not. For Eckhart the Armour drew
`eckhart_t2_head.skin`, `eckhart_t2_teeth.skin` and `eckhart_t2_eyes.skin`, the Entity Assembly the
generic Protos skins (`protos_human_male_face_t1_pu_*`) that the Records name.

## Decision

One implementation serves both paths (`bespoke.rs`): `Bespoke::of_head` finds the folder and the
prefix from the head Entity's class name and the material library of its geometry, and
`Bespoke::skin_for` names the own skin for a generic one.

- **Where.** After the body is known, the resolver (`EntityNode::wear_own_head_skins`) walks the
  character's `Char_Head` Entity and the Entities on the head's own ports. A part whose geometry
  is a generic head skin gets an `OwnSkin` on its node: the geometry naming the own skin and the
  material library of the head. The node's `geometry` and `fallback`, and so what the port flags
  or the body chose, stay as they were.
- **Archive check.** The resolver does not read the Archive, so it names a skin that may not
  exist: a generic head such as `PU_Protos_Head` gets a name no file has. The Assembly build reads
  the own skin and falls back to the geometry the node had when the Archive lacks the file, and
  from there to the main geometry as before. The absence of an own skin is never reported; every
  other report (`sub_geometry_missing`, `body_geometry_missing`) is unchanged, so a head without
  own skins behaves as before.
- **Material library.** A part that draws an own skin whose geometry names a material library with
  a folder that the Archive lacks takes the library of the head and lists a `material_library`
  issue that does not skip the part, as the Armour does for a part with an own skin. The Armour
  also lists a missing library of a head piece without an own skin and leaves it without one; the
  Entity Assembly lists that library as it does for every part. The library is asked of the
  provider (`ModelProvider::library`); the Armour asks the files of the provider for it
  (`ModelProvider::file`).

## Consequences

- `entity glb` and `armour glb` draw the same head skins for a story character.
- A head whose library names no folder, a tree without a head or without a named body, a hidden
  part, and parts whose skin is no generic head skin (hair, brows, lashes) are unchanged.
- A male generic NPC taking teeth and eyes from the female head folder (#1000) is not touched.
- `Piece::bespoke_in` is gone from the public API; the head pieces of `npc_head` carry the own
  skins as before.
