# Standalone field origins

`p4kx cryxml --out <file>` publishes XML with a sibling `<file>.p4kx-origin.jsonl`.
The companion preserves per-datum encryption evidence without changing the XML.
With raw XML on stdout, use `--origin-stderr` to receive the companion on stderr,
`--origin-out <file>` to save a copy, or both. Read stderr as a companion only
after a successful exit.

## Companion records

The [JSON Schema](../schemas/p4kx-source-origin.schema.json) describes each line.
The stream starts with a `header` containing `format: "p4kx-source-origin"` and
`formatVersion: "1.0"`. The binding identifies the file or stdout and carries the
actual SHA-256 digest and decimal-string byte length. A file binding records one
relative basename. It uses `utf8` for portable names, `unixBytes` for native Unix
bytes, or `windowsUtf16` for native Windows code units. Treat native encodings as
basenames on their originating platform.

`source` and `sourceUse` rows define the physical inputs and contributor
references. Each `field` row carries an artifact address, `sourceUsesRefs`,
`sourceSetComplete`, and `containsEncryptedSource`. The encryption value is
`true` when an encrypted contribution is proved, `false` when complete evidence
proves there is no encrypted contribution, and `null` when the answer is unknown.
Standalone CryXml input has unknown original P4K encryption.

Binary CryXml uses `xmlBytes` addresses with half-open output byte ranges,
including zero-length attribute values. Addresses distinguish element names,
closing names, attribute names, attribute values and text. Source coordinates
retain the original node and attribute table indices. A suffixed attribute name
also identifies the preceding name range consulted by the serializer. Text XML
retains its original bytes as one `opaquePayload` datum. The same wire format
provides `jsonField` addresses for exact JSON Pointers.

A final `end` row carries decimal-string counts of source, use and field rows.
Consumers should require this row, check the counts and references, and verify
the binding against the consumed artifact before relying on any marker. The
companion describes the tool's observation of the input and output.

## File publication

Cooperating writers share one persistent `.p4kx-origin.lock` per destination
directory. The tool stages each file independently. Once the artifact is ready
to replace its previous version, the tool removes the previous companion before
replacing the artifact, even when the emitted bytes are identical. An interruption
can leave an artifact with unknown origin until a later successful run publishes
its companion. File publication and companion publication are separate operations.

Explicit stdout companion paths may replace a prior stdout companion. Canonical
file companion names, publication lock names and unrelated existing files are
reserved. The write guards reject destinations that alias the observed input or
lie inside a protected installation.

## CHF conversion

`p4kx chf to-json` addresses the exact scalar and empty-container JSON values
with JSON Pointers. The original CHF values and its label hashes come from the
observed standalone file. A label that uses an Archive name records the item
Record's port or the material lookup and material Entry that supplied it. The
material coordinate records the original material ID and, for parameter names,
the attribute's ordinal across that material's `PublicParams` elements. Names
that lose the lookup and unread optional Entries supply no label data.

`p4kx chf from-json` records the emitted binary as one opaque datum and binds it
to the observed JSON conversion. An earlier loose file receipt does not establish
original P4K lineage for a later read. Both file-output commands add
`originReceipt`, `fieldOrigins` and `provenance` to their `--json` summaries.

With raw JSON on stdout, `chf to-json --origin-stderr` emits the JSONL companion
from its header through its end row in place of the usual name-count line.
Archive warnings may still appear before or after that block. Use
`--origin-out <file>` for a companion file. Raw stdout retains its original JSON.
