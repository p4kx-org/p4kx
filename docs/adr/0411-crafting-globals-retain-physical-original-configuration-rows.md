# Crafting globals retain physical original configuration rows

Schema v3 publishes each admitted physical `CraftingGlobalParams` owner under
`configurations.craftingGlobal`, replacing the merged root `craftingGlobal` object. A merged,
deduplicated GUID list cannot preserve repeated occurrences, source states or per-datum origins.
The configuration table retains the original defaults and both exclusion arrays with their
original names, ordered nullable slots and actual target states. Header facts and every independent
field carry immediate sourced cells. Additional owner-local arenas retain only unprojected or
rejected original fields, including nested selection fields; successfully typed fields are not
duplicated there. Numeric additions keep exact storage and the existing configuration receipts.

The Catalog producer and direct SDK use one original configuration reader. The same original
field inventory feeds the existing Blueprint default/exclusion rule interpreter; the union over
all admitted global owners remains independent of the output Filter. No synthetic Catalog Kind,
default quantity limit, Blueprint-body scan, inverse resource survey or localization read is
introduced. Same-GUID physical owners remain separate and rows use textual GUID order with
physical-index ties. Query coverage distinguishes unavailable headers, bodies and fields from a
healthy absent or declared-empty family. Consumers commit their cache only after outer success
and appropriate coverage checks. This source API is not a full-Catalog or client-memory acceptance.

This is an unreleased v3 normalization. The released-v2 fixture remains immutable and runtime
writers emit v3 only. Existing interpreted Rust `Crafting::global` remains the rule consumer's
union, not an additional serialized compatibility dataset.
