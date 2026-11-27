# Crafting station limits are a kind and research gating waits for a value

Decided for issue #831. Evidence: LIVE 4.10. Amended for issue #842; extends the catalog under ADR-0054.

## Context

VerseKit asked for the per-player blueprint cap. The Archive has none. The only crafting limits
are on the crafting stations, in `Components[CrafterComponentParams]`: `maxJobsInHistory` and, per
queue, `maxJobsInProgress` and `maxJobsWaiting`. LIVE 4.10 has three such entities, all under
`entities/basebuilding/interactables`. Separately, 926 of the 1,618 blueprint tiers carry a
`CraftingResearch` whose `researchCosts` and `unlockRequirements` are empty strong pointers on
every tier.

## Decision

- The catalog gets the kind `crafters`, one entry per entity with a `CrafterComponentParams`:
  `guid`, `className`, `recordPath`, `maxJobsInHistory` and `queues`, each queue with `name`
  (`debugName`), `maxJobsInProgress` and `maxJobsWaiting`. It is additive (ADR-0054), so
  `schemaVersion` stays 2. `Kind::Crafters` comes last in `Kind::ALL`.
- Crafters are found by the component, not by a folder. The kind's rule takes the
  `EntityClassDefinition` Records under `entities` that no other entity kind takes. An item, ship
  or commodity that is a crafter is read from its own Record and stays an item, ship or
  commodity as well (`Picked::Crafted`). Development leftovers follow the project rule: the
  `_template` bench of LIVE is skipped and counted, and only a leftover that holds the component
  is counted. Items and ships whose name marks them as leftovers are not checked for the component.
- Crafters are not limited to stations: ships, vehicles and buildings may carry them. The
  selection stays structural (the component, never a name or a station folder). For a crafter
  mounted as an item, the host is findable from the crafter: `hostGuids` lists the `ships` whose
  Default Loadout mounts it at any depth of the port tree, derived from `loadouts`. The data
  supports this link, whereas a link on the ship entry would add a field to a kind that other
  work extends. A crafter that is a building part has no loadout host and an empty list, and a
  ship whose loadout is empty or failed yields no hosts. A ship that carries the component itself
  is not its own host: its `hostGuids` stays empty and a reader matches `crafters[].guid` with
  `ships[].guid`.
- Categories and process types: see "Amendment (#842)" below.
- The tier field `research` is left out. Neither pointer holds a value on any tier, so there is
  no value to read the pointed-to type from, and a field typed without one would be a guess that
  ADR-0054's guarantees (an existing field never changes its type) forbid correcting. When a
  Build fills them, the fields join the tier as an additive change.

## Consequences

- A Record that no other kind takes is not read into a tree to look for the component. A probe
  (`DataForge::pointer_probe`, `TreeBuilder::holds` in p4kx-dataforge) reads only the
  `Components` pointer array and compares the struct index; it is skipped entirely when no
  instance of `CrafterComponentParams` exists. Items, ships, commodities and mineable entities
  are checked on the tree they already have.
- Measured with the real-Archive insurance test on LIVE 4.10 (one full catalog build plus the
  process start, two runs each): main 19.5 s and 26.3 s; crafters with a tree per entity 18.0 s
  and 23.4 s; crafters with the probe 22.7 s and 19.3 s. The differences are within the noise of
  the machine. The failed count (0) and the skipped count (1,287, plus the one template crafter)
  match main on LIVE and PTU.
- The Basetool reads a missing `crafters` as an older catalog.

## Amendment (#842)

Evidence: the structure of the modifiers was observed in a test build, not in LIVE 4.10, which has
the queue rules but no modifiers and no `baseCraftingTime`. The amendment replaces the decision
that the accepted categories and process types are not exported.

- A `crafters` entry gets `type`, `subType` and `size` (the `AttachDef` of the entity),
  `baseCraftingTime`, `modifiers` and `issues`; a queue gets `blueprintCategoryAvailability` and
  `processTypeAvailability`. All are additive (ADR-0054), `schemaVersion` stays 2.
- A modifier has a `type` (`time`, `none`, `unknown`), the `structName` of its type struct, the
  values of the type (`timeMultiplier`, as stored) and a `condition`. Modifier types, modifier
  conditions and queue availabilities are polymorphic structs. They are recognised by struct name,
  never by the name of an entity or a value. A rule (`condition` or `blueprintCategoryAvailability`)
  has a `kind`, the `structName` and `blueprintCategoryGuids`; a new struct is a new `kind` and
  a new type a new `type`, both additive, because the strings are a closed set the game may extend.
- A struct this export does not know is kept with its name, `unknown` and no values. An unreadable
  value (a missing type, a non-finite number, a category Reference to no Record, a
  `processTypeAvailability` entry that is no boolean) is kept as null or empty and reported as a
  code in the crafter's `issues`, a per-crafter summary. Nothing is dropped and nothing panics. A
  modifier is exported with its `condition` of kind `none` when the Record stores none; no
  meaning is guessed.
- A field that is absent (or a null pointer) differs from one that is stored with the wrong type:
  the first reads as `null` or `[]` with no issue, so the data before the modifiers exist stays
  clean; the second reads the same and sets an `unreadable_*` code (`baseCraftingTime`, the
  `AttachDef` fields, `queues`, a queue, `modifiers`, `processTypeAvailability`, `categories`).
- `unreadable_category` means a Reference that links to no Record. Whether the Record is a
  blueprint category is not checked by the export, because a Filter may leave it out of
  `blueprintCategories`; the real-Archive test checks that on an unfiltered catalog.
- `processTypeAvailability` is the list of booleans as stored. The Archive does not name the
  process types, so the export does not map positions to names; a name would be a guess.
- Ports for crafters: a crafter that is not an item (a fabricator below `basebuilding`) is not in
  `items`, so its `AttachDef` type and size are part of its `crafters` entry. A reader matches the
  `types` and the size range of a `loadouts` port with them.

## Schema v3 amendment (#1076)

Schema v3 records the actual Research envelope, declared strong-pointer bases and original
value occurrences. The interpretation distinguishes absent, null, present-empty, present-known,
unsupported and unreadable source shapes. A known populated shape requires the exact fieldless
declared base. Other populated shapes keep their original payload and an explicit source diagnosis.
See the [Blueprint v3 contract](../catalog-blueprints.md).
