# Item stat blocks are raw facts without derived values

Decided for issue #317. Builds on ADR-0054 and ADR-0063.

The Basetool wants the typed stats of weapons, shields, quantum drives, power plants, coolers and
missiles from the Archive. The community tools it replaces compute derived values on top of the
raw fields: a weapon's damage per second, a charged weapon's cycle time, a ship's acceleration, a
quantum drive's fuel rate. The issue asks which stat blocks are exported raw and which derived
values, if any, `p4kx` computes.

## Decision

- **Raw facts only.** Every number in a stat block is a field of the Archive, in the unit and
  scale the game stores it. `p4kx` computes no derived value: no damage per second, no charge or
  burst cycle, no acceleration, no fuel rate, no sum of damage types. A unit conversion is left
  to the consumer too (the reference tools divide `quantumFuelRequirement` by 1e6; the catalog
  carries it as stored), and each field is named after the stored field.
- **Why.** A derived value is a model of the game's rules, not a fact of the Archive. The
  formulas depend on the fire-mode type, on how a charged mode ends and on which ammunition a
  weapon holds, and CIG changes them between patches. A model that is wrong would reach the
  Basetool as if it were data, and a correction would be a change of meaning, which version 1
  forbids (ADR-0054). Raw facts are stable under that rule; the Basetool owns the formulas and
  can change them without a new catalog.
- **The blocks.** An item has `stats`, `null` unless it has at least one of six blocks. The
  blocks are `weapon` (fire modes and ammunition), `shield`, `quantumDrive`, `powerPlant`,
  `cooler` and `missile`. A block follows from the item's components (`SCItemWeaponComponentParams`,
  `SCItemShieldGeneratorParams`, `SCItemQuantumDriveParams`, `SCItemMissileParams`) or, for the
  power plant and the cooler, from `AttachDef.Type`, because the resource component they share
  does not tell them apart. No class name or record name enters the rule. Every block is a
  required key of `stats`; `null` means the item does not have it. A field inside a present block
  is `null` when the Archive does not state it, never 0, and a value that is not a finite number
  is `null` too.
- **Shared fields.** An item with at least one of the six blocks also carries `health`
  (`SHealthComponentParams.Health`) and `resourceStates`: the states of
  `ItemResourceComponentParams` with their EM and IR signatures and their generation and
  consumption flows (power draw, coolant, fuel), each with the unit struct the Archive uses and
  without conversion. They are the signature, power and durability inputs; an item without a stat
  block does not carry them, which keeps doors, props and clothing out of the kind. The separate
  power and heat connection components (5 and 3 Records on LIVE, none of them an item with a
  block) are not exported: version 1 can never remove a field, and `resourceStates` carries the
  same inputs. The cross-section is not a signature of these items: `signatureParams` of a
  resource state holds `EMSignature` and `IRSignature` only, and `crossSectionParams` of
  `SSCSignatureSystemParams` is empty on every item with one of the six blocks on LIVE (the
  radar cross-section is a property of ships and other entities). The one item-level scalar,
  `signalCrossSection` of `SCItemVehicleArmorParams`, belongs to armour, which has no block.
- **Ammunition.** `ammoCapacity` is the capacity of the ammo container, that is the magazine, not
  the weapon body. A ship weapon holds the container itself; a personal weapon links its magazine
  entity through `ammoContainerRecord` and the container is read there, one hop. A linked Record
  that is missing, has another struct or cannot be built leaves `ammo` `null` and never fails the
  item. An unbounded quantum range is `f32::MAX` as stored.
- **Flat and nullable, on the item.** The stats are a field of the item, not a new kind: the item
  is the key, the import already joins on it, `completeness.items` already states whether the list
  is complete, and no second GUID list has to stay in step with `items`. This is ADR-0063's
  reasoning for the ship base values. The field is additive in version 1.
- **Closed sets are strings.** `kind` of a fire mode (the struct of the mode) and
  `trackingSignalType` of a missile are strings in the schema, so a new value is additive and a
  reader maps an unknown one to nothing.
