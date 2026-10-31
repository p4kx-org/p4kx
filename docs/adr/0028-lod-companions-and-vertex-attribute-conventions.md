# LOD companions and the conventions of tangents and vertex colours

Decided for issue #103. Evidence: docs/model-lods-and-vertex-attributes.md.

Amended by ADR-0114: the 16-byte tangent layout is no longer read. Amended by ADR-0119: the
opt-in lenient level `LodSelection::OnlyOrBase` falls back to the base companion. Amended by
ADR-0150: an Assembly reads a model that has no level at or below the request at its lowest level.

## LOD companions

The meshes of a level of detail from 1 on are in `<stem>_lod<n>.<extension>m` beside the model,
e.g. `foyer_lod1.cgfm` for `foyer.cgf`; level 0 stays in `<name>m`. `foyer_lod1.cgf` itself holds
a `MeshInfo` chunk alone and is not read. `_lod0` files (28 skins in LIVE) are not read. The nodes of every level are those of the base model
file: the submeshes of a LOD companion name the nodes of the base `NodeMeshCombo`.

- `Model::read` reads every level that has a companion, probing levels 1 to 15; a level without
  companion is left out, so the levels may have gaps. `Model::read_with(.., LodSelection::Only(n))`
  reads one level and fails with `NoSuchLod` when it is missing, so exporting one LOD does not
  decode the others.
- A LOD companion without mesh or with a corrupt mesh fails the read, naming the companion.
- The chunks of a LOD companion are listed in `model info` as `lod<n>-geometry`.
- A model that holds its mesh itself, or no mesh at all, has level 0 only.
- There is no fallback from a missing LOD companion to the base companion: `--lod 3` of a model
  without LOD3 is invalid input (exit code 2), not LOD0 under another name. ADR-0119 adds the
  opt-in `LodSelection::OnlyOrBase` for callers that place a model; the strict selections and the
  CLI keep this rule.

## Vertex colours

The vertex stream stores a colour of four bytes before the texture coordinates. The bytes are
read as blue, green, red, alpha and written as RGBA, after StarBreaker and the engine's
`B8G8R8A8` vertex colour; Cryengine-Converter reads them as RGBA. The names of the models of LIVE
confirm BGRA: counting the files that are mostly red or mostly blue under that reading, those
named `blue` are 16 red and 104 blue, `red` 60 and 20, `orange` 84 and 4, `yellow` 52 and 4; the
frequent colours of the orange ones are orange (164, 104, 70) and red (137, 0, 0). A real-Archive
test asserts it. The choice is one function (`VertexStream::colour`).

A mesh whose colours are all opaque white has none (`Mesh::colours` is `None`), because viewers
multiply the material's base colour by `COLOR_0`. The GLB writer writes `COLOR_0` for the other
meshes, on the primitives whose material takes them as its colour (ADR-0253, ADR-0318), unless
asked not to (`GlbOptions::vertex_colours`, `--no-vertex-colours`).

## Tangents

`Mesh::tangents` holds a unit tangent and the handedness in `w`: the bitangent is
`cross(normal, tangent) * w`, as in glTF. They come from the tangent frame streams of 8 bytes
(`QTANGENTS` and `TANGENTS`); the unverified 16-byte layout gave none and is no longer read (ADR-0114). The bitangent points along
the V axis of the stored texture coordinates, which glTF shares (V down). The frame is the
engine's own. ADR-0042 settles what the open question left to the material export: glTF's
bitangent points up the image, so the GLB writer negates `w` and flips the green channel of the
normal maps; `Mesh::tangents` stays as stored.

## Considered Options

- Fall back to the base companion for a missing LOD (StarBreaker): hides that the LOD is absent
  and makes `--lod n` of different n identical; rejected as the default, and offered only as
  the opt-in `LodSelection::OnlyOrBase` that reports an issue (ADR-0119).
- Stop probing at the first missing level: a model of LIVE has only LOD4, and probing is a
  lookup.
- Read colours as RGBA (Cryengine-Converter): contradicted by the colour words in the file names,
  see above.
