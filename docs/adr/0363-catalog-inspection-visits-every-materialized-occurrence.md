# Catalog inspection visits every materialized occurrence

Catalog inspection consumes the existing cancellable owned-value walk to completion. It applies
no separate reference-count, metadata-byte, nesting-depth or value-count cutoff. References are
captured in original declaration/element order without following their targets. Every occurrence
retains its field path and actual physical target Record index/GUID, including repeated links.

The Record/tree pairing, original parent/container ordinals, exact non-finite payloads, first
walk failure and sticky cancellation remain authoritative. A failure returns an error rather than
a successful partial inspection. Errors already present in the canonical tree are still visited;
canonical depth/value-limit errors retain their incomplete-inventory diagnostic. Removing the
Inspector's cuts does not remove the upstream DataForge reader's resource policies.

One inspection shares exact immutable source-type, target-type and target-path spellings. String
equality selects storage only: it grants no original source, field, Record or target authority.
Distinct occurrences remain separate, and pool iteration never chooses their output order.
Inspection clones share these immutable strings. Unique field paths retain their own storage.

Storage costs include all captured occurrences, field-path capacities, unique metadata spellings,
numeric/error facts and traversal state. Repeated metadata lookups still hash/compare text;
sharing allocations is not a constant-time lookup or throughput guarantee. Canonical trees,
public diagnostic output, distinct path text, worker concurrency and allocator overhead have
independent costs. Downstream carriers can still expand repeated strings; this decision does not
bound full-report memory or prove peak RSS.

Current-API source controls first reproduce the reference-count and metadata-byte cuts on valid
resolved References. Their successors require every original occurrence, later InvalidBool and
dangling-reference evidence, and exact non-finite width/bits/ordinal. Actual frame/Record pairing
and cancellation controls remain. The unchanged synthetic 6,000-link source must pass normal
publication with a complete field inventory, without a synthetic field-walk-limit diagnostic.

The representative diagnostic-example limit remains a sampling policy; it does not end traversal
or remove the complete unresolved-reference list. Broader canonical, source-origin and output
completeness requirements remain separate from this inspection contract.
