# Version the crafting quality formula separately from the Catalog

Consumers exchange component Crafting quality by original Blueprint cost position and evaluate
the same values from Catalog v3. Formula version 1 specifies ordered curve evaluation,
aggregation across recipe positions, and comparison against unmodified factory values. A
measured game result that changes the computation requires a new formula version even when the
Catalog schema remains the same.

Catalog v3 publishes exact authored inputs and a versioned factory weapon projection by physical
component and original fire mode. The separate projection confines the modeling assumptions
and leaves the item stat blocks as authored facts under [ADR-0065](0065-item-stat-blocks-are-raw-facts-without-derived-values.md).
Each result uses the per-datum source evidence of [ADR-0357](0357-source-encryption-provenance-is-explicit-per-data-value.md).
Consumers apply quality and aggregate their selected loadouts. Each occupied port is counted
independently from the identity of the component installed in it.

[The formula contract](../catalog-combat-quality.md) defines calculation, comparison, and input
identity. [Versioned conformance vectors](../examples/catalog-combat-quality-v1.json) are the
shared test input for consumer implementations.
