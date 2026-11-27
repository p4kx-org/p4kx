# Gameplay properties have no Archive link to a stat, and armour stats follow the components

Decided for issue #478. Builds on ADR-0054, ADR-0063 and ADR-0065.

A blueprint modifier names a gameplay property by `propertyGuid` (#396). The Basetool wants the
base value of the field that the property changes, to show "550 to 605" as real numbers, and so
needs to know which `stats` field a property modifies and to find stats for armour, clothing,
radars, tractor beams and salvage modifiers, which `stats` did not carry. The issue asks whether
the catalog states the property-to-stat link and how armour and clothing stats are shaped.

## Decision

- **No link in the catalog.** A `CraftingGameplayPropertyDef` of LIVE 4.10.2 holds a name key
  (`propertyName`), a unit format, a display transformation and in one case a name override; it
  names no item field, struct or path. No other Record refers to a property either (a search for
  the name key finds the Record itself). The catalog does not invent a link. `gameplayProperties`
  stays as it is, and `docs/catalog.md` documents, per `className`, the `stats` field the
  property modifies. The consumer keeps that mapping; a real-Archive test checks that every
  property of the Archive has a row, so a property the game adds fails the test instead of going
  unnoticed.
- **Why not an exported link field.** A `statField` string per property would be a mapping the
  project maintains by hand, as part of the data. It would read as a fact of the Archive and a
  change of it would change the meaning of existing data (ADR-0054). In the document it is
  reviewed with the stats it names.
- **Armour and clothing follow the components.** An armour piece carries two components, and the
  catalog mirrors them as two blocks rather than one invented "armour" model:
  - `armor` from `SCItemSuitArmorParams`: `integrityMilestoneToBreak` and `damageResistance`, the
    `damageResistance` struct of the `DamageResistanceMacro` Record that the component links to.
    Per damage type it keeps `multiplier`, `threshold`, `damageCap` and `damageCapLimit` as stored.
    A Record that is missing, of another struct or unreadable leaves `damageResistance` `null`
    (ADR-0065: followed one hop, never fails the item).
  - `clothing` from `SCItemClothingParams`: `temperatureResistanceMin` and `Max`,
    `radiationCapacity`, `radiationDissipationRate` and `gForceResistance`. Armour pieces carry
    this component too, so one block serves clothing and armour.
- **No derived mitigation.** The `multiplier` is the factor damage is scaled by. The Basetool
  displays `1 - multiplier` as mitigation; the catalog does not (ADR-0065).
- **Other kinds.** `radar` (the aim assist distances and the ping cooldown), `tractorBeam` (the
  first `SWeaponActionFireTractorBeamParams` of the weapon) and `weaponModifier` (`salvage` and
  `tractor`, the first `SSalvageModifier` and `STractorModifier` of an attachment's
  `EntityComponentAttachableModifierParams`) are blocks with the same rules as the six of
  ADR-0065. The power of a mining laser is the `damagePerSecond` of its beam fire mode, added to
  every fire mode (`null` for a mode without one).
- **Spread.** The spread is kept inline (`launchParams.spreadParams`, or `spreadParams` of a
  beam) and every fire mode carries it as `spread`, raw.
- **Properties without a stat field.** Recoil kick, handling and smoothness and the reload speed
  have no single field: recoil is the weapon's procedural recoil configuration and the `recoil`
  of its fire modes, Records of curves per stance. Turning them into three numbers is a model of
  the game. They are listed as none, with the reason, in the document; so are the two
  properties of crafting stations. A later issue may add a typed recoil block.
- **Items without the source component.** Some produced items hold no field for a property their
  blueprint modifies (backpacks and one shoe for the damage mitigation and the temperatures, fuel
  nozzles for the health, beam-only personal weapons for the fire rate). The block stays `null`;
  the catalog invents no default. A real-Archive test checks every modifier of every blueprint
  against its produced item and lists these cases.
- **Damage mitigation covers all types.** The property is one factor and the Archive does not
  say which damage types it scales; `armor.damageResistance` gives all six `multiplier`s.
- **Additive in version 1.** The five blocks, `damagePerSecond` and `spread` are new fields; every field is
  required in the schema as before, and a consumer that ignores unknown fields is unaffected
  (ADR-0054).

## Considered Options

- A `statField` on every `gameplayProperties` entry: rejected, see above.
- One `armor` block that merges the suit and clothing components: rejected; the components are
  separate in the Archive and clothing without armour exists.
- Exporting mitigation as `1 - multiplier`: rejected by ADR-0065.
- Exporting the full recoil configuration now: deferred; it is a tree of Records and the issue
  asks only for the fields the archive holds as plain values.
