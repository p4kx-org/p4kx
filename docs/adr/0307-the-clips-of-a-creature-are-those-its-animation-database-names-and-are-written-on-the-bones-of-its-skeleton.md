# The clips of a creature are those its animation database names and are written on the bones of its skeleton

Decided for issue #925, on top of ADR-0275 (the skins of a creature are written bound to the
skeleton of its root). Evidence: the LIVE Archive, `Kopion_CombatPet` and the census of every
Entity whose root geometry is a character file outside the human rigs.

## What the data says

An Entity Record names no clip. Its `SAnimatedCharacterComponentParams` name, in `animDatabase3P`,
a Mannequin animation database (`Animations/Mannequin/ADB/Creatures/Webhound/webhoundAnims.adb`;
the Kopion uses the Webhound's). The database is CryXml; each `Fragment` below its fragment lists
holds `Animation name="…"` elements, and `SubADB File="…"` elements nest further databases. The
name is the file name of a clip without extension (`ai_webhound_stand_alerted_idle_04` is
`ai/ai_webhound_stand_alerted_idle_04.caf`); blend spaces (`1d-bspace_…`) have no `.caf`.

The files lie where the parameter file of the skeleton, `kopion_skel.chrparams` beside
`kopion_skel.chr`, says: an `AnimationList` of `#filepath` folders (the Kopion's is
`Animations/Characters/Creatures/webhound`; a Vanduul's names its own folder first and one per
story chapter after it, so every `#filepath` counts, not the last) and, where there is one, a
`$TracksDatabase` `.dba`.

Census, creature Entities that write a skinned GLB (LIVE): 14 distinct databases. 724 rigged
Entities outside the human rigs name none (weapons, gadgets, ships). Per database the clips found
in the folders and the ones that fit the skeleton: Quasigrazer 75 / 67, Vlk 5 to 45 / 5 to 37,
Webhound 61 / 56, Yormandi 25 / 25, Vanduul 6 / 6, a powersuit 26 / 26. The Aloprat draws no skin,
so nothing is read. A player database (`playerAnims.adb`) names clips none of which is in the
folders of the skeleton it is paired with. The player, Vanduul and powersuit databases nest more than
64 databases, so the limit reports them. The GLBs with clips are 8 to 44 MB.

## Decision

- **A creature gets the clips its animation database names.** `creature_animations` reads the
  database and the databases nested in it (at most 64, breadth first, each name once), finds the
  skeleton's parameter file (the skeleton file itself, or the one a character definition names),
  and reads the clips below all its `#filepath` folders and its `$TracksDatabase`. A name no clip
  has is filtered out against the file names below the folders before the clips are read, so a
  blend space costs nothing (with a track database below them the names show only inside it, and
  a name it lacks costs a second read). Nothing is read
  for an Entity that names no database or whose Assembly writes no skinned creature.
- **A clip is written on the bone nodes.** `Assembly::animate` fits the clips to the skeleton of
  ADR-0275: an additive clip, a clip with no track for a bone of the skeleton and a clip beyond
  the budget of `MAX_ANIMATION_VALUES` are left out and listed as `animation_not_used`, one
  Issue per reason. `Scene::with_animations` of `p4kx-gltf` writes the rest as glTF animations
  whose channels target the bone nodes, as the armour and model writers do; a scene without a
  skinned mesh has no bone nodes and writes none.
- **What cannot be found is a note.** A database, parameter file or folder the Archive lacks or
  cannot read is `animations_unavailable` on the root, `skipped` false, and the creature is
  written without clips.
- **Limits are reported.** At most 64 databases, 65,536 animation names and, per parameter file,
  64 folders and 64 track databases are kept (the last folder or database is always the file's
  last); what a limit drops is an `animations_unavailable` note, never silent.
- **Memory.** The clips are decoded in full before they are fitted, so the memory of a run is
  bounded by the clips of the Archive the names select, not by the budget of values. The budget
  of `MAX_ANIMATION_VALUES` applies when the clips are fitted for writing. A cap on decoded keys
  needs a change to the animation reader and is not part of this decision.
- **`entity glb --animations` writes the clips**; without it the GLB stays static, as `chf glb`,
  `character package` and `armour glb` do (owner's decision on #925). The report's
  `animations` lists them as `model glb` does.
- **`entity package` stays static**, as in ADR-0275.

## Consequences

Facial and procedural animation (the `ProcLayer`, aim and look poses, which are additive) and
blend spaces are not written; rigid parts on a bone follow the bone only in a viewer that parents
them to it (#881). A clip named by a database of the human rigs is not read: the rule is the
creature rule of ADR-0275.
