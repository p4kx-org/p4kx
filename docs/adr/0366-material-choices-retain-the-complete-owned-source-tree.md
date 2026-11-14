# Material choices retain the complete owned source tree

Status: accepted

## Context

Material choices are ordered nodes with tags, optional material paths and owned children.
Neither six levels nor 4,096 nodes is a demonstrated format maximum. Valid synthetic DataForge
inputs contain nine readable levels and 4,097 sibling choices. Fixed projection cutoffs can
replace valid children with an empty list or silently remove a selectable final material.

The resulting public tree is also cloned, compared, formatted and dropped by its consumers.
Removing only the read fence would leave those operations and material selection dependent on
thread stack depth.

## Decision

Read every material choice from the already materialized owned source, preserving child order,
tags and optional paths. Use an explicit continuation stack for construction and resolution.
Do not follow external References or change the canonical DataForge reader's own admission here.

Selection retains its established semantics: all nonempty node tags must match active tags
without ASCII case sensitivity; descendants of a nonmatching node are not candidates. Search
the first matching branch in source order, prefer a descendant material, then use its parent
fallback. A materialless branch does not prevent the next sibling from matching.

Clone and equality traverse the entire owned tree iteratively. Drop detaches descendants before
destroying them. Debug uses a complete flat preorder list carrying depth, tags, optional material
and child count, with standard debug formatting. Its layout changes to avoid recursive formatting;
it is not a serialized asset or catalog representation.

## Consequences

The root Rust type and its borrowed getter signatures remain available. Readable source
membership is independent of an extra material depth or count quota. The upstream canonical
DataForge admission and target rendering/resource policies remain separate.

The two original-source regressions check source readability, exact slots, tags and material
paths before projection. Separate controls cover tag gating, first-branch selection, parent
fallback and a materialless branch. An 8,193-node chain exercises selection, clone, equality,
a changed final material, complete Debug and destruction on a 256 KiB thread stack. This is
specific regression evidence, not a measured process-memory bound or proof of unlimited input.
