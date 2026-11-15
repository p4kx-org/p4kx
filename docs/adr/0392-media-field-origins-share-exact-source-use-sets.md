# Media field origins share exact source-use sets

A complete ATL lookup includes the evidence for inspecting its directory,
including documents that yielded no match. A flat list repeats that whole
directory proof for each lookup field, even when the encoded list is cached.
Share the proof once per physical directory to keep serialized membership
proportional to the size of the directory and the local contributions.

ATL directory queries use their own selector plus the original directory scan
and admitted document evidence. Store that immutable coverage once per physical
directory scope and carry it through both field and aggregate origins. Match
counts use each query and its actual matched declarations. Read and selection
health remain independent of source-set completeness.

The first released Media format uses companion protocol `p4kx.field-origins/2`.
Each field keeps an immediate encryption marker and source-set completeness.
Its actual uses are the sorted unique union of its local references and one
optional shared set. A set member is one actual use ID in
`origin-uses.jsonl`. The verifier checks each set once and reuses its proven
encryption state and membership when it checks fields. Source-use roles,
physical identities, capture cuts and precedence remain authoritative.
[ADR-0357](0357-source-encryption-provenance-is-explicit-per-data-value.md)

Frame sets as a header, ordered members and a closing row with the same set
ID. The stream ends with exact set and member counts. The Index declares the
set artifact's length and digest alongside the other provenance artifacts.
Publication commits the complete generation as one unit. Payload stream
admission and provenance artifact accounting retain their documented budgets.
[Media export contract](../media-export.md)
