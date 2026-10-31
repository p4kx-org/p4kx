# The Requirement of a loadout assortment is decided where it names no tag and listed where it does

Decided for issue #955, on top of ADR-0289 (a generated Loadout takes the first alternative of
each group of its assortments) and in the manner of ADR-0279 (conditions stay undecided unless they
restrict nothing). Evidence: the LIVE Archive and the census
`crates/p4kx-assembly/tests/real_archive/real_assortment_requirement.rs`.

## What the data says

`SLoadoutAssortment` has a `Requirement` field and so has each of its `GroupedLoadouts`. It is a
strong pointer to one of three Requirement structs:

- `SLoadoutRequirementTags` with `RequiredTags` and `BannedTags`, arrays of References to Tags;
- `SLoadoutRequirementOR` with `Requirements`, an array of Requirements;
- `SLoadoutRequirementNOT` with `RequirementToInvert`, one Requirement.

No other Requirement type exists in LIVE, and a Requirement nests three deep at most (a NOT around
an OR of tag sets).

LIVE holds 321 assortments and 2316 groups. No assortment has a Requirement, as #882 found for the
creature assortments. 1055 groups have one: 1034 `SLoadoutRequirementTags`, 14 `OR`, 7 `NOT`. Of
the 1034 tag Requirements, 248 name no tag at all, neither required nor banned.

A tag Requirement matches Tags against the Tags of the spawn that asks for the Loadout. The Archive
holds neither those Tags nor the rule of the match (every required Tag or any, and against which
Tags); the game decides at spawn time. The Tags themselves are in the Archive, the spawn is not.

## Decision

- **A Requirement is `Met`, `Unmet` or `Undecided`.**
  - No Requirement, a null pointer, and a tag Requirement with neither required nor banned Tags
    restrict nothing: `Met`.
  - A tag Requirement that names a Tag is `Undecided`, as is a Requirement of a type not listed
    above, an `OR` without parts, a `NOT` without a part, and one nested deeper than 16 levels.
  - `OR` is `Met` when a part is `Met`, `Unmet` when all parts are `Unmet`, otherwise `Undecided`;
    `NOT` swaps `Met` and `Unmet` and keeps `Undecided`. Nothing in LIVE reaches `Unmet`; the rule
    follows from the two above and is there for a patch that adds such a Requirement.
- **An `Unmet` assortment or group gives nothing.**
- **An `Undecided` assortment or group is used and noted, never an issue.** It contributes its
  first alternative as ADR-0289 states, so that no export changes. The Loadout of the Entity gets
  one note `assortment_requirement` per assortment GUID, with the undecided groups collected
  (`LoadoutError::AssortmentRequirement`, listed by `EntityNode::notes`). A script that wants the
  exact picture of a spawn knows which groups the game might have left out. The group is not
  dropped: the Archive does not show that the game would drop it, and dropping the groups of a
  human NPC would leave the body bare more often than the game does.
- **A note is not an issue.** `issues` means what kept part of a Loadout from being read (the
  catalog schema, `docs/cli-json.md`); an undecided Requirement keeps nothing from being read. So
  the note stays out of `EntityNode::issues`, out of the catalog's `loadouts[].issues` (the
  catalog shape is unchanged), out of the creature note `loadout_unreadable` of ADR-0289 and out of
  every issue count. `LoadoutError::is_note` tells the two apart. `entity loadout` prints a note
  as `note: …` and adds `notes` to the entity JSON only when it is not empty.
- **The picks stay those of ADR-0289.** The alternative within a group is still the first; the
  option to choose another is #954. The assortment cache is keyed by GUID and holds the first
  alternatives only, so a pick needs the pick in the key, or the cache must keep every
  alternative.

## Consequences

- LIVE has Entities that note an undecided Requirement, among them the human NPC archetypes of
  ADR-0289; their ports are those of ADR-0289. The census prints the counts of the Archive that was
  read.
- The note code `assortment_requirement` and the `notes` field are in `docs/cli-json.md`. The
  catalog and its schema do not change.
- The census counts the assortments, the groups with a Requirement by type, the tag Requirements
  that restrict nothing, and the undecided ones reached by an Entity.
