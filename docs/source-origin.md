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

Cooperating writers exclusively create `.p4kx-origin.lease` to serialize
publication in each destination directory. The same namespace reservation
applies through native paths and writable Documents portal grants. Existing
empty regular `.p4kx-origin.lock` markers remain reserved and untouched.
The tool stages each file independently. Once the artifact is ready
to replace its previous version, the tool removes the previous companion before
replacing the artifact, even when the emitted bytes are identical. An interruption
can leave an artifact with unknown origin until a later successful run publishes
its companion. File publication and companion publication are separate operations.

Explicit stdout companion paths may replace a prior stdout companion. Canonical
file companion names, publication lock names and unrelated existing files are
reserved. The write guards reject destinations that alias the observed input or
lie inside a protected installation.

Ordinary Archive extraction and loose folder conversion retire proven companions
for replaced files before rename and preserve unrelated files. If the target
and both receipt forms are absent, an ordinary new file can use an atomic
creation that cannot replace another file. Other writes use the destination
directory's publication lock. A directory or redirect at the reserved lock name
causes a destination conflict when a replacement needs the lock. A native file name ending
in `.p4kx-origin.jsonl` remains usable when the path is free of an owned companion.

The writer moves its lease out of the shared name before removal. If a process
ends without releasing its lease, another writer refuses the existing name
and preserves the file. If retirement fails, the error states whether the artifact
was committed. Conflict policies that keep existing bytes also keep their proof.

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

## Action-map exports

Action-map file exports publish a sibling `<file>.p4kx-origin.jsonl`. The raw
JSON and CSV retain their existing fields and bytes. Use `--origin-out <file>`
for a companion bound to stdout, or `--origin-stderr` for the JSONL block on
stderr. Both switches can be used together. The stderr block starts at the
`p4kx-source-origin` header and ends at its end record. Human diagnostics may
appear outside that block. Require a successful exit and verify the binding,
counts and references before using the proof.

JSON addresses are pointers into the exported object. CSV addresses use
`/rows/<index>/<column>` with zero-based data rows and the exact header name.
Their half-open byte ranges include any CSV quotes and formula neutralisation.
An empty cell has a zero-length range at its own logical address.

The companion retains the selected profile's original text XML attribute
range or binary CryXml node and attribute positions. A label records the
authored selector and the language Entry consulted. The selected INI value
uses its byte range in the decoded INI. An unresolved label records the
lookup without a payload use. A placeholder that bypasses the lookup
carries the profile selector alone. Bindings identify their own input and
their inherited or overridden activation mode. A device cell uses the
bindings actually joined into it.

## Localisation tables

`p4kx loc export --out <file>` publishes `<file>.p4kx-origin.jsonl` beside
the table. JSON addresses use `/languages/<column>`, `/entries/<row>/key`
and `/entries/<row>/texts/<language>`. CSV addresses carry the same logical
fields and half-open byte ranges, including quotes or formula neutralisation
as emitted. An empty CSV cell may have a zero-length address.

The row key uses the exact decoded source spelling and the earlier language
lookups that selected it. Translation cells retain the row-key selectors and
the actual language's lookup, selected payload and conversion evidence. An
authored empty value has a source payload with a zero-length range. An absent
value has only lookup evidence. A language selected by an argument is a
tool control. An automatically selected column comes from the Archive's
directory survey. File summaries also include provenance for their key count
and language values.

Use `--origin-out <file>` for a durable stdout receipt or `--origin-stderr`
for a JSONL block after any localisation notes. Both preserve the original
JSON or CSV stdout bytes. Require the terminal record and verify the binding
before using origin markers.

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

## Standalone WAV

The WAV sibling companion addresses each source-based header or loop value
and the sample body as a `wavBytes` field with a half-open output range.
The CLI summary records the source uses for the written frame count, RIFF
size and loop separately from the companion's output byte ranges.

