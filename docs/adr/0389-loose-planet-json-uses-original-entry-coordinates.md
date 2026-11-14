# Loose planet JSON uses original entry coordinates

Planet resources, climate assets and ecosystem definitions are authored as loose
JSON Archive Entries. Use the existing entry token walker shared with shop
inventories. Keep the original tokens, field positions, duplicate members and
physical Entry identity through typed projection. This preserves the evidence
behind missing, explicit null, invalid and present fields.

The source length and platform representation bound the read. Verify checksums
and the decoded end, use fallible allocation for the materialized token tree,
and check cancellation during reads and traversal. Report read or projection
failure against the original Entry. Diagnostic publication can retain other
readable definitions.

A tint palette path in the DataForge Record namespace is a Record reference
resolved by the original path and exact TintPaletteTree type. Archive-relative
paths use exact directory evidence and the applicable Data root prefix. Retain
the authored extensionless path, observed candidates and lookup state.
