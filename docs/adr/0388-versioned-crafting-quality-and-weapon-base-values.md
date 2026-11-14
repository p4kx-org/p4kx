# Version the crafting quality formula separately from the Catalog

Basetool and VerseKit exchange component Crafting quality by original Blueprint cost position
and evaluate the same values from Catalog v3. The first agreed formula uses Basetool's ordered
curve evaluation and VerseKit's aggregation across recipe positions. It compares crafted
equipment with unmodified factory values. This contract has separate integer formula version 1.
A measured game result that changes the computation will require a new formula version even if
the Catalog schema remains the same. The agreement is recorded in the
[Basetool Exchange amendment](https://github.com/krt-profit/basetool/issues/2821#issuecomment-6878138700).

Catalog v3 publishes exact authored inputs and a versioned factory weapon projection by physical
component and original fire mode. The separate projection confines the modeling assumptions
and leaves the item stat blocks as authored facts under [ADR-0065](0065-item-stat-blocks-are-raw-facts-without-derived-values.md).
Each result uses the per-datum source evidence of [ADR-0357](0357-source-encryption-provenance-is-explicit-per-data-value.md).
Consumers apply quality and aggregate their selected loadouts. Each occupied port is counted
independently from the identity of the component installed in it.

[The formula contract](../catalog-combat-quality.md) defines calculation, comparison, and input
identity. [Versioned conformance vectors](../examples/catalog-combat-quality-v1.json) are the
shared test input for consumer implementations.
