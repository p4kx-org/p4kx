# Levels of detail, tangents, vertex colours and the node chain

What the reader of `p4kx-model` assumes about these four, and what the real Archives say about it.
The tests are `crates/p4kx-extraction/tests/real_archive/model_lods.rs` and
`real_archive/vertex_attributes.rs` (`#[ignore]`d, run with `P4KX_DATA_P4K`, and with
`P4KX_DATA_P4K_B` for a second Archive; they pass on LIVE). Surveyed on 2026-10-20.

## Levels of detail

Level 0 is the geometry companion `<name>m` (`foyer.cgfm`). Level `n` from 1 on is
`<stem>_lod<n>.<extension>m` (`foyer_lod1.cgfm`); `foyer_lod1.cgf` beside it is a chunk file of
one `MeshInfo` chunk (216 bytes in the example) and carries no nodes, which is why the nodes of
every level come from the base `.cgf`. Model Media preserves the original positioned words
of the base file's measured `LodDistances` layout. The relation between individual words
and runtime switch thresholds or companion levels remains unverified.

LIVE holds 80,537 geometry companions of levels 1 to 5 (`.cgfm` 68,205 of LOD1, `.cgam` 8,669,
`.skinm` 3,663). Their levels are gapless but for `freelancer_pfx.cgfm`, which has only LOD4, so
the reader probes levels 1 to 15 and keeps the gaps. 28 skins also have a `_lod0` file; it is not
read (level 0 is `<name>m`).

The test reads 120 models with a `_lod1` file per extension (`.cgf`, `.cga`, `.skin`) and checks
that the levels ascend from 0 and include 1, that every mesh belongs to a node of the base file,
that 90 % of the level pairs have no more triangles than the one before, that every level
exports as a valid GLB, that a level read alone equals the same level read with the others, and
that a level the model lacks is `NoSuchLod`.

## Tangents

`QTANGENTS` (370,905 mesh chunks of LIVE) and the 8-byte `TANGENTS` stream (20,131, the skins)
store a tangent frame per vertex. The tangent is the packed tangent (`QTANGENTS`, the handedness
is bit 31) or the rotated X axis of the quaternion (`TANGENTS`, the handedness is the sign of W).
A 16-byte `TANGENTS` layout (two quaternions in Cryengine-Converter) occurs in none of the
391,036 mesh chunks of LIVE (`the_streams_of_every_mesh_chunk_are_counted_by_tag_and_element_size`
counts every tag and element size), so its layout cannot be confirmed. The reader does not read it
(ADR-0114); a chunk that holds one has the stream skipped like any unknown tag.

The test derives a tangent from each triangle's positions and texture coordinates
(`dP/du`, and `dP/dv` for the bitangent) and compares: of the vertex corners with a clear
direction (|cosine| above 0.5) the decoded tangent points along `dP/du` in 98 % (`.cgf`), 99 %
(`.cga`) and 99.9 % (`.skin`), and `cross(normal, tangent) * w` points along `dP/dv` in 98 %,
99 % and 98.5 %. The remaining corners are triangles with mirrored or degenerate texture
coordinates. So the tangent runs along the U axis and the bitangent `cross(normal, tangent) * w`
along the V axis of the texture coordinates as stored (V down the image, as glTF).

This frame is the engine's own, with the bitangent pointing down the image. glTF's bitangent
points up it, which is what viewers without a `TANGENT` attribute derive and what its normal maps
(green up) assume; the model keeps the engine's `w`, and the GLB writer negates it and flips the
green channel of the normal maps (ADR-0042, docs/model-materials.md).

## Vertex colours

Each vertex of the 16-byte (quantised) and 20-byte (float) vertex stream carries four colour
bytes before the two half floats of the texture coordinates. Of the meshes of 400 files per
extension, 286 of 774 (`.cgf`), 787 of 944 (`.cga`) and 293 of 400 (`.skin`) hold a colour other
than opaque white. The most frequent are white with a varying alpha, magenta (`.cga`), red masks
(`.cgf`), a purple (`.skin`) and black.

The order of the bytes is blue, green, red, alpha (BGRA). StarBreaker reads it so and
Cryengine-Converter reads red, green, blue, alpha; `p4kx` follows StarBreaker (ADR-0028) and the
`B8G8R8A8` format CryEngine uses for vertex colours. The names of the models confirm it: the
review grouped the models of LIVE (`.cgf`, `.cga`, `.skin`, LOD0) by colour words in their file
names and counted the files that are mostly red or mostly blue under the BGRA reading (|R−B| above
64 on more than a quarter of the vertices). Files named `blue` are 16 red and 104 blue, `red` 60
red and 20 blue, `orange` 84 and 4, `yellow` 52 and 4. The most frequent colours of the orange
files are orange (164, 104, 70) and red (137, 0, 0). Read as RGBA, the same files would turn out
the other way round. The test `models_named_after_a_colour_have_it_under_the_bgra_order` of
`real_archive/vertex_colours.rs` asserts it on whole words of the file names (LIVE: `blue` 16 and
105, `red` 120 and 38, `orange` 84 and 4, `yellow` 56 and 4). A reader that finds the order wrong swaps one
function.

The second colour stream (`COLORS2`, `0xD9EED421`) is not read.

## The second texture coordinate set

The `NORMALS2` stream (`0x38A581FE`) with an element size of 4 bytes occurs in 17,902 mesh chunks
of LIVE, 316,111,502 vertices in all. Read as two
half floats per vertex, as the first set is, 316,111,489 of them are finite, 97.8 % lie in
−64…64 and 95.0 % in 0…1 (the first set of the same chunks: 66 %); read as a packed unit vector
they were uncorrelated with the normals of the tangent frames of the 17,902 chunks. Of the first
2,000 triangles of each chunk 55 % have an area in this set. So the stream is a second UV set, the
one lightmaps and detail layers use, and `Mesh::second_uvs` holds it (ADR-0114). It is never the
only normal source of a chunk, so it is no longer a normal source. The GLB writer does not write
it: no material the writer emits samples a second set, and `TEXCOORD_1` would grow every such
mesh by 8 bytes per vertex.

`real_archive/second_uvs.rs` repeats the survey (`the_four_byte_second_normal_stream_holds_half_float_texture_coordinates`)
and checks the model reader on a sample of models.

## The node chain

The nodes of a `.cga` or `.cgf` carry a transform to their parent, and the vertices of a mesh
are in its node's space. glTF has the same structure, so the writer gives every node its
transform and parent. To see whether that is right, the test places each mesh's vertices through
the whole chain of its node and compares the bounds with the box of the `MeshInfo` chunk, which
the model reader does not use. Of the models with several nodes, in a sample of 600 files per
extension of LIVE, the chain fits the box for 27 of 32 `.cgf` and 77 of 118 `.cga` models; the
node's own transform alone fits 22 and 41, and no transform 22 and 44. The review compared the
orders on 150 models: the chain as implemented (child to root, each node's 3×4 matrix applied to
the point) fits 104, the reversed order 79, the transposed matrices 87 and no transform 66.

Of the 46 misses, about 14 are displaced rather than larger than their geometry, and most of
those match the untransformed box: the `MeshInfo` box is in local space for some models, e.g.
`landing_pad_entrance_a.cgf`. The other misses have a larger box than their geometry (hidden or
removed parts). The test lists the misses, with whether the untransformed box fits, and asserts
that the whole chain fits better than either alternative and at least half of the models.
