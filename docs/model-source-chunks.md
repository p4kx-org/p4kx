# Model source chunk-table observations

`ModelChunk.layout` retains the existing chunk parser's table facts when the model reader builds
its ordered usage inventory. This applies to every actually read root, geometry, LOD geometry and
LOD skeleton-model file, including a VisAreas-only LOD. No additional parser or companion read is
performed. Existing node, mesh, bone, material, geometry and effective-LOD behavior stays the same.

`p4kx_extraction::ModelChunkSourceInfo::of(&ModelChunk)` projects one such observation into a
versioned canonical source DTO. It reuses `ChunkInfo::of` for the logical file label, original table
ordinal, known type name or null, type ID and coarse usage. The existing `ModelInfo` and `ChunkJson`
outputs keep their current fields and numeric meanings. The source DTO is an additive API; it is
not automatically emitted by a command, provider or package writer.

```json
{
  "schema_version": 1,
  "chunk": {
    "file": "model",
    "index": 0,
    "type": null,
    "type_id": 3735928559,
    "usage": "unknown"
  },
  "layout": {
    "container": "#ivo",
    "version": 4294967295,
    "id": null,
    "byte_order": "little",
    "offset": "32",
    "span": {
      "basis": "derived-ivo-next-boundary-including-padding",
      "bytes": "3"
    }
  }
}
```

The example is synthetic. The DTO's `JsonSchema` implementation describes the serialized shape;
nullable `chunk.type` and `layout.id` keys remain required. New unsigned 64-bit wire values use the
shared `DecimalU64` convention. This does not change the existing `ChunkJson.offset` and `size`
numeric output. `schema_version` is the DTO version, not a game, container-header or chunk version.

| Field | Meaning |
| --- | --- |
| `chunk.file` | Existing logical role: model, geometry or a level-specific model/geometry label. It is not a physical source identity. |
| `chunk.index` | Original table ordinal within that logical file; aliases retain separate ordinals. |
| `chunk.type` / `type_id` | Existing canonical type mapping; unknown names remain null with the complete numeric ID. |
| `chunk.usage` | Existing read/skipped/unknown classification. Read does not claim every field or payload byte was interpreted. |
| `layout.container` | Actual parsed signature profile, `#ivo` or `CrCh`; no source header version is invented. |
| `layout.version` | Parsed chunk-layout version. In CrCh the endian flag is removed; this is not the original unmodified table word. |
| `layout.id` | Declared CrCh chunk ID, or explicit null because Ivo declares none. |
| `layout.byte_order` | The existing reader's data byte order; retained input bytes are not transformed. |
| `layout.offset` | Declared offset within this exact owning decoded file, encoded as decimal text. |
| `layout.span` | Declared CrCh byte length, or the existing Ivo reader's inferred span to the next greater offset/table/file boundary, including padding. |

CrCh uses `basis: "declared-crch-size"`; Ivo uses
`basis: "derived-ivo-next-boundary-including-padding"`. Ivo has no declared size. Duplicate offsets
share a derived span but remain separate table rows. A known unused chunk or unknown version is
retained without claiming a new physics, morph, cloth or other semantic decoder.

Only actually parsed files have these observations. Not-read/missing/unsupported inputs do not
gain fabricated tables. Existing typed reader errors and `LodSkeletonOrigin` states keep those
outcomes distinct; a missing resolver result remains distinct from proven physical Archive absence.

No field here is a usable RawReference or proof of source identity, publication, retention, range
validity or a range digest. A later model provider must bind the logical role to the actual shared
input receipt and published raw artifact, prove the parser-input identity, and validate the exact
span before exposing usable ranges. It must retain every ordinal, including Read chunks and aliases.
Necessary conversion reads, callback selection and raw-publication selection remain separate; the
excluded-container NoOpen rule remains binding. This DTO introduces no input-policy exception.

Nodes, original stored transforms, bounds, stream descriptors, source/output submesh ranges,
source-vertex arrays, bone provenance, checked model processing and actual GLB/artifact mappings
remain separate model-export work. Existing bit-preserving float representations must be reused
when those source DTOs are added. No complete model provider or package-format change is advertised.
