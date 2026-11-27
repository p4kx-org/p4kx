# Hauling orders are a contract field and cargo manifests are a kind

Decided for issue #814, on top of ADR-0054 (additive schema changes), ADR-0065 (facts, no
computed rewards) and ADR-0237 (the layers of a contract). Evidence: LIVE 4.10, the DataForge
field statistics of the hauling types.

## Decision

- **Where the Archive states what a contract hauls.** In three places, none of which is a Record of
  its own:
  1. a `MissionPropertyValue_HaulingOrders` property in the `propertyOverrides` of a sub contract
     or contract, the `contractParams` of a handler, or the `contractProperties` of a
     `ContractTemplate`; its `haulingOrderContent` lists the commodity, entity class, entity class
     list or mission item to haul, or an `or` of alternatives;
  2. the `haulingOrders` of an `ObjectiveHandler_Hauling` in the `objectiveTokens` of a
     `ContractTemplate`, which state a fixed commodity or entity themselves;
  3. the same objective handler in a `MissionBrokerEntry`, which the contracts without template
     (the `HaulCargo` career contracts) use.
- **`contracts[].haulingOrders`.** One new field on every contract entry: a list of orders, each
  with a `source`, the `variableName` of its property and its `requirements`. A property level is
  chosen like every other contract value (ADR-0237): per `missionVariableName` the nearest of sub
  contract, contract, handler and template wins. The resolution lives in `HaulingLevels::orders`
  and is keyed by the variable name, not by the token a title refers to (these properties have an
  empty `extendedTextToken`). A property without a name fills no variable, shadows nothing and is
  listed at every level. The objective orders of the template follow, else
  those of the broker entry. The field is a list, empty for a contract that hauls nothing; no
  existing field changes.
- **Requirements.** A requirement has `options`, each a list of items that are all hauled; a plain
  order is one option with one item, an `or` has one option per alternative. An item has a `kind`
  (`resource`, `entityClass`, `entityClasses`, `missionItem`), the links `resourceGuid` (the
  `subtypeGuid` of a commodity), `entityClassGuid` and `entityClassesGuid` and the numbers as
  stored: `minScu`, `maxScu`, `maxContainerSize` for a resource, `minAmount`, `maxAmount` for the
  rest. A zero maximum and a container size of -1 are the game's "no limit"; they are not turned
  into `null`.
- **What an objective does not give.** The pickup and drop-off of an objective order are
  references to properties of the template (`ObjectiveProperty_Referenced`) that the game fills
  in when the mission starts. They are not facts of the Archive and are not exported; the places a
  contract is offered at stay in `pickupPlaces`. An objective order that only names a drop-off,
  points at a property (`HaulingOrder_Property`, whose content is the property of item 1) or names a mission
  item (a reference to a property, not a value) is left out.
- **Cargo manifests are a kind.** `cargoManifests` lists the `CargoManifest` Records under
  `cargomanifest`, ordered by GUID: the fill type (`even`, `random`, `custom`), the capacity
  range, the resources with `resourceGuid`, `probability` and, for `custom`, the allocation
  range, and the spawn rules (entities or resources, `probability`, quantity algorithm with
  `quantity` or `proportion`). Tags and descriptions are not exported.
- **No link from a contract to a manifest.** The Archive refers to a `CargoManifest` from the
  members of an `AIWaveCollection` only (ship encounters), never from a contract. The kind is
  therefore standalone; a consumer reaches it through the ship encounters.
- **Additive.** Schema version 2 keeps its number: the kind and the field are additions.

## Catalog v3

`definitions.cargoManifests` carries the physical definitions, sourced values and ordered original
fill and spawn inputs. The Kind remains `cargoManifests`. Original tag and description text
and keys in the authored values use the protected game-text domain. See the
[cargo-manifest field map](../catalog.md#cargo-manifests).

## Consequences

- The contract reader gains one small module for the orders (`mission_hauling.rs`); the entry has
  one field more. The kind follows the pattern of the mission kinds.
- The SCMDB data (scmdb.net) was a hint for what a consumer looks for; no value of it enters the
  repository.
