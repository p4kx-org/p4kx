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

Cooperating writers use `.p4kx-origin.lock` to serialize publication in each
destination directory. Receipt writers that create the lock retain it there.
The tool stages each file independently. Once the artifact is ready
to replace its previous version, the tool removes the previous companion before
replacing the artifact, even when the emitted bytes are identical. An interruption
can leave an artifact with unknown origin until a later successful run publishes
its companion. File publication and companion publication are separate operations.

Explicit stdout companion paths may replace a prior stdout companion. Canonical
file companion names, publication lock names and unrelated existing files are
reserved. The write guards reject destinations that alias the observed input or
lie inside a protected installation.

Ordinary atomic replacements retire proven companions for the replaced file
before rename and preserve unrelated files. A replacement shares an existing
publication lock or creates a temporary lock in a fresh destination. The
temporary lock is moved out of the shared name before it is removed. Successful
ordinary writes leave only their requested outputs in fresh destinations.
Existing receipt locks remain in place. If temporary lock retirement fails,
the error says whether the artifact was committed. A counted write that keeps
the existing bytes keeps their proof and needs no publication lock.

An ordinary replacement of a symbolic link changes its directory entry. The
writer preserves the referent and receipts proven to address other names.
A directory receipt with an ambiguous lexical match is retired.

## Directory publication

A directory writer binds its committed files in the reserved
`.p4kx-origin.jsonl`. The `header` binding type is `directory`. Each
`artifact` row gives the exact relative basename, measured SHA-256 and
decimal-string byte length. An `artifactTableEnd` row gives the number of
artifact rows in their publication order. Source and use rows follow this
table. Each `artifactField` row identifies a zero-based artifact ordinal
as a decimal string and the address within that file. The final `end`
row counts these fields. Readers verify both counts, every reference, and
the named file's actual bytes before relying on its origin.

Cooperating writers hold the same directory lock while publishing the
artifacts and their companion. Each file retains its own staged-write
commit. The first replacement invalidates the old directory companion
before the artifact is renamed. A new companion identifies only files
committed by that run, including the successfully written files of a
partial run. A run that fails before replacing any file can keep the
earlier companion. An interruption after invalidation can leave published
bytes with unknown origin.

A file replacement also invalidates any existing sibling or directory
companion that covers the replaced name. A verified directory artifact
table for disjoint names remains usable after a single-file replacement.

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

Text `p4kx chf diff` uses `textBytes` addresses when requested with
`--origin-stderr` or `--origin-out <file>`. Each address has a half-open byte
range and a logical field such as `/changes/0/old`, `/changes/0/status` or
`/identical`. The status describes an added or removed value. A completed
comparison with changes exits with 1. Its receipt still ends with the same
completion row and binds the exact stdout text.

## Movie rendering

`p4kx ui render` publishes a sibling `<file>.p4kx-origin.jsonl` for PNG and SVG.
The companion binds the committed bytes as one opaque payload. It records the
actual movie geometry, bitmap mips, font outlines, layout metrics and language
values used by the renderer. SVG also records the input to its document title.
Partial render reports retain known inputs and mark an unresolved contributor
set as incomplete.

`--json` adds `originReceipt`, `artifactOrigin`, `fieldOrigins` and
`provenance`. The stage and explicit size options have independent origins.
A diagnostic field whose contributing inputs cannot be established carries an
unknown marker.

## Screen rendering

`p4kx ui render --screen` and the UI tab publish the same sibling companion for
PNG and SVG Screen exports.
Its artifact use includes the measured DataForge payload and the texture
mips, vector files, font definitions and winning language values used by
composition and drawing. A PNG records sources used by actual paints.
An SVG also records sources embedded in its document. Screen dimension
values identify the exact canvas Record fields used to derive them.
Explicit dimensions and command options have their own tool-authored origins.
A report value whose contributors cannot be established has a null marker.

## Hologram rendering

`p4kx ui hologram` publishes `<file>.p4kx-origin.jsonl` alongside the PNG.
The opaque payload row binds the committed filename, byte length and SHA-256.
Its uses include the placed and drawn model reads, selected material slots,
authored port placement and camera settings. Model and character-definition
files without retained internal coordinates carry an explicit opaque use.
Unobserved inputs keep the contributor set incomplete. A known encrypted
input still gives the artifact a positive marker.

## CHF GLB

`p4kx chf glb` writes a sibling `<output>.p4kx-origin.jsonl` companion. Its
file binding records the final GLB basename, byte length and SHA-256. The
opaque payload row links the character input and the model, rig, material,
palette, texture and animation reads used by the written model. The same
companion identifies glTF data in the JSON and binary chunks. A
`glbJsonField` path is a JSON Pointer within the GLB JSON chunk. A
`glbBinaryBytes` path identifies the written `/bufferViews/N` entry and
provides its absolute, half-open byte range in the file, excluding alignment
padding. Each addressed datum carries its own contributing source set. A
DataForge selector identifies the original Record and stored array positions
used to choose an item model or material library. A selected encrypted
DataForge field can mark the result when the chosen model and texture files
are clear. Shaped surface and morph values also identify the actual CHF
face blend coordinates they consume. UV and index ranges have independent
origins. The receipt also identifies CHF colour and numeric values actually
used by a material factor or a baked image.

The `--json` report adds `originReceipt`, `fieldOrigins`, `provenance` and
`textureImageOrigins`. Field origins use JSON Pointers into the report. The
texture array gives the origin of each embedded image by its zero-based GLB
image index. A normal image can use a clear color mip while a roughness image
from the same texture uses an encrypted alpha mip. The image origin also
includes the selected material, any contributing layers and an applied
palette when their values affect the image.

Original P4K lineage for the standalone CHF remains unknown. Report values
whose contributing reads cannot be proved carry an incomplete origin. A
successfully written partial model has a companion for its actual bytes and
retains its existing issue and exit-code behavior.
