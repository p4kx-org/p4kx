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

For doors, proximity sensors and entities with an inline default loadout, `class` links the
original `EntityClassGUID` to an entry in `classes`. That table identifies the DataForge record
by its physical ordinal. A duplicate GUID retains every candidate. The class components
describe class defaults; the occurrence retains its own inline overrides. An inline geometry
component's `geometryPaths` gives the original paths, including any empty declaration.

A door class's `fragment` selects named nodes in its controller definition and the ordered
fragment alternatives in its Mannequin database. Controller and database resources retain
the exact archive entry, and the selected fragment and animation rows retain XML positions.
`physicalClip: unresolved` means the authored animation selector has no proven CAF/DBA
subrecord identity in the scene export. A name can occur in multiple databases. Class
loadouts preserve original port and item selectors and the hierarchy of XML `Items` groups,
including explicit empty groups. `classSensor` identifies a placement whose uniquely matched
class contains the proximity sensor component, including placements without an inline sensor
declaration. A proximity sensor retains its class's original exit
delay and, for the declared box shape, the sensor-local extent and offset. Its authored
links identify the target independently of trigger shape.

An entity with authored light components includes `lights` and/or `lightGroups`.
Modes retain source order, including repeated or empty declarations and `offState`. Type, intensity,
temperature, style and the temperature switch carry their original value states and
source uses. Colour elements, empty baked sections and relative-frame declarations
point to their original XML coordinates in the same chunk. These fields describe
authored inputs to light assembly.

For direct lights, `effective.class` reports the selected class or its explicit
absence, ambiguity or read failure. A nonzero `EntityClassGUID` takes precedence.
The original `EntityClass` name is used when the GUID is absent or zero.
`lightClasses` stores the original `LightComponentParams` scalar values once per
physical Record. Component and preorder ordinals distinguish repeated declarations.

`effective.fields` shows where a unique class field is inherited or overridden by
a matching XML attribute. Invalid or repeated overrides are retained as invalid,
including cases where a usable class default exists. The field origin includes
the XML and DataForge sources used for selection and interpretation. Positioned
array leaves remain available under the class defaults. Baked light members keep
their original source declarations.

A light group's `helper` records its original selector. An entity's
`tileAttachments` refer to the port declarations of its own descriptor and preserve
`ItemPortIndex` independently from the exact port name and its bracket suffix.
Each matched or ambiguous candidate retains its original position and interaction
offset in the container owner's frame. An unavailable descriptor leaves the
target incomplete. The source row does not assume a container-to-owner transform.

An assembled Entity can anchor lights of a directly declared vehicle container when
its owner frame and descriptor resolve. The group origin moves to the item-port
offset in the owner frame. The move preserves the group's basis and offsets between
baked lights. Nested and standalone containers keep their original placement. The
Assembly exposes the original light frame and separate correction evidence. A
unique explicit helper, or a unique MultiLight point from the effective owner's
vehicle implementation, can also provide a helper-local rest frame. This binding
preserves the owner and helper evidence without choosing animation or power states.

Each sourced value includes `containsEncryptedSource`, `sourceSetComplete` and
`sourceUsesRefs`. References resolve in the same document's `sourceInventory`. The inventory
distinguishes selector and payload roles. A matched link includes both the selecting source
and the observed target contribution. For raw XML rows, the same filter is directly available as `containsEncryptedSource`. The
exact source use is the XML document's mandatory physical source and chunk identity, together
with each row's original local coordinate. The document carries their common
`sourceSetComplete`. The declaration state stays separate from the encryption
state.

## Binary placements

When a selected stream has binary IncludedObjects chunks, its
`includedObjects` indices refer to the top-level table with the same name.
Each physical member and chunk ordinal is stored once. Two containers or
stream declarations can refer to the same physical chunk while retaining
their own occurrence coordinates. The chunk's `byteOffset` and
`byteLength` are relative to the decoded SOC member. Each `raw.value`
is the original chunk encoded as standard padded Base64.

`tables` separates model, material and tint-palette slots. Each slot keeps
its ordinal and its complete 256-byte range within the chunk. Paths retain
empty and repeated spellings. Invalid UTF-8 or missing NUL termination has
an explicit invalid interpretation and the original slot remains in `raw`.
A known kind-1 placement records the original model index and byte at
palette selector offset 172. The nullable local slot links describe
in-range table indices. Material entries are separate declarations.
Their presence does not establish a material override.

Placement transforms retain row-major binary64 values in container space,
with Z up and translation in metres. A nonfinite component is null in the
numeric matrix. Its original word remains available in `raw` at the
transform field's byte range. `prefixF64` retains the two numeric triplets
at row offsets 4 through 51. Their role is unconfirmed. Kind-16 rows and
unsupported or truncated tails retain their original ranges. An unmeasured layout keeps its
opaque payload. `tables.sourceSetComplete` and `tables.value.complete`
describe the available framing. A complete range or value carries its own
source origin even when the rest of the table is incomplete.

The source locations use `socIncludedObjects` and indices
`[chunk ordinal, member-relative start, member-relative end]`.
These intervals address the decoded entry. Offsets stored inside each table
row address the chunk itself. The Blender extractor and standalone package
validator check that each stream reference is distinct and belongs to its
physical member. The optional hierarchy continues to render the usual scene
meshes and lights.

Each stream's optional `geometry` list addresses a model slot by its
physical SOC chunk ordinal and local model ordinal. The selected tier uses
the same path spellings as the preview model provider and searches the
actual container chain from the nearest owner out. A tier records the
owner depth and matching path form. Missing and invalid selectors remain
visible and all duplicate entries in the first matching tier are retained.

A target reference uses archive directory evidence. It carries no model
payload contribution. The encrypted SOC selector may mark the lookup as
encrypted even when the target directory is clear. Directory coverage
evidence for proving uniqueness or absence is incomplete in this
projection. `sourceSetComplete` remains false for directory lookups
while their known contributing sources remain filterable. The same binary
table can be referenced from different branches with different target entries.

## Blender package

`p4kx interior package <archive> <container>... --out <folder> --authored-soc` includes the
same source document at `authored/soc-scene.json`. The package manifest 1.13 references this
file with `authored_soc`. Blender 5.3 imports the companion as a separate **Authored SOC**
collection of non-rendering empties for containers, rooms, doors, sensors and area segments.
Authored local transforms use the source Z-up basis and metres. A missing or invalid
transform is marked unavailable. Visible meshes and lights use the package scene as usual.
