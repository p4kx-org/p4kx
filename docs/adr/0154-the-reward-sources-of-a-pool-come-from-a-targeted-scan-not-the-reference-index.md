# The reward sources of a pool come from a targeted scan, not from the reference index

The `rewardSources` of a blueprint pool (ADR-0054, `docs/catalog.md`) were found through
the full `ReferenceIndex`: every Reference of every Record, capped at 2^23, then the referrers of
the pool Records. Decided for issue #433, the follow-up of the review of #431 (#396).

- **One scan, no cap.** `read_crafting` walks the Record tree of every Record once, in parallel,
  and keeps each struct with a Reference to a pool GUID it has read. Nothing is indexed, so there
  is no limit to exceed. `CraftingWarning::RewardSourcesTruncated` is gone, and so is the case it
  described. A Record the scan cannot read is no `failed` entry (it is not known to be selected);
  it gives the warning below, and `rewardSources` may be incomplete exactly then.
- **The warning is narrower.** A Record whose tree cannot be built could hide a reward source, but
  most unreadable Records are of types that never hold one. `CraftingWarning::UnreadableRewardRecords`
  counts only the unreadable Records whose struct type is the type of a Record in which a reward
  source was found. An unrelated oversized Record no longer makes `p4kx catalog` exit 1. This is
  a heuristic: an unreadable Record of a type that holds a source in no readable Record is not
  reported. When no source is found at all (say, every contract Record became unreadable in a
  new Build), every unreadable Record counts. A pool without a source is no warning: 49 of the 160
  pools of LIVE have none.
- **Progress.** The scan reports `CatalogProgress` with the phase `Crafting`, so the windows and the
  CLI show the crafting step instead of a bar that sits still.
- **The GUI reference index is not reused.** It lives in the front-end's cache and is built on
  demand for the Reference browser, while `p4kx-export` is a library below the front-ends
  (ADR-0003) that takes a DataForge alone, so the CLI would need its own anyway. The targeted scan
  costs less than building an index and needs none.
- **Versioning (ADR-0054).** The sources and their order are the same (1,039 on LIVE, as before).
  The one change to the catalog data is that a gameplay property whose `propertyName` is a
  placeholder key now keeps the key in `name.key`, as items and blueprints do; on LIVE none of the
  29 properties has one, so no v1 reader sees a difference. The warnings of `p4kx catalog --json`
  change in a breaking way for a reader of that output: `reward_sources_truncated` is removed and
  `unreadable_reward_records` replaces `unreadable_reference_records`.

- **Unknown modifier ranges (issue #661).** The same rule holds for the other silent loss of the
  crafting reader: a modifier range of a struct type that is neither `..._Linear` nor
  `..._LinearIntegerAdditive` is left out of the modifiers. `CraftingWarning::UnknownModifierRanges`
  counts those ranges over the selected blueprints and names the distinct struct types in
  `details`, as `kind` `unknown_modifier_ranges` in `p4kx catalog --json`. It appears only when such
  a range exists, so a Build without one is not made incomplete by it. The catalog data is unchanged; the new
  kind is an additive extension of `warnings`.

## Considered Options

- Keep the index and add a flag on the pool when sources are cut: keeps the cap, the 8 million
  References held in memory and the warning class.
- Narrow by the Record path instead of the struct type: the sources of LIVE sit in contract
  Records, but the contract is "any Record", so a path rule would silently lose a source elsewhere.
- Index only the Records of the types known to hold a source: needs the types in advance, which is
  the knowledge the scan is supposed to find.
