# A creature without a body port takes the first alternative of its generated Loadout

Decided for issue #882, on top of ADR-0255 (a creature without files is a listed gap) and
ADR-0275 (the skins of a creature are written bound to the skeleton of its root). Evidence: the
LIVE Archive, `Quasigrazer_Tundra` and `Aloprat`, and the census of
`real_creature_census.rs`.

## What the data says

Creature base Entities (`libs/foundry/records/actor/actors/*`) whose root geometry is a skeleton
(`.chr`) draw nothing when their body port is unset. The census of #859 listed `Aloprat` and the
`Quasigrazer*` variants.

- **The Quasigrazers have a choice, and the Assembly did not read it.** Their default Loadout is
  `SItemPortLoadoutRandomGeneratedParams`, not manual entries: it names `SLoadoutAssortment`
  Records (`libs/foundry/records/loadoutkits/pu_loadoutbundles/creature_loadoutbundle/*`). An
  assortment has groups (`GroupedLoadouts`, e.g. `Body` and `TorsoAttachments`), each with
  alternative Loadouts, here XML Loadouts of `Scripts/Loadouts/Character/Creatures/Qausi_graser`.
  The game picks one alternative per group at random. Those XML files are in the Archive and put
  the body skin (`m_body_quasigrazer_*`) on `Body_ItemPort` and the eggs on the egg ports.
- **`Aloprat` has no choice in the Archive.** Its base Loadout is the XML file
  `Scripts/Loadouts/Character/Creatures/aloprat.xml`, which the Archive lacks, and no Entity
  Record holds an Aloprat body skin (`aloprat.skin` is a file nothing names). The statue
  `AnimatedCharacterStatue_Aloprat` names its body by a Reference to a Record the Archive lacks.
  The data is not there: a missing file, not a missing reader.

## Decision

- **A default Loadout of type `SItemPortLoadoutRandomGeneratedParams` is read.** It is the
  Loadout `LoadoutSpec::Assortments`: the GUIDs of its `LoadoutAssortments`, read when the
  Loadout is resolved.
- **The first alternative of every group is taken**, the groups in order and the assortments in
  order, laid over each other like the layers of an outfit (`overlay`). The game's pick is
  random; the first alternative is the deterministic default, so an export is reproducible. A
  pick option is deliberately not offered here: #954 tracks one. The `Requirement` of an
  assortment or group is read as ADR-0302 states.
- **The first alternative is not replaced by the next readable one.** A group without
  alternatives gives nothing. A first alternative that is no Loadout is an `assortment` issue and
  the group gives nothing; the later alternatives are not tried, so an export never silently
  shows an alternative the default does not name.
- **Assortments are read once per resolver and a cycle ends.** An assortment names assortments
  through its alternatives; one that is used while it is being resolved, alone or through
  another, is an `assortment` issue and is skipped. The other assortments still apply.
- **An assortment that is missing (`dangling_reference`), cannot be read (`unreadable_record`) or
  is no assortment (`assortment`) is a Loadout issue** of the Entity, and the other assortments
  still apply. Without any usable assortment the Loadout is empty and carries the issues.
- **Every kind of Entity reads it.** `read_loadout` is shared, and a generated Loadout is the
  same data wherever it stands. The census `real_assortment_census.rs` over LIVE finds 538
  actors (5 skeleton-rooted creatures, the rest human NPC archetypes; 523 get Entities attached)
  and 9 corpse items (8 get Entities attached) with such a Loadout, and no other kind. Before,
  their generated Loadout was ignored and their ports stayed empty, so their exports change
  from an unclothed or empty body to the first alternative; that is the correct reading of the
  Record, not a side effect.
- **A creature whose Loadout could not be read lists why.** The Assembly of an Entity whose root
  geometry is a creature file (ADR-0275, `is_creature_path`) reports the issues of its own
  Loadout as `loadout_unreadable` on the root, with `skipped` false: `Aloprat` says that its XML
  Loadout is not in the Archive. It is limited to creatures because there an unread Loadout is
  the reason for an empty scene; the many Entities of other kinds with a missing XML Loadout
  would otherwise gain a note in every export. Exit codes are those of ADR-0255: the root
  geometry is present, so the export succeeds with an empty body.
- **The census counts them.** It prints the number of skeleton-rooted Entities that draw nothing
  with the codes of the issues that explain each, and the real-Archive test fails for one without
  a reason. The Entities are found by the census rule, not by name.

## Consequences

In LIVE all six Quasigrazers draw their body. Left without a body, each with its reason: `Aloprat`
(the XML Loadout and the statue's Record are not in the Archive) and the five `Tauroch_*`
(their skeleton file is not in the Archive, ADR-0255). Which alternative the game picks stays
unknown to an export; a Loadout of its own, per port, overrides it.