For PCM and ADPCM, the companion identifies the selected format, the data
length used to admit frames and the sample bytes used by the writer.
`wemImaAdpcmByteRuns` indices record a decoded WEM base offset, occurrence
count, byte stride, field offset and field length. They omit each IMA
sub-block's reserved byte. The byte containing the final used nibble is
retained. A truncated trailing sample frame or discarded block does not
contribute audio bytes.

For Wwise Vorbis and Opus, the companion distinguishes original packet
ranges checked during preparation from the ranges replayed for PCM.
Vorbis records the packet lookahead used to reconstruct the final
decoded packet. Opus records the replayed packet-size words from its
selected seek table when one exists. A loose WEM retains its original
file identity with unknown Archive encryption.

## Audio in extraction and folder conversion

Conversion of loose PCM, ADPCM, Vorbis and Opus WEM files to WAV records the
WAV fields and the sample or packet ranges that contributed to them. For
Vorbis and Opus written as Ogg, the companion records the output page groups
and original codec ranges described below. Both companions record the physical
Archive Entry or the observed file that supplied the WEM.

For a soundbank member, the companion records the physical bank, the selected
DIDX ID, offset and length words and the DATA section size. Every `bnkWem.*`
location starts with the half-open decoded bank range of the member, followed
by the original WEM-relative coordinates for that location.

Linear WAV files selected automatically under `--audio ogg` also carry a
companion. The `sound_artifacts` JSON field points to newly committed
companions still associated with their artifact when the run finishes. An
artifact retained by the conflict policy retains its companion.

The Sounds window, `p4kx wwise extract` and `p4kx entity audio --extract`
publish the same audio byte-range companions for written WAV and Ogg files
and an opaque payload companion for unchanged WEMs. The origin comes from
the Entry that supplied the bytes. Embedded WEMs include the selected
member's DIDX and DATA ranges. Trigger names and their resolved paths have separate
source evidence. The result names the newly published companion
and leaves kept files and their existing companions in place.

The CLI file reports use `fieldOrigins` for trigger selection, WEM identity and
measured file size. The size cites the bytes used by the completed write, even
when the companion could not be published. A kept file has an unknown original
size source. Its planned trigger and WEM still describe the attempted export.
Failure rows cite the original trigger and terminal declaration. A fileless
source can cite its authored source ID in the explanation even when its WEM ID
is null. Missing search evidence and unobserved operational error sources remain
incomplete or unknown. A skipped TXTP event reports its authored trigger and
event under `/txtp/skips`. Its reason can cite the terminal declaration that
blocked playback. A publication error without bound inputs keeps unknown origin.

## Unchanged files in extraction

The Extraction report's `raw_artifacts` entries give the relative file path,
byte length, SHA-256 and companion path for each reported unchanged file.
The companion addresses the full output as one opaque payload and gives the
decoded Archive Entry or the observed standalone file. The same receipt covers
the original bank retained during unpacking and a loose audio file written
unchanged when no audio conversion applies or completes. For unchanged embedded
WEM, prefetch and MIDI data, the companion identifies the selected DIDX
words and the exact retained byte range within the bank DATA section. The
standalone file's original P4K encryption state is unknown. A folder scan
excludes recognized P4KX companions and admits ordinary files with the same
suffix. Kept files retain their existing proof. The report uses null
`origin_receipt` if a new file is admitted without the publication lock or if
foreign content occupies its companion path. A replacement requires the guarded
lock, retires a previous owned companion and preserves foreign content.

## Standalone Ogg

