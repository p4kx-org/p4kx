# Authored SOC scene

`p4kx interior source <archive> <container>... --out <file>` writes the authored inputs of one
container and its declared nested placements. It replaces the target after capture and writing
succeed. The [JSON Schema](../schemas/soc-scene.schema.json) defines the
`p4kx-soc-scene` format and its independent `schemaVersion` 1.

`occurrences` is a parent-before-child list. A placement identifies the parent occurrence and
the ordinal in its `children` array. Reusing one container creates separate occurrences with
the same physical source reference. Each occurrence carries its selected descriptor, room
registry, stream selectors and nested child selectors. Missing, ambiguous, unreadable and
invalid selections have explicit states.

XML elements and attributes keep their original source coordinates. Text positions are
half-open byte ranges. Binary positions are CryXml node and attribute table indices. A SOC XML
coordinate is relative to the chunk, whose physical ordinal, byte offset and length appear
with it. The top-level `documents` array stores each physical source and chunk once.
Descriptor `data`, registry `data.xml` and chunk `xml` use zero-based document indices.
Document `nodes` are in source order, with zero-based parent indices and sibling
ordinals. Typed rows use the same source coordinates. A room link targets a physical room
declaration in that occurrence's registry. An entity link targets a physical stream, chunk
and entity inside its owning container occurrence. Cyclic references remain ordinary links.

Room segment coordinates are stored in the original segment-local frame. Quaternion order is
W, X, Y, Z. Triangle-soup points retain their original order. Source XML attributes remain
available alongside typed values, including additional attributes and repeated declarations.
Inline loadouts keep their declaration, group and item entry order. A nested loadout references
its parent entry inside that component. A stored empty entries element is an explicit empty
group. An omitted declaration has no corresponding row.

Each sourced value includes `containsEncryptedSource`, `sourceSetComplete` and
`sourceUsesRefs`. References resolve in the same document's `sourceInventory`. The inventory
distinguishes selector and payload roles. A matched link includes both the selecting source
and the observed target contribution. For raw XML rows, the same filter is directly available as `containsEncryptedSource`. The
exact source use is the XML document's mandatory physical source and chunk identity, together
with each row's original local coordinate. The document carries their common
`sourceSetComplete`. The declaration state stays separate from the encryption
state.

## Blender package

`p4kx interior package <archive> <container>... --out <folder> --authored-soc` includes the
same source document at `authored/soc-scene.json`. The package manifest 1.13 references this
file with `authored_soc`. Blender 5.3 imports the companion as a separate **Authored SOC**
collection of non-rendering empties for containers, rooms, doors, sensors and area segments.
Authored local transforms use the source Z-up basis and metres. A missing or invalid
transform is marked unavailable. Visible meshes and lights use the package scene as usual.
