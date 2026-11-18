# Separate Blueprint pool declarations, weight sums and reward references

A pool contains an ordered list of weighted Blueprint references. The same Blueprint can appear
at several positions, and different physical pool Records can share a nominal GUID. Collapsing
those rows or nesting incoming reward declarations under a nominal pool loses which original
Record or slot a consumer is reading.

Publish physical `BlueprintPoolRecord` owners under `definitions.blueprintPools`. Preserve the
shared eight-field sourced header, each authored `blueprintRewards` position and its original
`blueprintRecord` Reference and exact `weight`. Retain additional original declarations at their
owning pool or slot. Preserve source states and per-datum origins on the typed fields.

`projections.blueprintPoolWeights` identifies an owner within the Build by its DCB input and
original Record index, with its nominal GUID for consistency checks. Preserve the existing
f32 sum of positive finite weights in original slot order. Exact stored weights remain
authoritative, and overflow in the derived sum is represented by the raw f32 bits.

Publish observed direct scalar Record links under `declarations.blueprintRewardSources`.
`poolStorage` distinguishes a Reference from a WeakPointer to a foreign Record root. Local weak
pointers and self-links do not enter the survey. Each separate
declaration keeps its original referencing header and physical occurrence ordinal, its authored
pool link and the available nearest contract identity, name and chance. The observed Reference
and the projection's physical identity serve different purposes. A nominal GUID cannot resolve
an absent, foreign or dangling link to a physical owner. A weighted pool alone does not establish
an in-game drop probability.

The direct SDK shares the definition and weight readers with the Catalog and reads only the
selected pool owners. This supports clients that need the authored slots while avoiding the
cost of unrelated Record discovery. The complete Catalog's inverse survey retains its own
coverage and warnings. The released v2 fixture stays immutable, and P4KX 0.5.0 writes only v3.
The independent interpreted Rust crafting interface retains its existing contract.