The companion for [Ogg conversion](cli-json.md#p4kx-audio-ogg-archive-entry---out-file---json)
uses contiguous half-open `oggBytes` ranges for the identification pages,
the comment and setup pages and the audio pages through EOS.

For Wwise Vorbis, the setup contribution includes the original setup
packet and the packet prefix used to select its codebooks. The audio range
uses the replayed packets. Opus records the replayed packet-size words when
it uses a seek table, the original embedded header pages when present and
the exact framing and duration bytes used to derive Switch Opus pre-skip.
An original embedded Ogg page can contribute to more than one output range.
The summary records its own source uses for the written sample count,
playback rate, duration, codebook choice, size and loop.

## Hologram rendering

`p4kx ui hologram` publishes `<file>.p4kx-origin.jsonl` alongside the PNG.
The opaque payload row binds the committed filename, byte length and SHA-256.
Its uses include the placed and drawn model reads, selected material slots,
authored port placement and camera settings. Model and character-definition
files without retained internal coordinates carry an explicit opaque use.
The model and material selectors retain independent DataForge Record and
array positions. A selected sub-geometry can inherit a material from its
main geometry, and the two uses preserve their own original fields.
When an admitted piece uses a model node for its placement, the original
helper name, local matrices and parent links are bound to the actual
primary model Entry. The `modelNodePlacementBytes` indices give the chunk
ordinal, node ordinal and half-open range in the decoded Entry. They include
CrCh parent target chunk IDs when the route uses them. The model selector
also contributes when the helper's own mesh is absent from the scene.
Successful child offset inversions also retain their node route. Bone and
animation inputs outside these routes keep the artifact's contributor set
incomplete.
A port anchored to a parent model bone uses `modelBonePlacementBytes`.
Its indices identify the physical skeleton chunk, bone ordinal and byte
range in the selected Entry. The selected name and the parent and
relative-pose fields belong to the effective main or LOD skeleton.
Equipment anchored through a parent skin uses the original character-root
rig when that rig supplied its placement.

For a landing gear used by the scene, the receipt records the Entity's
`landingSystem` reference and the original `gears` array slot's
`geometry.path`. The authored `bone` contributes when placement
evaluates it against the parent model, including when the name chooses
the origin fallback.

A deployed landing gear contributes animation origins only where its keys
supply a used bounds vertex or an admitted visible triangle range. The
receipt identifies the physical CAF or DBA Entry, original clip and
controller ordinals, and last key ordinal in each used channel through
`gearDeployClip`, `gearDeployController` and `gearDeployKey`. It includes
the original CDF skeleton `File` and the effective `.chrparams` clip and
winning folder or database selectors when used. Parameter locators use
`gearParameterTextBytes` for half-open original text byte ranges and
`gearParameterCryXmlAttribute` for the original node and global attribute
ordinals.

The actual CDF main model selection is recorded as the selected `Model.File`
or attachment `Binding`. A `CA_SKIN` preference also records the
winning attachment's `Type`. A CDF `Material` attribute contributes when
that override supplies a used surface or suppression after applying
loadout and Entity geometry overrides. Main model attributes use
`cdfModelTextBytes` for original half-open text byte ranges and
`cdfModelCryXmlAttribute` for original node and global attribute ordinals.
These selectors also apply when the main model supplies a consumed node
or bone transform.

CDF bone attachments retain the type and binding that selected the
model. A placed attachment retains its bone name and the authored
relative transform it used. Text attributes use the half-open original
byte range in `cdfAttachmentTextBytes`. CryXml attributes use the original
node ordinal and global attribute table ordinal in
`cdfAttachmentCryXmlAttribute`. A material attribute contributes when
its selected library supplies a used surface or suppression. A placed
attachment with a resolved bone also contributes the original skeleton
pose fields.

Direct mesh conversion uses `modelMeshNodeBytes` for the node matrices,
parent links and CrCh parent IDs that supply the bounds of an admitted
piece or the vertices of a visible draw range. The first two indices
identify the actual LOD and decoded mesh ordinal, followed by the chunk
ordinal, node ordinal and decoded-Entry range. A suppressed mesh counts
only where it sets an admitted bound. A mesh that uses its skin skips its
node matrix.

The receipt also records the original library slots that suppress a piece
or a visible range. A whole-piece suppression affects the scene bounds and
counts. Suppressing one range of a drawn piece affects the PNG.
Unobserved inputs keep the contributor set incomplete. A known encrypted
input still gives the artifact a positive marker.

## Model inspection

Installed LIVE 13220821 and PTU 13260598 each yielded 20 Model chunk rows from seven
checksum-verified Entries. The comparison checked the physical owner and header ranges for eight
primary chunk rows and 12 geometry companion rows per Build. It also checked the primary path
used by each companion lookup and the original container and kind ranges for all nine `read` and
11 `skipped` rows per Build. File and usage origins carry incomplete contributor sets. Synthetic
coverage includes encrypted and unbound inputs, the `unknown` usage state and MCP pagination.

The same installed comparison checked 182 node rows and 729 original source
sites in each Build. An independent read of the primary Entry's authored node
chunk matched the node count, names, parent links, geometry type and helper
flag, including their exact field coordinates and physical source identity.
Synthetic fixtures check original IVO and CrCh transform and property ranges,
loose inputs and the shared MCP node page.

On LIVE `13220821` and PTU `13260598`, the installed verifier independently
read one primary compact skeleton Entry for each Build. The CLI at
`1b18d6a7040b3530a2e59721d287a5afa18277e6` matched all 182 bone rows,
4,551 field-origin addresses and 2,548 pose words on each Build. The
comparison checked original chunk selection, per-field byte ranges, source
roles and the physical Entry behind every matching source-use reference.

## Standalone model GLB

`p4kx model glb` and **Export as GLB** in the GUI model window publish a
digest-bound sibling `<file>.p4kx-origin.jsonl` using the same writer.
The GUI reports an export as finished once both files are published. A failure
publishing the companion after the GLB commits reports that the GLB was written
and leaves no stale receipt.
The `glbJsonField` pointers at `/accessors/N/count` identify emitted primitive index
accessor counts. When the writer has the original IVO submesh, the receipt binds
the count to its exact four-byte word in the selected geometry Entry. Loose models
and unbound submesh layouts receive an explicitly incomplete origin.
The pointers at `/meshes/N/primitives/P/material` also carry the exact two-byte
IVO selector when it chooses an authored sub-material. The reference stays
incomplete because the final GLB index depends on material ordering and reuse.
Single-material libraries, fallback and unbound references report unknown.
The opaque artifact row includes the observed geometry contributions and remains
incomplete while other layout, material, texture and animation inputs are unresolved.

The `--json` report carries `fieldOrigins` and `provenance` for the fields of
its `unknown_chunks` rows. The paths use the row number in the filtered report.
A numeric type ID cites the observed chunk-table word in its owning Archive
Entry. Unbound chunk fields, including loose model inputs, report unknown origin.

The same report identifies the selected Archive directory path at `/model`
and the original model's library-name field at `/library/path`. Loaded
material fields describe the observed MTL payload and the model selector
used to find it. Resolved target and status fields keep partial evidence
where lookup decisions are unavailable. A failed library read adds its
attempted Entry to the inventory without assigning it to material values.
Loose inputs use explicit unknown markers. The report's `materials` count
describes what the GLB writer emitted.

The report binds each animation name, additive flag and duration to the selected
CAF or original DBA clip. Its animation rows include selected clips with zero
written channels, so the report row index can differ from the GLB animation index.
Duration uses the clip timing words and the original key when it extends past the
authored end. Unread rows retain the observed source path, authored name and
additive rejection flag where available. An observed failed decode contributes an
attempted payload. Missing coordinates and loose animation inputs receive unknown
origins. Baked channel and key counts and skipped controller IDs carry explicit
unknown origins. Skipped morph-weight counts receive an origin when present.

## Interior GLB

`p4kx interior glb` publishes a sibling `<output>.p4kx-origin.jsonl` through the guarded
file writer. Its artifact record binds the exact produced GLB and retains the observed
container payloads used to reach the requested scene. The artifact's source set is
incomplete because the opened container chain alone cannot establish all scene,
geometry, material and light contributions.

The JSON report adds `originReceipt`, `fieldOrigins` and `provenance`. Each `containers`
element identifies the stored path from the actual owning directory. The inner path
inherits the encryption of its containing Entries. Unproved derived counts and issues
keep explicit unknown field origins.

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

## Fitted vehicle paint

A `p4kx entity glb` with a selected fitted paint publishes a digest-bound
`<file>.p4kx-origin.jsonl`. Its `glbJsonField` addresses point into
`/asset/extras/p4kxPaintAppearance`. The selected appearance kind references the original
variant occurrence and the fitted paint's tag field. For a compatible explicit replacement it
also retains the observed required tags, including an effective vehicle XML assignment.
The material library and palette Record GUID link to the actual selected payloads and
retained selectors. Unknown contributors keep their field's source set incomplete.

## GUI assembly GLB

Entity and Interior GLB exports write a sibling `<output>.p4kx-origin.jsonl` and expose its
path in the finished export summary. The receipt binds the committed GLB bytes, the
explicitly opened container chain, and observed POSITION accessor counts with their
producing Archive geometry chunks. A count keeps
incomplete coverage when the export has no proof of all contributing values. The
receipt does not establish complete material, light, transform or container-declaration
coverage. An origin publication failure after the GLB has been committed is reported
as a failed export without a stale receipt.

## GUI Character GLB

The Character window publishes its single-file GLB with a digest-bound sibling
receipt. The GLB keeps the selected appearance, textures and morph targets.
The receipt uses the rigged Armour writer's observed mesh and worn-mount
fields. Unobserved contributors leave the artifact origin incomplete.
The export summary exposes the companion path for single-file GLB outputs.

GUI Model, Character, Entity and Interior exports report a stop during
precommit path identity checks as cancellation. A failed cleanup or lock
retirement retains its error, including when the stop request is active.

## Rigid worn mounts

`p4kx armour glb` publishes a sibling `<file>.p4kx-origin.jsonl` for emitted rigid
worn mounts. Each `glbJsonField` points to the actual child entry under a bone
or an emitted transform of the mounted root. The contributors include the
original physical port record, the selected bone fields and the applied item
locator when observed. An unproved contributor set stays partial.

Character packages carry the same projection under `rig.worn_mount_provenance`
as of package format 1.21. The package validates the bound GLB file and its
addressed fields at import. See [Blender package provenance](blender-package.md#worn-mount-provenance).

## Merged DDS

`p4kx texture merge` writes `<file>.p4kx-origin.jsonl` beside the standard DDS.
`ddsHeaderBytes` covers `[0, endExclusive)`. `ddsMipBytes` covers one emitted
mip surface in `[start, endExclusive)`. Mip level 0 is the largest and the
surface ordinal follows DDS order. The offsets and surface ordinal are decimal
strings. The source-use row records the original decoded input offset and byte
length, with the DDS header recorded as a selector when it determines the mip
layout. Unused sibling candidates and a stripped CryEngine trailer supply no
output field. Loose DDS files identify the observed file input and retain
unknown original P4K encryption.

## Bulk texture conversion

Archive extraction and loose folder conversion write a sibling `.p4kx-origin.jsonl`
for each newly merged DDS,
including the separate alpha DDS when no PNG consumes it. The companion binds the
emitted file's byte length and SHA-256. Its header and mip ranges use the same
addresses as `texture merge`. Stable loose contributors are observed files with unknown original
P4K encryption and complete actual source sets. Files kept by the conflict
policy retain their existing companions. If a companion fails after its DDS was
renamed, the output remains merged and the run reports the companion failure.
A DDS replacement without a new receipt retires an owned companion before replacing
the file, even when the bytes match. This applies to Archive fallback and loose-folder
conversion. A foreign file at the sibling path stays.

The `texture_artifacts` field in `p4kx extract --convert --json` and
`p4kx convert --json` binds each written PNG or OpenEXR image to its
target-relative path, byte length and SHA-256. Its direct encryption marker
and source-use references describe the decoded bytes for that particular
mip, array layer, cubemap face or volume slice. References resolve in the
texture's shared inventory. A `ddsMipPayload` use records the original
decoded Entry's mip level, byte offset, span length, stride and span count.
PNG uses the observed alpha tail when merged. OpenEXR uses the colour mip.
Unused sibling parts are outside the image's contributor set. For a folder
without proof of original P4K lineage, the marker is unknown. A conflict
policy that retains a file leaves its origin outside the current report.