- **Facts, no game text.** A stat block holds numbers, struct names and enum names. None carries
  `x-gameText`, and none is a description. The identifier `name` of a fire mode is a debug name
  of the DataForge, not a game text.
- **Sources.** Each field's source path is in `docs/basetool-catalog.md`. The catalog leaves out
  the per-value file and field of `p4kx ships`; the table is the audit trail.
- **Ammunition is followed once.** A weapon's ammunition is the `AmmoParams` Record that its
  `SAmmoContainerComponentParams` links to. It is read while the weapon is scanned; a dangling
  Reference leaves `ammo` `null` and is not a failure.

## Considered Options

- Computing damage per second, cycle time and similar values in the catalog, marked as derived:
  rejected. The consumer cannot tell a corrected formula from changed data, and the Basetool
  already owns the game rules it displays. The issue's formulas stay documented as the reference
  for a consumer, not implemented.
- A separate `itemStats` kind keyed by item GUID, like `paints`: rejected for the reason above;
  it would duplicate the key and split `completeness`. The size cost is one `null` per item
  without stats, about 0.3 MB uncompressed on LIVE.
- Nesting each value as `{value, source}`: rejected as in ADR-0063.
- Exporting every component of the item as raw trees: rejected; that is the Record export, and
  the Basetool needs a stable typed shape, not the DataForge's.

## Catalog schema 3 durability

Schema 3 publishes item Health, Degradation and Distortion in independent configuration tables
under ADR-0356. An owning item Record and its original component ordinal identify each row.
The component model preserves repeated components and makes Health available on selected items
without a specialist stat block. Every authored field follows ADR-0357. A separate row per
physical component preserves these identities without duplicating the item or choosing one
component to represent the others.

The retained specialist projection obtains finite `stats.health` from the first original Health
component. The independent durability rows retain exact Single bits and all selected component
occurrences. Explosion payloads use the shared authored explosion model. Linked Records retain
their own identity and scope. Item resource self-repair, vehicle hull health and vehicle armour
tuning remain distinct source families.

## Catalog schema 3 quantum drive configuration

Schema 3 follows ADR-0356 with one `configurations.quantumDrives` row per original
`SCItemQuantumDriveParams` occurrence. Each row identifies its physical item and component slot.
The standard and spline parameter Classes have separate source states and values. Their declared
types, optional heat and boost Classes, state/effect subtrees and other authored members carry
their own source evidence. The row preserves original numeric widths and representations.

The previous compact flat values continue to feed variant identity in their original byte order.
Their mode-specific meanings map to the standard block sourced from `params`. The public
configuration is the source-typed row. Source paths and units are documented in
[Quantum drive configurations](../catalog-quantum-drives.md).

## Catalog schema 3 fuel and resource capacity

Under ADR-0356, fuel tanks, fuel intakes and ResourceContainer components have independent
configuration tables keyed by their physical owning Record and component slot. A capacity
retains the concrete unit and its stored member. A new unit retains its original type and
members so no canonical unit or magnitude is inferred.

Commodity authoring links each original ResourceContainer slot to the canonical row. Its
first-entry presentation choice still reads the original authored order. Resolved loadouts
join by the actual selected target Record index and keep one placement for each port.
[Fuel and resource capacity](../catalog-fuel-capacity.md) specifies the field paths.

## Catalog schema 3 resource components

Under ADR-0356, one physical ItemResourceComponentParams occurrence owns
its states, deltas and control settings. Resource-only items expose these
facts independently of specialist statistics. The source delta supplies
the pairing of input and output, original position and unit declarations.

The compact power, coolant and flat state bytes remain private variant
identity inputs. Consumer field mapping and the original ordering and
numeric projection are documented in [Resource components](../catalog-resource-components.md).

Global resource rules have their own GameMode-owned rows. The original
`params.resourceNetworkParams` declaration supplies the typed settings and
their per-value origin. Component and placement rows continue to describe
their own authored settings.
