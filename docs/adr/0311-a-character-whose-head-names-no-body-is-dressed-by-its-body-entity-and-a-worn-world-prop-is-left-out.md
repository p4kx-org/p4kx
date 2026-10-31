# A character whose head names no body is dressed by its body Entity, and a worn world prop is left out

Decided for issue #968. It extends ADR-0219 (the clothing of a character takes the sub-geometry of
its body), is the follow-up of ADR-0297 (which left these items in the bind pose) and applies to
the parts that ADR-0304 hides by their Record. Evidence: the LIVE Archive, the 1441
`NPC_Archetypes` Entities, and the real-Archive tests in
`crates/p4kx-assembly/tests/real_archive/real_generic_npc_clothing.rs`.

## What the data says

Worn `Char_` items of the NPC archetypes resolved to a cardboard box (`box_clothing_1_*.cgf`) or an
armour crate (`crate_armor_*.cgf`), drawn at the origin of the character: 4744 worn items in 1393
of the 1441 archetypes. The cause is neither a missing `SubGeometry` nor a gap in the resolver's
lookup. 3407 `Char_` Entities of the Archive name a world prop as their main geometry and hold
their real models as `SubGeometry` tagged `Male` and `Female`.

ADR-0219 takes the body from the sub-type of the `Char_Head` Entity. The 1393 archetypes wear the
generic head `PU_Protos_Head`, whose sub-type is `UNDEFINED` and which has no model. The body was
unknown, `dress_body` did nothing and the placeholder stayed. Only the 20 story characters with a
head of their own (`*_t1_head`, sub-type `Male` or `Female`) were dressed.

Every one of these characters has a `Char_Body` Entity (`m_body_01`, `f_body_01`) whose sub-type
names the body (`Male`, `Female`) and whose model lies in the folder of the body (`male_v7`,
`female_v2`). 14 prisoners wear `body_01_noMagicPocket` with the sub-type `Fluid`; their model
lies in `male_v7`.

Two `Char_` Entities of the whole Archive (every Entity, not only the NPCs) name a world prop as
their main geometry and no `Male` or `Female` sub-geometry at all. Both are placeholders and
nothing is worn, and no backpack, helmet attachment or accessory is among them:

- `Clothing_Torso2_Unified` (`Char_Clothing_Torso_2`), `box_clothing_1_jacket_a.cgf`, whose only
  sub-geometry is tagged `heldEntity`;
- `bnu_shirt_01_01_01` (`Char_Clothing_Torso_0`), `box_clothing_1_shirt_a.cgf`, without any
  sub-geometry.

A model is a world prop when it lies outside `Objects/Characters` and is no `.cdf`, `.chr` or
`.skin` file; a `.skin` file outside the characters folder is kept.

## Decision

- **The body of a character is the one its head names, else the one its `Char_Body` Entity
  names:** the sub-type of the body Entity when it is `Male` or `Female`, else the body whose
  folder holds the model of the body Entity. A tree with neither is left as it is, as in ADR-0219.
  The head keeps priority. `EntityNode::naming` is the one place that decides; the clothing
  (`dress_body`), the cut of the body skin under the clothing (`BodyCut`), `character_body` and
  `npc_head` all use it, through `body_variant` and `character_body`.
- **A generic head is a head with a body.** `character_body` and `npc_head` take the body from
  `naming`, so a generic-head NPC has a character body and an NPC head. `CharacterBody::named_by`
  tells which Entity named the body, and the `body` of the `entity glb` report says so with
  `named_by` and `named_by_head`. A head that names no model gives no `head` piece.
- **A worn item whose only geometry is a world prop has no geometry once the body is known.**
  Such an item is a `Char_` item without sub-geometry per body whose main geometry is a world
  prop. The Assembly lists the issue `world_prop_not_worn` with the path of the prop, apart from
  `no_geometry` (the Record names none) and `body_geometry_missing`. A part that a worn Record
  hides (ADR-0304) stays hidden by its Record whatever its geometry. An item whose main geometry is
  a character model keeps it. A character whose body is unknown keeps every main geometry.

## Consequences

The generic NPCs of the Entity Assembly wear their `Male` or `Female` clothing, and the body skin
is cut under it, instead of boxes at the origin: no `Char_` item of the 1441 NPC archetypes has a
world prop as its geometry, and every archetype resolves. Items that have no sub-geometry for the
other body, such as a hat with a `Male` variant on a female character, are left out with
`no_geometry`, as before.
