# Cargo grid capacity uses physical inventory bindings

## Context

The ship cargo sum includes each readable Cargo Grid occurrence in the default Loadout.
Cargo Grid components carry the `miningOnly` and `invisible` flags. The capacity operands
belong to the container selected by the Entity's item-storage component.

## Decision

- Each Cargo Grid facility has a `capacity` object. It carries its storage component slot,
  resolved physical container Record index, resolution status and sourced SCU value.
- Projection requires one Cargo Grid and one item-storage component in the Entity's
  original Components array and an unambiguous inventory Reference. Missing, unreadable,
  ambiguous and excluded inputs retain distinct states. The SCU value cites the selectors
  and operands actually used, with the original encryption markers.
- Reuse the ship calculation for SCU conversion and rounding. Per-grid capacity,
  classification and visibility remain independent. A consumer joins by physical Record
  and component slot and counts each occupied port occurrence in its chosen Loadout.
- The wire extension is additive in catalog schema v3. The ship cargo sum keeps its
  existing semantics. Consumers can compute commodity capacity from the per-grid values
  and `miningOnly` flags.
