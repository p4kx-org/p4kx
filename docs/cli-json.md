# JSON output of `p4kx`

Every `p4kx` command takes `--json` (before or after the subcommand). With it, the command writes
exactly one line of compact JSON to stdout and nothing else; the exit codes stay the same. Errors
that end the command (an unreadable Archive, invalid input) are still printed to stderr as
`error: …` and produce no JSON. Warnings, such as an unreadable build manifest, go to stderr.
So does the test-build notice: the first time a run opens a given Archive of the `PTU`, `EPTU`,
`HOTFIX` or `TECH-PREVIEW` Channel, one line `notice: …` on stderr says that the Channel carries
Star Citizen test builds whose material is confidential (community tests under the Terms of
Service, `HOTFIX` treated the same way, `EPTU` possibly under a non-disclosure agreement). `LIVE` and
Archives in other folders get none. The notice never reaches stdout, so the JSON stays as it is.

Paths of files on disk are strings as the operating system displays them. Entry paths are given
as stored in the Archive, with `\` as separator. Sizes are byte counts. Snapshots of every shape
live in [`crates/p4kx/tests/it/snapshots`](../crates/p4kx/tests/it/snapshots).

## `p4kx media export <archive> --out <directory> --json`

The successful response is `{"status":"published","index":{...}}`; `status` is `published`,
`unchanged`, `skipped` or `incomplete`. The `index` is the exact versioned
[media index contract](media-export.md), independent of the catalog schema. An incomplete response
requires explicit `--diagnostic`, has `index.complete: false` and returns exit code 1. Failures
before publication write no success response and leave the previous complete generation in place.
A stdout reporting failure does not undo an already committed generation.

`p4kx media schema` prints [media-index.schema.json](../schemas/media-index.schema.json).
`--record`, `--reference` and `--source` print the corresponding
[built-in record](../schemas/media-builtin-record.schema.json), [reference](../schemas/media-reference.schema.json)
and [source](../schemas/media-source.schema.json) schemas. These schemas also describe the streamed
files referenced by `index`; the JSONL streams and binary artifacts stay in the output directory.
The library's [generic record envelope](../schemas/media-record.schema.json) stays available without
built-in payload restrictions. Custom provider namespaces remain generic in the CLI schema as well.

[WEM source metadata](media-audio-metadata.md) and [Soundbank metadata](media-bank-metadata.md)
document the typed `audio.jsonl` records, their half-open raw ranges and the field-origin companion.

## Entry

The shape of one Entry in `list` and `search`:

| Field | Type | Meaning |
|-------|------|---------|
| `path` | string | Entry path in the Archive |
| `compression` | string | `store`, `deflate`, `zstd`, or `method-<n>` for an unsupported ZIP method id |
| `encrypted` | bool | Whether the data is encrypted |
| `compressed_size` | number | Size of the data in the Archive |
| `size` | number | Size after decompression |
| `parts` | array of string | Only on the header of a Texture group in `list`: the paths of its parts and Alpha tail; `compressed_size` and `size` are then those of the whole group |
| `metadata` | object | Shared physical-owner, raw DOS and fixed directory facts described in [Archive Entry metadata](archive-entry-metadata.md) |

`metadata.owner_entry_index` identifies the physical base Entry of a grouped Texture;
the existing sizes remain group totals. Its `original_directory_ordinal` is a
canonical unsigned decimal string, or explicit `null` after actual directory
recovery. ZIP-only words and the v2 encryption word remain explicitly `null` in the
other layout. `socpak list` and MCP Entry listings use this same metadata object.

## Build

The shape of a Build in `info`, `channels` and the `source` of the comparison and schema drift
reports; `null` where the Build is unknown.

| Field | Type | Meaning |
|-------|------|---------|
| `branch` | string | e.g. `sc-alpha-4.11.0` |
| `version` | string | e.g. `4.11.42.1234` |
| `change_number` | string | The requested Perforce change number |
| `build_id` | string | The BuildId |
| `public_patch` | string or null | The public patch, e.g. `4.7.2`, from the RSI Launcher log line with the same change number; `null` when the log has none (ADR-0053, ADR-0170) |

## `p4kx list <archive> --json`

```json
{"archive": "C:\\…\\LIVE\\Data.p4k", "entries": [Entry, …]}
```

Entries are in Archive order. A Texture group is one Entry; `--parts` lists every part as an Entry
of its own, without `parts` fields.

## `p4kx search <archive> <filter> --json`

```json
{"archive": "C:\\…\\LIVE\\Data.p4k", "filter": "*.xml", "entries": [Entry, …]}
```

`filter` is the supplied Filter. `entries` holds the matching Entries in Archive order. When nothing
matches, a search without `--exclude` exits with code 0 and returns `entries: []`. With `--exclude`,
an empty selection exits with code 2 and produces no JSON.

## `p4kx info <archive> --json`

```json
{"archive": "…", "channel": "LIVE", "build": Build, "entry_count": 4, "encrypted_count": 1,
 "issue_count": 0, "compressed_size": 35, "size": 118, "metadata": ArchiveLayoutInfo,
 "fieldOrigins": FieldOrigins, "provenance": SourceInventory}
```

`issue_count` is the number of Entry issues found in the directory of the Archive (ADR-0025); `info`,
`list`, `extract` and `verify` print the first ten to stderr as `warning: Entry <path>: <issue>`.
With the global `--recover` flag an Archive whose end records or central directory are damaged is
rebuilt by a scan for directory records; `issue_count` then counts one more issue, the recovery
itself (two when a scan limit cut the scan short), which every command prints once on opening as
`warning: <archive>: <issue>`. The `key` commands ignore `--recover`.

`channel` is the name of the folder holding the Archive, or `null` when there is none; `build` is
`null` when the Build is unknown.

`metadata` is the shared cached `ArchiveLayoutInfo` object: `version`, decimal-string
`sourceLength`, actual `layout`, `declarationState`, and mandatory `zip`/`v2` keys with explicit
null for the absent layout. Nested ZIP/ZIP64/v2 declarations retain their original words and
source-relative extents. This adds no body reads or lazy-row validation. MCP `archive_info`
returns the same object. `fieldOrigins` addresses each layout field at `/metadata/…` in the CLI
and `/structuredContent/metadata/…` in MCP. Resolve each field's `sourceUsesRefs` against
`provenance`. The companion also addresses the four admitted-directory totals and the original
Build manifest fields. The launcher-derived `public_patch` has incomplete contributors.
`issue_count` carries an explicit unknown marker because its full directory-validation receipt
is unavailable. MCP `archive_info`
additionally addresses its listed Issue paths and cached descriptor evidence.
[Archive source layout](media-archive-layout.md) defines the field evidence.

## `p4kx dcb info <archive> --json`

```json
{"archive": "…", "entry": "Data\\Game2.dcb", "version": 8, "record_count": 117709,
 "struct_count": 6696, "enum_count": 774}
```

`entry` is the path of the DataForge Entry as the Archive stores it or the standalone file name.
The `fieldOrigins` map addresses `/version`, `/record_count`, `/struct_count` and
`/enum_count` with their distinct original DataForge header fields. `/entry` uses the Archive
directory or standalone file metadata. Its Archive marker follows the directory scope.
`/archive` records the caller's input path with an empty, complete contributor set. Source
references resolve in `provenance`, which preserves unknown original Archive lineage for
standalone files.

## `p4kx dcb export <archive> --json`

```json
{"archive": "…", "entry": "Data\\Game2.dcb", "format": "xml", "target": "…", "exported": 3,
 "skipped": ["libs/foundry/records/ships/avenger.xml"],
 "failed": [{"guid": "…", "path": "libs/foundry/records/odd", "error": "…"}], "cancelled": 0,
 "error_values": 1, "records_with_errors": 1, "error": null}
```

`format` is `xml`, `json`, `ndjson`, `flat-csv`, `flat-csv-raw` or `flat-ndjson`; the flat forms
write one file of one row per value ([flat values](record-flat.md)) and count and list like
`ndjson` with `values.csv` or `values.ndjson` for `records.ndjson`. `exported` counts the Record files written, or for
`ndjson` the lines of `records.ndjson`. `skipped` lists the files, relative to `target` with `/`
as separator, whose existing file was kept under `--conflict`; for `ndjson` it can only list
`records.ndjson`. `failed` lists the Records not written with GUID, record path and the reason
and its causes joined by `: `. Both are in definition order for `xml` and `json`; for `ndjson`,
`failed` lists the Records of another DataForge first, then the others in record-path order.
`error` is the reason and causes why `records.ndjson` could not be written, otherwise `null`.
`error_values` counts the values written as errors (see
[record-xml.md](record-xml.md#child-elements)) in the Records written or kept, and
`records_with_errors` the Records holding at least one. The exit code is 1 when `failed` is not
empty or `error` is set.

With `--lang <language>` the report also holds `"language": "de"`, the code of the language the
texts were resolved in; without it the field is absent.

When a Record's file does not follow its name or record path, the report also holds `"renamed":
[{"guid": "…", "path": "records/a|b", "reason": "…"}]` with one item per cause, in definition
order; without such a Record the field is absent. `path` is the record path as the DataForge
states it and `reason` says which rule broke and what was done: the Record name is no valid file
name on Linux and Windows and the GUID names the file, or a part of the record path is escaped
(`%` and two hex digits) or, when no escape fits, the Record is written to `%long/<GUID>`
([ADR-0248](adr/0248-written-file-names-are-valid-on-linux-and-windows-on-every-platform.md)).
Renamed Records are written, so they do not make the exit code 1.

With `--tags` the report also holds `"tags": true`: the Records carry the tag path of every tag
Reference ([record-xml.md](record-xml.md#tag-paths)); without it the field is absent.

With `--combined` the report also holds `"file": "Game2.xml"`, the combined file relative to
`target`, and `exported` counts the Records written into it. When an existing file is kept under
`--conflict`, `skipped` holds its name and `exported` is 0; under `if-changed` the document was
still built, so `error_values` and `records_with_errors` still count its Records. A cancelled
run writes no file and counts every Record not failed in `cancelled`. Without `--combined` the
field is absent.

## `p4kx entity loadout <archive> <entity> --json`

```json
{"archive": "…", "entry": "Data\\Game2.dcb", "entity": Node, "ports": 100, "attached": 71,
 "empty": 29, "unresolved": 0, "docked_vehicles": 0,
 "fieldOrigins": FieldOrigins, "provenance": SourceInventory}
```

The `fieldOrigins` map uses JSON Pointers from the report root and resolves its source references
through `provenance`. It covers `entry`, each Entity's `guid`, `record_path`, `name` and
`class_name`, and each port's `name`, including nested ports. The selected port carries the
original DataForge field occurrence or the XML attribute that supplied its spelling. An XML
override carries the winning attribute's source. An unavailable coordinate gives
`sourceSetComplete: false` for that field.

Attached Entity identity fields also carry the selected class or Reference. An XML class includes
the loadout caller and the Include attributes on its actual route. A stored unresolved Reference
has its original GUID receipt at `/error/reference` beneath the port. Diagnostic receipts carry
known inputs and remain incomplete when the failed lookup evidence is unavailable. An applied
`--exclude` or `--replace` carries the authored port-name selectors matched by the operator path
at the port's `source` field. The same selectors supply the exclusion's empty attachment or the
replacement's Entity identity.

A port's DataForge `definition` carries the original declaration selected by the resolver. Its
members retain the original type and subtype occurrences, attachment helpers and individual
position and rotation coordinates. DataForge visibility carries the original `Flags` conversion.
The vehicle's `path` carries the authored request and the selected archive directory entry. A
class-name fallback carries the original Record name. The `patch_path` carries the applied patch
selection, including the original Modification name and `patchFile` attributes when present.
The `modification` field carries the original request from DataForge. When the resolver reuses
one vehicle for another Entity, the file receipts also retain the current caller's authored
cache selectors. Earlier missing candidates and unproved Modification searches leave the affected
field incomplete.

Vehicle visibility uses the first matching effective part and carries its original flag attribute,
including inherited values, patch attributes and the last applied `Elem` value. A missing flag or
`Elem` value carries the original element scope. The vehicle selection supplies additional source
uses to that visibility decision. Visibility stays incomplete while the full effective part
search is unproved. A missing definition without original lookup evidence is also marked incomplete.

An Entity whose default Loadout is an outfit (an NPC actor, `SItemPortLoadoutOutfitParams`) also
has `outfit`: `{"base_files": ["…"], "outfits": [{"tags": ["Outfits.S42.Work"], "files": ["…"],
"applied": true}, …]}`, the XML Loadouts of the base Loadout and, per outfit in the Record's order,
its name tags (tag paths), its XML Loadouts and whether it is worn: every outfit without name tags
is; when all have name tags, the first one is (ADR-0181). The text output lists the same as
`outfit base:` and `outfit N worn|not worn` lines. The key is absent for any other Entity.

`--exclude <port>` and `--replace <port>=<entity>` (each may be given several times) apply an
own configuration, see [ADR-0076](adr/0076-loadout-changes-are-applied-while-the-tree-is-resolved-and-checked-against-the-port.md).
A port is named by its path of port names from the root joined by `/`, as `entity loadout` lists
them (`hardpoint_weapon_nose/hardpoint_class_2`), ignoring case. An excluded port has `source`
`excluded` and holds nothing, so nothing below it is listed; a replaced port has `source`
`replaced` and holds the new Entity with its own default Loadout. `excluded` and `replaced` count
those ports and are absent when 0. A change that is no port path, names a port the tree lacks,
contradicts another, names an Entity that no Record or several Records match, or replaces a port
with the root Entity or an Entity the port lies below, exits with code 2 and prints the reason; a
replacement whose Record cannot be read exits with code 5.

`--loadout-pick <group>=<index>` takes another alternative than the first from a group of a
generated default Loadout (`SItemPortLoadoutRandomGeneratedParams`, which the game picks at
random), see [ADR-0309](adr/0309-the-alternative-of-a-generated-loadout-group-is-picked-by-index.md).
Several picks are separated by commas (`Body=1,Eggs=0`) or given as several options. The group is
its `Group` name in the assortment, ignoring case, in every assortment that has it; the index
counts from 0 and is digits only (no sign). A bare `--loadout-pick <index>` takes it in every
group that has it, and a named pick wins over it. Groups that are not picked keep their first
alternative, so without the option nothing changes.

A named pick that cannot apply is an `assortment` issue of the Entity and the export still
succeeds: an index beyond its group (the group keeps its first alternative), and a group name that
no group of the whole Loadout has (`Bodyy=2`; the issue names the pick). A bare index beyond every
group is no issue, because it means "every group that has it" and none having it is the defined
result; a bare typo such as `9` therefore stays silent. A pick that is no `<group>=<index>` or
index (`Body=`, `=2`, `Body=-1`, `Body=+1`) exits with code 2 before the Archive is opened.
`entity glb`, `entity package` and `ui hologram` take the option too, and the MCP tool
`entity_loadout` its `loadout_pick` parameter, a list of the same texts (commas allowed).

An Entity that does not fit the port's item types, sub-types and sizes is attached all the same
(ADR-0076): the command warns on stderr, exits with code 0, and lists the mismatch in
`incompatible` (absent when empty): `{"port": "…", "entity": "…", "kind": "item_type" |
"item_sub_type" | "too_small" | "too_large", "reason": "…"}`. `entity glb`, `entity package` and
`ui hologram` report it the same way.

`<entity>` is a class name such as `AEGS_Avenger_Titan` (also as the Record name
`EntityClassDefinition.AEGS_Avenger_Titan`), a GUID, or a record path, which is recognised by a
`/` or `\`; case does not matter. An Entity that nothing or several Records match exits with code
2, a root Record that cannot be read with code 5. `ports` counts every port in the tree, and
`attached`, `empty` and `unresolved` count them by what they hold. `docked_vehicles` counts the
attached Entities that are docked vehicles. Ports that cannot be resolved do not change the exit
code.

`--stop-at-docked-vehicles` leaves the ports of docked vehicles out: they stay in their port,
marked, with `expanded` false and no `ports`. Without it they are expanded like any Entity. A
change to a port below such a docked vehicle is not applied and is no error: the port is not in the
tree. The ports as given are listed in `ignored` (absent when empty) and warned about on stderr, so
a typo does not look like a success. The same holds for `entity glb`, `entity package` and `ui
hologram`, which never expand docked vehicles (`entity glb` unless `--expand-docked-vehicles`).

A **Node** is one Entity:

| Field | Type | Meaning |
|-------|------|---------|
| `guid` | string | GUID of the Entity's Record |
| `record_path` | string | Record path without `.xml` |
| `name` | string | Record name, e.g. `EntityClassDefinition.AEGS_Avenger_Titan` |
| `class_name` | string | The Record name after `EntityClassDefinition.` |
| `docked_vehicle` | bool | The Entity is a docked vehicle: its `AttachDef.Type` starts with `NOITEM_Vehicle`, ignoring case, e.g. a snub craft in a ship's hangar; the root Entity is never marked, even if it is a vehicle |
| `expanded` | bool | The Entity's ports are built; false only for a docked vehicle the resolver stopped at (the root Entity is always expanded) |
| `issues` | array | Errors (without target) that do not stop the Entity: a part of its Loadout that cannot be read, an ambiguous class name, a vehicle implementation or Modification that is not read or applied in full, or the port limit |
| `notes` | array | Only when not empty: notes in the form of `issues` that report no problem, as nothing was kept from being read. The only kind is `assortment_requirement` (the Requirement of a loadout assortment or of some of its groups names tags that only the game's spawn knows, so the Archive does not decide it; the assortment or group is used as if it applied; one note per assortment, ADR-0302) |
| `vehicle` | Vehicle | Only for an Entity whose Record names a vehicle implementation that was read: the implementation with the Entity's Modification applied |
| `landing_gear` | array | Only for an Entity whose `VehicleComponentParams.landingSystem` refers to a `VehicleLandingGearSystem` Record: its gears as objects with `bone`, `geometry` (the geometry path, empty when the Record gives none) and `mass` (kilograms or `null`) |
| `ports` | array | One Port per port, see below for the order |

A **Vehicle** is the vehicle implementation XML of
[ADR-0050](adr/0050-the-vehicle-implementation-is-patched-by-the-modification-before-it-is-read.md):

| Field | Type | Meaning |
|-------|------|---------|
| `path` | string | The implementation file that was read, as looked up: the Record's spelling with `/` as separator and without a leading `Data` |
| `patch_path` | string or `null` | The Modification's patch file that was applied, spelled like `path` |
| `modification` | string or `null` | The Modification the Record selects (`VehicleComponentParams.modification`) |
| `name` | string | The `name` of the `Vehicle` element |
| `ports` | array | One object per part with an item port, depth first in file order, parts marked `skipPart` left out: `name` (the part name, which is the port name), `visibility` (`invisible` when the item port's `flags` contain `invisible`, else `visible`), `min_size` and `max_size` (numbers or `null`) and `flags` (as stored) |
| `treads` | array | The parts of class `Tread`: `name` and `wheels`, the part names of the wheels the tread runs on |
| `wheels` | array | The names of the parts of class `SubPartWheel` |

A **Port**:

| Field | Type | Meaning |
|-------|------|---------|
| `name` | string | The port name as the Loadout or the port definition spells it |
| `source` | string | `loadout` (the Entity's own default Loadout), `override` (the Loadout of the entry that attached the Entity, which replaces the Entity's own entry for this port), `port_default` (the default item of the port definition), `unset` (a defined port that nothing names), `empty` (a port only the Entity's vehicle implementation declares: no Loadout entry names it and the Record defines none, so it has no `definition`; `--replace` can attach an Entity to it), `excluded` (left out by `--exclude`) or `replaced` (holds the Entity `--replace` names); new values may be added in later versions |
| `visibility` | string | `visible` or `invisible` as the Entity's vehicle implementation says when it has a part of this name (ignoring ASCII case): `invisible` when its item port carries the flag `invisible`. Without such a part, `invisible` when the port definition carries that flag. Absent otherwise |
| `definition` | object or `null` | The port's `SItemPortDef` in the Entity's Record; `null` when the Record defines no port of this name, e.g. a ship port that only its vehicle definition declares |
| `entity` | Node or `null` | The attached Entity |
| `error` | Error or `null` | Why the attachment cannot be resolved |
| `nested` | array | Ports of the nested Loadout of an empty port, each a Port with `source` `override` and no `definition`; `[]` for every other port |

`entity` and `error` are both `null` for an empty port. A Loadout entry that names no Entity
(an XML `<Item>` with an empty `itemName`, or a manual entry without target) but carries nested
entries keeps its port empty and lists those entries in `nested`, so the gear below e.g.
`Body_ItemPort` of a character stays visible; they count in `ports`, `attached` and `empty` like
every port. The text output prints them indented below the empty port. A **definition** has
`display_name` (a localisation key), `flags` (as stored), `min_size` and `max_size` (numbers or
`null`), `types` (each with `type` and `sub_types`) and `transform`: `null` without an
attachment helper, otherwise `attachment` (`Bone`, `Entity`, `Skin` or `StatObj`), `helper` (the
helper in the parent's geometry), `offset_helper`, `position` (`[x, y, z]` in metres) and
`rotation` (`[x, y, z]` in degrees as stored).

`definition.rotation_constraint` is absent when the port declares no rotation constraint.
Its `no_rotation` object has `state` and `value`. State `present` carries the exact
Boolean, including `false`. A null constraint or value uses state `null`. Unreadable
and incompatible source values have states `unreadable` and `invalid`. These states
carry a JSON null value. An unknown source state uses `unsupported`.
The `fieldOrigins` entry for `no_rotation` identifies the original constraint evidence.
The separate `animation_semantics` value is `unresolved`.
Static placement applies the full helper transform.

An **Error** has `kind`, a stable code, `reason`, text for people, and for an unresolved port the
target: `reference` (the GUID) or `class_name`, the other `null`. The kinds are:

- at a port: `dangling_reference`, `invalid_reference`, `unknown_class`, `not_an_entity`,
  `unreadable_record`, `cycle` (the Entity is already attached further up the branch) and
  `too_deep` (more than 32 attached Entities below the root);
- among the `issues` of an Entity: `ambiguous_class` (several
  Entities have the class name the entry names; the first in definition order is attached and
  the reason lists every candidate), `no_loadout_files` and `xml_loadout` (an XML Loadout or one
  it includes that is missing, corrupt, includes itself or nests deeper than 8 includes; what
  could be read is still listed), `assortment` (a loadout assortment of a generated default
  Loadout is no assortment, names itself, directly or through another, or has a chosen
  alternative that is no Loadout; a `--loadout-pick` index beyond its named group, or a named
  pick of a group that no group of the Loadout has; the other
  assortments still apply; a generated Loadout resolves to the first alternative of each group of
  its assortments unless `--loadout-pick` chooses another, ADR-0289, ADR-0309), and `vehicle_implementation` (the
  implementation file is
  missing, corrupt or no `Vehicle` document, so `vehicle` is absent, or its Modification names a
  patch file that is missing (even when a fallback file is applied), corrupt, outside the implementation folder or too large to apply within the limits, a patch child or `id` that matches nothing in the base and has no base child of its name to replace, or no Modification of that name exists; the
  implementation without the patch is still listed). A landing system that refers to a Record that does not exist or cannot be read gives `dangling_reference` or `unreadable_record` among the issues.

New kinds may be added in later versions; a script should treat an unknown `kind` as an error it
cannot interpret rather than fail.

The tree is resolved from the Records ([ADR-0012](adr/0012-per-port-loadout-override.md)): an
Entity's ports are the entries of its default Loadout
(`SEntityComponentDefaultLoadoutParams`: a manual Loadout's entries, or an XML Loadout read from
the Archive with the Loadouts it includes) in Loadout order, each replaced in place by the entry
of the same port, ignoring case, in the Loadout given by the entry that attached the Entity, with
that Loadout's other entries appended; then the ports its `SItemPortContainerComponentParams`
defines and no Loadout names, in definition order; then the item ports only its vehicle
implementation declares, in file order, with the source `empty`. They count in `ports` and
`empty` of the report like every port. An entry names its Entity by Reference, or by
class name when it has none; of several Entities with one class name, the first in definition
order is attached. The same Archive always gives the same document.

## `p4kx entity glb <archive> <entity> --out <file> --json`

```json
{"archive": "…", "entity": "AEGS_Gladius", "guid": "…", "out": "…", "lod": 0, "parts": 83,
 "modelled_parts": 55, "hidden_parts": 16, "palettes": 12, "nodes": 899, "meshes": 249, "primitives": 948,
 "materials": 204, "textured_materials": 0, "textures": 0, "texture_pixels": 0,
 "texture_fallbacks": [], "default_material_ids": 0, "hidden_primitives": 0, "bones": 0,
 "skinned_meshes": 0, "lod_fallbacks": 0, "shared_meshes": 33, "shared_materials": 103,
 "shared_textures": 0, "vertices": 744831,
 "triangles": 818426, "bytes": 44750744, "skipped": 0, "unresolved_layers": 0,
 "placements": [Placement, …], "animations": [Animation, …], "issues": [Issue, …],
 "fieldOrigins": {"/entity": {…}, "/guid": {…}, "/nodes": {…}}, "provenance": {…}}
```

`fieldOrigins` addresses `/entity` and `/guid` in the selected physical DataForge Record and
`provenance` lists their original sources and uses. Report counters, including the nested Interior
counts when emitted, carry `containsEncryptedSource: null` and `sourceSetComplete: false` until
their own contributors can be established. These metadata fields leave the GLB and the existing
JSON values unchanged.

`unresolved_layers` counts the layer files of the material libraries of all parts, their bone
attachments and the Interior that the Archive lacks, that cannot be read or that exceed the load
budget ([ADR-0291](adr/0291-layer-files-are-read-once-per-model-set-within-a-byte-budget.md)); the
materials that name them keep the colours of their own file. A library lists a file once per
`Submtl` it takes, as the Export as GLB summary counts it. Across libraries, spellings of one path
that differ in case, in `\` and `/`, in a leading `Data/` or in the `.mtl` extension are one file,
which counts as often as its most frequent library lists it. Each library lists at most 256 files, so the number is a lower bound.
`first_unresolved_layer` is the first such path, as the layer names it; it is absent when
`unresolved_layers` is 0 (the example shows the case without any).

`default_material` describes the physical Archive lookup when a drawable assignment needed a fallback.
It reports the state, original Entry and effective encryption marker, decoded SHA-256 when read,
and two original witnesses for an ambiguous lookup. Missing or unusable physical sources leave
the synthetic fallback in use. Authored library failures remain in `issues`.

`<entity>` is named as for `p4kx entity loadout`, with the same exit codes. `animations` is absent
without any; for a creature it lists each clip the GLB writes or leaves out, shaped as in `p4kx
model glb`: `name`, `duration_seconds`, `additive`, `channels`, `keys` and `skipped_tracks`. A
creature whose skins are bound to its skeleton gets, with `--animations`, the clips its animation database names
(`animDatabase3P` of `SAnimatedCharacterComponentParams`, a Mannequin `.adb` with the databases
nested in it), looked for by file name without extension in the animation folders (`#filepath`) and
the track databases of its skeleton's parameter file ([ADR-0307](adr/0307-the-clips-of-a-creature-are-those-its-animation-database-names-and-are-written-on-the-bones-of-its-skeleton.md));
`--animations` asks for them, without it the GLB is static. The optional `animation_sources` object
reports the selected Entity's database and geometry, the skeleton selection and parameter file,
and the physical clip path and original ordinal. Each `graph_selectors` item identifies an
original ADB name occurrence that contributed to that clip's selection, including the traversed
SubADB path. The report includes the source inventory and per-value encryption evidence.
`selection_complete` is false when a preview limit or unread source leaves part of the scan
unavailable. Observed clip contributors are retained in a partial scan. These
are the clips selected before checking whether they fit the exported skeleton. Names no clip
has, such as blend spaces, are skipped, additive
clips and clips that move no bone are left out as `animation_not_used` (one Issue per reason,
`skipped` false), and a database, parameter file or folder the Archive lacks is
`animations_unavailable` (`skipped` false), as is a limit that dropped databases, names or folders. A docked vehicle is
one part without the parts of its own Loadout; `--expand-docked-vehicles` places those too. A
change to a port below an unexpanded docked vehicle, as `entity loadout` lists it, is not applied
and no error; the report lists the ports as `ignored` (absent when empty), as for `entity loadout`.
A replacement that does not fit its port is written all the same and listed in `incompatible` (absent
when empty), as for `entity loadout`.
`parts` counts the root Entity, every attached Entity that resolved and every landing gear of those
Entities ([ADR-0210](adr/0210-landing-gear-are-parts-placed-at-their-bone.md)), `modelled_parts` those with a model, `hidden_parts` those on a port flagged `invisible`, at a port a worn Entity hides by its `HiddenParts` (ADR-0304), or below
one (kept, but with no model, no object containers and not drawn), and `nodes` the glTF nodes: one per part and the nodes of each model. A landing gear is written deployed: posed at the end of the clip of its
skeleton's parameters that extends or deploys it; without one it keeps the pose of its model and an Issue says so. `lod_fallbacks` counts the
models that lack the requested level and are written at the nearest lower one, or at their
lowest level when they have none up to the requested one;
`shared_meshes` counts the meshes not written again because an earlier part holds the same model
at the same level with the same material library and the same palette (their nodes reference one glTF mesh); `meshes`, `primitives`, `vertices` and `triangles` count
stored geometry once, so `meshes` can be lower than the number of mesh-bearing nodes;
`shared_materials` the material requests answered by an equal material already written and
`shared_textures` the texture references that reuse an embedded image. `palettes` counts the
parts drawn with a tint palette ([ADR-0080](adr/0080-assembly-materials-take-the-palette-of-their-part-and-share-textures.md)).
With `--materials textures` the textures of all parts are embedded at the mip `--texture-mip`:
`textures` is the number of PNG images (an image is embedded once however many materials and
parts use it), `texture_pixels` their pixels, `textured_materials` the materials that have a
texture, and `texture_fallbacks` the textures that were left out (objects with `material`,
`role`, `file` and `reason`, as for `p4kx model glb`): their materials keep the plain colour. All
four are 0 or empty without textures.
`default_material_ids` and `hidden_primitives` are summed over all models as for
`p4kx model glb`. `bones` and `skinned_meshes` count the bones written as nodes and the meshes
that carry bone weights: a creature Entity whose root geometry holds a skeleton is written with its
skins bound to it ([ADR-0275](adr/0275-the-skins-of-a-creature-are-written-bound-to-the-skeleton-of-its-root.md)),
otherwise both are 0. `skipped` counts the issues that leave a part or its model out. With `--exclude`
and `--replace`, as for `p4kx entity loadout`, the Assembly is built from the changed Loadout and
the report lists `excluded` (the ports as given) and `replaced` (objects with `port` and
`entity`, as given); both are absent without the options. An excluded branch is no part and no
issue. The file is
written even when parts are skipped; an Archive key that is missing or wrong exits with code 7
and writes nothing, an `--out` that is the Archive with code 2.

With `--interior` the report also has `interior`, an object with `containers` (the top-level
socpak containers of the root Entity and of every attached Entity that were opened, not the
containers nested in them; each sits relative to the Entity that names it; the text
and the window call them object containers, while `containers_opened` of `p4kx interior glb`
counts the nested ones too), `parts`,
`model_parts` (props included), `modelled_parts` and `lights` (as for
`p4kx interior glb`, the lights being written with `KHR_lights_punctual`; the models of the containers are not counted in the Entity's `parts`), and
`issues` lists what the containers could not resolve, with the kinds of `p4kx interior glb` plus
`container_missing` for a container the Entity's Record names and the Archive lacks,
`helper_not_found` for its bone the Entity's model lacks, `offset_differs` for an offset that disagrees with
its bone and `invalid_offset` for one that is not finite, and the limits `too_many_containers`,
`too_many_parts` and `too_many_vertices`, which hold for the Assembly as a whole; their `path` is `Interior` for the root Entity and
`<port path>/Interior` for an attached one, or the container's name (prefixed by the port path
for an attached Entity) followed by the path inside it ([ADR-0086](adr/0086-an-entity-places-its-object-containers-at-its-bones.md)).
Without the flag `interior` is absent.

When the tree has a `Char_Head` Entity and a body can be named, the report has `body`, an
object with `variant` (`male` or `female`), the body the clothing of the character was chosen for,
`head`, the class name of the head Entity, `named_by`, the class name of the Entity that names the
body, and `named_by_head`, whether that is the head. The head names the body by its sub-type; a
generic head names none, and the `Char_Body` Entity names it by its sub-type or the folder of its
model, so `named_by` is that Entity and `named_by_head` is `false`
([ADR-0219](adr/0219-the-clothing-of-a-character-takes-the-sub-geometry-of-its-body.md));
otherwise `body` is absent. The text report has the line
`body <variant>, named by the head <class name>`, or `named by the body <class name>`.

`placements` has one object per part, the root first and every part after its parent, with
`path` (as for an Issue), `placement` (`root`, `helper`, `port_node`, `skin` or `origin`). A `skin`
is a part on a skin port, with or without a helper: it is drawn in the parent's bind pose with its
helper and offset ignored, and without an Issue; a rigid prop on a skin port that is no character
item (the mobiGlas) is a `helper` at the bone its port names in the skeleton of the root
(ADR-0297).

A part on any other port sits at the helper its port names in the parent's model, else at that
bone of the root's skeleton when the parent is a skin part; failing that, at the node of the
parent's model named like the port, or at the parent's origin, which is also where a port with no
helper and no node of its name lands, without an Issue. A landing gear sits at the node of its
Entity's model named like the gear's bone, and at the origin when its Entity has no model.

A placement object also has `helper` (the helper's name, or the bone of a landing gear; only for `helper`), `modelled`
(the part has a model) and `hidden` (the part is hidden, see `hidden_parts`; a part below a hidden
one has no model to anchor to, so its `placement` may read `origin`; so is a landing gear of a
hidden Entity).

A transform of a part that carries a shear is written without it: the glTF node holds a
translation, a rotation and a scale only.

An **Issue** has `path` (the port names from the root joined by `/`, empty for the root; for a
landing gear `…/landing_gear/<bone>` after the path of its Entity), `name`
(the class name, the bone for a landing gear, or what the Loadout names for an attachment that cannot be resolved), `kind`, a
stable code, `skipped` and `reason`, text for people. The kinds are `unresolved` (the Loadout
names an Entity that cannot be resolved, so the port holds no part), `no_geometry` (the Entity's
Record, or the landing gear, names no model; the part stays as an empty node), `geometry_missing` and
`geometry_unreadable` (the model is not in the Archive or cannot be read, or its character
definition names none; the part stays as an empty node; for the Entity itself the command fails
with exit code 2 or 3 instead and writes nothing, ADR-0255), `sub_geometry_missing` (the
sub-geometry the port's flags choose is not in the Archive, so the part keeps the main geometry of
its Entity; a note, `skipped` is false), `body_geometry_missing` (the sub-geometry tagged for the
body of the character is not in the Archive; the part has no model, the placeholder main geometry
is not drawn instead; `reason` names the body and the path), `world_prop_not_worn` (a worn item
of a character whose body is known names only a prop of the world, such as a cardboard box, as its
geometry and has no sub-geometry per body; the part has no model, ADR-0311; `reason` names the
path of the prop), `part_hidden` (the worn clothing or
armour at the port is hidden by the `HiddenParts` of another worn Entity of the same character, so
the part has no model, ADR-0304; `name` is its class name and `reason` names the port and the part
that hides it; a part below it is hidden with it, and an attachment that cannot be resolved at, or
below, such a port is reported as `part_hidden` instead of `unresolved`),
`gear_not_deployed` (a landing gear has no clip that
deploys it, or the clip cannot be read or applied to its model; the gear keeps the pose of its
model; a note, `skipped` is false), `attachments_left_out` (the geometry is
a character definition, `.cdf`. Its main model comes from its first bound skin. When no skin is
bound, P4KX uses a declared `Model File` with drawable geometry at the selected LOD. An expressly
declared rig without drawable geometry owns the rigid nodes, including the first bound attachment,
when that attachment is rigid. Otherwise the first bound attachment supplies the main model, or
the declared file when no attachment is bound. Drawable bone attachments have their own nodes
below the part. The issue lists other bound attachments and unreadable bone attachments once per
definition. `skipped` is false),
`skeleton_missing` and `skeleton_unreadable` (the skeleton file, `Model File`, of a character
definition is not in the Archive, is larger than 16 MiB or cannot be read as a model. The note
can accompany a drawable main attachment. Reported once per definition, `skipped` is false),
`loadout_unreadable` (the Loadout of a creature, an Entity whose root geometry is a character file
below `Objects/Characters` outside `Human`, cannot be read in full, such as an XML Loadout the
Archive lacks, so its body port stays empty; reported on the root, `skipped` is false, ADR-0289),
`material_library` (the library is missing or unreadable; the part is
drawn with the default material; reported once per library, for the first part that draws with
it), `palette` (a tint palette the Entity or its library names
cannot be resolved; the part falls back to the palette of its nearest ancestor, else to the
library's default palette, else to its plain colours), `helper_not_found` (the port's helper, the bone of a
landing gear, or the bone of a bone attachment, is no node of the Entity's model; the part sits at the
parent's origin, a bone attachment at the part's; not reported when the parent has no model,
whose own Issue says why) and `too_many_parts`, `too_many_vertices`
and `too_deep` (a limit of [ADR-0040](adr/0040-the-assembly-places-parts-at-port-helpers-and-owns-their-material-libraries.md)
was reached). `port_helper_unresolved` records that an authored helper selector or tree
could not establish a more specific transform. The Issue text identifies the physical port
and original helper position. A usable root transform still places the attachment.
If a loaded attachment cannot use its declared item offset helper, it keeps the port placement.
`item_offset_helper_not_found` means the attached model lacks the named node.
`item_offset_helper_invalid_placement` means its node hierarchy does not yield a placement.
`item_offset_helper_not_invertible` means the placed node's transform has no inverse.
All three issues have `skipped: false`.
New kinds may be added in later versions.

## `p4kx entity package <archive> <entity> --out <folder> --json`

The report's `fieldOrigins` and `provenance` identify the original DataForge Record that
supplied `entity` and `guid`. Output options have an empty tool-authored source set.
Generated statistics and other values without exact observed contributors carry
incomplete origin coverage.

```json
{"archive": "…", "entity": "AEGS_Gladius", "guid": "…", "out": "…", "lod": 0, "texture_mip": 0,
 "nodes": 83, "drawn_nodes": 56, "containers": 0, "objects": 0, "lights": 0, "meshes": 31,
 "shared_meshes": 25, "materials": 48, "palettes": 3, "textures": 120,
 "texture_pixels": 98304000, "texture_failures": 0, "files": 152, "bytes": 123456789,
 "vertices": 744831, "triangles": 818426, "lod_fallbacks": 0, "skipped": 0, "issues": [Issue, …]}
```

`<entity>` is named as for `p4kx entity loadout`, with the same exit codes. A creature with its
skeleton is written as one rig mesh (ADR-0316); the report then also has `bones` and
`skinned_meshes`, which are absent otherwise. `lod_fallbacks` counts
the models that lack the level of detail asked for and are written at the nearest lower one, as
for `p4kx entity glb`, but a mesh that several nodes draw counts once. `nodes` counts the
nodes of `scene.json`, `drawn_nodes` those with a mesh. `--interior` adds the Interior as for
`p4kx entity glb`; `containers`, `objects` and `lights` count its nodes of the kinds `container`,
`object` and `light` and are 0 without it. `materials` and `palettes` count the material
descriptions and tint palettes of the manifest. `meshes` counts the
GLB files written and `shared_meshes` the nodes that draw a mesh another node draws. `textures`
and `texture_pixels` count the PNG files written and their pixels, `texture_failures` the
textures that could not be loaded or encoded (their materials keep plain colours). `files` and
`bytes` count the manifest, the parts and the textures; `vertices` and `triangles` count each
mesh once. `excluded`, `replaced`, `ignored` and `incompatible` are as for `p4kx entity glb` and
absent without the options. An **Issue** is an entry of the `issues` of `scene.json` ([docs/blender-package.md](blender-package.md)):
`origin` (`assembly` or `texture`), `kind`, `path`, `name`, `skipped` and `reason`; the assembly
kinds are those of `p4kx entity glb`, the texture kinds are `texture` and `texture_path`. An
`--out` that is the Archive, that lies in a Star Citizen installation or that exists and is no
earlier package exits with code 2 and writes nothing; an Archive key that is missing or wrong
exits with code 7.

## `p4kx character package <archive> --out <folder> --json`

```json
{"archive": "…", "name": "masculinedefault", "chf": "…/masculinedefault.chf", "body": "male",
 "rig": "protos_human_male_face_t1_pu", "skeleton": "Objects/…/bhm_skeleton_v7.chr", "out": "…",
 "lod": 0, "loadout": {"name": "…", "source": "entity", "class_name": "…"},
 "replaced": [{"name": "…", "by": "…", "item_type": "Char_Armor_Torso"}, …],
 "parts": [{"name": "body", "origin": "body", "file": "…", "morph_targets": 0}, …],
 "nodes": 1, "bones": 403, "skinned_meshes": 9, "morph_targets": 399, "materials": 14,
 "embedded_textures": 30, "vertices": 118540, "triangles": 151601, "files": 2,
 "bytes": 46406858, "skipped": 0,
 "animations": [{"name": "…", "duration_seconds": 3.27, "additive": false, "channels": 353,
                 "keys": 10714, "skipped_tracks": []}],
 "issues": [Issue, …]}
```

The inputs are those of `p4kx chf glb` and `p4kx armour glb`: `--chf`, `--body`, `--piece`,
`--loadout`, `--expression`, `--no-expressions`, `--materials colours|textures` (default
`textures`), `--texture-mip`, `--anim`, `--anim-from` and `--anim-folder`, see
[docs/blender-package.md](blender-package.md#characters). `--body` is optional with a `--loadout`
whose Entity has a `Char_Head`: the head decides the body, and the head's own skins replace the
generic Protos head (ADR-0109); an outfit Loadout of a story character such as Miles Eckhart gives
its base Loadout and its clothing. With `--chf` beside `--loadout`, the CHF's head is kept and the
head of the Loadout is dropped. `--anim` looks first in the character's own animation folder
`Animations/pu/<body>/<name>` for a bespoke head and then in the folders of the body (`--anim all`
writes the own folder's animations only); `--anim-folder FOLDER` (repeatable) looks only in the
named folders. `name` is the Character's name (`--name`,
else the CHF's file name without extension, else the body variant). `chf` is the CHF path as
given and `null` without `--chf`. With `--default-chf --body male|female`, `default_chf` contains
the selected Archive Entry's original `path` and zero-based `entry_index`. The option cannot be
combined with `--chf`. `rig` names the selected head rig when present. `loadout` is
`null` without `--loadout` and `replaced` lists the pieces of the Loadout that a `--piece` took the
place of, both as for `p4kx armour glb`. `parts` lists the skins worn on the skeleton, the body
first (`origin` `body` or `piece`), with the skin `file` and the number of `morph_targets` of the
part's mesh. `nodes` counts the nodes of `scene.json`, which is 1; `bones`, `skinned_meshes`,
`morph_targets` and `materials` count what the GLB file holds, `embedded_textures` the PNG images
embedded in it, and `vertices` and `triangles` its geometry. `files` counts the manifest and the
mesh and `bytes` their size. `animations` lists each animation written, as in `p4kx chf glb`, with
`skipped_weights` (a number, only when not 0). `skipped` counts the issues that leave a piece out.
An **Issue** is an entry of the `issues` of `scene.json`, with `origin` `assembly`: the kinds of
`p4kx armour glb` and `p4kx chf glb`, among them `animation_not_used`. An `--out` that is the
Archive, that lies in a Star Citizen installation or that exists and is no earlier package, a
`--chf` that holds no face DNA or whose DNA belongs to an unknown head rig, and a `--chf` beside
`--body` exit with code 2 and write nothing; a head that could not be shaped, or expressions that
could not be made, exit with code 1 after the package is written; an Archive key that is missing
or wrong exits with code 7. For `--default-chf`, a missing or ambiguous Entry or a head rig for a
different body exits with code 2. An unreadable or malformed default CHF exits with code 3,
unless the read requires an Archive key. These failures leave the package destination untouched.

## `p4kx interior glb <archive> <container>... --out <file> --json`

```json
{"archive": "…", "containers": ["Data\\ObjectContainers\\Ships\\RSI\\Mantis\\base_int_body_main.socpak"],
 "out": "…", "lod": 0, "parts": 87, "containers_opened": 1, "model_parts": 36,
 "modelled_parts": 36, "props": 21, "lights": 46, "nodes": 205, "meshes": 37, "primitives": 158,
 "materials": 32, "default_material_ids": 0, "hidden_primitives": 12, "lod_fallbacks": 0,
 "shared_meshes": 8, "shared_materials": 118, "shared_textures": 0, "vertices": 170632,
 "triangles": 159528, "bytes": 9931796, "skipped": 1, "unresolved_layers": 0,
 "issues": [Issue, …]}
```

`unresolved_layers` and `first_unresolved_layer` (absent when there is none) are those of
`p4kx entity glb`, for the material libraries of the parts.

`default_material` is the same lookup receipt as in `entity glb`. The canonical fallback comes from the outer game Archive.

`containers` are the Entry paths of the requested chain. A missing or ambiguous Entry in any
explicit segment causes exit code 2. A container that cannot be opened causes exit code 3.
`parts` counts the root container, the `.soc` files, the nested containers, the placed models,
the props and the lights;
`containers_opened` the containers that were opened, the root included; `model_parts` the placed
models and props, `modelled_parts` those whose model was read, `props` the props among the model
parts and `lights` the lights written with `KHR_lights_punctual`; their intensity is the raw
engine value and their colour the stored linear one (with a colour temperature tinted by a black
body), not a conversion to photometric units
([ADR-0087](adr/0087-an-interior-places-the-props-and-lights-of-its-soc-entities.md),
[ADR-0199](adr/0199-interior-props-from-class-records-and-linear-light-colours.md)). When the
Archive has a DataForge, `props` also counts the entities whose geometry comes from their class
Record, and the parts are drawn with their tint palettes. The other
counts are those of `entity glb`. An
**Issue** has the fields of the `entity glb` Issue: `path` is the names of the containers from the
root joined by `/`, `name` the container, `.soc` file or model, and the kinds are
`container_missing`, `container_ambiguous`, `container_unreadable`, `container_cycle` (the
container is nested in itself and not entered again), `file_missing`, `file_unreadable`,
`invalid_placement`,
`objects_left_out`, `unsupported_objects`, `entities_left_out` (props or lights without a usable
placement), `light_unsupported` (ambient, planar or unknown lights, which glTF cannot express; the
`name` is the `.soc` file and the message gives the type and count), `geometry_missing`,
`geometry_unreadable`, `material_library`, `palette` (a tint palette that cannot be used; the
part keeps the next choice), `too_many_parts`, `too_many_containers`,
`too_many_vertices` and `too_deep`
([ADR-0081](adr/0081-an-interior-is-the-socpaks-soc-files-placed-in-nested-containers.md)). The
file is written even when parts are skipped. For `container_ambiguous`, the message gives the
layer and Entry indexes of every physical match at the first applicable path.

## `p4kx interior source <archive> <container>... --out <file>`

The JSON document is written to the requested file. It uses the independently versioned
[authored SOC scene schema](../schemas/soc-scene.schema.json). See [SOC scene](soc-scene.md)
for occurrence ownership, source coordinates and origin fields. Missing or ambiguous explicit
container paths cause exit code 2 before the source file is written.

## `p4kx interior package <archive> <container>... --out <folder> --json`

`fieldOrigins` and `provenance` bind each canonical container path to the original
Archive directory entry selected by the command. A path reflects the encryption of any
outer Container that supplied its directory. Generated counts have incomplete origin
coverage.

```json
{"archive": "…", "containers": ["Data\\ObjectContainers\\Ships\\DRAK\\Cutlass\\base_int_body_main.socpak"],
 "out": "…", "lod": 0, "texture_mip": 0, "containers_opened": 1, "model_parts": 4, "modelled_parts": 4,
 "props": 4, "nodes": 77, "drawn_nodes": 4, "container_nodes": 2, "objects": 4, "lights": 70,
 "meshes": 4, "shared_meshes": 0, "materials": 34, "palettes": 0, "textures": 79,
 "texture_pixels": 5242880, "texture_failures": 0, "files": 84, "bytes": 18847044,
 "vertices": 229192, "triangles": 246389, "skipped": 0, "issues": [Issue, …]}
```

`containers`, the exit codes and `containers_opened`, `model_parts`, `modelled_parts` and `props`
are those of `p4kx interior glb`; a package folder that holds something else than an earlier
package, or lies inside a Star Citizen installation, is 2. The other counts are those of
`p4kx entity package`, with `container_nodes` for its `containers`: `nodes` counts the nodes of
`scene.json`, `drawn_nodes` those with a mesh, `container_nodes`, `objects` and `lights` those of
the kinds `container`, `object` and `light`. An **Issue** is that of `p4kx entity package`. The
package is written even when parts are skipped.

## `p4kx prefab list <archive> [<library>] --json`

```json
{"archive": "…", "libraries": ["Constellation_Cargo_Prototype", "pu/derelict/RSI/Constellation", …]}
{"archive": "…", "library": "pu/derelict/RSI/Constellation", "file": "pu/derelict/rsi/constellation",
 "prefabs": [{"name": "Spc_Sctd_01", "id": "{AD763CD6-CA16-4F91-AC77-6C520949824F}", "objects": 240,
              "nested": 3, "left_out": 0}, …]}
```

The examples are illustrative, with library and prefab names from LIVE and made-up counts. `file` is the library file as `list` and `dump` take it on the command line (`library` is the name the file gives itself and may differ). Without a library, `libraries` are the prefab libraries below `Data\Prefabs`, named like their
files without `.xml`, sorted ignoring case. With one, `library` is the name the file gives itself
(its root's `Name`) and the prefabs follow in file order. `objects` counts the objects of the
prefab with its groups flattened, `nested` those of type `Prefab` that instance another prefab, and
`left_out` the objects whose transform is not usable. A library the Archive lacks is exit code 2,
one that cannot be read 3.

Both list forms include field-level source markers in `fieldOrigins` and resolve their
references through `provenance`. Each `libraries` string uses the corresponding physical
directory path, including when another Entry has the same display name. In the detail
list, `file` uses directory metadata, and the library name and Prefab rows use the
original XML payload. Names and IDs retain their attribute or element location. The
three counts use the relevant original `Objects` subtree or the Prefab element where
the parser determined it was absent. XML repaired during decoding retains its physical
payload source with an explicit coordinate omission.

## `p4kx prefab dump <archive> <library> <prefab> --json`

```json
{"archive": "…", "library": "pu/derelict/RSI/Constellation", "file": "pu/derelict/rsi/constellation",
 "name": "RSI_constellation_001_Plnt_Crsh_01_sand", "prefabs_entered": 17,
 "objects": [{"name": "personal_flare_1_a-001", "kind": "Entity", "id": "{…}",
              "class": "personal_flare_1_a", "library": "pu/derelict/components",
              "prefab": "visual.flare", "container": "Align_Flare_Crsh_01-003", "depth": 1,
              "model": null, "unexpanded": null, "attached": false,
              "position": [-3.077, -2.434, 0.019],
              "transform": [[1, 0, 0, -3.077], [0, 1, 0, -2.434], [0, 0, 1, 0.019]]}, …],
 "issues": [{"container": "", "name": "…", "kind": "missing", "reason": "…"}, …]}
```

The prefab with every nested prefab expanded
([ADR-0182](adr/0182-nested-prefabs-are-resolved-by-guid-with-composed-transforms.md)). `objects`
are in file order, the objects of a nested prefab in place of the object that instances it. `kind`
is the object's `Type`, `class` its `EntityClass`, and `model` the geometry file it names, a path
below `Data`; entities whose geometry comes from their class Record have none. `position` and
`transform` (a 3×4 row-major matrix) map the object's space to the dumped prefab's, `CryEngine`
coordinates, Z up, in metres, with the rotation and scale of every group and nested prefab object
around it applied. `container` is the names of the nested prefab objects the object sits in, joined
by `/`, `depth` their number, `library` and `prefab` the prefab the object belongs to. An object
that instances a prefab that was not expanded stays in the list with its `PrefabName` as
`unexpanded`. `attached` marks an object with another `Parent` than its group or an
`AttachmentTarget`, whose transform is relative to that attachment and is used as it stands.
An **Issue** has `container` and `name` (the object; empty for a whole prefab), `kind` and
`reason`; the kinds are `missing` (no library holds the prefab), `cycle` (the prefab contains
itself and is not entered again), `too_deep` (nested more than 16 deep), `library_unreadable`,
`guid_mismatch` (the prefab of the `PrefabName` has another `Id` than the `PrefabGUID` and no
library holds that one), `objects_left_out`, `attached`, `too_many_issues` (more than 4096 issues; the rest is not listed), `too_many_objects` (more than 524288
objects) and `too_many_visits` (more than 4194304 objects looked at). A library or prefab the
Archive lacks is exit code 2, a library that cannot be read 3; issues leave the exit code 0.

When an instance authors `GUIDRemapping` blocks, `guid_remappings` lists those occurrences
in visit order, including instances that could not be expanded. `instance_path` holds the
zero-based flattened object ordinal in each successive Prefab from the root. Two placements
with the same name have different paths. `target`, when present, gives the entered library
file and zero-based Prefab index. `blocks` holds the ordered lists of authored pairs. Each pair
contains the original `default_guid` and `instance_guid` text, or null when the corresponding
attribute is absent. Empty blocks, duplicate pairs, and invalid GUID text remain visible.
The regular object list continues to report original IDs.

An object with direct `EntityLinks` children adds an `entity_links` array. Each group
has a `links` array of its direct `Link` children. The group and link order, empty
groups and repeated links follow the source. Each link has `name`, `target_id` and
`version` strings from the corresponding attributes. An empty attribute stays an
empty string and an absent attribute is null. `target_id` is an opaque authored
value. Link fields retain the original XML byte ranges or CryXml node and attribute
indices. Collections include the selected child sites and report incomplete coverage
where a filtered sibling's selector site was not retained. When the input requires
repair, the source Entry remains identified and its coordinates are unavailable.

The dump includes `fieldOrigins` and `provenance`. Origin keys are JSON Pointers from the
document root. They cover the selected library, prefab, collections, object fields and
resolution issues. Source uses retain the original XML or CryXml positions and the physical
Archive Entries. For a nested object, its origins include the selectors used along its
instance route. Each vector component and matrix cell has its own origin path. The matrix's
last column uses the same origin as the position. For the other columns, position attributes
are selectors for accepting the local transforms. The position treats leaf rotation and scale
as selectors and retains outer rotations and scales as conversion inputs.

Failed lookups appear as attempts in the inventory. A diagnostic that incorporates content
from the failed input also retains that content's origin on its reason. A global GUID lookup whose
candidate survey could not be established leaves the affected contributor set incomplete. Inspect
`containsEncryptedSource`, `sourceSetComplete` and `sourceUsesRefs` together.

## `p4kx armour bodies --json`

```json
{"bodies": [{"code": "male", "tag": "Male", "skeleton": "Objects/…/bhm_skeleton_v7.chr",
             "skin": "Objects/…/m_body.skin"}, …]}
```

`code` is what `--body` takes, `tag` the tag a piece's Record gives its geometry for the variant,
`skeleton` and `skin` the files of the default body.

## `p4kx armour glb <archive> --piece <entity>… --loadout <loadout> --out <file> --json`

```json
{"archive": "…", "body": "male", "skeleton": "Objects/…/bhm_skeleton_v7.chr", "out": "…",
 "lod": 0, "loadout": {"name": "…", "source": "entity", "class_name": "…", "files": ["…"]},
 "head": {"class_name": "eckhart_t2_head", "body": "male", "bespoke_folder": "Objects/…/eckhart",
          "parts": 6},
 "pieces": 2,
 "replaced": [{"name": "…", "by": "…", "item_type": "Char_Armor_Torso"}, …],
 "parts": [{"name": "body", "origin": "body", "file": "…", "material": "m_body"}, …],
 "nodes": 223, "bones": 220, "meshes": 3, "skinned_meshes": 3, "primitives": 51,
 "materials": 19, "textured_materials": 0, "textures": 0, "texture_pixels": 0,
 "vertices": 43317, "triangles": 68932, "bytes": 3671144, "skipped": 0,
 "unresolved_layers": 0,
 "animations": [{"name": "…", "duration_seconds": 3.27, "additive": false, "channels": 233,
                 "keys": 10714, "skipped_tracks": []}],
 "issues": [Issue, …]}
```

`--materials textures` (default `colours`) and `--texture-mip N` embed the textures as for
`p4kx chf glb`, counted in `textured_materials`, `textures` and `texture_pixels`.
The JSON report adds a `fieldOrigins` map and its shared `provenance` inventory. The selected
Entity Loadout class is tied to the original DataForge Record. Known worn model and material
selectors and the observed base rig contribute to their report fields. Inferred body selection,
XML Loadout paths and other unresolved fields carry incomplete coverage. An explicit `--body`
is a tool-authored selection.

All Armour GLB exports publish a digest-bound `<out>.p4kx-origin.jsonl`. It binds observed
POSITION accessor counts to their original geometry chunks. Mounted items also bind the
actual child entry under a bone and the written transform to the original port and observed
model placement fields. An unproven encryption state stays `null`.
`unresolved_layers` and `first_unresolved_layer` (absent when there is none) are those of
`p4kx entity glb`, for the material libraries of the parts. `files` of `loadout` lists the XML
Loadout files the Loadout is made of in the order they are laid over each other: an Entity whose
default Loadout is an outfit gives its base Loadout and the Loadouts of its outfits that apply
(ADR-0109, ADR-0181), and `outfit` beside `files` (absent for any other source) lists every outfit
as for `p4kx entity loadout`. `head` is `null` unless the Loadout has a head (an Entity of item
type `Char_Head`) and a body can be named; then `class_name` is the head Entity, `body` the variant
its sub-type names, else (a generic head) the one the sub-type or model folder of the Loadout's
`Char_Body` Entity names, which `--body` overrides, `bespoke_folder` the folder of the character's own head skins
(or `null` when its material library is in no folder of its own) and `parts` the pieces of the
head. `material` of a part is the material library it is drawn with, `null` when it has none.

`worn_mount_unresolved` has `skipped: true` when an explicit loadout overlay cannot
bind a rigid mesh to a unique compatible holder port and its helper. `port_helper_unresolved`
has `skipped: false` when a worn piece uses the root helper with an unresolved selector or
stays at the scene origin because it has no usable root helper. The `reason` includes the
original physical port and helper position for that note. A mounted piece keeps its port placement
when its item offset helper is unusable and uses the same `item_offset_helper_…` issue kinds as
`entity glb`.

`--anim NAME` (repeatable, `all` for every one) and `--anim-from ENTRY` write the animations of
the body variant's folders on the body skeleton, those of the character's own folder
`Animations/pu/<body>/<name>` first when the head is a bespoke one and the folder exists, and
`--anim-folder FOLDER` (repeatable) looks only below the named folders; `animations` lists each one written, as in
`p4kx chf glb`, with `skipped_weights` (a number, only when not 0) for runs of morph weights that
did not fit. Three Issue kinds belong to animations: `animation_not_used` (no track fits the
Character, or the animations would exceed the budget of 33,554,432 values for all animations of
one command, the reason then starting with `limit`; the animations are not written; animations
left out for the same reason are one Issue, named `animations` when it sums up several, whose
message gives their count and the names of the first three, and `skipped` counts it once),
`face_animation_failed` (the rig of the face cannot be read, or gives a pose that is not finite,
and the animation has no track for a bone) and `face_joints_failed` (see `p4kx chf glb`).

`--expression NAME` (repeatable) and `--no-expressions` choose the expressions of the head a
`--loadout` gives its character, as for `p4kx chf glb`: all of them by default at LOD 0, none at
other levels. Only a story character's own head with a `.RigLogic` file has any (ADR-0281); a
head without one stays neutral and these options do nothing for it. The Issue kinds of the
expressions are `unknown_expression` (a named one the head lacks), `empty_expression` (a named
one that moves no vertex) and `morphs_failed` (the geometry has no `BShapesGpu` chunk or does not
fit the rig logic or the skin); the head is kept and the file is written. `face_joints_failed`
covers an unreadable `.RigLogic` file and a skeleton file that does not match the skin.

`pieces` counts the pieces: the `--piece` options and, with `--loadout`, the armour and clothing of
the Loadout that were not replaced, including those that could not be resolved. `loadout` is
`null` without `--loadout`, else it has the text given as `name`, the `source` of the Loadout
(`entity` for an Entity's default Loadout, `archive` for an XML Loadout Entry, `file` for a file
on disk), for an Entity its `class_name`, the `files` of the XML Loadouts it is made of and, for
an outfit, `outfit` as for `p4kx entity loadout`. `replaced` lists the pieces of the Loadout that a
`--piece` of the same item type took the place of. `parts` lists what was written: the skin of
the body (`origin` `body`) and one part per piece that was fitted (`origin` `piece`), named after
the piece's class name, or `<port>/<class name>` for a piece of a Loadout, with the model `file`
as the Record spells it. `bones` and `nodes` count the
glTF nodes: the bones, written once, and the nodes of the models. `skipped` counts the issues that
leave a piece out. An Entity that no Record matches, or several do, exits with code 2 and writes
nothing; a skeleton that is missing exits with code 3; an Archive key that is missing or wrong
with code 7, and an `--out` that is the Archive with code 2.

An **Issue** is as for `p4kx entity glb`, with the kinds of the pieces: `piece_not_found` (the
Entity's Record cannot be read), `no_worn_geometry` (the Record names no geometry), `no_variant_geometry`
(the Record names geometry per body, but none for this one; the piece is left out),
`geometry_missing` and `geometry_unreadable` (the model is not in the Archive or cannot be read),
`loadout_unreadable` (the Loadout of a worn Entity cannot be read in full, such as a missing XML
include; the piece is fitted and the issue is a note, ADR-0194),
`main_geometry_worn` (the Record names no geometry per body, so its main geometry, which is not a
skin, is worn on the body; a note only), `bones_not_in_skeleton` (some bones of the model are not in
the skeleton and follow their nearest parent that is) and `not_skinned` (the model has no bone
weights and its port names no bone to hang it on, so it is placed as it is; one that hangs on
the bone of its port is not listed, ADR-0231), both described at the exported level of detail,
`body_zones_hidden` (the reason lists the body zones the worn clothing covers, whose triangles of
the body are left out, ADR-0140), `body_zones_unknown` (no part, or several, of the character is a `Char_Body` with a model, the
skin is not one of the variant's body, or the submeshes of the body skin do not fit the zone
table, so nothing is left out), `clothing_conditions_undecided` (the reason lists zones of the body
that clothing would cover under a condition on ports or tags, which is not evaluated, so the body
stays whole there, ADR-0279), `part_hidden` (the piece sits at a port that another worn piece hides
by its `HiddenParts`, so it is left out, ADR-0279; a note, not a skip), `layers_without_diffuse` (only with `--materials textures`; the
reason counts the layered
materials of the part that have no diffuse map of their own and none in a layer, and are drawn in their
layer colours, ADR-0147) and
`material_library`. `path` is empty and `name` is `body` for the body's skin, else the piece's
class name or `<port>/<class name>`; for `piece_not_found` it is the text given to `--piece`, or
`<port>/<class name>` of the Loadout entry that cannot be resolved (`<port>/<GUID>` when it names
a Reference, `<port>/unreadable` when the Reference cannot be read), or the Loadout itself when
part of the root's Loadout cannot be read. Only entries at the root or below worn Entities
count; an entry below a weapon is not listed (ADR-0194). A `--loadout` that is neither an Entity nor an XML Loadout that can be
read exits with code 2 and writes nothing.

## `p4kx entity audio <archive> <entity> --json`

```json
{"archive": "…", "entry": "Data\\Game2.dcb", "language": {…},
 "entity": {"guid": "…", "record_path": "…", "name": "EntityClassDefinition.ORIG_890Jump",
            "class_name": "ORIG_890Jump"},
 "triggers": [Trigger, …], "counts": {"triggers": 47, "matched": 47, "unmatched": 0},
 "issues": [Issue, …], "fieldOrigins": FieldOrigins, "provenance": SourceInventory}
```

`<entity>` is named as for `p4kx entity loadout`, with the same exit codes. The command reads
every ATL definition below `Data\Libs\GameAudio\` (text XML or CryXml) and every `.bnk`
soundbank of the Archive once. `triggers` lists the audio triggers the Entity's Record names
through `GlobalResourceAudio.audioTrigger` fields and string fields of other structs whose name
contains `audioTrigger` (ADR-0115), each once (names ignoring ASCII case) in the order the Record
first names them. `matched` counts the triggers one of whose Wwise events a
soundbank holds; triggers that reach no soundbank do not change the exit code.

A **Trigger**:

| Field | Type | Meaning |
|-------|------|---------|
| `name` | string | The trigger name as the Record first writes it |
| `location` | string | The first field naming it, e.g. `Components[3].triggers[0].audioTrigger` |
| `references` | number | How many fields of the Record name it |
| `definition` | string or `null` | Path of the ATL definition Entry; `null` when no definition has the trigger |
| `events` | array | One Event per `WwiseEvent` of the definition, in order |
| `unmatched` | object or `null` | Why the trigger reaches no soundbank: `kind` and `message` |
| `sounds` | object | Lists `truncated_by_limits`, `variants` and `breaks` for each resolved trigger with `--sounds` or `--extract`, using the same fields as `p4kx wwise resolve` |

With `--sounds`, each trigger that has a definition is resolved as by `p4kx wwise resolve` (the
soundbanks of the trigger are read again) and the document gets a `language` object with
`requested` and `fallback`, the same as the one of `wwise resolve`; `--lang` and `--fallback-lang`
choose the languages. Without `--sounds` the two flags change nothing but are still checked.

An **Event** has `name`, `id` (its Wwise short ID, the 32-bit FNV-1 hash of the name in lower
case, which is the ID of its Event object in a soundbank's `HIRC` section), `localised` (the
definition's `wwise_localised`) and `soundbanks`, each with `path` and `language` (the language
folder below `Sounds\wwise\`, e.g. `English(US)`, or `null`), in path order. `events` holds at most
the first 256 `WwiseEvent`s of a definition; further ones are left out without an issue.

The `unmatched` kinds are `no_definition` (no ATL definition has the trigger), `no_event` (its
definition posts no Wwise event) and `no_soundbank` (no soundbank holds any of its events). An
**Issue** is an ATL definition or soundbank left out of the index, with `path`, `kind`
(`too_large`: a soundbank above 32 MiB or an ATL definition above 8 MiB, `unreadable`, `atl`,
`soundbank`, `too_many_triggers`: the index already holds 262,144 triggers, or `too_many_media`:
the index already holds 4,194,304 loose WEMs or 1,024 language folders) and `message`; without
`--json` each issue is a warning on stderr. New kinds may be added in later versions.

The JSON includes `fieldOrigins` and a shared `provenance` inventory. The first original DataForge
occurrence supplies the trigger name and location, and every original occurrence contributes to
its reference count. The ATL declaration, matching physical soundbank entries and any survey
needed for an unmatched result carry their own evidence. With `--sounds` or `--extract`, pointers
under `/triggers/{index}/sounds` use the same inventory and include the original DataForge trigger
selector. Files and the report written by `--extract` have a separate output contract.

## `p4kx wwise events <archive> [--search <text>] --json`

```json
{"archive": "…", "search": "spool", "offset": 0, "limit": 100, "total": 210, "next_offset": 100,
 "entities_skipped": null, "unreadable_records": 0,
 "triggers": [{"name": "Play_QDrive_Spool", "definition": "Data\\Libs\\GameAudio\\SC_SSQD_QDrive.xml",
               "matched_trigger": true,
               "events": [{"name": "Play_QDrive_Spool", "id": 2178236557, "localised": false,
                           "matched": true,
                           "soundbanks": [{"path": "Data\\Sounds\\wwise\\SSQD_QDrive.bnk", "language": null}]}],
               "unmatched": null,
               "entities": {"count": 12, "listed": [{"guid": "…", "record_path": "…", "class_name": "…"}]}}],
 "issues": [Issue, …]}
```

`search` is `null` without `--search`. `total` counts every trigger that matched; `triggers` holds
the page from `offset` on, at most `limit` (1 to 10,000) of them, in the order of the ATL
definitions' Entry paths, and `next_offset` is the offset of the next page or `null` after the
last one. A trigger matches when its name or the name of one of its events does; `matched_trigger`
and `matched` say which. `events`, `unmatched` and `issues` are those of `p4kx entity audio`.
`entities` counts the Entities that name the trigger through those fields
and lists the first `--entities` of them in definition order; only Entities whose own Record names
the trigger count, not those that reach it through a Loadout or a Reference. In `--search`, `/`
and `\` match each other. `entities` is `null` with `--no-entities`. It is
also `null` when the DataForge cannot be found, decrypted or read: `entities_skipped` then holds
the reason, a warning goes to stderr and the exit code is 1. Otherwise `entities_skipped` is
`null`. `unreadable_records` counts the Entity Records that could not be read and so name no
trigger; above 0 it is also a warning on stderr. A page without triggers does not read the
DataForge, so `entities_skipped` is `null` and `unreadable_records` is 0 there.

`fieldOrigins` addresses `/triggers/{row}/name`, `/triggers/{row}/definition`,
`/triggers/{row}/events/{event}/name`, and `/triggers/{row}/events/{event}/localised`.
Names use their original ATL attributes. Localisation uses its attribute or the owning
event element when the attribute is absent. The definition path uses the defining Entry's
directory metadata. Equal paths retain distinct physical Entry identities. The source and use
references resolve in `provenance`.

## `p4kx wwise extract <archive> <trigger> --out <folder> --json`

```json
{"archive": "…", "language": {…}, "trigger": {…}, "breaks": 0,
 "target": "…", "format": "ogg", "written": 3, "kept": 0, "unconverted": 1, "failed": 1,
 "cancelled": false,
 "files": [{"trigger": "Play_Multi", "wem_id": 100, "file": "…\\Play_Multi_100.ogg",
            "origin_receipt": "…\\Play_Multi_100.ogg.p4kx-origin.jsonl", "origin_receipt_failure": null,
            "format": "ogg", "size": 4096, "kept": false, "unconverted": null}, …],
 "failures": [{"trigger": "Play_Multi", "wem_id": 104, "reason": "…"}, …],
 "issues": [Issue, …]}
```

`trigger`, `language` and `issues` are those of `p4kx wwise resolve`; `breaks` counts its broken
links. The top-level `format` is the value given to `--format` (`ogg`, `wav` or `raw`); each file's `format` is the one
written, `wav` for a codec without an Ogg form and `wem` (raw) for a sound that cannot be
decoded, which then carries the reason in `unconverted`. A file is named `<trigger>.<ext>` for a
trigger with one WEM and `<trigger>_<wem_id>.<ext>` for several. The name follows the portable
file-name rule of ADR-0248: each character it forbids (`<>:"/\|?*`, control characters), a trailing
dot or space and the first letter of a Windows device name is written as `%` and two upper-case hex
digits, and `%` itself as `%25`; so are `#` and a leading space, which a TXTP line cannot hold, and
an empty name becomes `%`. A name that is still too long for a file (more than 228 bytes) becomes
`sound`. A name already taken under the case fold of that rule (Unicode simple lower case, `\` as
`/`) gets `_2`, `_3`, … `kept` is true for an existing file left alone by `--conflict skip` or an identical one under `if-changed`.
`origin_receipt` names the digest-bound sibling for the newly written audio. It is null for a
kept file and when no companion was published. If receipt publication fails after the sound
commits, its file row carries the error in `origin_receipt_failure` and the command exits with
code 1. Both fields are available in `p4kx entity audio --extract` results.
The extract JSON report also includes `fieldOrigins` and `provenance` for its trigger,
language selection, broken-link count and index issues. The trigger event ID cites the
original event name used in its calculation. The broken-link count cites the observed
resolution routes, including successful routes when the count is zero.
Each `/files/{row}/trigger` and `/files/{row}/wem_id` cites the original route selected by
the export plan. The measured `size` cites the original WEM input used by the successful
write, including a full donor WEM when it replaces a prefetch. The original size source is
unknown for retained files. The same fields appear under `/extraction/files` in entity audio.
`failures` lists the sounds that got no file: source plug-ins and external sources (`wem_id` null),
WEMs no soundbank or loose Entry holds, and sounds whose absolute file path exceeds the limit of
ADR-0248 (260 UTF-16 units for the file, 248 for its folder), each with its reason; the other
sounds are still written. `cancelled` is true when Ctrl+C stopped the export before it handled
every sound; no partial file is left. Exit code 1 when anything is in `failures` or `unconverted`,
or `cancelled` is true.

Each failure retains the original trigger and nullable WEM ID under
`/failures/{row}`. A fileless source's `reason` uses the authored plug-in or
external kind and its declared source ID, which is distinct from the nullable
WEM ID. An unsuccessful media search cites the known selectors with incomplete
evidence. Operational failures report unknown reason origin when their exact
inputs were unavailable. Entity audio carries those origins under
`/extraction/failures`.

With `--txtp`, the object has a `txtp` member, absent without it:
`{"written": 1, "kept": 0, "skipped": 1, "files": [{"trigger": "Play_Multi", "event": "Play_Multi",
"file": "…\\Play_Multi.txtp", "size": 120, "kept": false, "breaks": 0, "approximations": []}, …], "skips": [{"trigger": "Play_Multi",
"event": "Play_Other", "reason": "plug-in source 9 has no file"}, …]}`. One `.txtp` file in
vgmstream's TXTP syntax is written per Wwise event that leads to sounds, named after the event by
the rule of the sound files (`event` for a name too long for a file) and holding the names of the exported files, which lie in the same
folder. Commands use the physical WEM selected for each resolved path, so the same short ID
can appear in multiple sound file rows and name different files in one TXTP.
An Event's actions, blend containers, music segments and music tracks are layers (`group =
-L<n> #@layer-v`), sequence containers and music playlists segments (`-S<n>`), and a random or
switch container or music switch a choice that the file settles on the first child (`-R<n>>1`);
the loop of a sound's WEM is written as `#I <start> <end>`. An event with a sound that has no
whole file (a source plug-in or external source, a WEM nothing holds, only the prefetched start of
a streamed WEM) is listed in `skips` and gets no file, as does an event whose file cannot be
written, e.g. because its path exceeds the limit of ADR-0248. An event with broken links is written
with the header comment `incomplete: <n> broken links`, and `breaks` of its file is `n`. A `group`
line carries only the macro; the comment on the object is the line above it.
Each `files` row includes an `approximations` array for written or verified retained text.
A choice with several rendered alternatives includes `"select-first-alternative"`, also written
in the TXTP header. Rendered MusicTrack sources can add `omit-music-clip-timing`,
`omit-music-clip-events` or `nonfinite-music-clip-timing` from their owning track's clip fields.
A rendered path through a MusicPlaylist, MusicSwitch or Switch MusicTrack can add
`omit-music-transition-fades` for nonzero original fade time or offset. An allocated
transition object with a nonzero segment ID adds `omit-music-transition-segments`.
A rendered path adds `omit-music-transition-scheduling` for a known scheduled
source sync selector, destination entry or jump selector, or an enabled source
post-exit, destination pre-entry or match-source-cue flag on a MusicPlaylist
or MusicSwitch. A Switch MusicTrack can add the same code from its original
transition sync selector. Immediate and unknown sync codes do not prove a
scheduled transition.
A rendered Trigger route can add `omit-music-stinger-scheduling` from the
selected stinger declaration's known non-immediate sync word.
Rendered paths can also add `omit-container-transitions` for a nonzero declared
Random/Sequence transition mode and `omit-blend-curves` for original checked
Blend association curve points. Mode zero does not activate stored timing words.
The same rendered paths can add
`omit-container-loops` for authored Random/Sequence or MusicPlaylist loop controls
beyond a single play, `omit-container-conditions` for checked switch, state or music
decision data, and `omit-music-track-automation` for original track automation points.
`omit-music-track-subtrack-selection` reports the omission of Random or Sequence selection
when the authored track declares more than one subtrack. The origins identify the track kind
byte and count word.
Playlist loop and automation evidence describe the owning playlist or track along the path.
These codes appear in both the JSON report and the TXTP header, including on
single-child paths. The WEM's own loop marker remains part of the file command.
An empty array means the writer emitted no reported approximation code.
Each reported code has a `fieldOrigins` entry at `/approximations/{index}` on the file row.
`sourceUsesRefs` index its `provenance` inventory. Diagnoses for authored container and music
controls cite the first triggering checked fields reached in playback order and their BNK layout
version. This includes the exact loop control, switch default or membership, clip time or event,
automation point count and transition fade or object segment. Evidence for a music switch tree
includes the group count and tree size. A transition object also cites its allocation flag.
`select-first-alternative` describes the first rendered multi-child choice. Nested groups are
considered before their containing group. The file commands within that choice carry the child
links on their resolved paths, including ancestors and single-child groups collapsed by the
renderer. Each child ID uses `[start, endExclusive, physicalChildOrdinal]` in its owning decoded
bank. The evidence includes the owner's type and count and the layout version. Random and Sequence
containers also contribute their mode. Repeated IDs keep their separate positions.
These paths carry their successfully followed Event and Action links.
Each Event action ID is addressed by its original `[start, endExclusive, actionOrdinal]`.
The Action type and target use half-open ranges in the owning decoded bank. An explicitly
selected foreign Play bank also contributes the original bank ID. The source evidence
includes the Event action count and each contributing bank's layout version.
The selection also cites the WEM range used to write each newly written audio file named by
that group. `wemBytes/mediaPayload` has `[start, endExclusive]` in decoded Entry coordinates.
An embedded WEM also cites its DIDX triplet and the selected DATA section. Kept audio has no
new write evidence. The routing and media contributor set remains incomplete.
`sourceSetComplete` is `false` and `containsEncryptedSource` is `true` for a proved encrypted
contributor or `null` while no encrypted contributor has been proved.

`originReceipt` is the sibling `<file>.p4kx-origin.jsonl` for a newly published file. Its digest
binds the exact TXTP bytes and `textBytes` addresses identify the code tokens in the header.
The companion covers the diagnosed fields and the start and end tokens of each `#I` command.
When any sound line has a loop, `commands` lists every rendered sound line in order, excluding
headers and group lines. A row with a rendered loop has frame values at `loop.start`
and `loop.end`. Their immediate field origins are addressed as `/commands/{index}/loop/start`
and `/commands/{index}/loop/end`. The end is exclusive. The sidecar addresses each numeric token
by its exact emitted byte range.

Loop origins use the WEM that supplied the written audio. For embedded WEMs its local coordinates
are prefixed by the physical DIDX section and triplet ordinals. The evidence also includes the
used DIDX triplet fields and checked DATA section routing. When the authored last-frame word is
zero, the end uses that sentinel and the source frame count. A kept audio file leaves both loop
origins unknown in a new TXTP receipt.

A kept TXTP preserves any existing companion and has `originReceipt: null` in the current report.
The current report gives byte-verified retained fields unknown provenance. Text equality proves
their values only.

A retained file has `"approximations": null` when its text was unverified or differed from the
generated text. The sound export determines the exit code and reports missing audio files in
`failures`. A cancellation stops TXTP generation.

`p4kx entity audio --extract <folder>` writes the same files for every trigger of the Entity and
adds an `extraction` object with exactly `target`, `format`, `written`, `kept`, `unconverted`,
`failed`, `cancelled`, `files` and `failures`, as described here; `archive`, `language`, `trigger`, `breaks`
and `issues` are not repeated in it.

## `p4kx wwise resolve <archive> <trigger> --json`

```json
{"archive": "…", "language": {"requested": "German", "fallback": "English(US)"},
 "trigger": {"name": "Play_QDrive_Spool", "definition": "Data\\Libs\\GameAudio\\SC_SSQD_QDrive.xml",
             "events": [{"name": "Play_QDrive_Spool", "id": 2178236557, "localised": false}]},
 "truncated_by_limits": false, "variants": [Variant, …], "breaks": [Break, …],
 "counts": {"variants": 14, "located": 0, "unlocated": 14, "plugins": 0, "breaks": 0},
 "issues": [Issue, …]}
```

`counts.plugins` is the number of variants without a file: source plug-ins and external sources. `<trigger>` is the trigger name as its ATL definition writes it, ignoring ASCII case; a trigger no
ATL definition has exits with code 2. The audio index is built as for `p4kx entity audio`, with
the same `issues`; each soundbank holding one of the trigger's events is then read again and its
`HIRC` section followed from the Event object through Play actions (other actions lead to no
sound) and random, sequence, switch and blend containers to Sound objects. Variants and breaks
come in the order of the trigger's events, their soundbanks in path order and each container's
children in bank order. Broken links do not change the exit code.

`truncated_by_limits` is `true` when the caller's `--max-visits <count>` stops the walk. The budget
covers each attempted HIRC object and each MusicTrack source across the trigger's Events and
soundbanks. Omit the flag for complete traversal. A budgeted inspection still succeeds and
`variants` and `counts` describe the part reached. The corresponding entry in `breaks` gives
the limit and why the walk stopped.

A **Variant**:

| Field | Type | Meaning |
|-------|------|---------|
| `event` | string | The Wwise event it is reached from |
| `soundbank` | object | `path` and `language` of the soundbank whose `HIRC` section leads to it |
| `sound_id` | number | ID of the Sound object |
| `wem_id` | number or `null` | WEM ID of its media; `null` for a source plug-in or an external source, which have no file |
| `source` | string | `embedded` (held in memory), `prefetch_streamed`, `streamed`, `plugin` (generated, no file), `external` (supplied by the game at runtime, no file), `midi` (a MIDI file, located like a WEM) or `other` |
| `localised` | boolean | The source is language-specific or its soundbank lies in a language folder |
| `language` | string or `null` | The language folder the WEM was found in, for a localised variant with a location |
| `fallback` | boolean | `language` is the fallback language, not `requested` |
| `location` | object or `null` | `kind` `soundbank` with the `soundbank` path, `offset` in its `DATA` section and `len`, or `kind` `entry` with the `entry` path of a loose WEM and its `len`; `null` when neither holds the WEM |
| `path` | array | Container path from the Event object to the Sound object, each step with `kind` and `id` |

A streamed or prefetched WEM is looked for as a loose WEM Entry named `<id>.wem` below
`Data\Sounds\wwise\` (also in its `Media` folder): a localised one in the language folder of its
soundbank, else of `requested`, else of the fallback language, else among the WEMs that are not
localised; a prefetched WEM without such an Entry has the start in the soundbank as its location.
An embedded WEM is looked for in the soundbank of the hierarchy first, then in a soundbank of the
same language folder, then in a soundbank that is not localised, then in every other soundbank,
and last as a loose WEM. A localised event is followed only in the soundbanks of `requested` or,
where it has none, of the fallback; `requested` and `fallback` are `null` for a language not
named, and the command line fills the fallback with `English(US)` or the first language folder.
`--lang` and `--fallback-lang` must name a language folder of the Archive (`Data\Sounds\wwise\<folder>`),
otherwise the exit code is 2. Step kinds are
`event`, `action`, `random`, `sequence`, `switch`, `blend` and `sound`, and for any other object its
`HIRC` type, e.g. `music_playlist`, `music_segment` or `music_track`; a music track is the last
step of a variant, its ID is the variant's `sound` and its WEM one of the track's sources. A Play
action (`0x04xx`) is followed to its target, a PlayEvent action (`0x21xx`) to the Event it posts
and a Trigger action (`0x1Bxx`) to the segments of the stingers that name its trigger. Where the
target lies in another soundbank, the walk follows its eligible indexed bank references, sharing
loaded banks on the active path (ADR-0362). The variant's `soundbank` holds the sound.
A **Break** has `event`, `soundbank` (or `null` when no soundbank holds the event), `object` (the
step where the chain breaks, the last of `path`), `target` (for `missing`, `other_soundbank` and
`cycle`, the ID the object refers to, otherwise `null`), `path`, `kind` and `message`. The kinds are
`no_soundbank`, `unreadable` (a soundbank cannot be read again; the Break's `soundbank` is the
one that cannot, which is the other soundbank when the walk was leaving the soundbank of the
event), `missing` (the object refers to an ID the soundbank does not hold; for a Trigger action,
a trigger no stinger names), `other_soundbank` (a Play action names no eligible indexed
soundbank), `malformed`, `unknown_layout` (an object of a bank version other than 154),
`unsupported` (an object kind that is not followed, e.g. an actor-mixer), `cycle`,
`too_many_objects` (the trigger exceeds the caller's visit budget) and
`no_localisation` (a localised event or streamed sound has no soundbank or WEM in `requested` nor
in the fallback language).
New kinds may be added in later versions.

`fieldOrigins` addresses the trigger, language choice, visited paths, resolved variants, breaks,
counts and reported issues by JSON Pointer. `provenance` holds the bound original sources
and the coordinates or roles referenced by those fields. ATL evidence addresses the original
XML attributes and owning elements. In decoded soundbanks, HIRC evidence addresses half-open
byte spans for original object IDs, type or mode values and the inspected route selectors.
Field origins for a synthetic Event step are unknown. A break can name a different bank from
the one that supplied the last visited object. Its `target` retains the authored selector
that led to the lookup.

The original terminal plug-in selector and media ID determine `wem_id`. The terminal plug-in
and storage selectors determine `source`. A MusicTrack receipt retains the authored source
count and the physical row ordinal as the third index of each source word. An embedded
`location` uses the selected DIDX entry and DATA header. A loose location uses the selected
Archive directory Entry. A WEM payload itself is unread during this metadata resolution.
A failed or partial inspection leaves the corresponding contributor set incomplete.

## `p4kx dcb query <archive> <query> --json`

Without `--out`, the line is the Query result itself in its JSON format, as `--format json`
writes it ([query.md](query.md#json)):

```json
[{"record path":"libs/foundry/records/ships/avenger","mass":25000.5,"size":"Large"},…]
```

One object per row in record-path order, keyed by `record path` and then by each `select` path
in column order, with typed values. `--json` with `--format table` or `--format csv` and
without `--out` exits with code 2, because stdout would not hold JSON.

With `--out <file>`, the result in `--format` (`json` by default) goes to the file and stdout
gets the summary:

```json
{"archive": "…", "format": "csv", "output": "…\\ships.csv", "records": 3, "failed": 0,
 "unresolved_references": 2}
```

`records` counts the rows written and `failed` the Records whose layout cannot be read; each of
them is also reported on stderr, and the exit code is 1 when `failed` is not 0.
`unresolved_references` counts the null or dangling links that `->` met in the `select` paths
 of the rows written, each link once per row ([query.md](query.md#following-references)); they
reach `null` and change no exit code.

## `p4kx dcb refs <archive> [<record>] --incoming|--outgoing|--files|--unresolved [--format table|json] --json`

`<record>` is a GUID or a record path; a record path that several Records share lists each of
them. `--incoming` lists the Records that hold a Reference to the Record, `--outgoing` the
Records it refers to, with the Reference to a GUID no Record has listed apart, `--files` the edges of the
file graph ([ADR-0029](adr/0029-references-are-indexed-once-per-archive-from-record-trees.md)),
limited to the edges from or to the file of `<record>` when one is given, and `--unresolved`
every Reference to a GUID no Record has. `--json` is `--format json`; with `--format table` it is
invalid input (exit code 2), as is a `<record>` no Record matches.

```json
{"archive": "…", "direction": "incoming", "records": [{"record": {"guid": "…", "record_path":
 "libs/makers/aegis", "name": "Manufacturer.Aegis", "type": "Manufacturer"},
 "reference_count": 2, "record_count": 2, "references": [{"record": {"guid": "…",
 "record_path": "libs/ships/alpha", "name": "Ship.Alpha", "type": "Ship"}, "location":
 "Components[3].maker", "field": "Components[].maker", "indices": [3]}], "unresolved_count": 0,
 "unresolved": []}]}
```

`records` has one entry per Record `<record>` names. `reference_count` is the number of
References listed in `references` and `record_count` the number of distinct GUIDs among their
other ends. In an entry, `record` is the Record the
listing is about; in each of its `references`, `record` is the other end: the Record that holds
the Reference for `incoming`, the Record it links to for `outgoing`. `location` is the place of
the Reference in the Record that holds it as the Record preview writes it, `field` the same with
`[]` for every array on the way, and `indices` the index of the element in each array,
outermost first. `unresolved` is empty for `incoming`; for `outgoing` it holds the Reference to
GUIDs no Record has, with `"record": null` and the `guid` they name.

`--files --json` writes `{"archive": "…", "file_count": 62736, "edge_count": 475075, "edges":
[{"from": "libs/ships/alpha", "to": "libs/makers/aegis"}]}`, the files by record path, and
`--unresolved --json` writes `{"archive": "…", "count": 880, "unresolved": [{"record": {…},
"guid": "…", "location": "…", "field": "…", "indices": […]}]}` where `record` is the Record that
holds the Reference. `file_count` covers all files with a readable record path and `edge_count`
counts the distinct returned pairs after any filter.

The index attempts every Record without a fixed reference quota. Unreadable Records are
reported on stderr. An allocation refusal reported by the canonical read or origin preparation
fails the JSON command.

All four JSON modes include the shared `provenance.sources` and `provenance.uses` inventory.
A `fieldOrigins` map addresses values relative to its immediate listing, link, edge or
count envelope. Each origin contains `sourceUsesRefs`, `sourceSetComplete` and
`containsEncryptedSource`. A known encrypted contribution makes the last value true.
False requires a complete set of known unencrypted contributors. Null preserves unknown
origin, including the original Archive lineage of a standalone DCB.

- A listing addresses `/record/guid`, `/record/record_path`, `/record/name`, `/record/type`,
  `/reference_count`, `/record_count` and `/unresolved_count`. Incoming counts use the whole
  reference scan and the physical Record selected for the listing. Outgoing counts use the
  selected Record's body. An incoming `unresolved_count` is a fixed zero.
- A link addresses `/record`, its present metadata fields, `/location`, `/field`,
  `/indices`, each `/indices/<index>` and a present unresolved `/guid`. The
  `dataforgeReferenceRoute` coordinates are the physical source Record index and zero-based
  canonical preorder position, excluding the root Struct. An individual array index adds the
  zero-based position among array steps, outermost first. The declaration and array-position
  observations support the route. The Reference occurrence selects its related Record.
  An unresolved GUID payload comes from the original `$error.guid`.
- A file edge addresses `/from` and `/to`. Both retain the actual supporting Reference
  occurrences, including distinct occurrences that give the same pair. Each displayed
  endpoint retains its source path and any other member names and paths used to choose the
  spelling. The envelope addresses `/file_count` and `/edge_count`.
- The unresolved envelope addresses `/count` over the whole reference scan.

Coverage for a count is incomplete when a read failure can conceal a contributor in its
scan scope. An error in a field declared as a scalar string cannot conceal a Reference. A
record body failure can make incoming counts incomplete while an outgoing count for a
different, readable Record stays complete.

## `p4kx dcb tags <archive> [<tag>] [--tree] [--format table|json] --json`

Lists the tag tree: the Tags of the `TagDatabase` with their tag paths. `<tag>` is a GUID or a tag
path (ignoring ASCII case; sibling Tags may share one, so it can name several); with it only that Tag and
everything below it is listed, without it every Tag. `--json` is `--format json`; with
`--format table` it is invalid input (exit code 2), as is a `<tag>` no Tag has.

```json
{"archive": "…", "tag_count": 18891, "shown_count": 3, "issues": {"unclassified": 0, "unreadable": 0, "dangling": 0,
 "repeated": 0, "too_deep": 0, "too_long": 0, "duplicate": 0, "unlisted": 0}, "tree": false, "tags": [{"guid": "…", "name": "Gun",
 "path": "Weapon.Gun", "legacy_id": null, "parent": "…", "depth": 1, "children": ["…"]}]}
```

`tags` is flat in pre-order: a Tag, then everything below it, then its next sibling; the roots are
in the order the `TagDatabase` lists them, then the Tags it does not list. `name` is the Tag alone,
`path` the names from the root down joined by `.`, `legacy_id` the number stored as `legacyGUID` or
`null` for none (4294967295), `parent` the GUID of the parent or `null` for a root, `depth` the number
of Tags above it and `children` their GUIDs in the order the Tag lists them. `tag_count` is every Tag
of the DataForge and `shown_count` the Tags listed, which differs with `<tag>`. `issues` counts what
the tree could not take as the DataForge lists it, and each non-zero count is also a warning on
stderr: `unclassified` Records whose unreadable type prevents deciding Tag/TagDatabase membership; they are not counted as read Tags, `unreadable` Tag Records left out, `dangling` listed Tags that are no Tag Record,
`repeated` Tags listed more than once (the first listing counts, which also cuts a cycle),
`too_deep` and `too_long` are retained JSON keys and always zero,
`duplicate` Tag Records left out because an earlier one has
their GUID, and `unlisted` Tags no `TagDatabase` reaches, which stand as roots of their own.

With `--tree` the document holds `"tree": true` and `tags` holds only the top of the listing, the
roots or the Tags `<tag>` names, each with its `children` nested as objects of the same shape; they
have no `parent` and `depth`, and `children` is no list of GUIDs:

```json
{"tree": true, "tags": [{"guid": "…", "name": "Weapon", "path": "Weapon", "legacy_id": 7,
 "children": [{"guid": "…", "name": "Gun", "path": "Weapon.Gun", "legacy_id": null,
 "children": []}]}]}
```

Both JSON forms carry `provenance.sources` and `provenance.uses`, plus immediate
`fieldOrigins` maps. Each origin entry contains `containsEncryptedSource`,
`sourceUsesRefs` and `sourceSetComplete`. Its references resolve in the document's shared
inventory. `true` proves an encrypted contributor or ancestor, `false` requires a complete
set of unencrypted contributors, and null preserves unknown origin. A standalone DCB has
unknown original Archive lineage.

The envelope map addresses `/tag_count`, `/shown_count` and each `/issues/<key>`.
Each Tag has its own relative map addressing `/guid`, `/name`, `/path`, `/legacy_id`,
`/children` and each `/children/<index>`. Flat rows also address `/parent` and `/depth`.
Nested child objects carry their own maps. Their parent's indexed child entry records
the relationship that selected them. A GUID entry has this shape.

```json
{"/guid":{"containsEncryptedSource":true,"sourceUsesRefs":["u-guid"],"sourceSetComplete":true}}
```

Header identities, original array positions, name conversions and legacy-ID sentinel
conversions retain their own use sets. A selected path includes the names and selected
relationships of its ancestors, including ancestors hidden by a query. Depth uses the
selected topology. Repeated original operands keep their own occurrence identity.

Membership and diagnostic counts retain the actual classification survey and relevant
read or relationship observations. Successful source-type intervals use
`dataforgeRecordHeaderRange` with decimal inclusive-start/exclusive-end indices and
`sourceType`. The original `dataforgeHeader.recordCount` bounds the survey. Failed
reads remain attempted observations. The historical zero-valued limit counters have a
complete empty set of contributors. Unlisted-root topology can have an incomplete
contributor set when the observed walk supplies no selected incoming relationship.
`archive` and `tree` are command inputs.

## `p4kx dcb fields <archive> [<type or Query>] [--path <glob>] [--format table|csv|csv-strict|json] --json`

The statistics per field path of the selected Records ([query.md](query.md#field-statistics)).
`--json` is `--format json`; with `--format table` or `--format csv` it is invalid input (exit
code 2). The example omits the origin inventory and proof fields.

```json
{"archive": "…", "records": 29693, "unreadable_records": 0, "unreadable_values": 0,
 "max_distinct": 32, "field_count": 87514, "fields": [{"path":
 "Components/SCItemWeaponComponentParams/fireActions/SWeaponActionFireRapidParams/fireRate",
 "occurrences": 161, "records": 161, "empty": 0, "kinds": [{"kind": "Int32", "count": 161}],
 "distinct": 17, "distinct_capped": false, "min": 42, "max": 1600}]}
```

`records` is the number of Records examined, `unreadable_records` those not counted and
`unreadable_values` the values not counted. `field_count` is the length of `fields`, after
`--path`; `fields` is in path order. In a field, `occurrences` is the number of values,
`records` the number of Records that have the path, `empty` the number of empty values, `kinds`
the data types of the values with their counts (`Single`, `Int32[]` for the elements of an array,
`StrongPointer`, `Class` for a path that holds fields, …), `distinct` the number of distinct
non-empty values, at most `max_distinct`, with `distinct_capped` true when there are more, and
`min` and `max` the smallest and the largest number, `null` for a path without numbers (or an
infinite float). Integers are JSON integers, floats JSON numbers; `-0.0` is `0`.

`fieldOrigins` addresses individual values with JSON Pointers. A filtered report addresses the
first displayed row at `/fields/0`, regardless of its original sort position. Examples include
`/fields/0/min`, `/fields/0/empty`, `/fields/0/kinds/0/count`, and the summary
`/unreadable_values`. Each carries its immediate `containsEncryptedSource` and
`sourceSetComplete`. Its `sourceUsesRefs` list holds the complete set of actual contributor references.
The sorted, unique references resolve in `provenance.uses`. A raw standalone DataForge file
has unknown original Archive lineage. `archive` and `max_distinct` describe the invocation
and the statistic algorithm.

`sourceProof.selection` uses the same origin shape. The counters and field metrics that use the Record selection retain its original contributor references.
`selectionMode` distinguishes `allRecords`, `incompleteHeaders`, and
`observedQuery`. `referenceCoverage` concerns the Query's Reference lookup and is null without
a Query. Query evidence includes actual type and path declarations and predicate reads.
`dataforgeRecordHeaderRange` uses decimal half-open physical Record indices for `fileName`,
`recordName` and `sourceType`. The `dataforgeRecordPathTextRanges.text` selector gives the
exact original filename and ordering-name bytes as consecutive pairs of decimal half-open
decoded DataForge offsets. The ranges are sorted and disjoint. Adjacent strings can share
a range through their verified NUL terminators.
A Query using only `FROM` or `AT` has complete selection evidence after a full physical
Record census with readable path, name and type decisions. A failing admitted Record
body affects its field metrics separately from that selection proof. A `WHERE` condition
can establish complete selection when at least one candidate evaluates it, all its comparisons
on candidates use fixed numeric, Boolean, GUID, string or locale slots, and the original
census and runtime reads have no unresolved inputs. Its actual selection contributors
include the demanded operands of
admitted and rejected candidates. Other runtime selection, including `REFERENCING`, retains
its observed uses with incomplete selection coverage.

Installed LIVE 13220821 and PTU 13248297 each verified a literal Record query over a fixed
nonempty string. The ordinary and source-observing queries agreed on the admitted and rejected
row counts. Both decisions retained the original fixed slot, the source text range and its
terminator. A PTU conjunction also verified that two demanded operands retain their own ranges
and a short-circuited operand contributes neither its slot nor its text range. The observations
use the CLI built at `38fbbafcf`.

`selectionAttempts`, `rootAttempts`, and `valueAttempts` retain failed original uses separately
from completed contributors. `rowAttempts` associates these observations with displayed row
positions when applicable. Failed attempts for paths that emit no row remain in `valueAttempts`.
`unresolvedAncestryPositions` and `unavailableTraversals` identify unresolved table positions or
missing requested slot or instance extents without claiming readable bytes at those positions.
A missing numeric extremum still carries the comparison scope that produced the null result.
Table and CSV use their existing output format and statistics.

## `p4kx grep <archive> <text> [--in records,loc,entries] [--filter <filter>] [--lang <language>,…] [--limit <n>] [--per-item <n>] [--max-entry-size <bytes>] [--format table|json] --json`

Finds `<text>` as part of a longer text, ignoring case, in the string, localisation key and enum
values of Records, in the texts of each language's `global.ini` and in the lines of text and
CryXml Entries ([ADR-0044](adr/0044-content-search-scans-on-demand-and-keeps-no-index.md)).
`--json` is `--format json`; with `--format table` it is invalid input (exit code 2), as is an
empty `<text>`. No match is not an error: the exit code is 0 with empty arrays.

```json
{"archive": "…", "text": "Gladi", "elapsed_ms": 812,
 "records": [{"record_path": "libs/ships/aegs_gladius", "name": "Gear.AEGS_Gladius",
  "type": "Gear", "guid": "…", "field": "Components[3].name", "text": "AEGS_Gladius Fighter",
  "match": [5, 10]}],
 "loc": [{"language": "en", "key": "vehicle_NameAEGS_Gladius", "text": "Aegis Gladius",
  "match": [6, 11]}],
 "entries": [{"path": "Data\\Scripts\\ship.xml", "line": 2, "offset": 7,
  "text": "  <Part name=\"AEGS_Gladius\"/>", "match": [19, 24]}],
 "more": ["records"],
 "unavailable": [{"source": "records", "reason": "…"}],
 "stats": {"records_scanned": 117709, "records_unreadable": 0, "texts_scanned": 90946,
  "languages_scanned": 1, "languages_unreadable": 0, "entries_scanned": 44874,
  "entries_binary": 0, "entries_too_large": 0, "entries_key_required": 11,
  "entries_unreadable": 0}}
```

The hits of a source are in its order: Records in Record order, localisation by language in the
order of `p4kx loc languages` and by line, Entries in path order; the order does not depend on
the number of threads. `text` is a short excerpt around the first match in the value, with a
`…` where it is cut and control characters replaced by U+FFFD, and `match` the byte range of the
match in it. `field` is the path of the field with the index of every array element on the way,
like the Record preview writes it. `line` counts from 1 in the text of the Entry, a CryXml Entry
in its XML form, and `offset` is the byte offset of the start of that line in the same text.
A localisation key with a comma suffix, `item_Gun,P`, is a line of its own. A Record reports
at most `--per-item` hits and a source at most `--limit` (1,000 by default, at most 10,000);
`more` names the sources that hold more matches. `--filter` narrows the record path of a Record,
the key of a text and the path of an Entry, and for Entries replaces the list of extensions.
`unavailable` names a source without anything to search in, e.g. the Records of an Archive
without DataForge. `stats` counts what was read and what was left out: Entries that hold binary
data, Entries above `--max-entry-size` (64 MiB by default), encrypted Entries the Archive key
does not open and Entries that cannot be read are skipped and counted, and the same counts are
printed as a note on stderr. An encrypted DataForge without the key is exit code 7.

JSON adds a `fieldOrigins` map and the shared `provenance` inventory. Paths such
as `/records/0/text`, `/loc/0/key`, `/entries/0/path` and
`/entries/0/match/0` address the existing values directly. Each entry contains
the immediate `containsEncryptedSource` flag, the actual `sourceUsesRefs` and
`sourceSetComplete`.

Record hits identify the DataForge read and original Record position. The
original record field supplies its excerpt. The type name also uses the
referenced structure definition. A localization hit identifies its language
Entry and authored logical INI line. Distinct physical Entry hits retain their
own identities even when their paths repeat. A path or language label uses
directory metadata. Excerpts and match coordinates use the actual searched
payload. The original UTF-8 line is a logical source coordinate.

For UTF-16 and CryXml, the wire `offset` refers to the searched UTF-8
representation. Their conversions bind the actual Entry and omit unproved
original coordinates. A proven encrypted contribution still yields
`containsEncryptedSource: true`. The `sourceSetComplete` marker is false at
these incomplete coordinates. A keyless discarded search supplies no hit
contributor. The `more`, `unavailable` and `stats` sections describe search
accounting separately from returned data values.

Plain table output retains its existing fields and formatting.

## `p4kx catalog <archive> --out <file> --json`

The marker-only example shows one failed Record and the text entries it references. Other
summary and health fields are omitted. The complete `export_health` object matches the
catalog's `exportHealth`. Its nested keys retain camelCase inside the CLI's snake_case summary.
With `--provenance full`, the summary includes a `provenance` inventory, and each sourced cell
also carries `sourceUsesRefs` and `sourceSetComplete`.

```json
{
  "archive": "Data.p4k",
  "out": "diagnostic.json.zst",
  "schema_version": 3,
  "export_health": {
    "status": "incomplete",
    "read": "incomplete",
    "projection": "complete",
    "publicationMode": "diagnosticPartial",
    "metadata": {
      "texts": [
        {"state": "present", "value": "libs/foundry/records/crafting/blueprints/crafting/broken", "containsEncryptedSource": false},
        {"state": "present", "value": "source read failed", "containsEncryptedSource": null}
      ]
    }
  },
  "failed": [{
    "guid": {"state": "present", "value": "90000000-0000-4000-8000-000000000009", "containsEncryptedSource": false},
    "path": {"state": "present", "value": {"atomIndex": "0"}, "containsEncryptedSource": false},
    "error": {"state": "present", "value": {"atomIndex": "1"}, "containsEncryptedSource": null}
  }]
}
```

`archive_key` is `given`, `missing` or `wrong` and `encrypted_entries_unread` counts the distinct
encrypted Entries the export could not read for want of a usable Archive key, as in the catalog's
`source`; the same count is printed as a note on stderr, and neither changes the exit code.

`quantizations` counts two facts of the game data that do not affect the exit code: `unused`, the
quality quantizations of the catalog that no `ResourceType` uses, and `unresolved_links`, the links
of `ResourceType` Records to a Record that is no readable quality quantization of the catalog (of
another type, failed or a development leftover; a Reference to a GUID no Record has reads as null).

`store_slugs` counts the curated RSI store slugs against the ships of the catalog and lists the
`stale` ones whose mapping no longer holds; `stale_check_skipped` is `true` and `stale` empty when slugs are
mapped but none resolves, as for an Archive that is not the game's ([catalog.md](catalog.md), "Ship store
slugs").

The catalog itself goes to `out` ([catalog.md](catalog.md)); a name ending in
`.zst` writes a zstd frame, and a name ending in `.gz` is refused with exit code 2 before anything is
written. `size` is the size of the JSON in bytes, uncompressed;
`compressed_size` is the size of the file written when it is compressed and `null` otherwise.
`size_warning` is `near_limit` from 90 % of `upload_limit`, `over_limit` above it, and `null`
otherwise; it compares the file written (`compressed_size`, else `size`). `decompressed_warning`
does the same for `size` against `decompressed_limit` (1,000,000,000). `kinds` lists every kind in
catalog order with the count written, development leftovers `skipped` and selection completeness in
`complete`. With `--filter`, `tags` and `shopInventories` remain complete. `skipped` sums the
per-kind counts.

`failed` lists selected unreadable Records omitted from the catalog, ordered by GUID. Inspect each
cell's `state` first. A present GUID is a string in `guid.value`. For a present `path` or `error`,
parse `value.atomIndex` as a zero-based decimal-string index into the same summary's
`export_health.metadata.texts`. Read the referenced text cell's `value`. A readable empty path
has a present text value of `""`. An unreadable path has state `unreadable` and value `null`.
Use the immediate occurrence's encryption marker and evidence for classification. The shared text
cell can aggregate origins from multiple occurrences. See the [health contract](catalog-export-health.md#original-record-subjects-and-shared-text).
Normal publication refuses incomplete read/projection health or blocking source warnings, with
exit code 1, empty stdout and no destination creation or replacement. Use `--diagnostic-partial`
explicitly to write an incomplete diagnostic artifact and its JSON summary; incomplete diagnostic
output still exits with 1. Its `export_health.publicationMode` is `diagnosticPartial`, including
when all selected sources happened to succeed. The same marker travels in the file's
`exportHealth`, so a copied file retains the publication contract. Filter-only selection scope
does not itself make health incomplete or require diagnostic permission. `warnings` lists what
makes the crafting data incomplete or ambiguous although every Record was read, each as an object
with a `kind`: `unreadable_reward_records` with a `count` (Records that cannot be read although
their struct type is that of a Record with a reward source, so a source may be missing) and
`unknown_modifier_ranges` with a `count` (modifier ranges of blueprints whose struct type the reader does not know, so they are missing from the modifiers) and `details` (the struct types, ordered, without repeats), `unknown_template_values` with a `count` (stored names in the contract templates and the mission broker entries that the catalog does not know, a state, a category, a comms timing or reason, a kind of objective property, a comparison, a month, a repeat type, a reaction or a licence type, each `null` in the catalog) and `details` (ordered category counts and unsupported declared struct names, with authored values retained in their sourced catalog records),
`ambiguous_dismantle` with a `count` (several dismantle processes, so no blueprint links one),
`unreadable_vehicle_files` with a `count` (ships whose vehicle file exists and cannot be read, so they have no paints; a file the Archive lacks is no such ship) and `details` (the path and the reason of each file, ordered, without repeats; the JSON lists every file, stderr the first eight) and
`unplaced_starmap_containers` with a `count` (unreadable containers, malformed or out-of-range Children and walk
limits that left starmap objects without a position), `unreadable_harvest_containers` with a `count` and `details` (system containers that cannot be opened or whose `.soc` cannot be read, so a provider preset may be missing from `harvestableLocations`; the path and the reason of each, ordered),
`unreadable_loot_containers` with a `count` and `details` (containers or entity files that cannot be read, a `.soc` that cannot be counted or a placed container the Archive lacks, so a multi config may be missing from `lootContainers` or a container has no counts; as above),
`ambiguous_loot_entity_files` with a `count` and `details` (entity files of containers that name several different containers or multi configs, of which only the first of each is used; the container and file of each, ordered),
`unreadable_harvestables` with a `count` and `details` (`HarvestablePreset` or entity Records that are missing or cannot be built, so the harvestables that name them lack their entity and composition; the role, GUID and reason of each, ordered),
`harvest_locations_without_object` with a `count` and `details` (harvestable locations with no starmap object although one is recorded above their container: the object is not in the starmap objects, mining claim markers form a cycle above, or a marker has no object above it; one line per reason with its number; a container with no object recorded above is the normal case and is not counted),
`unreadable_shop_registry` (the shop registry exists but
cannot be read, so `shops` is empty), `missing_shop_registry` (starmap objects but no registry at the known path)
`unreadable_shop_containers` with a `count` (shop containers that cannot be read) and
`unreadable_shop_inventories` with a `count` (inventory files that cannot be read) and `details` (as above: every file in the JSON),
`missing_shop_inventories` with a `count` (distinct inventory files that containers name and the Archive lacks) and
`no_shop_components` (shop containers name shops but no `.soc` holds a shop component) and
`unknown_scan_signature_slot` (the radar parameters do not name the scan signature slot, so `scanSignature` of the mineable entities is null; no `count`); a reader ignores a `kind` it does not know. The prose of each warning goes to stderr; a healthy
Archive has none (the example shows one). `item_classes` counts the items whose `Class:` line names an item class (`classified`),
holds another value (`other`) or is missing, also for an item without an English description
(`missing`). `paints` counts the paints (`paints`), those that fit at least one ship (`assigned`) and those that fit
none (`unassigned`), the ships with at least one paint (`ships`) and the ships whose vehicle
implementation is present but cannot be read in full (`unreadable_vehicles`; a file the Archive lacks is not counted). `loadouts` counts the loadouts of
ships (`ships`) and of items that have ports (`items`), the ports of all loadouts with the nested
ones (`ports`), the ports that hold an Entity (`attached`), the ports that hold a docked vehicle
(`docked_vehicles`), the attachments that cannot be resolved (`unresolved`), the distinct attached
Entities that are no entry of `items` or `ships` (`outside`) and the loadouts that list an issue
(`with_issues`). `starmap` counts the starmap objects
(`objects`), those with a position (`placed`) and without (`unplaced`), the records that Child
elements place more than 1 m apart (`ambiguous`, no position), the star systems found (`systems`),
the containers that cannot be read (`unreadable_containers`), the external containers the Archive
lacks (`missing_containers`, no warning) and the Child elements skipped for a malformed placement
or a limit (`malformed_children`); the unreadable containers and the malformed Children together
are also the warning `unplaced_starmap_containers` with their `count`. `shops` counts the shops
(`shops`) of the `registry_entries` read, the entries left out for a malformed or repeated `ID`
(`malformed`, `duplicate`), the shop containers found, unreadable and naming an `SCShop` Entity,
the shops whose Entity exactly one container kind names (`with_kind`), several differing ones
(`mixed_kind`) or none (`without_container`), the shops with an item list (`with_items`) and those
without one because their containers name differing inventory files (`inventory_ambiguous`) or a file
the Archive lacks (`inventory_missing`) or that cannot be read (`inventory_unreadable`), those whose
containers have no readable shop component at all (`inventory_unscanned`) or name a designer
placeholder (`inventory_placeholder`), the containers outside the shop folders and the prefabs read
(`other_containers_scanned`, `other_containers_unreadable`, `other_containers_with_shop`,
`prefabs_listed`, `prefabs_unreadable`, `prefab_entities`, `prefab_ambiguous`, `prefab_only_shops`),
the inventory paths no Entity could be told for (`soc_paths_unattributed`), the distinct files named and present
(`inventories_named`, `inventories_in_archive`), the found components
(`soc_components`), the distinct files that cannot be read or are missing (`inventories_unreadable`,
`inventories_missing`) and the `ID` values dropped for being no GUID (`item_ids_dropped`), the
commodity exchanges, the shops linked to a
starmap object (`linked`) and not (`unlinked`, `ambiguous_link` of them because several objects
carry the name) and the distinct location names without a link, up to 200; the numbers in the
example are those of LIVE (Build 4.10.197). `playability` counts the
items and the ships per value of their `playability`, and the ones no rule classifies
(`unclassified`, `null` in the catalog). `variants` counts per kind the variant groups of at
least two members (`groups`) and the entities with a `variantOf` (`variants`).

The summary's `texts` lists the available languages and also reports English and German when
unavailable. For each language, `available` records whether the Archive holds its `global.ini`.
The counters operate per lookup occurrence. `resolved` counts a nonempty display result and
`unresolved` counts a lookup without one. `placeholder` counts original placeholder keys
separately. The top-level `unresolved` sums the counts of the reported languages.

`text_kind_coverage` records successful canonical key registrations by owning catalog dataset and
supported language. Each row has an `owner` identifier and a language code. Its mutually exclusive
counts are `resolved` for nonempty display text, `missing_language` when no language source
was loaded,
`missing_key` for an absent lookup result in a present language, `empty_source` for a winning
empty source string, `empty_display` for source text that becomes empty after display conversion,
and `placeholder` for a recognized token that bypasses lookup. Placeholders count separately even
for missing languages. Repeated registration of the same key counts each time, and internal-only
mission render lookups remain in the legacy counters. The order is catalog Kind order, then
`configurations.itemRecoveryPolicy` and `definitions.consumableSubtypes`, with supported language
order within each dataset. Each dataset with registrations has a row for every supported
language. Language-source decoding and malformed-line notes are separate file observations.
A recovered directory can leave absence uncertain. See the `sourceSetComplete` value in
`texts.languageSources` for the directory evidence.

These counters describe display lookup results. In the Catalog document, a missing key has absent
text forms, a present empty value stays present and a missing language has no member in
`translations.value`. See [TextForms](catalog-text-forms.md).

## `p4kx coverage <archive> [--format table|json] --json`

The coverage of the reference list in [coverage.md](coverage.md). The exit code is 1 when a
listed type (candidates excepted), component or folder is missing, unreadable or does not convert.

```json
{"archive": "…", "complete": false, "unreadable_entities": 0,
 "types": [Type, …], "components": [Component, …], "starmap_types": [StarMapType, …],
 "entries": [Folder, …], "fieldOrigins": {}, "provenance": {"sources": [], "uses": []}}
```

`unreadable_entities` counts the `EntityClassDefinition` Records that cannot be built and so cannot
be searched for components; above 0 it makes `complete` false.

`types` hold one object per Record type of the list, in list order:

| Field | Type | Meaning |
|-------|------|---------|
| `area` | string | The topic area, e.g. `missions` |
| `name` | string | The type name |
| `candidate` | bool | The type is named as a likely future need and does not count against `complete` |
| `defined` | bool | The DataForge defines the type |
| `records` | number | The Records of the type and of the types deriving from it |
| `unreadable_records` | number | Records whose layout or instance cannot be read |
| `error_values` | number | Values that cannot be read (`Value::Error`) over all its Records |
| `sample` | object or null | The first readable Record exported as Record XML and Record JSON; `null` without Records |

`sample` has `record_path`, `xml_bytes`, `json_bytes` and `error` (string or `null`).

`components` hold one object per component of an `EntityClassDefinition`:

| Field | Type | Meaning |
|-------|------|---------|
| `area`, `name` | string | As above |
| `defined` | bool | The DataForge defines the component struct |
| `entities` | number | The `EntityClassDefinition` Records with at least one such component, structs deriving from it included |
| `instances` | number | The components of that type, derived ones included, in all of them |
| `reachable` | bool | The Component selector `Components[<name>]` resolves in a Query |

`starmap_types` hold one object per `starmap-type` line of the list:

| Field | Type | Meaning |
|-------|------|---------|
| `area` | string | As above |
| `name` | string | The `StarMapObjectType` name, e.g. `SolarSystem` |
| `records` | number | The `StarMapObject` Records, derived types included, whose Reference `type` links to a `StarMapObjectType` Record of that name |
| `templates` | number | Of `records`, those whose record path contains `template` (case-insensitive) |
| `sample` | object or null | The first such Record that is no template, exported as Record XML and Record JSON; `null` without one |

`entries` hold one object per Entry folder:

| Field | Type | Meaning |
|-------|------|---------|
| `area` | string | As above |
| `folder` | string | The folder path as the list spells it |
| `entries` | number | The Entries in the folder |
| `converted` | number | The Entries converted from CryXml to XML |
| `text` | number | The Entries that are text XML already |
| `failures` | array | `{"path": …, "reason": …}` for each Entry that cannot be read or converted |

`complete` is true when every non-candidate type has Records that read without error and export,
every component occurs and is reachable, every `starmap_types` entry has Records whose sample
exports, and every folder has Entries that all convert.

`fieldOrigins` addresses these source-backed paths by zero-based output ordinals. Each cell
carries `containsEncryptedSource`, `sourceUsesRefs`, and `sourceSetComplete`. The references
resolve against the document's `provenance` inventory.

- `/types/{row}/sample/record_path` and `/starmap_types/{row}/sample/record_path` use the selected
  physical Record's filename field and original DataForge text bytes.
- `/entries/{folder}/failures/{row}/path` uses the failed physical Entry's directory path. Entries
  with identical paths retain separate physical origins. The marker describes directory bytes independently
  of the Entry payload's encryption.

Counts, sample byte lengths, and failure reasons are outside this origin coverage.

## `p4kx ships <archive> --out <file> --json`

```json
{"archive": "…", "out": "…", "format": "json", "archive_key": "missing",
 "encrypted_entries_unread": 412, "ships": 1087, "skipped": 57, "mass": 1087,
 "hull": 1087, "cargo": 565, "issues": 3,
 "failed": [{"guid": "…", "path": "libs/foundry/records/entities/spaceships/odd", "error": "…"}]}
```

The ship data itself goes to `out` ([ship-base-data.md](ship-base-data.md)). `format` is `json` or
`ndjson`; `archive_key` is `given`, `missing` or `wrong` and `encrypted_entries_unread` counts the
distinct encrypted Entries the export could not read for want of a usable Archive key, as in the
file's `source` (neither changes the exit code); `ships` counts the ships written, `skipped` the
development leftovers; `mass`, `hull`
and `cargo` count the ships with a mass, with a hull (a part that states hit points) and with at
least one cargo grid in their Loadout; `issues` the ships whose
`issues` list is not empty. `failed` lists the ships that could not be read and are left out, with
GUID, record path and reason, ordered by record path; the exit code is 1 when it is not empty.

## `p4kx loc languages <archive> --json`

```json
{"archive": "…", "languages": [{"code": "en", "folder": "english", "entry": "Data\\Localization\\english\\global.ini"}, …]}
```

`languages` lists the languages the Archive holds a `global.ini` for, in the order `en`, `de`,
`fr`, `it`, `es-ES`, `es-419`, `pt-BR`, `ja`, `ko`, `zh-Hans`, `zh-Hant`.

The `fieldOrigins` map addresses the original directory information for each
language's `code`, `folder` and `entry`. Listing a language reads its Entry path,
and its translation payload may have a different encryption state.

## `p4kx loc get <archive> <key> [--lang <language>] [--fallback <language>] --json`

```json
{"key": "item_Name", "language": "de", "fallback": "en", "status": "fallback",
 "text_language": "en", "text": "Rifle"}
```

`key` is the key as given. `status` is `found` (the text is in `language`), `fallback` (it is in
`fallback`), `placeholder` (the key is one of `@LOC_EMPTY`, `@LOC_PLACEHOLDER`,
`@LOC_UNINITIALIZED` and `@blank_space`, which stand for no text) or `missing`. `text_language`
and `text` are `null` for the last two; `text` has `\n` decoded to a line feed. A `missing` key is
printed and then ends the command with exit code 2.

The `fieldOrigins` map and shared `provenance` inventory describe each output
field. `/text` cites the winning value and its byte range in the decoded
UTF-8 localization input. It also retains the selectors that chose the result.
`/status` and `/text_language` cite those selectors. A missing or empty preferred
value can select a fallback, so a plain translation can carry an encrypted
selector. A UTF-16 Entry retains its original Archive evidence. A missing
lookup uses the normalized key in the queried Entry. Placeholder results
require no lookup.

## `p4kx loc export <archive> [--lang <language>,…] [--format json|csv] [--out <file>]`

Without `--out` the table goes to stdout and `--json` changes nothing. The JSON format is

```json
{"languages": ["en", "de"], "entries": [
{"key": "item_Name", "texts": {"en": "Rifle", "de": null}}, …]}
```

with one entry per key, sorted by key ignoring case; a language without a text for the key is
`null`, a text the `global.ini` holds empty is `""`. CSV starts with a UTF-8 byte order mark, so
Excel shows non-ASCII text correctly, has CRLF line ends, the header `key,en,de` and the same
rows, with an empty cell for either; cells with a comma, quote or line break are quoted. A key is
left out only when none of the chosen languages' `global.ini` holds a line for it; a key held
only with an empty text stays.

CSV neutralises spreadsheet formulas by default. `--csv-raw` writes the original cell text.

With `--out` and `--json`, the summary contains `archive`, `out`, `format`, `languages`,
`keys`, `originReceipt`, `fieldOrigins` and `provenance`. The artifact itself retains the table
shape shown above. The companion identifies each emitted table value and is saved beside the
file. For stdout use `--origin-stderr` or `--origin-out <file>`. See
[localisation origins](source-origin.md#localisation-tables).

## `p4kx actionmap settings <archive>`

The command writes the structured JSON [client profile report](client-profile-settings.md)
to stdout, including the profile documents and their per-value provenance.

## `p4kx actionmap export <archive> [--format json|csv] [--csv-raw] [--language <language>] [--out <file>]`

Without `--out` the export goes to stdout and `--json` changes nothing. The JSON format is

```json
{"language": "en", "action_maps": [
{"name": "seat_general", "label_key": "@ui_CGSeatGeneral", "label": "Vehicles - Seats and Operator Modes",
 "actions": [{"name": "v_eject", "label_key": "@ui_CIEject", "label": "Eject",
  "activation_mode": "press",
  "bindings": [{"device": "keyboard", "input": "ralt+y", "activation_mode": "press"}]}, …]}, …]}
```

in document order. `label_key` and `activation_mode` are `null` when the profile names none; `label`
is `null` when there is no key or the language has no text for it, and has `\n` decoded to a line
feed. `device` is `keyboard`, `mouse`, `gamepad` or `joystick`; `input` is the profile's spelling,
with `+` between the keys of a combination. A binding's `activation_mode` is its own, else the
action's. An input that is empty or blank is no binding. When an action authors
`UIDescription`, JSON adds `ui_description` with the original decoded value, including an empty
string. A localisation key keeps its leading `@`. The field has its own source coordinate in the
origin companion.

The CSV starts with a UTF-8 byte order mark, has CRLF line ends and the header
`action_map,action_map_label,action,label,activation_mode,keyboard,mouse,gamepad,joystick`. A
device cell joins its bindings with ` | `; a binding whose activation mode differs from the
action's reads `input (mode)`. Text cells follow the rules of ADR-0159: a cell that starts with
`=`, `+`, `-`, `@`, a tab or a carriage return gets a leading `'`. `--csv-raw` writes the exact
texts without the `'` and keeps the byte order mark; it is invalid input (exit code 2) with
`--format json`.

With `--out` and `--json` the command prints `{"archive": "…", "out": "…", "format": "csv",
"language": "en", "action_maps": 50, "actions": 1103, "unresolved_labels": 1, "origin_receipt": "actions.csv.p4kx-origin.jsonl"}`;
`unresolved_labels` counts the label keys, of actions and of action maps, with no text in the
language. The report also includes `fieldOrigins` and `provenance`.

## `p4kx key set|show|remove --json`

`p4kx key set [<hex>] [--archive <archive>] --json` prints
`{"action": "stored", "file": "…", "checked_against": "…"}`; `checked_against` is the Archive
whose encrypted Entry the key decrypted, or `null` when no Archive with a suitable encrypted
Entry was found and the key was stored unchecked. `p4kx key remove --json` prints
`{"action": "removed", "file": "…"}`, or `"none_stored"` when there was nothing to remove.

`p4kx key show --json` prints `{"file": "…", "stored": "****…1234" | null, "in_use": {"source":
"flag" | "environment" | "profile", "key": "****…1234"} | null}`. Keys are always masked: every
digit but the last four is `*`. No command prints or logs a key in full.

## `p4kx channels --json`

```json
{"channels": [{"name": "LIVE", "build": Build, "source": "default", "archive": "…"}, …]}
```

`source` is `default` (below the default Library folder), `launcher` (below a Library folder from
the RSI Launcher log) or `manual`.

`p4kx channels add <path> --json` prints `{"action": "added", "kind": "folder", "path": "…"}`;
`action` is `added` or `already_added`, `kind` is `folder` or `archive`, and `path` is the
absolute path stored in the settings. `p4kx channels remove <path> --json` prints
`{"action": "removed", "path": "…"}`.

## `p4kx texture info <archive> <entry> --json`

Also `p4kx texture info <file> --json` for a header file on disk.

The example below shows the compatible fields; successful output also includes `source_metadata`.

```json
{"texture": "Data\\Textures\\hull_ddna.dds", "format": "BC7_UNORM_SRGB", "width": 64,
 "height": 32, "depth": 1, "mip_count": 7, "array_size": 1, "faces": 1, "cubemap": false,
 "parts": 4, "alpha_tail": {"format": "BC4_UNORM", "parts": 4,
 "error": null}}
```

`texture` is the Entry path, or the path of the file on disk. `format` is the DXGI name of the
pixel format; legacy headers report their DXGI equivalent (`DXT1` is `BC1_UNORM`), and a format
this version does not know is described by its DXGI code, `FourCC` or bit masks. `depth` is 1
unless the texture is a volume, `faces` is 6 for a cubemap and 1 otherwise. `parts` is the number
of mip parts `.1` … `.N` the header needs, or `null` when the format's size is unknown.
`alpha_tail` is `null` without an alpha or smoothness tail `.a`; otherwise it gives the tail's
`format` and its number of `parts` `.1a` … `.Na`. A tail that cannot be read keeps the command
working: `format` and `parts` are then `null` and `error` says what is wrong.

`source_metadata` version 1 preserves the authored legacy and DX10 header words, including flags,
reserved words, pitch, pixel masks and `miscFlags2`. Its `raw` fields retain source zeros and unknown
bits; `normalized` describes the current reader's defaults and layout separately. Header-source byte
lengths use canonical unsigned 64-bit decimal strings. Unknown format identifiers and absent DX10/layout
values stay explicit; no image decode or material-specific alpha meaning is implied. See
[DDS source metadata](dds-source-metadata.md) for the exact boundary and types.

## `p4kx texture merge <archive> <entry> --out <file> --json`

Also `p4kx texture merge <file> --out <file> --json` for a header file on disk.

`origin_receipt` names the digest-bound DDS companion written in both text and JSON modes.
The `field_origins` map covers `texture`, `parts` and `size`. Resolve its use references in
`provenance`. The example shows the output summary fields. [DDS merge origins](source-origin.md#merged-dds)
describes the companion addresses.

```json
{"texture": "Data\\Textures\\hull_ddna.dds", "output": "…\\hull_ddna.dds", "parts": 4,
 "size": 2916, "origin_receipt": ".\\hull_ddna.dds.p4kx-origin.jsonl"}
```

`parts` is the number of mip parts merged and `size` the size of the written DDS file.

## `p4kx texture png <archive> <entry> --out <file> [--mip <level>] [--cross] [--tone-map] --json`

Also `p4kx texture png <file> --out <file> [--mip <level>] [--cross] [--tone-map] --json` for a
header file on disk.

```json
{"texture": "…\\hull_diff.dds", "output": "…\\hull_diff.png", "format": "png",
 "files": ["…\\hull_diff.png"], "mip": 1, "width": 32, "height": 16, "alpha_tail": false}
```

`output` is the `--out` path and `files` the files written: `output` itself, or for a cubemap
the six faces `<stem>_posx.png`, `_negx`, `_posy`, `_negy`, `_posz` and `_negz` next to it, or
with `--cross` `output` holding one horizontal cross; a texture array has one file per element
`<stem>_layerN.png` and a volume one per slice of the mip `<stem>_sliceN.png`, and a cubemap
array has `<stem>_layerN_posx.png` … or, with `--cross`, one cross `<stem>_layerN.png` per
element, the numbers padded with zeros to the width of the last. `format` is `png` for 8-bit
RGBA PNG and `exr` for OpenEXR with 32-bit float RGB, which a high-range BC6H or floating-point texture is written as without
`--tone-map`; `files` then carry the extension `.exr` instead of the one of `output`. `mip` is the
exported level, 0 being the largest and the default; `width` and `height` are the size of each
written image. `alpha_tail` says whether an alpha or smoothness tail was merged into the alpha
channel. `--tone-map` on a texture that is not high-range writes the PNG as usual and prints a
note on stderr. A format without a decoder fails with exit code 1 and a
message naming the format; a level the texture does not have fails with exit code 2.

PNG reports include the output transfer, channel transform and companion filenames.
The JSON keys are `output_transfer`, `png_transform` and `origin_receipts`. Resolve each
companion filename beside the corresponding image in `files`. Each companion binds the
exact PNG bytes and their physical DDS sources. The `field_origins` and `provenance`
members identify the header words used for the two report values. Linear RGB carries
a `gAMA` value of 1.0, sRGB RGB carries an `sRGB` chunk, and unknown or data samples stay
untagged. Floating-point tone mapping produces sRGB. The transfer tag leaves the encoded
RGB and alpha samples unchanged.

Automatic OpenEXR JSON reports also include `origin_receipts`, `field_origins` and
`provenance`. The report dimensions use the original DDS header of the decoded mip.
The CLI writes the companions with both text and JSON reports. Each companion binds
its exact OpenEXR bytes to the DDS header and the selected color
surface or cubemap surfaces. OpenEXR leaves separate DDS alpha tails unread.

## `p4kx audio info <archive> <entry> --json`

Also `p4kx audio info <file> --json` for a WEM file on disk.

```json
{"sound": "Data\\Sounds\\wwise\\Media\\100.wem", "codec": "pcm", "codec_name": "PCM",
 "format_tag": 65534, "channels": 2, "sample_rate": 48000, "sample_count": 1200,
 "duration_seconds": 0.025, "loop": null}
```

`sound` is the Entry path, or the path of the file on disk. `codec` is one of `pcm`,
`ima_adpcm`, `vorbis`, `opus`, `ptadpcm`, `xwma`, `xma2`, `aac`, `dsp`, `hevag`, `atrac9` or
`unknown`; `codec_name` is its readable name and `format_tag` the raw tag of the `fmt` chunk.
`sample_count` is the number of frames (samples per channel) and `duration_seconds` the playing
time. Both can be `null` when the selected header and codec leave the count unknown. A selected
`loop` has `start`, `end` and `frames` in samples per channel. The `end` is exclusive. The example
omits the additional `fieldOrigins` and `provenance` metadata.

`fieldOrigins` maps each output path to the evidence used by that value. The sound name comes
from input metadata. Format and derived values retain their original WEM inputs, including the
last applicable format chunk before the first data chunk. For an encrypted top-level P4K Entry,
the name can be unencrypted while the format payload is encrypted. A standalone file has unknown
original Archive encryption. The source and use identifiers resolve in `provenance`.

A Vorbis count is read within the selected `vorb` chunk. A chunk too short to supply a count uses
the existing embedded-format fallback when available.

## `p4kx audio wav <archive> <entry> --out <file> --json`

Also `p4kx audio wav <file> --out <file> --json` for a WEM file on disk.

```json
{"sound": "Data\\Sounds\\wwise\\Media\\100.wem", "output": "…\\100.wav", "channels": 2,
 "sample_rate": 48000, "sample_count": 1200, "size": 4844}
```

`sample_count` is the number of frames written and `size` the size of the written WAV file. A
codec without a decoder prints no JSON; it fails with exit code 1 and an error naming the codec.

`fieldOrigins` addresses `/sound` from the opened input's path metadata. The `/channels` and
`/sample_rate` cells retain the selected WEM format and the parser decisions used for the
WAV header. Opus keeps the effective playback rate. The immediate encryption markers apply
to those input bytes. A standalone WEM has unknown Archive encryption. The corresponding
source and use identifiers resolve in `provenance`.

For PCM, IMA ADPCM, PTADPCM, Wwise Vorbis and Opus input, `originReceipt` names the sibling
`<file>.p4kx-origin.jsonl` bound to the written byte length and SHA-256.
The same companion is published without `--json`. The JSON summary binds `/sample_count`
to the frame extent used by the writer and `/size` to the emitted RIFF size. The `/loop` origin
records the decision to admit or omit the loop in the WAV. A retained loop has separate
origins for its start, exclusive end and frame count.
Wwise Vorbis and Opus receipts distinguish the packet data checked while preparing the
conversion from bytes consumed during PCM replay. Vorbis tracks lookahead and Opus keeps
the original seek words and page framing when used.

## `p4kx audio ogg <archive> <entry> --out <file> --json`

Also `p4kx audio ogg <file> --out <file> --json` for a WEM file on disk.

```json
{"sound": "Data\\Sounds\\wwise\\Media\\300.wem", "output": "…\\300.ogg", "codec": "vorbis",
 "channels": 2, "sample_rate": 48000, "sample_count": 48000, "duration_seconds": 1.0,
 "codebooks": "aotuv", "size": 9876}
```

`codec` is `vorbis` for an Ogg Vorbis file rebuilt from Wwise Vorbis and `opus` for an Ogg Opus
file repackaged from an Opus WEM. `sample_count` is the number of frames the stream plays, the
granule position of its last page less the pre-skip for Opus, and `duration_seconds` its playing
time; Opus always plays at 48000 Hz. `codebooks` names the codebooks the Vorbis setup header was
rebuilt with: `default`, `aotuv` or `inline`; it is `null` for Opus. `size` is the size of the
written Ogg file. A codec other than Wwise Vorbis and Opus, a Vorbis WEM that no codebooks fit or
a malformed Opus WEM prints no JSON; it fails with exit code 1 and an error giving the reason.

`originReceipt` names the sibling `<file>.p4kx-origin.jsonl` bound to the written Ogg file's
byte length and SHA-256. The companion is also published in text mode. The summary's
`fieldOrigins` records `/sound`, `/codec`, `/channels`, `/sample_rate`, `/sample_count`,
`/duration_seconds`, `/codebooks`, `/size` and `/loop`. A retained loop has separate origins
for its start, exclusive end and frame count. The name uses the input path metadata. Vorbis
uses the declared WEM rate, and Opus selects the fixed playback rate through its codec.
A standalone WEM has unknown Archive encryption. Source and use identifiers resolve in
`provenance`.

## `p4kx bnk list <archive> <entry> --json`

Also `p4kx bnk list <file> --json` for a soundbank file on disk.

The JSON response includes `fieldOrigins`, keyed by JSON Pointer from the report root.
Each marker's `sourceUsesRefs` resolves in the shared `provenance` inventory. The soundbank
name uses the selected Archive directory Entry or observed
standalone file metadata. The bank header and embedded media values refer to their original
decoded positions. A standalone file can have complete source positions with unknown original
Archive encryption. A WEM parse failure preserves the attempted source and marks the evidence
incomplete. A proven non-sound class or prefetch explains the absent parsed values.

```json
{"soundbank": "Data\\Sounds\\wwise\\SSAM_Test.bnk", "version": 154, "bank_id": 3512036681,
 "wems": [{"id": 100, "size": 4852, "class": "wem", "codec": "pcm", "codec_name": "PCM",
           "sample_count": 1200, "duration_seconds": 0.025, "prefetch": false, "error": null},
          {"id": 400, "size": 120, "class": "wem", "codec": null, "codec_name": null,
           "sample_count": null, "duration_seconds": null, "prefetch": true, "error": null}]}
```

`version` and `bank_id` come from the `BKHD` section. `wems` lists every embedded media item in
ascending ID order: `size` is its size in the bank, and `codec` to `duration_seconds` are as in
`p4kx audio info`. `class` tells what the item holds from its first bytes: `wem` (a RIFF or RIFX
WAVE file), `midi` (a MIDI file), `impulse` (Wwise plug-in data such as a convolution reverb's
impulse response) or `unknown`. `prefetch` is `true` when the bank holds only the start of a
streamed WEM, whose whole file is the loose WEM of the same ID; its codec is then not read.
`error` is the reason an item of class `wem` or `unknown` cannot be read, else `null`; the codec fields are `null` for `midi`,
`impulse`, prefetched and unreadable items. A corrupt soundbank
prints no JSON; it fails with exit code 1 and an error naming the section.

## `p4kx bnk objects <archive> <entry> [--type <type>] [--id <id>] [--limit <n>] [--offset <n>] --json`

Also `p4kx bnk objects <file> --json` for a soundbank file on disk. It has no JSON Schema file:
the shape is documented here and fixed by a snapshot test.

The JSON response includes `fieldOrigins` and the referenced `provenance` inventory.
`/soundbank` addresses the selected Archive directory or standalone file name. `/version`
and `/bank_id` address the original decoded BKHD words. Each displayed object's
`/objects/<page index>/id`, `/objects/<page index>/type` and, when emitted,
`/objects/<page index>/type_byte` use the actual object envelope in the selected bank.
The decoded type and selection of the raw type-byte field also use the original bank version.
These paths follow the emitted page after filtering. A standalone bank has unknown original
Archive encryption.

An illustrative example, shortened and with values that do not come from one bank:

```json
{"soundbank": "Data\\Sounds\\wwise\\MUS_Music_Stanton.bnk", "version": 154, "bank_id": 528808344,
 "total": 15, "layouts": {"decoded": 10, "undecoded": 5, "unsupported": 0, "malformed": 0},
 "matched": 2, "offset": 0, "limit": 100, "next_offset": null,
 "objects": [
  {"index": 6, "id": 327692280, "type": "music-segment", "status": "decoded", "error": null,
   "parent": 828139056, "children": [119759791],
   "detail": {"duration_ms": 11527.659, "tempo": 120.0, "beats_per_bar": 4, "beat_value": 4,
              "markers": [{"id": 43573010, "position_ms": 576.162, "name": ""}],
              "stingers": [{"trigger_id": 1358954497, "segment_id": 1358954498}]}},
  {"index": 9, "id": 744, "type": "music-playlist", "status": "decoded", "error": null,
   "parent": null, "children": [10],
   "detail": {"rule_count": 2,
              "items": [{"depth": 0, "item_id": 1, "segment_id": 0, "children": 1, "play_mode": 1,
                         "loops": 0, "loop_min": 0, "loop_max": 0, "weight": 50000,
                         "avoid_repeat_count": 0, "uses_weight": false, "shuffle": false}]}}]}
```

`total` is the number of objects in the `HIRC` section and `layouts` counts all of them by how far
the reader decodes them. `decoded` identifies a readable layout. Containers, music objects
and buses must use their whole body. `undecoded` identifies a type without a decoder, such as
an LFO or an unrecognized type byte. `unsupported` identifies a recognized soundbank object
whose version has no verified decoder. The bus layout requires version 154. `malformed`
identifies an invalid body for a supported layout. `matched` counts the objects that pass `--type` and `--id`;
`objects` holds `limit` of them from `offset` on, in bank order, and `next_offset` is where the
next page starts, `null` on the last. A bank without a `HIRC` section has `total` 0.

Each object has `index` (its position in the section), `id`, `type` (`state`, `sound`, `action`,
`event`, `random-sequence-container`, `switch-container`, `actor-mixer`, `audio-bus`,
`layer-container`, `music-segment`, `music-track`, `music-switch`, `music-playlist`,
`attenuation`, `dialogue-event`, `fx-share-set`, `fx-custom`, `auxiliary-bus`, `lfo`, `envelope`,
`audio-device`, `time-modulator`, or `other` with `type_byte` giving the byte), `status` (as in
`layouts`) and `error` (the reason for `unsupported` and `malformed`, else `null`). `parent` is the
ID of the parent node, `null` for a node at the top and for an object without a parent field;
`children` lists the IDs the object holds in bank order: the actions of an event, the children of
a container, the tracks of a segment, the segments of a playlist and, for a music track, the media
IDs of its sources. `detail` holds the available eager type-specific fields. Event actions
appear in `children`. Checked bus declarations appear in `node_fields`.

Version-154 Attenuation objects include `source_fields` with original flags, optional cone
words, signed curve selectors, ordered conversion curves and initial RTPC fields.
[Original soundbank attenuation](audio-attenuation.md)

DialogueEvent objects include `source_fields` with the checked original declaration. Its
`layout` gives decoded, malformed or unsupported source coverage. The original object `status`
describes the playback reader's coverage. The `fields` member includes authored header words and
checked table extents. `rows` keeps original argument, node and property occurrences in order.
A malformed declaration keeps every readable row prefix. A readable link word carries an
`undetermined` interpretation, a child-window candidate and, for a nonzero word, a
target candidate. The child window keeps both halfwords, its admitted count and coverage.
The key role distinguishes the root from default and value candidates. Known decision mode
and argument type names accompany their original codes. An unsupported layout reports
its version and an empty row list.

Source values carry `containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete`. The
object-local `source_inventory` resolves each reference to its physical input and exact half-open
bank-relative byte span. A standalone file has unknown original Archive encryption lineage.
The same object shape is returned by MCP `list_soundbank_objects` within its existing whole-object
response budget. The CLI writes the complete checked rows for each selected object.

Version-154 Random/Sequence objects include `source_fields` with original loop counts and
modifiers, transition words, avoid-repeat count, mode codes, flags, child count and checked
playlist extent. Finite transition values use milliseconds. The raw binary32 words also
preserve nonfinite values. The playlist retains every readable original play ID and weight
word in authored order. Its signed `weight` and unsigned `weight_bits` share the same original
span. A short final weight keeps its play ID and checked row range. Source coverage distinguishes
a complete declaration, a malformed prefix, an unreachable continuation and an unsupported
version.

Version-154 Switch objects include `source_fields` with original group and default values,
continuous-validation byte, child extent, membership groups and child configurations.
Group, member and configuration rows keep physical order and repeated IDs. Each fade
retains its signed value and original 32-bit word. A readable member remains visible
when the declared membership table is truncated. Known group codes identify Switch or
State. Membership references carry unchecked resolution.

Version-154 Blend objects include `source_fields` with original children, Layer and
association counts, crossfade RTPC selectors and the continuous-validation byte.
Rows carry physical Layer, curve, association and point positions. Initial RTPC data
includes its checked complete curves. Crossfade points keep each readable binary32
word and the original interpolation code. Association references carry unchecked
resolution. Unknown and non-finite words remain available.

Version-154 MusicSegment, MusicSwitch and MusicPlaylist objects expose `source_fields.stinger_count` and their original
`stingers` in physical order. Every row carries the trigger and segment IDs, sync code,
cue filter, signed repeat time with its raw word, and look-ahead count when readable.
The final truncated row retains its checked byte range and complete words. Older layouts
have an explicit unsupported status and no interpreted rows.

Version-154 AudioBus and AuxiliaryBus objects expose their checked source layout in
`node_fields.object_status`. The `node_fields.bus` extension keeps original bus
settings, root device share-set, recovery and volume words, ordered duck targets
and any raw remainder. The shared node fields include the original properties,
positioning, Aux routing, effects, metadata, inline State and RTPC. An
unsupported bus version reports that disposition and retains its original
body. Every exported value cites the owning bank bytes.
[Bus source contract](media-bank-metadata.md#audiobus-and-auxiliarybus)

Version-154 MusicTrack objects include `source_fields` with original track
controls, source, clip, automation and switch-member rows. Clip records preserve
their authored track IDs and exact 16-digit binary64 words. Finite clip start,
trim and duration values are in milliseconds. Original automation
points retain readable words when the final point is truncated. Switch transitions
retain their original signed fade, synchronization and cue words. Each checked
value links to its original byte range in the owning bank.

Version-154 Sound, ActorMixer, Random/Sequence, Switch, Blend and music nodes also
include `node_fields`. The document binds the object status, bank version, original
parent ID (including zero), override bus ID and packed priority/MIDI byte to the owning
bank. The ordinary and ranged property lists preserve authored order, original IDs,
raw words and each list's checked counts. Qualified version-154 voice properties
also include their property name and unit plus any readable finite number.
Nonfinite values carry a null interpretation beside their exact raw bits.
Effect and metadata slots retain their
physical order, indices, reference IDs and raw flag bytes. Aux routing retains the
four authored user send slots and reflections ID. Advanced settings preserve the
original instance limit, behavior codes and flags. Positioning rows preserve original
vertex, playlist and automation words and indicate a checked partial final row.
Inline State includes the original binding, group and member sequence and its raw
property words. RTPC includes the original curve count, checked complete curve
prefix, and ordered raw curve and interpolation words. Fields and words are omitted when the reader
has no complete source value. Each sourced value can be resolved through
`node_fields.source_inventory` and includes its encryption marker. The same object
shape is returned by MCP `list_soundbank_objects`.
ActorMixer node fields also retain the original child count, checked child
range and each readable child ID in physical order.

Standalone State objects include sourced version, layout and raw body range in
source_fields. A checked bundle adds original count, ordered property IDs and
binary32 words, finite values, consumed and trailing ranges. Unsupported
layouts retain the original body range. Action source fields include the
original type and target when readable. Version-154 Play properties include
the authored target and fade flags plus both original planar lists. The
property layout distinguishes absent, unsupported and malformed observations.

Music switches and playlists also expose a `source_fields.transition_rules` object.
Its `layout` distinguishes a decoded table, a malformed table with a checked prefix,
an unavailable rule count and an unsupported bank version. Its sourced
`declared_count`, `complete_rules`, `complete_range` and `checked_range` keep
the authored count separate from the half-open bank-relative byte positions.
A malformed suffix outside the rule table does not invalidate a complete table.
The `rows` list retains each `rule`, then its source and destination `id` occurrences.
IDs retain their list and original positions, including duplicates, zero and sentinel words.
The rule preserves both fades, cue filters, jump and entry controls, the raw allocation byte
and the allocated object's segment, fade and playback fields. Signed fade words include
their original bits and signed value. Each value carries its original field origin.
This describes authored rules without computing a transition or choosing a playable sequence.

Music switches expose `source_fields.decisions` with the sourced playback byte,
group count, tree size, mode byte and checked tree range. Its layout distinguishes
`decoded`, `malformed`, `unavailable` and `unsupported` decision blocks independently of
the stingers and transition table. Its rows retain original argument groups and every
complete physical node, including unvisited nodes. Each node preserves its raw key,
link, weight and probability. Target rows list the checked nonzero occurrences in
traversal order with the physical node, level and ancestors. The target ID cites
the leaf word and selecting branch links.

When derived route replay would exceed the input-proportional allowance,
`decisions.targets_compact.value` is `true`. The rows retain the original physical
graph and omit expanded target occurrences. The layout still reports whether
the graph is decoded or malformed.
The detailed rows account for ancestor references as well as visits. The summary
can still list target IDs when the detailed rows use a compact projection.
To follow a decoded graph, start at node ordinal 0 at level 0 if a node is present.
Below `group_count`, the low 16 bits of `link_word` give the first child ordinal
and the high 16 bits give the child count. Each child is one level deeper. At
`group_count`, the whole `link_word` is the target ID, with zero denoting no target.
This representation preserves shared nodes and duplicate target IDs.

Other object details:

- `state`: standalone State versions 128 through 145 expose `property_count`, `count_range`,
  ordered `properties`, `consumed_range` and `trailing_range`. A property has its original RTPC
  parameter `id`, binary32 `value_bits`, and `value`, which is null for nonfinite values.
  Its `id_range` and `value_range` identify the original words. Ranges are half-open
  bank-relative offsets written as unsigned decimal strings.
- `sound`: `plugin_id`, `storage` (`embedded`, `prefetch-streamed`, `streamed` or `other`),
  `source_id` (the WEM ID), `in_memory_size`, `language_specific`, `source_plugin` (a plug-in
  generates the sound, no WEM holds it).
- `action`: `action_type`, `target` and `soundbank_id`, the soundbank a play action of a version
  154 bank names (0 for none); it is `null` for every other action.
- `random-sequence-container`, `switch-container`, `layer-container`: `container_kind` (`random`,
  `sequence`, `switch` or `blend`).
- `music-segment`: `duration_ms`, `tempo`, `beats_per_bar`, `beat_value`, `markers` (`id`,
  `position_ms`, `name`) and `stingers` (the segment `segment_id` a `trigger_id` plays).
- `music-track`: `track_kind` (`normal`, `random`, `sequence`, `switch` or `other`), `clips`
  (`source_id`, `event_id` (0 for none), `play_at_ms`, `begin_trim_ms`, `end_trim_ms`,
  `source_duration_ms`) and, for a switch track, `switch` (`group_type` 0 for a switch group and
  1 for a state group, `group_id`, `default_switch`, `switches` in clip order).
- `music-switch`: `rule_count`, `group_ids` (the groups the decision tree tests, outermost first)
  and `targets` (the nodes its leaves play, in tree order). When the ID projection
  uses the compact graph, the detail omits `targets` and reports `targets_compact: true`.
- `music-playlist`: `rule_count` and `items` in bank order. An item with `children` of N is
  followed by its N child items, and `depth` is the nesting that gives; the bank does not
  validate the counts, so `depth` may be any value (the text output draws at most 8 levels of
  indentation and marks deeper items with `+N`). `segment_id` is 0 for a group. `play_mode` is 0 random, 1 sequence continuous, 2 random step, 3 sequence step. `loops`
  is how often the item repeats, 0 for endlessly; `loop_min` and `loop_max` are the range of a
  random repeat count; `weight` counts when `uses_weight`.

Durations and positions are in milliseconds. A corrupt soundbank prints no JSON; it fails with
exit code 1 and an error naming the section. Malformed or unsupported objects do not fail the
command.

## `p4kx bnk extract <archive> <entry> --out <folder> [--format wem|wav|ogg] --json`

Also `p4kx bnk extract <file> --out <folder> --json` for a soundbank file on disk.

```json
{"soundbank": "Data\\Sounds\\wwise\\SSAM_Test.bnk", "target": "…\\out", "format": "wav",
 "extracted": [{"id": 100, "file": "…\\out\\100.wav", "size": 9644, "prefetch": false}],
 "failed": [{"id": 300, "error": "… WEM 300: converting Wwise Vorbis to WAV is not supported"}]}
```

`format` is `wem`, `wav` or `ogg`, the extension of the files (MIDI media written with `wem` has the extension `mid`). `extracted` lists the WEMs written as
`<id>.<format>`, `failed` the WEMs that were not, with the reason and its causes joined by `: `;
both in ascending ID order. With `--format wav` a codec without a decoder and a prefetched start
fail; with `--format wem` a prefetched start is written as the bank holds it and marked with
`"prefetch": true`, and MIDI media is written as `<id>.mid`. With `--json`
failures are not repeated on stderr.

With `--format wem` (the default), each installed media file has a source-origin companion. For a
published companion, `originReceipt` names the sibling file relative to `target` and
`originReceiptDigest` gives its `byteLength` as a decimal string and lowercase SHA-256 as `sha256`.
Verify this digest against the companion before using its origin, then verify the companion's
artifact binding against the copied media. This identifies the receipt generated by the run even
if the same output path and bytes were later published from another source. The companion addresses
the bytes as `opaquePayload` and records their exact extent in the decoded bank. Standalone bank
files carry unknown original encryption. This receipt applies to copies, including MIDI and
prefetched bytes. WAV and OGG conversion reports omit these receipt fields.

A receipt or receipt-lock failure after the raw file was installed is reported as
`originReceiptErrors` on that `extracted` member. The receipt reference only appears when that
receipt was installed. A reserved path collision before the media commits is a `failed` member
and preserves the prior media file. The exit code is 1 when either kind of failure occurs.

## `p4kx ui assets <archive> <entry> --json`

Also `p4kx ui assets <file> --json` for a GFx or SWF file on disk.

```json
{"movie": "Data\\UI\\Screens\\SWF\\Panel.gfx", "signature": "CFX", "version": 10,
 "compression": "zlib", "length": 512,
 "stage": {"x_min": 0, "x_max": 20480, "y_min": 0, "y_max": 15360},
 "frame_rate": 30.0, "frame_count": 1, "frames": 1,
 "exporter": {"version": 879, "flags": 16, "bitmap_format": 19, "prefix": "", "swf_name": "Panel"},
 "tags": [{"code": 0, "name": "End", "count": 2}, {"code": 777, "name": null, "count": 1}],
 "unknown_tags": [{"code": 777, "offset": 498, "length": 6}],
 "shapes": [{"id": 4, "version": 3, "bounds": {"x_min": 0, "x_max": 2000, "y_min": 0,
             "y_max": 1010}, "fill_styles": 1, "line_styles": 1, "edges": 2,
             "export_name": null}],
 "morph_shapes": [{"id": 6, "version": 1, "bounds": {…}, "fill_styles": 1, "line_styles": 0,
                   "edges": 0, "export_name": null}],
 "bitmaps": [{"id": 1, "kind": "lossless", "width": 2, "height": 1, "format": "argb32",
              "file": null, "image_id": null, "export_name": "icon_alert"},
             {"id": 2, "kind": "external", "width": 8, "height": 4, "format": "tiff",
              "file": "../Textures/panel_bg.tif", "image_id": null, "export_name": null},
             {"id": 3, "kind": "sub_image", "width": 4, "height": 2, "format": null,
              "file": null, "image_id": 2, "export_name": null}],
 "fonts": [{"id": 8, "format": "DefineFont3", "name": "Electrolize", "full_name": null,
            "bold": true, "italic": false, "glyphs": 1, "outlines": true, "export_name": null}],
 "sprites": [{"id": 9, "frame_count": 2, "frames": 2, "labels": ["idle"],
              "frame_labels": [{"frame": 1, "name": "idle"}], "placed": [4],
              "export_name": "PanelButton"}],
 "imports": [{"url": "../../fonts/Shared/fonts_en.swf",
              "assets": [{"id": 12, "name": "$OrbitronMed"}]}],
 "symbol_classes": [{"id": 9, "name": "PanelButton"}],
 "scaling_grids": [{"id": 9, "splitter": {"x_min": 100, "x_max": 900, "y_min": 100, "y_max": 400}}],
 "unread_tags": [{"code": 12, "name": "DoAction", "count": 3}],
 "failures": [{"code": 2, "offset": 508, "error": "unexpected end of data at offset 510: …"}]}
```

`signature` is `FWS`, `CWS` or `ZWS` for SWF and `GFX` or `CFX` for GFx; `compression` is `none`,
`zlib` or `lzma`, and `length` the size of the uncompressed movie. `stage` and every `bounds` are
in twips, a twentieth of a pixel. `frames` counts the `ShowFrame` tags of the main timeline,
`frame_count` is what the header declares. `exporter` is the GFx `ExporterInfo` tag, `null` for a
SWF movie. `tags` counts every tag code, sprites included, in ascending code order, with `name`
`null` for a code that is not known; `unknown_tags` lists those tags with the offset of their body
in the uncompressed movie. Shapes, morph shapes, bitmaps, fonts and sprites are in file order;
`edges` counts the straight and curved edges (of the start state of a morph shape), and
`export_name` is the first name an `ExportAssets` tag gives the ID. A bitmap's `kind` is
`lossless`, `jpeg`, `png` or `gif` for an embedded image, `external` for a GFx external image whose
`file` is the path the movie gives, relative to it, and `sub_image` for a rectangle of the bitmap
`image_id`; `format` is the pixel format of a lossless bitmap, `with alpha` or `opaque` for an
embedded image and the GFx file format (`dds`, `tiff`, `tga`, …) of an external one; `width` and
`height` are `null` where the movie does not state them. A font's `format` names its tag; `glyphs`
is its glyph count and `outlines` whether glyph shapes are stored. A sprite lists its frame
`labels` and, as `frame_labels`, each label with the index of the frame it names (from 0), and the
IDs it `placed`, ascending. `imports` lists the assets the movie imports from other movies, each
`url` as the movie states it with the local `id` and the exported `name`; `symbol_classes` the
names bound to classes, `scaling_grids` the nine-slice rectangles. `unread_tags` counts like `tags`
the known tags whose content is not read (actions, buttons, videos, debug and text-rendering
hints). `failures` lists the tags that could not be read;
they leave out their definition and the rest of the movie is still read, but the exit code is 1.
A file that is no movie, or one larger than 64 MiB, prints no JSON; it fails with exit code 3.

## `p4kx ui extract <archive> <entry> --out <folder> --json`

Also `p4kx ui extract <file> --out <folder> --json` for a GFx or SWF file on disk.

```json
{"movie": "Data\\UI\\Screens\\SWF\\Panel.gfx", "target": "…\\assets",
 "extracted": [{"kind": "bitmap", "id": 1, "file": "…\\assets\\bitmap_1.png", "size": 77},
               {"kind": "shape", "id": 4, "file": "…\\assets\\shape_4.json", "size": 1081},
               {"kind": "morph_shape", "id": 6, "file": "…\\assets\\morph_shape_6.json",
                "size": 1038}],
 "failed": [{"kind": "tag", "code": 2, "offset": 508, "error": "unexpected end of data at …"},
            {"kind": "bitmap", "id": 5, "error": "Data\\UI\\Screens\\SWF\\Panel.gfx definition 5
             refers to ../Textures/gone.dds, which does not exist; tried
             Data\\UI\\Screens\\Textures\\gone.dds"}]}
```

`extracted` lists the files written, `<kind>_<id>.png` for bitmaps (8-bit RGBA, straight alpha)
and `<kind>_<id>.json` for shapes and morph shapes. `failed` lists first the tags that could not
be read, as `tag` with their `code` and the `offset` of their body in the uncompressed movie, then
the definitions that could not be written, with their `id`; each with the reason and its causes
joined by `: `. Written and failed definitions list bitmaps, then shapes, then morph shapes, each
in file order. An external image of a movie on disk is read only as DDS from inside the nearest
ancestor folder named `Data`, or else the movie's folder; a path leaving it fails. A shape file
holds the shape as the movie stores it, in twips: `id`, `version`, `bounds`, `edge_bounds`, the
`fill_styles` and `line_styles` numbered from 1 by the records, and the `records` in drawing order,
each a `style_change` (a `move_to` position and new style numbers), a `straight_edge` with its
`delta`, or a `curved_edge` with its `control` and `anchor` deltas. With `--json` failures are
not repeated on stderr. The exit code is 1 when `failed` is not empty.

`originReceipt` is the relative name `.p4kx-origin.jsonl` when this run wrote files,
or null when it wrote none. `fieldOrigins` gives the immediate marker and source
uses for exact pointers in the summary, such as `/extracted/0/id` and
`/extracted/0/size`. The definition supplies the ID. The written length also
depends on the actual decoded inputs, including a selected external DDS color
or alpha part. The `provenance` inventory resolves those source-use references.

The [directory companion](source-origin.md#directory-publication) binds each
committed file by its relative name, byte length and SHA-256. Its PNGs have one
opaque payload origin. Shape and morph JSON have a receipt for each JSON Pointer.
A sub-image cites its crop declaration and the actual base bitmap inputs. A
standalone input's original Archive encryption is unknown.

## `p4kx ui render <archive> <entry> --out <file.png> [--svg] [--size <W>x<H>|<W>] [--background …] [--lang <language>] --json`

Also `p4kx ui render <file> --out <file.png> --json` for a GFx or SWF file on disk.

The report carries `originReceipt` (the sibling companion basename),
`artifactOrigin`, `fieldOrigins` and `provenance`. The artifact receipt
binds the actual PNG or SVG bytes. See [source origins](source-origin.md#movie-rendering).

`--lang` (only with an Archive and an Entry) resolves the localisation keys (`@key`) of text fields
in the `global.ini` of that language; a key it does not hold is listed as `unlocalised`, and without
`--lang` keys are drawn as they are. A `global.ini` that cannot be indexed prints no JSON and fails
with exit code 3. The same language is the localisation folder (`Data\Localization\<folder>`) in
which fonts that the movie imports are looked for after the folders of the UI, English without
`--lang` (ADR-0152).

```json
{"movie": "Data\\UI\\Screens\\SWF\\Panel.gfx", "target": "…\\panel.png", "format": "png",
 "width": 64, "height": 48,
 "skipped": [{"id": 50, "depth": 7, "sprite": 9, "reason": "not_drawn", "drawn": false,
              "message": "DefineButton2 definitions are not drawn"}],
 "failed": [{"kind": "tag", "code": 2, "offset": 508, "error": "unexpected end of data at …"},
            {"kind": "bitmap", "id": 5, "error": "Data\\UI\\Screens\\SWF\\Panel.gfx definition 5
             refers to ../Textures/gone.dds, which does not exist; …"}],
 "imports": {"movies": ["Data\\UI\\fonts\\Shared\\fonts_en.gfx"],
             "unresolved": [{"url": "../../../fonts/Shared/fonts_en.swf", "name": "$Text1Heavy",
                             "reason": "no_such_export",
                             "message": "the movie does not export the name as a font"}]}}
```

`imports` reports the fonts the movie imports with `ImportAssets` that its texts use: `movies` are
the Entries read to resolve them (an Archive only), `unresolved` the imported names that were not
found, each with its `url` as the movie states it, a stable `reason` (`not_found`, `unreadable`,
`no_such_export`, `too_deep` or `over_budget`) and a `message` that says it in words; their texts
are drawn as boxes and listed as `missing_font` in `skipped`. Two imports of one name from
different URLs are two fonts, and a movie's bytes count towards `over_budget` as the larger of the
Entry size and the uncompressed length the movie declares.

`format` is `png`, or `svg` with `--svg`; `width` and `height` are the pixels of the PNG, or the
size of the SVG document, written to `target`. `skipped` lists in drawing order every element the
frame left out or drew without an effect: `id` and `depth` name the placement, `sprite` the sprite
whose timeline holds it (absent on the main timeline), `reason` is one of `unknown_definition`, `not_drawn`, `sprite_depth`, `too_many_placements`, `too_many_paths`,
`too_many_records`, `fill_budget`, `too_many_masks`, `nested_mask`, `gradient_stops`,
`missing_bitmap`, `invalid_geometry`, `raster`, `mask_memory`, `bitmap_memory`, `too_many_glyphs`,
`missing_font`, `missing_glyph`, `missing_glyph_index`, `unlocalised`, `text_truncated`, `filters`
and `blend_mode`, `drawn` is `true` for `filters`, `blend_mode` and the reasons of texts other than
`too_many_glyphs`, which are drawn without the effect or as a fallback (an empty box for a missing
font or glyph, the key itself for an unknown key), and `message` says it in words. `fill_budget`
and `too_many_glyphs` end the frame: this and every later element are not drawn. `failed` lists
first the tags that could not be read (`kind` `tag`, with `code` and
`offset` as in `ui extract`), then the bitmaps that could not be decoded (`kind` `bitmap`, with
`id`); a fill with such a bitmap is skipped as `missing_bitmap`. The exit code is 1 when `skipped`
or `failed` is not empty. A canvas that is empty or larger than 16384 pixels a side or 2^25
pixels, and a `--out` that is the movie file, print no JSON and fail with exit code 2.

## `p4kx ui hologram <archive> <entity> --out <file.png> [--size …] [--background …] [--view-angle …] [--elevation …] [--fov …] [--distance-scaler …] [--tint …] [--opacity …] [--glow …] [--lod <n>] [--software] --json`

```json
{"archive": "C:\\…\\LIVE\\Data.p4k", "entity": "AEGS_Avenger_Titan", "guid": "…",
 "out": "C:\\…\\titan.png", "width": 1600, "height": 900, "bytes": 56408,
 "parts": 72, "modelled_parts": 44, "drawn_parts": 43, "meshes": 25, "triangles": 589733,
 "camera": {"source": "record", "fov_degrees": 10.0, "view_angle_degrees": 60.0,
            "elevation_degrees": 20.0, "distance_scaler": 1.5},
 "adapter": "NVIDIA GeForce RTX 5090 (Vulkan)",
 "milliseconds": {"assembly": 1166, "scene": 11, "draw": 484, "write": 11, "total": 1674},
 "issues": [Issue, …]}
```

`camera.source` is `record` when the `UIHoloVehicle_Config` Record of the Archive set the camera,
`defaults` when it has none (a field of view of 10 degrees, a view angle of 60 and a distance
factor of 1.5), and `… and options` when `--fov`, `--view-angle` or `--distance-scaler` replaced
a value. The camera values are the ones used: values that are not finite fall back to the
defaults and the rest are clamped, as the camera does. `drawn_parts` counts the parts placed
in the scene before camera culling. `triangles` counts the triangles of their meshes. `issues`
contains the notes and
skipped parts of the Assembly as `entity glb` lists them. `milliseconds` splits the run into
reading and assembling, building the scene, drawing and reading the picture back, and writing
the file. An Entity the Archive lacks, a bad option, a `--out` that is the Archive or lies in a
Star Citizen installation and a `--size` that is empty or beyond what the device draws
print no JSON and fail with exit code 2. A replacement that does not fit its port is drawn all the
same and listed in `incompatible` (absent when empty), as for `entity loadout`; `--loadout-pick`
works as there. A machine without
a graphics adapter fails with `no graphics adapter` on stderr and exit code 1.

`fieldOrigins` and `provenance` address the selected Entity and individual camera settings.
Each setting uses the actual field of the selected `UIHoloVehicle_Config` Record or a command-line
option. An explicit override is tool-authored even when its number equals the stored value.
A readable Record with a missing setting retains the declaration survey that chose the fallback.
`originReceipt` names the sibling `<file>.p4kx-origin.jsonl`. `artifactOrigin` addresses the
committed PNG as one opaque payload and is also the origin of the measured `bytes`. It includes
model and character-definition reads used for placed geometry, resolved material slots, authored
port helper and offset fields, and the camera settings. Models that place scene bounds contribute
even when the camera later culls them. An unused LOD or material library carries no contribution.
The known contributors of `drawn_parts` and `triangles` come from the placed scene. An incomplete
set can still have a positive encryption marker when a known contributor is encrypted. Other
Assembly counters and diagnostics retain unknown origins where their inputs are unproved.

## `p4kx ui screens <archive> <entity> --json`

The `BuildingBlocks` screens of an Entity and of everything attached in its resolved default
Loadout (ADR-0089). A screen is any Reference of an Entity's Record to a `BuildingBlocks_Canvas`,
found at any depth of the Record, e.g. the default view of `UIBuildingBlocksEntityComponentParams`.

```json
{"archive": "C:\\…\\LIVE\\Data.p4k", "entry": "Data\\Game2.dcb",
 "entity": "AEGS_Avenger_Titan", "brand": "aegs",
 "screens": [{"port": "hardpoint_dashboard_pilot/Support_L", "entity": "Vehicle_Screen_MFD",
              "entity_record_path": "libs/foundry/records/entities/scitem/ships/displays/vehicle_screen_mfd",
              "field": "Components[12].layers[0].views[0].component.canvas",
              "canvas_guid": "33bda02c-099a-447f-ba1e-2e6b59bfafce",
              "canvas": "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/…/m_mfd_screen",
              "width": 800.0, "height": 600.0,
              "assets": {"canvases": ["…"], "textures": ["UI/Textures/…/gradient_radial.dds"],
                         "vectors": ["UI/Textures/Vector/…svg"], "movies": [],
                         "localisation_keys": ["…"]},
              "elements": [{"element": "DisplayWidget", "count": 54, "rendered": true},
                           {"element": "WidgetText", "count": 1, "rendered": false}]}],
 "movies": [{"port": "…", "entity": "…", "movie": "Data/UI/….gfx"}]}
```

`port` is the path of port names from the root Entity, empty for the root Entity itself. `brand`
is the manufacturer code of the class name prefix when the Archive has the HUD style
`styles/s_<code>_hud`, and absent otherwise; `ui render --brand` takes it. `canvas` is the record
path `ui render --screen` takes. `assets.canvases` starts with the screen's own canvas and holds
every canvas it nests; `.tif` textures are listed as the `.dds` files the Archive holds;
`localisation_keys` lose the leading `@`. `elements` counts the widgets of every canvas of the
screen by type, `rendered` says whether `ui render --screen` draws the type. `movies` lists the
`moviePath` of UI owner components. A canvas that cannot be read keeps its row with the GUID as
`canvas`, size 0 and no assets. The exit code is 0.

`fieldOrigins` and `provenance` describe the original DataForge declarations used by this listing.
The receipt distinguishes a screen reference, its admitted canvas path, each read dimension, the
authored field behind a deduplicated asset path, and every widget counted in an element group.
Widget addresses record the physical canvas Record, the original `scene` or `library` field, its
stored array position and, for an asset, the value's preorder position within the widget. The
listed texture, SVG, movie and localization paths come from authored DataForge fields. Origins with
unproved loadout port segments or nested canvas selection routes report an incomplete source set.
Default port segments retain the original port `Name` and `defaultItem.entityClass` from the
physical parent Entity Record. An explicit loadout name retains the evidence of other proven
segments of its path.
Direct nested canvases retain the actual `scene` or `library` widget `canvas` Reference used to
reach each read. A style or operation route whose selector is unobserved retains an incomplete
source set for dependent values.

## `p4kx ui render <archive> --screen <canvas> --out <file.png> [--svg] [--size <W>x<H>|<W>] [--background …] [--brand <code>] [--var <name>=<value>]… [--lang <language>] --json`

Renders a `BuildingBlocks` canvas, by record path or GUID, instead of a movie; an ENTRY excludes
`--screen`. Texts are drawn from the fonts of the movie the `UIElement` Record
`uielements/buildingblocks` names (`fonts_en.gfx`) and their localisation keys resolve in `--lang`,
English by default; when the font movie or the language cannot be read, a warning goes to stderr and
the texts are left out.

Screen PNG and SVG have a sibling `<file>.p4kx-origin.jsonl` companion.
The JSON summary adds `originReceipt`, `artifactOrigin`, `fieldOrigins` and
`provenance` as described in [Source origin](source-origin.md). An explicit
width is an option. The derived height uses both original canvas size fields.
An unknown diagnostic retains a null origin marker.

### Screen content diagnostics

The screen is an offline still of the authored widget properties, the supported style cascade and
the evaluated bindings. A source-enabled parent background can appear behind a text widget.
The RSI POWER reference keeps the authored backgrounds in the powered still. Without supplied
variables that reference leaves the POWER canvas outside the entered canvas set.
`variables` describes the caller's settings and whether reachable operations read them.
`unsupported` groups approximations and undecided bindings. `content_omissions` is present when
the renderer finds an omitted runtime target, list data source or field binding. Each row lists
`element`, `widget`, `widget_input` (`record_index`, `section` and `slot`), `field`, optional
authored `source` and a `code`. The widget position is its original `scene` or `library` slot.
Each original widget and authored field or operation has one detailed row per omission code. The grouped report
keeps per-visit counts when the renderer repeats a widget.

The grouped layout diagnostics name the affected containers and their policy fields.
Their `examples` list includes every distinct original widget site as a name,
canvas record index, and `scene` or `library` slot. Repeated instances count as
separate visits in `count` and list their original site once.
They cover fixed children grown by `Stretch`, separate cross justification and item
alignment in a single line, the cross-axis start for `WrapReverse`, and content-sized
automatic children across a stack. The report records a rule when it affects the
placement. See [ADR-0106](adr/0106-screens-repeat-library-templates-and-lay-widgets-out-by-line-anchor-pivot-turn-and-clip.md#layout-cases-without-a-discriminating-game-reference).

| Code | Condition |
| --- | --- |
| `runtime_target` | `imageSource` names a target drawn by the game. |
| `runtime_array` | `arrayVariable` names game data supplying list rows. |
| `source_missing` | The widget or binding has no target or variable name. |
| `variable_missing` | A direct localised binding has no caller value. |
| `unsupported_field` | The field or its binding kind is outside the renderer. |
| `unresolved_input` | The graph input could not be evaluated, or its value failed the field's checks. |

Report field origins identify the physical widget, direct authored field or binding operation.
The omission has an incomplete origin when its runtime inputs are unavailable. Named static
assets that fail to load are listed as missing or unreadable assets in the same report.
The origin companion
identifies the contributing DataForge payload, canvas size fields and observed asset sources.
See [ADR-0337](adr/0337-a-requested-brand-without-a-brand-style-in-a-canvas-gets-the-default-entries-only.md)
for the RSI power-page investigation and its runtime evidence boundary.

`--var <name>=<value>` (repeatable, at most 64, only with `--screen`) sets a game variable before
the bindings are evaluated ([ADR-0259](adr/0259-screens-take-game-variables-from-the-caller.md)).
Other variables keep their type's state-at-rest value (ADR-0110).
An explicit empty value clears directly bound localised text. `p4kx ui render … --screen <MFD> --var powerstate=1` draws the MFD of a powered ship.
The bindings of a screen read variables by path, such as `/seatdashboard/powerstate`: a name
without `/` is the last segment of that path, one with `/` is the whole path, and case is ignored.
The value is read as the kind of variable a binding names: `true`, `false` or a number for a
boolean, a finite number for a number or integer, any text for a string; a value of another kind
leaves the bindings that read it undecided and lists them as `Binding` (`the game variable <name>
is set to `<value>`, which is not <kind>`). A name is at most 128 bytes and a value 256, neither
holds a control character, and the last setting of a name wins. A malformed setting, more than 64
settings and a name or value beyond those bounds are refused (exit code 2); the values are never
evaluated beyond that reading.

A name may follow a namespace and `::`, as in `mapdisplaymaster::isradar=1` (ADR-0277): the
setting then reaches the bindings of that canvas only, named by the last segment of its Record
path, case ignored; a name without namespace reaches every canvas. The namespace is a canvas name
without `/`; one that names no canvas of the screen is reported as unknown, like any name, with the
namespace in the name. A setting holds at most one `::`; `a::b::c` and `a:::b` are refused (exit
code 2). Canvases of the same Record name are not told apart: a namespace reaches all of them.
A canvas widget's canvas that a style entry or a `StringFromBooleanCanvas`
binding picks is entered (ADR-0277); a canvas that only run-time data names is listed as
`Binding`. Screens the game renders into a texture (the star map) are not drawn and stay listed.

```json
{"screen": "libs/foundry/records/ui/buildingblocks/test/test_mfd",
 "target": "…\\mfd.png", "format": "png", "width": 200, "height": 100,
 "widgets": 14, "shapes": 12, "texts_drawn": 1, "texts_left_out": 0,
 "images_drawn": 2, "images_left_out": 1, "canvases": 2,
 "unsupported": [{"element": "WidgetText", "reason": "no font style: default font used",
                  "count": 1, "examples": ["label"]},
                 {"element": "WidgetImage", "reason": "texture missing or unreadable",
                  "count": 1, "examples": ["UI/Textures/Test/picture.dds: no Entry named …"]}],
 "missing_canvases": [], "truncated": false,
 "variables": [{"name": "powerstate", "value": "1", "used": true}],
 "skipped": [{"reason": "fill_budget", "message": "the fills cover more than the budget"}]}
```

`variables` is present only when `--var` was given: the variables in force, the last setting of
each name with its value as given, and `used`, whether an operation of a canvas the screen can
reach reads a variable that name matches. A setting with `used` false is reported on stderr as
`unknown game variable: <name> (no operation of the screen reads it)` and changes nothing; the
exit code does not depend on it. Without `--json`, a line `game variables in force: …` goes to
stdout before the summary.

`width` and `height` are the pixels written. `widgets` counts the widgets visited in every canvas
composed, `shapes` the shapes drawn (every placed glyph counts), `texts_drawn` and `texts_left_out`
the text widgets whose text was and was not drawn (empty or run-time texts, a missing font or
colour, a limit), `images_drawn` and `images_left_out` the widgets whose texture or vector graphic
was and was not drawn (a file that is missing, unreadable or beyond a limit, or a graphic the game
names only at run time, and the 3D scenes and run-time pictures of a screen, which keep their
box), `canvases` the canvases composed (the screen's own and the
nested ones). `unsupported` lists per element type and reason (`no renderer for this element
type`, `automatic size taken as the size of the parent`, `<layout> children are laid out as an
overlay` for spacer containers, `list rows require a run-time array supplied by the game`, `list
rows repeat the template without the values of their rows`, `fading edges of the overflow are not
drawn`, `gradient backgrounds are drawn in the plain colour of the background`, `borders that
differ are drawn straight on a box with rounded corners`, `the turn and scale of the parent are
not undone: they are inherited`, `a number of the geometry is not finite: the default is used`,
`the image is rendered by the game at run time; the Record field imageSource names it`, `3D content is drawn by the game at run time`,
`the line ends at a widget the game places at run time`, `the Archive has no canvas for this
standard widget`, `transform is not finite: not drawn`, `scale of the canvas is not finite: not drawn` (element
`Canvas`),
`no target that is a widget of the canvas: nothing repeated`, `the template contains a clone of
itself: not repeated`, `clipping stopped at the 24th mask: later content is not clipped`,
`colour slot <name> is not in the palette`, `colour of kind <type>`, `selector <type> is not
evaluated` for a `StyleEntry` (the selectors of the entries of the canvas, of its styles and of the
`stylesheetOverride` of a canvas widget, listed when the canvas is first laid out; `Count` of a
type other than `First` and `Every`, and `Child` that holds a `Parent` or `Ancestor`, are among
them), the `Binding` reasons below, and the reasons of circles, polygons and texts) how many
widgets it concerns and the names of up to three.

`Binding` lists what the state at rest of a canvas (ADR-0110) could not decide, per canvas once
for the fields and per set of parameters for the inputs. The bound fields `Instantiated`,
`IsActive`, `Alpha`, `ParamInput0` to `ParamInput4` and the number fields of the layout (`SizeX`,
`AnchorY`, `PivotX`, `ScaleX`, `PosX`, `PaddingLeft`, `MinSizeX` and the like, ADR-0296) that
evaluate to a value are applied and not listed, and so are the bound state tags
(`PrimaryStateTag` to `QuinaryStateTag`), which the styles of the widget select on like its own
tags (ADR-0287). The reasons are `bound layout fields (behaviours, depth, flex) are not
evaluated at rest`, `bound field
<field> is not evaluated at rest` for any other field, and `the input of the bound field <field>
is not evaluated: <why>` where the graph that feeds a field of those that are evaluated holds a
node that is not evaluated (`<kind> is not evaluated`), an input that is not connected, a cycle,
a number that is not finite, an operation that does not exist, a value of the wrong kind, or more
than 128 nodes in a chain. A root widget that a binding takes out of the screen is listed as `not
drawn at rest: Instantiated is bound to game variables, which are zero at rest`, or `IsActive` for
one that is hidden, so that an empty picture says why; with `--var` it reads `not drawn:
Instantiated is bound to game variables, whose values leave it off`. Since a list that is not
empty makes the exit code 1 (below), many screens of the game, whose bindings are not all
evaluated, exit with 1 although they are drawn. `missing_textures` is present only when a texture
is missing: the Archive paths (`UI/Textures/….dds`) that widgets name and the Archive does not hold
or cannot read, each once, at most 256; stderr prints them as `texture not in the Archive: <path>`.
`missing_canvases` names nested canvases that cannot be
read, that no Record answers or that contain themselves; widgets whose parents form a cycle are
listed in `unsupported` (`not reachable from a root: its parents form a cycle`). Textures and SVG
files add `texture missing or unreadable` and `vector graphic missing or unreadable` (an example is
`<path>: <cause>`), `names no vector graphic; the game sets it at run time, the Record field
svgPath is empty`, `the box is drawn; the game sets the graphic at run time, the Record field
svgPath is empty`, `the texture is chosen by the game at run time, the Record field urlOptional
is empty` (the example names of widgets are printed as the Records hold them, `<unnamed>`
included; only an SVG document escapes them) and the features an SVG file uses that are left out (`paint servers are
drawn as their first colour`, `clip paths, masks and filters are not applied`, `image and text
elements are left out`, `style rules beyond the limit are left out`, ...). `truncated` is true
when a limit stopped the composition: 2^18 widgets placed (a list of siblings is refused as a whole
when it would not fit, counting every sibling in it; widgets that are hidden or not instantiated
take part in no layout and add nothing to `widgets` or to the limit, except as siblings in that
check), 2^26 steps of the style cascade, 2^17 shapes, 2^14 instances of templates, 2^10 rows of
one list, a depth of 64 widgets and canvases (ADR-0106), more than 32 clones whose modifiers are
in force at once, or a clone with more than 256 modifiers (ADR-0110). `skipped` lists what the renderer
(raster or SVG writer) skipped of the scene, each with the stable `reason` code of
`p4kx ui render` and a `message`; it is `[]` when nothing was skipped.

The exit code is 0 when nothing is listed in `unsupported`, `missing_canvases` or `skipped` and
`truncated` is false, and 1 otherwise. It is 2 for an unknown, ambiguous or non-canvas record, for a
canvas side below 1 or above 16384 pixels and for a size the renderer cannot draw; 3 when the
Archive or its `Data\Game2.dcb` cannot be read; 5 when a Record of the DataForge cannot be read. In
these cases no JSON is printed.

## `p4kx socpak list <archive> <container…> --json`

```json
{"archive": "C:\\…\\LIVE\\Data.p4k",
 "containers": ["Data\\ObjectContainers\\Ships\\AEGS\\ship_interior.socpak",
                "ship_interior\\modules\\cockpit.socpak"],
 "entries": [Entry, …]}
```

`containers` names the container Entry in the Archive, then each container inside the one before,
with the paths as they are stored. `entries` holds the Entries of the last container in directory
order; folder Entries are left out.

The top-level `fieldOrigins` and `provenance` identify each `containers` value's
actual Entry path in the owning directory. Earlier selected names contribute as
selectors. An encrypted outer container can therefore give an inner name an
encrypted origin while the outer name comes from its plain parent directory.
Entry rows retain their own field origins and source inventories.

## `p4kx socpak extract <archive> <container…> --json`

The report of `p4kx extract … --json`, for the Entries of the last container.

## `p4kx chunks list <archive> <entry> --json`

Also `p4kx chunks list <file> --json` for a chunk file on disk.

```json
{"file": "Data\\Objects\\Test\\cube.cgf", "container": "#ivo", "version": 2304,
 "chunks": […,
            {"index": 2, "type": "Mesh", "type_id": 2459154904, "known": true, "version": 2305,
             "id": null, "byte_order": "little", "offset": 112, "size": 40},
            {"index": 3, "type": null, "type_id": 170165686, "known": false, "version": 2304,
             "id": null, "byte_order": "little", "offset": 152, "size": 8}]}
```

The example leaves out chunks 0 and 1 (`…`).
`container` is `#ivo` (version `0x900`) or `CrCh` (version `0x746`). `chunks` lists the chunk
table in file order; `index` is the position `p4kx chunks dump --chunk` takes. `type` is the name
of a known type and `null` for an unknown one, which `known` marks as well; `type_id` is the
32-bit hash of `#ivo` or the 16-bit ID of `CrCh`. `version` is the chunk's layout version, `id`
the chunk ID of `CrCh` and `null` in `#ivo`, and `byte_order` `little` or `big`, which only `CrCh`
chunks can be. `#ivo` stores no sizes: `size` is then the distance to the next greater offset of
any chunk, the chunk table or the end of the file, alignment padding included, so chunks that
share an offset share their span; the table may list chunks in any order. A file that is no chunk
file, has another container version or is truncated prints no JSON; it fails with exit code 3.

The report also includes field-addressed origins in `fieldOrigins` and the referenced
`provenance` inventory. Coordinates for stored words identify half-open decoded-file ranges.
A table coordinate lists the row ordinal followed by the byte range. A derived `#ivo` size
also links the offset column, table boundary and decoded extent. An Archive directory path
has its own origin. A standalone file has unknown original Archive encryption.

## `p4kx chunks dump <archive> <entry> --chunk <index> --out <file> --json`

Also `p4kx chunks dump <file> --chunk <index> --out <file> --json` for a chunk file on disk.

```json
{"file": "Data\\Objects\\Test\\cube.cgf", "out": "…\\mesh.bin",
 "chunk": {"index": 2, "type": "Mesh", "type_id": 2459154904, "known": true, "version": 2305,
           "id": null, "byte_order": "little", "offset": 112, "size": 40}}
```

`chunk` is the chunk written, as in `p4kx chunks list`; its `size` bytes are written unchanged,
except with `--text`, which writes the chunk as UTF-8 text instead: a `CryXmlB` or `CryXmlB2`
chunk (`0x0004`, `0x0014`) as XML, a `Json` or `ModelPaths` chunk (`0x0011`, `0x0013`) as stored.
`size` stays the size of the chunk. An index the table does not have, an output that is the chunk
file itself, or `--text` on a chunk of another type fails with exit code 2; `--text` on data that
is no valid `CryXml` or UTF-8 text fails with exit code 3.

The dump publishes a sibling `<output>.p4kx-origin.jsonl` bound to the written
bytes. The JSON summary includes its basename in `originReceipt` and uses the
same table-word origins as the listing at the `/chunk` paths in `fieldOrigins`.
Unchanged bytes use one `opaquePayload` address. Converted CryXml uses
`xmlBytes` addresses for the emitted fields. The embedded source coordinates
start with the chunk's decoded byte offset, followed by the original CryXml
ordinals. The dump file and its companion cannot alias the input.

## `p4kx model info <archive> <entry> [--anim-from <entry>]… --json`

Also `p4kx model info <file> [--anim-from <file>]… --json` for a model file on disk with its geometry
companion next to it.

```json
{"model": "Data\\Objects\\Test\\cube.cgf", "geometry": "cube.cgfm",
 "material_library": "objects/test/cube", "material_ids": [1, 2],
 "library": {"path": "objects/test/cube", "status": "loaded", "problem": null, "single": false,
             "default_palette": null},
 "materials": [{"id": 1, "name": "caps", "flags": 0, "shader": "Illum", "surface_type": "",
                "string_gen_mask": "", "diffuse": [0.8, 0.1, 0.1], "specular": [0.04, 0.04, 0.04],
                "emissive": [0.0, 0.0, 0.0], "shininess": 10.0, "opacity": 1.0,
                "alpha_test": 0.0, "glow": 0.0,
                "textures": [{"map": "TexSlot1", "file": "Objects/Test/caps_diff.tif"}]}, …],
 "node_material_indices": [1, 2], "issues": [],
 "nodes": [{"index": 0, "name": "cube", "parent": null, "transform": [[1.0, 0.0, 0.0, 0.0], …],
            "scale": [1.0, 1.0, 1.0], "geometry_type": 2, "helper": true, "properties": []},
           {"index": 1, "name": "cube_mesh", "parent": 0, "transform": [[1.0, 0.0, 0.0, 10.0], …],
            "scale": [1.0, 1.0, 1.0], "geometry_type": 0, "helper": false,
            "properties": [{"key": "class", "value": "AnimatedJoint"}]}],
 "lods": [{"level": 0, "vertices": 24, "triangles": 12, "skeleton_bones": null,
           "meshes": [{"node": 1, "vertices": 24, "triangles": 12, "normals": true,
                       "normals_computed": false, "tangents": false, "colours": false,
                       "second_uvs": false, "non_finite_uvs": 0, "skinned": false,
                       "submeshes": [{"material_id": 1, "first_index": 0, "index_count": 12},
                                     {"material_id": 2, "first_index": 12, "index_count": 24}]}]}],
 "bones": [{"index": 1, "name": "tip", "parent": 0, "controller_id": 2415919105, "node": null,
            "relative": {"rotation": [0.5, 0.5, 0.5, 0.5], "translation": [0.0, 0.0, 2.0]},
            "world": {"rotation": [0.5, 0.5, 0.5, 0.5], "translation": [0.0, 0.0, 2.0]}}, …],
 "animations": [{"name": "wave", "path": "wave.caf", "source": "Data\\Animations\\Objects\\Test\\wave.caf",
                 "frames_per_second": 30, "end_frame": 30, "duration_seconds": 1.0,
                 "additive": false, "tracks": 1, "bound_tracks": 1, "keys": 2}, …],
 "unread_animations": [{"source": "Data\\Animations\\Objects\\Test\\odd.caf", "name": null,
                        "reason": "…"}],
 "chunks": [{"file": "model", "index": 0, "type": "ExportFlags", "type_id": 3193850174,
             "usage": "skipped"}, …,
            {"file": "geometry", "index": 0, "type": "Skin2", "type_id": 3094706039,
             "usage": "read"}]}
```

`geometry` is the file name of the geometry companion the meshes come from, `null` when they are
in the model file itself or the model has no geometry. `material_library` is the path below `Data`
without `.mtl` the material IDs refer to, or `null`. `library.status` is `loaded`, `missing` (no
such `.mtl` file), `unreadable` (the file or its path is invalid) or `unnamed`; `problem` says
what is wrong for the second and third; `single` is true when one material serves every ID (then
`id` of the material is `null`); `default_palette` is the library's default tint palette.
`materials` lists the materials of a loaded library in ID order, with the colours as the file
stores them (linear), `flags` the engine's `MtlFlags` and `textures` the slots with their paths as
the file gives them. `default_material` reports the separate archive fallback lookup when
a read model requires it. Its state distinguishes a resolved material from missing,
ambiguous, unreadable, malformed or unsupported sources. An observed source includes
its physical Entry and container coordinates and an encryption marker. The decoded
SHA-256 is present when the candidate's original bytes were verified. An unreadable
candidate is evidence of the attempted source.

For a loaded library, `materialFieldOrigins` maps JSON Pointers in this result to the immediate
encryption marker, source-set completeness and use references. Covered fields include the library
mode and palette name, material slots and their scalar and colour fields, texture map and file
tokens, public parameter names and values, and all raw layer attribute names and values.
Under /library/authored, origins also reference canonical XML names, declared attributes,
mixed content, child positions and original submaterial slots.
`materialProvenance` resolves those references to the selected material Entry and the original model chunk
that selected the library. XML coordinates count original element children and parsed attributes
from zero. The marker includes the selector's encryption. Layer targets and textures named by
these fields are references that the listing leaves untouched. With a standalone model, the
covered values have a null marker and incomplete source sets. The library status reports
a missing or unreadable library.

For model submeshes, `fieldOrigins` and `provenance` describe the original geometry
words independently of the material library. In the supported IVO 0x900 Skin
layout, an Archive-backed `material_id` or `index_count` that matches the decoded
row is bound to its original word in the selected geometry Entry. The rebuilt
`first_index`, loose inputs and submeshes without a proven word have a null marker
and incomplete source sets.

Each mesh's `triangles` identifies the original index-count words when they account for
every retained submesh range. The LOD's `triangles` retains those words across its meshes.
These origins remain incomplete because the fields also depend on geometry and submesh
selection. An unbound or empty mesh adds no observed count words.

A selected model checked on LIVE 13220821 and PTU 13260598 had six LODs and 651
submeshes on each Build. Every material ID and index count had a complete,
resolvable source-use reference in both outputs. Every rebuilt start index
retained an unknown origin. The CLI at `726a1ae2593ecdb5504b88c5deb83e346081b5ce`
checked the same model on both installed Builds. On each Build, the 106 mesh and
six LOD totals resolved to the exact 651 original count words through 1,302
conversion references. The comparison checked physical Entry identity and byte
ranges.

`nodes` keeps the order of the file;
`parent` is the index of the parent node or `null` for a root, and `transform` the 3×4 row-major
matrix from the node's space to its parent's, translation last, in CryEngine axes (Z up) and
metres. A model without a `NodeMeshCombo` chunk has one root node named after the file. `scale` is
the scale the node's record stores (1 on every axis when it stores none), `geometry_type` the
record's geometry type (0 for geometry, 2 or 3 for a helper; `null` without a `NodeMeshCombo`
chunk), `helper` whether it is a helper, `properties` the `key=value` lines of the node's entry in
the chunk's property table in order, e.g. `class=AnimatedJoint`, and `node_material_indices` the
chunk's list of material indices. A transform or scale whose values are all below 1e-5 is the
identity or 1. The `/nodes` origin records the examined chunk selectors and original count
where stored. Node field origins use the selected primary model Entry for original names,
parent operands, transform and scale words, geometry type and property occurrences. Defaulted
values identify their original selector evidence. A generated root name can use the model Entry
path. Loose nodes have incomplete Archive origins.

The `/bones` origin records the primary skeleton selection, including the inspected chunk
header, chunk-type selectors and original bone count where available. A bone row retains
decoded primary Entry byte ranges for its name, parent, controller id, node link and pose
values. Each pose vector and component has its own address. A missing optional node link
uses the original skeleton selection when the layout establishes its absence. Loose-file
bone rows and selections have unknown Archive origins.

`issues` lists in words what a read worked around. `model info` reads
strictly, so no fallback of a lenient read appears, but a `CrCh` mesh that names physics data in
further slots (which are not read) is listed as `node <n> names physics data in <k> further
slot(s), which are not read`. `lods`
lists the levels of detail, LOD0 first, each with its meshes, at most one per node; a level is
read from its own geometry companion, `<stem>_lod<n>.<extension>m`, and a level without one is
absent from the list. `normals`, `tangents` and `colours` say whether the mesh has them
(`colours` is false for a mesh of opaque white vertices); `normals_computed` says that the mesh
stores no normals and the smooth ones were computed from its triangles; `first_index` and `index_count` locate a
submesh in its mesh's triangle list. `second_uvs` says whether the mesh has a second set of texture
coordinates, and `non_finite_uvs` counts its vertices that had a texture coordinate, of either
set, that was not a finite number and reads as 0. `skeleton_bones` of a level is the number of
bones of the skeleton the level's own file carries when it differs from the model's, `null`
otherwise. `skinned` says whether the mesh has bone weights.
`bones` lists the skeleton of the model file, empty for a model without one: `parent` is the index
of the parent bone or `null` for a root; `controller_id` is the identifier animations address the
bone by; `node` is the index of the node of the model the bone drives (the bones of a static
model with animated parts) or `null`; `relative` is the bind pose relative to the parent and
`world` the bind pose in the model's space, each a rotation as a quaternion `x, y, z, w` and a
translation, in CryEngine axes (Z up) and metres. A level of detail of a skin whose own file has a
different skeleton is weighted to that one; `model info` lists the skeleton of the model file.
`animations` lists the animations found for the model: those of the `.caf` files and `.dba`
databases in the model's folder, in the folder at the same place below `Data\Animations`, and in
the Entries or files `--anim-from` names. `name` is the file name of a `.caf` without extension or
the name a database lists, `path` the name or path the file stores, `source` the Entry or file it was
read from, `duration_seconds` its last frame over the frame rate, `additive` whether its keys are
offsets to a pose, `tracks` the number of bones it has keys for, `bound_tracks` how many of them are
bones of the model's skeleton (matched by `controller_id`) and `keys` the rotation and translation
keys together. `unread_animations` lists the files, and the animations of a database, that could
not be read or that are additive (`reason` is "additive animation — needs a base animation";
they are written only when named), with the reason; names in both lists have control characters
replaced. A file `--anim-from` names that is no `.caf` or `.dba` exits with code 2, one that
cannot be read with 3. The animations of the model's folders are described from their headers
without decoding a key (ADR-0157): one whose key format the reader refuses is listed in
`animations` instead of `unread_animations`; the files `--anim-from` names are decoded in full,
so such an animation in them is unread, as before. The `reason` of an unreadable file starts
with the Entry path or file, then the cause, e.g. `Data\x\a.caf: the CafData chunk is corrupt`;
the cause no longer repeats the file name.
`chunks` lists every chunk of actually read model and geometry files. `file` is `model`,
`geometry`, `lod<n>-geometry` or `lod<n>-model`; the last role identifies a level-specific model
file read for its skeleton.
For each `type_id`, `fieldOrigins` and `provenance` identify the original little-endian
type word in the selected decoded Entry. The word is four bytes in `#ivo` and two in `CrCh`.
The resolved `type` references that word and the container signature, including when the
numeric ID has no name and `type` is null. The per-file `index` references the original
row-count and table-offset words used to enumerate the table. A standalone or unbound
input has an unknown marker and an incomplete source set for these fields. The MCP
`list_model_items` chunks section uses the same evidence with page-local field addresses.
`usage` is `read` when the model was read from it, `skipped` for a known type the model does not
read and `unknown` for an unknown type.
For `file`, the evidence identifies the original owner Entry lookup and the table header
that admitted the chunk. Companion rows also identify the primary Entry path used to derive
the sibling lookup. The contributor set stays incomplete because the observer records the
selected paths and table headers, while other path and file admission inputs are possible.
For `usage`, the evidence identifies the original kind and container selectors. Its contributor
set stays incomplete while the model reader retains only part of its decoding decisions.
Loose inputs and Entries without an original binding have an unknown origin for both fields.
Installed Archive checks on LIVE 13220821 and PTU 13260598 verified all 20 chunk rows of one
model per Build. An independent table read from seven checksum-verified Entries per Build
matched each four-byte type word, the header words for its contributing sources and the row's
file-local ordinal. Each source-use reference selected the expected physical Entry. Synthetic
tests additionally cover unknown type names, the two-byte `CrCh` layout and MCP page addresses.

A geometry companion that does not exist exits with code 2, a corrupt or unreadable model or
companion with 3. A `CrCh` model that is well-formed but has a layout the reader does not
implement (a mesh in data streams, a big-endian chunk, a `Mesh` or physics data chunk of another
version, a physics primitive other than a polyhedron) exits with code 1, not 3
(docs/crch-models.md). A `CrCh` model is read like an `#ivo` model, from its own file: it has no
geometry companion, one level of detail, computed normals, no texture coordinates and `geometry`
is `null` (docs/crch-models.md).

## `p4kx model glb <archive> <entry> --out <file> [--lod <n>] [--materials colours|textures] [--texture-mip <n>] [--no-vertex-colours] [--max-influences <n>] [--anim <name>]… [--anim-from <entry>]… --json`

`--max-influences N` (1 to 8, default 8) keeps the strongest N bones of each vertex and scales
their weights to add up to 1; 4 writes the first set of joints and weights only.

Also `p4kx model glb <file> --out <file> --json` for a model file on disk.

```json
{"model": "Data\\Objects\\Test\\cube.cgf", "out": "…\\cube.glb", "nodes": 2, "meshes": 1,
 "primitives": 2, "materials": 2, "textured_materials": 1, "textures": 3,
 "texture_pixels": 192,
 "texture_fallbacks": [{"material": "caps", "role": "diffuse",
                        "file": "Objects/Test/plate_diff.tif",
                        "reason": "the texture does not exist"}],
 "default_material_ids": 0, "hidden_primitives": 0,
 "bones": 0, "skinned_meshes": 0, "vertices": 24, "triangles": 12, "bytes": 2412,
 "library": {"path": "objects/test/cube", "status": "loaded", "problem": null, "single": false,
             "default_palette": null},
 "animations": [{"name": "wave", "duration_seconds": 1.0, "additive": false, "channels": 2,
                 "keys": 4, "skipped_tracks": [3735928559]}],
 "unread_animations": [],
 "unknown_chunks": [{"file": "model", "index": 4, "type": null, "type_id": 170165686,
                     "usage": "unknown"}]}
```

The counts are those of the GLB written: one glTF node per node, one mesh per node with geometry,
one primitive per submesh and one material per sub-material the drawn submeshes use (plus one
default material when any ID falls back to it), of the level of detail `--lod` names
(LOD0 by default), in glTF axes (Y up) and metres; the counts do not say which of `NORMAL`,
`TANGENT`, `TEXCOORD_0` and `COLOR_0` are written (`COLOR_0` only on the primitives whose
material takes them as its colour, ADR-0318);
`bytes` is the length of the file. `default_material_ids` counts the material IDs drawn with the
default material because the `.mtl` file is missing, unreadable or has no sub-material for them,
and `hidden_primitives` the submeshes left out because their material draws nothing; `bones` the
bones written as nodes (those of a skinned model and those of a skeleton file such as a `.chr`, which
has no mesh; a skin gets a glTF skin with a joint per bone, and its `skinned_meshes` carry
`JOINTS_n` and `WEIGHTS_n`, ADR-0039); `library` is
the status of the `.mtl` file as in `p4kx model info`. `textures` counts the PNG images embedded
with `--materials textures` (an image several materials use counts once), `texture_pixels` their
pixels and `textured_materials` the materials that use at least one; all are 0 with
`--materials colours`. `texture_fallbacks` lists, per material, each texture file that is not used
because it is missing, cannot be decoded, has an unsafe path or would exceed the budget of 256
images or 2^29 pixels of the file: `role` is `diffuse`, `normal` or `specular`, `file` the name
the `.mtl` file gives and `reason` the cause; the material keeps its plain colour for it. The reason is
`the images of this export are as many as allowed (256)` when the image cap is spent (a higher mip does not help),
and `the textures of this layered image would decode more than 2^27 pixels together` for a layered image. For the
blended image of a layered material `file` names the texture that sizes it, the blend map (`_blend`) or else the
first layer's map, followed by `(layered base colour image of N layers)` or `(layered normal image of N layers)`;
the role is `diffuse` for the base colour image and its blend map and `normal` for the normal image. A layered
material whose layered normal image is refused is drawn with its own normal map, if it has one. The
names come from the Archive and are untrusted. `unknown_chunks` lists the chunks of unknown type
that the export skipped, as in `p4kx model info`. `animations` lists the animations asked for with `--anim`, in order (empty without it): `channels` is the number of glTF channels written (a rotation or a
translation of a bone each; an animation with none is not written), `keys` the keys in them and
`skipped_tracks` the controller IDs of the tracks left out because no bone of the skeleton has
them, ADR-0046; `unread_animations` is as in `p4kx model info`: `--anim all` leaves the additive
animations out and lists them there, and `additive` is `true` only for one that was named. An output that is one of the
model's files, a `--lod` the model lacks, a name `--anim` finds no animation for or an
`--anim-from` Entry that does not exist or is no `.caf` or `.dba` exits with code 2, an animation
file that is corrupt or not supported, an `--anim-from` one included, with 3.

The `originReceipt` field names the digest-bound sibling origin companion. Both
output modes publish the companion. The `fieldOrigins` and `provenance`
members record the original model name, material library fields, and each
`unknown_chunks` row. The material library evidence describes the loaded
source. The `materials` count describes the written GLB. See
[standalone model GLB origins](source-origin.md#standalone-model-glb).

## `p4kx extract … --json`

```json
{"target": "…", "extracted": 1, "skipped": ["Data\\readme.txt"],
 "failed": [{"path": "Data\\broken.xml", "error": "cannot read the Entry data: …"}],
 "cancelled": ["Data\\large.bin"], "archive_changed": false}
```

`p4kx convert … --json` prints the same report and, when it is not 0, `"not_converted": 2`, the
number of files left out because they have nothing to convert (without `--copy-other`), and
`"dataforge_left_out": ["Data\\Game.dcb"]`, the other DataForge files it found; `skipped`,
`failed` and `cancelled` name paths below the input folder.

With `--convert` the report also holds `"converted": 1`, the number of Entries written converted
(CryXml Entries as XML, the Entries of converted textures, sounds, soundbanks with at least one
converted WEM and the DataForge Entry as Records); they count among `extracted`. It also holds `"unconverted": [{"path":
"Data\\Textures\\hull_ddna.dds", "error": "cannot convert the texture: …"}]`, the textures by
their header Entry that could not be converted, whose raw Entries were written instead (except a
header whose path the merged DDS already took), and the sounds that could not be converted, e.g.
`{"path": "Data\\Sounds\\wwise\\Media\\700.wem", "error": "cannot convert the sound: … converting
XMA2 to WAV is not supported"}`, written unchanged; a soundbank is listed once for each embedded
WEM that could not be converted, the error naming `<bank> WEM <ID>`. The exit code is then 1. Without `--convert` both
fields are absent. When `--convert` converted the DataForge Entry, the report also holds
`records` with the fields `exported`, `skipped`, `failed`, `cancelled`, `error_values` and
`records_with_errors` of [`p4kx dcb export`](#p4kx-dcb-export-archive---json), and, only when a
Record's file does not follow its name or record path, `renamed` as there
(`{"guid", "path", "reason"}`); `skipped` lists
the Record files relative to the folder of the DataForge Entry. Otherwise that field is absent.
`texture_artifacts` contains the images published by this run when texture image conversion
is enabled. Each texture groups its `images` with a shared `provenance` inventory. The group contains its declared `source_transfer`, encoded `output_transfer` and
any `png_transform`, with their precise `field_origins`. Every image
has a target-relative `path` with forward slashes, the measured `digest` of the written file,
the selected `level`, `layer`, `face` and `slice`, and its own `containsEncryptedSource`,
`sourceUsesRefs` and `sourceSetComplete`. Existing images retained by the conflict policy do
not gain a receipt from this run. The field is omitted when no image was written.

`merged_texture_artifacts` lists newly written merged DDS files with a portable
origin companion. Each has a target-relative `path`, `origin_receipt` and the actual
artifact's `digest`. An emitted alpha DDS has its own receipt. The summary omits this
field when the run published no merged DDS companion.

`sound_artifacts` lists newly converted WAV and Ogg files from loose WEMs and
soundbank members whose companions are present at the end of the run. Entries
carry the target-relative `path` and `origin_receipt` and the measured
`digest` of the written audio file. The report omits this field when no such
companion is present.

An Entry whose path has a part that is no valid file name on Linux and Windows, a path that is too
long for Windows or that differs only in case from an earlier written Entry is listed in `failed`
with the part and the rule it breaks
([ADR-0248](adr/0248-written-file-names-are-valid-on-linux-and-windows-on-every-platform.md)).
`skipped` lists the Entries whose existing file in the target was kept under `--conflict`;
`failed` lists the Entries that were not written, with the reason and its causes joined by `: `;
`cancelled` lists the Entries not written because the run was cancelled and, once a cancelled run
had written a soundbank itself, each of its WEMs not unpacked as `<bank> WEM <ID>`, after the
Entries. The Entries in all three are in Archive order. With `--verify` the report also holds `"mismatched": [{"path":
"Data\\x.xml", "kind": "crc32c", "expected": "12345678", "actual": "b492e797"}]`, the Entries
whose data did not match the checksum of the directory, as in [`p4kx verify`](#p4kx-verify-archive---filter-filter---json);
they are not repeated in `failed`, and none of them is written. Without `--verify` the field is
absent. With `--json` failures are not repeated on stderr. `archive_changed` is `true`
when the Archive changed on disk after it was opened: the Extraction then stops, the Entries not
yet written are listed in `cancelled`, and the exit code is 4. Otherwise the exit code is 1 when
`failed`, `mismatched` or `cancelled` is not empty.

## `p4kx verify <archive> [--filter <filter>] --json`

```json
{"archive": "C:\\…\\LIVE\\Data.p4k", "verified": 1362045,
 "mismatched": [{"path": "Data\\Libs\\x.xml", "kind": "crc32c",
                 "expected": "12345678", "actual": "b492e797"}],
 "failed": [{"path": "Data\\broken.xml", "error": "cannot read the Entry data"}],
 "cancelled": [], "archive_changed": false}
```

`verified` counts the Entries whose data matched the checksum of the directory. `mismatched`
lists the Entries whose data did not, with the kind of the checksum (`crc32c` in an Archive,
`crc32` in a container, `sha256` for the SHA-256 of the stored bytes) and the stored (`expected`)
and computed (`actual`) value as lower-case hex digits, eight for a CRC and 64 for a SHA-256;
`failed` lists the Entries that could not be read or decoded, with the reason and its causes
joined by `: `; `cancelled` lists the Entries not checked because the run was cancelled. The
three lists are in Archive order. `archive_changed` is `true` when the Archive changed on disk: the
run then stops and the exit code is 4. Otherwise the exit code is 1 when `mismatched`, `failed` or
`cancelled` is not empty. `p4kx socpak verify <archive> <container…> --json` prints the same
document for the Entries of the last container; its `archive` is the Archive path followed by the
Entry paths of the containers.

`fieldOrigins` maps JSON Pointers to the actual contributing Entries and their immediate
encryption markers. The `provenance` inventory identifies the physical Entry independently of its
path, including when several Entries share the same spelling. An expected checksum cites
its directory declaration, and a computed checksum cites the half-open logical byte range in
`entryDecodedBytes` for CRC or `entryStoredBytes` for SHA-256. A successful count cites the
completed checks it counts, including a zero-length decoded range when applicable. Read failures
cite the attempted Entry without claiming a completed byte range. Cancelled Entries cite their
stored paths. An observed Archive change leaves the completed byte proof incomplete.

## `p4kx compare <old> <new> --json`

The Comparison report, the same document `--report <file>` writes (there pretty-printed). Its
JSON Schema (draft 2020-12) is generated from the report types and checked in as
[`schemas/comparison-report.schema.json`](../schemas/comparison-report.schema.json); a test fails
when the two drift apart, and `P4KX_BLESS=1 cargo test -p p4kx-comparison --test report_schema`
regenerates it. The output is deterministic: Entries are ordered by path and Records by record
path, both ignoring case and separator kind, Records with the same record path by GUID. The exit
code is 1 for differences or incomparable Entry pairs and 0 when neither occurs.

| Field | Type | Meaning |
|-------|------|---------|
| `format_version` | number | Version of the report shape, now `1` |
| `source.old`, `source.new` | object | `archive` (path), `channel` (string or `null`) and `build` (a [Build](#build) or `null`) of each side |
| `levels` | array | The levels compared, e.g. `["entries", "records"]` |
| `filter` | string or `null` | The Filter that selected the Entries by path and the Records by the record path of either side |
| `entries.counts` | object | `added`, `removed` and `changed` Entry counts |
| `entries.directories` | array | The same counts per top-level directory (`directory`, empty for the root), ordered ignoring case |
| `entries.fieldOrigins`, `entries.provenance` | objects | Immediate origins for the emitted Entry path, change decision, pairing ambiguity and each physical old or new side's size and checksum, with a shared source and use inventory. Paths are JSON Pointers from the report root. Historical v1 reports may omit these members. |
| `entries.changes` | array | One object per Entry: `path` (as the new side stores it, the old side for a removed Entry), `change` (`added`, `removed` or `changed`), and `old` and `new` with `size` and the checksum for the sides that have it: `crc32c` for an Archive Entry, `crc32` for an Entry of a plain ZIP archive (a SHA-256 would be `sha256` as 64 hex digits; no Entry states one) |
| `entries.incomparable` | array | Optional matched equal-size pairs whose declared checksum algorithms differ. Each row has `path`, required `old` and `new` directory operands, `reason: checksum_algorithms_differ` and, when applicable, `pairing_ambiguous: true`. |
| `records.fieldOrigins`, `records.provenance` | objects | Immediate origins for the reported Record GUID and present old/new header members. Paths are JSON Pointers from the report root. Historical v1 reports may omit these members. |
| `records.counts` | object | `added`, `removed`, `moved`, `changed` and `unreadable` Record counts |
| `records.changes` | array | One object per Record: `guid`, `change` (`added`, `removed`, `moved`, `changed` or `unreadable`), `old` and `new` with `record_path`, `name`, `tag` (version 8 only, absent without one) and `type` for the sides that have it and can be read, `fields`, and `errors` for an unreadable Record |
| `records.changes[].fields` | array | One object per changed field or array element, nested ones included: `path` (its location, e.g. `hull.plates[1].armor`), and `old` and `new` values for the sides that have it; empty for an added, removed or unreadable Record |
| `records.changes[].errors` | array | One object per side whose Record cannot be read: `side` (`old` or `new`) and `reason`; absent for every other Record |

`entries` is absent when the Entry level was not compared, `records` when the Record level was
not compared.
An Entry count retains the change-decision sources of the emitted rows counted in it.
A directory count also retains the selected path of each row and the first selected path
that identifies that group. The `directory` label comes from that first path, including
the empty label for root files. Zero counts remain unknown without a retained absence survey.
A nonzero count inherits any incomplete one-sided decision evidence. A filter scopes these
observations to the selected rows. The change counts and directory totals cover the emitted
`entries.changes` rows. An incomparable pair retains its two declared operands, the Archive
layout facts that determine their algorithms and the join proof in `fieldOrigins`.

When several physical Entries have the same normalised path, comparison first removes unchanged
occurrences with equal path spelling, size and declared checksum. It then pairs unique
equal spellings, removes unchanged equivalent aliases and pairs a single remaining occurrence
on each side. Remaining ambiguous occurrences are reported individually with
`pairing_ambiguous: true`. This marker also applies when original directory facts cannot
distinguish a surplus duplicate from an unchanged occurrence. Counts refer to the emitted
changes. Each collision decision retains the physical positions, path spellings, sizes and
declared checksums that contributed to the match.

The Entry field markers describe the directory metadata used by the report. The source inventory
records the Entry's payload encryption and its container ancestry separately. A missing comparison
side has no stored fields to attach an origin to. Each `change` carries the direct join evidence.
A matched pair retains both path selectors and both compared sizes. The checksum is also retained
when equal sizes required the checksum comparison. A one-sided row retains its original observed
entry and its compared opposing path when available. A source used only as an opposing selector
retains its physical Entry identity and encryption proof with compact source details. The
one-sided contributor set is incomplete because
the earlier join decisions do not form part of that receipt.

The Record header origins describe the exact DataForge bytes read from each Archive.
The GUID comes from the selected new Record header, or from the old header when the new side
has no Record. Each present side's path, name, tag and type have its own origin. Type retains
both the Record's struct selector and the selected definition's name. A library comparison
supplied only opened DataForges has unknown header encryption. An absent side or tag has no
field to annotate.

Changed fields use the original paired field positions, even when declarations share a name.
The `path` retains the declarations used to spell it and the array headers used to select
its indices. Each present side's `type` and `value` retain their own original storage
and ancestor selectors. Missing sides and null values have no `value` address to annotate.
Unbound input or absent positional evidence stays unknown. Unreadable value diagnostics may
retain partial storage evidence. A stored Reference GUID retains the caller GUID bytes.
Its target path and type retain the same caller bytes as selector and the selected Record
header or type definition. A weak pointer retains its original struct and instance words as
selector. Its reported GUID and record path come from the selected Record header, and an
owned target's `at` and type use the actual admitted field and array positions.

Records are joined by GUID. A Record is `moved` when its record path differs, ignoring case and
separator kind; its changed fields are listed as well. It is `changed` when its name, its type
or a field differs. It is `unreadable` when its record path, name, type or fields cannot be read
on a side; it is then not compared, and the Comparison goes on with the other Records.

The fields are compared down the whole Record tree. `path` is the location of the value in the
same notation as `at` in [Record XML](record-xml.md#child-elements): the field names from the
Record down separated by `.`, with `[index]` after an array field. The changes are listed depth
first: within one struct in field order of the new side and then the fields only the old side
has, each followed by the changes inside it. Fields are joined by name, so a field only one side
has is listed with only `old` or only `new`. Array elements are joined by index, so elements
added or removed at the end are listed with only `new` or only `old`; arrays are not matched by
key. A struct of another type, or a value of another kind (e.g. a scalar against a struct, or
null against a struct), is a change of its own, and the fields and elements of a struct or array
that only one side has follow it, each with only `old` or only `new`.

Each value is an object with `type` and `value`:

| `type` | `value` |
|--------|---------|
| `bool`, `int8` … `int64`, `uint8` … `uint64` | The number or boolean |
| `single`, `double` | The number; `NaN`, `INF` or `-INF` as a string for a float that is not finite |
| `string`, `locale` | The text; a localisation key as the key, e.g. `@item_Name` |
| `guid` | The GUID |
| `enum` | The option name |
| `reference` | The target: `guid`, `record_path` and `type`, and for a weak pointer `at`, its location inside the Record |
| `struct` | The name of its type; its fields follow as changes of their own |
| `array` | The number of elements; its elements follow as changes of their own |
| `null` | None: a null pointer or a null Reference has no `value` |
| `error` | The reason the value cannot be read; the rest of the Record is still compared |

Floats compare by their bits, every `NaN` equal to every other, so `-0.0` differs from `0.0`.
References compare by target GUID, so a target that moved is no change, and weak pointers, which
all share the GUID of their own Record, by their `at` as well. Values that cannot be read compare
by their reason.

## `p4kx compare <old> <new> --schema --json`

The schema drift between the DataForges of both Archives. Its JSON Schema (draft 2020-12) is
generated from the report types and checked in as
[`schemas/schema-drift-report.schema.json`](../schemas/schema-drift-report.schema.json); a test fails
when the two drift apart, and `P4KX_BLESS=1 cargo test -p p4kx-comparison --test
schema_report_schema` regenerates it. Structs and enums are joined by name and listed ordered by
name; the exit code is 0 without and 1 with differences, 5 when an Archive has no DataForge or it
cannot be read. A different number of definitions left out on each side (`unreadable`) is a
difference as well.

| Field | Type | Meaning |
|-------|------|---------|
| `format_version` | number | Version of the report shape, now `1` |
| `source.old`, `source.new` | object | `archive` (path), `channel` (string or `null`) and `build` (a [Build](#build) or `null`) of each side |
| `counts` | object | `structs`, `properties`, `enums` and `options`, each with `added`, `removed` and `changed` counts; `properties` counts the property changes in added, removed and changed structs and `options` the option changes of the changed enums |
| `structs` | array | One object per added, removed or changed struct: `name`, `change` (`added`, `removed` or `changed`), `records`, `parent` and `properties` |
| `structs[].records` | object | `old` and `new`: the Records on each side whose struct is this struct or inherits from it, `0` on a side that lacks the struct; instances that are no Record, such as the targets of strong pointers, are not counted |
| `structs[].mapped_instances` | object (optional) | `old` and `new`: DataForge mappings of this exact struct, including nested instances, with `0` for an absent struct. Earlier version 1 reports may omit these counts. |
| `structs[].parent` | object | `old` and `new` parent struct name (`null` for none); present only when a changed struct inherits from another struct than before |
| `structs[].properties` | array | The properties the struct declares itself that differ, ordered by name. An added or removed struct lists all of its own declarations: `name`, `change`, `aspects`, and `old` and `new` for the sides that have it |
| `structs[].properties[].aspects` | array | What differs in a changed property, in this order: `data_type`, `conversion`, `target`; empty for an added or removed property |
| `structs[].properties[].old`, `.new` | object | `data_type` (e.g. `Reference`, `String`, `Class`; `Unknown(0x…)` for an unknown code), `conversion` (`0` for one value, any other code for an array) and `target` (the struct or enum name, or `null`) |
| `enums` | array | One object per added, removed or changed enum: `name`, `change`, `records` and `options` |
| `enums[].records` | object | `old` and `new`: the Records on each side whose struct declares or inherits a property of the enum, each Record counted once |
| `enums[].options` | array | The options added or removed from a changed enum, ordered by name: `name` and `change` (`added` or `removed`) |
| `unreadable` | object | `old` and `new`: the structs, enums, properties, options, parents and Records left out on each side because a name or struct cannot be read, or because a struct, enum or property name is defined twice (the first definition wins) |

A property is joined by name within its struct. Its data type, conversion type and target
determine whether it differs. A struct also differs with a different parent. Records are counted
by their declared struct and rolled up its parent chain until the first repeated struct.

Archive-backed reports include `fieldOrigins` and `provenance` in version 1. The field map uses
JSON Pointers. Declaration values, such as `/structs/0/properties/0/new/target`, resolve to the
corresponding physical DataForge definitions on their own side. A change decision uses both
sides. An absent definition uses the inspected name range on that side. Mapping counts resolve
to the physical mapping count or the mapping-selector range used to establish absence. Record
and report counts carry the inspected selector ranges. Historical unbound reports can omit
these two members.

## `p4kx compare <old> <new> --diff <entry> --json`

The line diff of one Entry, with three unchanged lines of context around each change:

```json
{"path": "Data\\Libs\\text.xml", "change": "changed", "differs": true, "content": "text",
 "old": {"size": 64, "crc32c": 3653534010, "encoding": "text"},
 "new": {"size": 83, "crc32c": 1515590036, "encoding": "text"},
 "hunks": [{"old_start": 1, "old_lines": 4, "new_start": 1, "new_lines": 5, "lines": [
   {"kind": "context", "text": "<Items>\n"},
   {"kind": "removed", "text": "  <Item name=\"a\" mass=\"1\"/>\n"},
   {"kind": "added", "text": "  <Item name=\"a\" mass=\"2\"/>\n"}]}]}
```

| Field | Type | Meaning |
|-------|------|---------|
| `path` | string | The Entry path as the new side stores it, the old side for a removed Entry |
| `change` | string | `added`, `removed`, `changed` or `unchanged`, by size and checksum from the central directories |
| `differs` | boolean | Whether the Entry differs: added, removed, changed in size or checksum, or with differing lines; it decides the exit code |
| `content` | string | `text`; `binary` when a side holds a NUL byte, or starts with a UTF-16 byte order mark but does not decode as UTF-16 without NUL characters; `too_large` when a side, or the XML converted from it, is larger than 128 MiB or has more than 4 194 304 lines |
| `old`, `new` | object or `null` | `size`, the checksum (`crc32c` for an Archive Entry, `crc32` for an Entry of a plain ZIP archive (a SHA-256 would be `sha256` as 64 hex digits; no Entry states one)) and `encoding` of each side, `null` for a missing side; `encoding` is `text` (compared byte for byte), `utf-16le` or `utf-16be` (decoded after the byte order mark), the CryXml signature (`CryXmlB`, `CryXml`, `CRY3SDK`) the side was converted from, or `null` when it is not text |
| `hunks` | array | Empty unless `content` is `text`. Each hunk has `old_start` and `new_start` (one-based, as in a unified diff header, the line before for an empty side), `old_lines` and `new_lines` (counts) and `lines`, each with `kind` (`context`, `removed` or `added`) and `text` (the line with its terminator, for display with invalid UTF-8 replaced by U+FFFD; the last line of a side may have none) |

A missing side is diffed as empty text. Lines are compared by their bytes, so a change in bytes
that are not valid UTF-8 still shows as a hunk even where both `text` values look the same. The diff is a patience diff with a Myers fallback
where no line is unique; so that pathological pairs stay fast, a region that needs more than 1024
edits there, or every region once a fixed work budget is spent, is shown as removed and added as a
whole. An Entry whose bytes changed but whose lines (for CryXml: the converted XML) did not has
`change` `changed`, `differs` true and no hunks; without `--json` a note says so. The exit code
is 0 when `differs` is false, 1 when it is true, 2 when neither Archive has the Entry and 3 when an Entry cannot be read, does not
hold the size its central directory states, or holds corrupt CryXml.

The JSON adds `fieldOrigins` and `provenance`. Each line's `kind` uses the two-sided alignment.
Its `text` points to the displayed input, with exact original byte ranges for raw text and
UTF-16. UTF-16 also records the byte order mark. Converted CryXml lines cite half-open ranges
in the original decoded CryXml bytes, with named selectors for offset words and string
boundaries and conversion ranges for the bytes used in the rendered line. Multi-line node text
has a separate range for each emitted section. Indentation cites the original parent words
used for the displayed depth. The generated XML declaration has no payload range. A leaf's
structural choice cites the `cryXmlChildAbsence` selector. Its `field` gives `littleEndian` or
`bigEndian`. Its `indices` begin with the node index and the half-open range of the original
size and node-directory words. When later nodes exist, the indices then give the first
parent-word offset, word count, byte stride and word width. Where a structural decision has
incomplete positional evidence, the line carries its known ranges and an explicit opaque
use of the Entry. A directory size rejection uses only
the decisive directory evidence. The comparison retains line proofs for emitted hunks.

## `p4kx cryxml <file> --out <file> --json`

```json
{"input": "…", "output": "…", "format": "CryXmlB", "byte_order": "little"}
```

`format` is the signature, `CryXmlB`, `CryXml` or `CRY3SDK`, or `text` for text XML written
unchanged; `byte_order` is `little`, `big`, or `null` for text XML. `--json` needs `--out`,
because the XML would otherwise share stdout with the JSON; without it the exit code is 2.

The summary also includes `originReceipt` with the sibling companion's filename,
`fieldOrigins` keyed by JSON Pointer, and `provenance` with the physical sources and
uses. A standalone file has unknown original P4K encryption. The chosen output and
companion names are independent of the input bytes. The companion addresses each emitted
binary CryXml datum and binds it to the final artifact by relative name, byte length and
SHA-256. See [Standalone field origins](source-origin.md).

## `p4kx chf glb <archive> <file> --out <file> --json`

```json
{"archive": "…", "chf": "…", "body": "male", "rig": "protos_human_male_face_t1_pu",
 "skeleton": "Objects/…/bhm_skeleton_v7.chr", "out": "…", "lod": 0,
 "parts": [{"name": "body", "origin": "body", "file": "…", "morph_targets": 0},
           {"name": "head", "origin": "head", "file": "…", "morph_targets": 399},
           {"name": "teeth", "origin": "head", "file": "…", "morph_targets": 0},
           {"name": "eyes_itemport", "origin": "head", "file": "…", "morph_targets": 0},
           {"name": "hair_itemport", "origin": "head", "file": "…", "morph_targets": 246}],
 "nodes": 412, "bones": 403, "meshes": 9, "skinned_meshes": 9, "primitives": 41,
 "materials": 14, "morph_targets": 645, "textured_materials": 12, "textures": 20,
 "texture_pixels": 36700160,
 "texture_fallbacks": [{"material": "…", "role": "diffuse", "file": "…", "reason": "…"}],
 "default_material_ids": 0, "vertices": 61582, "triangles": 66225, "bytes": 37236432,
 "skipped": 0,
 "animations": [{"name": "facerom_t0_face", "duration_seconds": 28.0, "additive": false,
                 "channels": 353, "keys": 292000, "skipped_tracks": [123456]}],
 "issues": [Issue, …]}
```

`body` and `rig` are the body variant and the head rig that the DNA's `gender_hash` names; `parts`
lists the skin of the body (`origin` `body`), the head, its teeth and every item of the
character's head port that has geometry for the body variant (`origin` `head`): the head is named
`head`, an item after its port, e.g. `eyes_itemport`, `hair_itemport`, `eyebrow_itemport`,
`eyelashes_itemport`, `beard_itemport`, `universal_scalp_itemport`, and a port this tool does not
know `port_<hash>`. The eyes and the teeth are shaped by the DNA like the head; the other items
follow the shaped head (ADR-0090). The materials the character chose are applied (its head
and body material, skin tone, iris colour and hair melanin and dye), and `--materials textures`, the
default, embeds the textures of the materials as PNG images at the mip level `--texture-mip N`
(default 1); `--materials colours` writes plain colours. `textured_materials`, `textures`,
`texture_pixels` and `texture_fallbacks` are those of `p4kx entity glb`: a texture that cannot be
read leaves the plain colour and is listed. The Issues are those of `p4kx armour glb`, and a head
that cannot be shaped adds `shape_failed` (the DNA rig beside the head skin is missing, belongs to
another rig, does not cover the vertices of the model, has a different vertex count than the
skin, gives moves that are not finite, or the DNA names a head the rig lacks); the head is then
left out, `skipped` counts it, the GLB is still written and the command exits with code 1 after
printing the report. Items that cannot be worn leave the item out and count in `skipped`, the exit
code staying 0: `piece_not_found` (an item whose Entity the Archive does not have),
`geometry_missing` (its skin is not in the Archive) and `no_variant_geometry` (its Record has no
geometry for the body variant). Three Issues leave the piece in and are notes only:
`material_choice_unknown` (the character's base material is in no lookup table, so the piece keeps
its own), `look_ignored` (the texture choices of a material, such as blemishes, makeup and
tattoos, are not drawn) and `body_type_mismatch` (the body type of the character is not the one of
the head rig its DNA names, on the piece `head`; ADR-0197). The normals and tangents of a shaped
head are turned with its new surface (ADR-0197).

The piece named `head` has `morph_targets`: the number of expressions written. They are the
expressions of the head rig that move a vertex of the character's face, all of them by default,
only the ones named by `--expression NAME` (repeatable) otherwise, none with `--no-expressions`.
At `--lod 1` and up, whose rigs have none, the targets of level 0 are carried to the vertices of
the level (ADR-0293). The hair, brows, lashes and beard
have a target for each expression that moves one of their vertices by at least a fifth of the
target's largest move, named like the head's and moved as they follow the neutral shape
(ADR-0200), at every `--lod` (ADR-0305); the teeth and eyes have none. Only the first mesh of such
a piece gets targets: a piece with several meshes adds the note `follow_morphs_first_mesh_only`, and
`follow_weights_skipped` says that a facial animation sets no weights for a piece whose targets do
not match the head's. These pieces are also bound to the facial joints of the head
(ADR-0299); `follow_joints_skipped` says that one is not, because the head mesh has no weights or
the bones would not fit 16-bit joint indices, and it keeps its own bones.
A facial animation's budget counts the hair weights too. A name the rig lacks adds
`unknown_expression`, a named one that moves no vertex `empty_expression`, and expressions that
cannot be made (the rig of the level has no expression morphs, the names of the skin do not fit
the rig) add `morphs_failed`; the head is kept, the GLB is written and the command exits with code
1 after printing the report. `--lod N` picks the level of detail and `--no-vertex-colours` leaves
the vertex colours out.

The 183 bones of the head skin that the body skeleton lacks are written as joints below their
nearest body ancestor at the character's own joint positions (ADR-0098), so `bones` is 403 for
the head at LOD 0 and the head binds to them. `--anim NAME` (repeatable, `all` for every one;
`--anim-from ENTRY` adds `.caf` or `.dba` Entries) looks for animations below the folders of the
character's body and head. A track that names a bone of the skeleton moves it by its keys; a
track that names a control of the head rig is evaluated through the rig logic into the poses of
the facial joints and the weights of the head's morph targets, sampled at the frames where a
control has a key. `animations` lists each animation written with `channels`, `keys` (samples
times channels for a facial animation), `duration_seconds` and, in `skipped_tracks`, controller
IDs of two kinds, both the CRC-32 of a name and not told apart: those of the animation's tracks
that fit neither a bone nor a control, and those of the joints of the face rig that move in the
animation but have no node in the skeleton. An animation that fits nothing is not
written and adds the Issue `animation_not_used`; so does an animation that would exceed the budget
of 33,554,432 values (floats of key times, poses and weights) that all animations of one command
share: it adds `animation_not_used` with a reason starting with `limit`, the clips before it in
the order of the files are kept (a facial clip of 4,500 samples costs about 7 million values).
A clip is reported at most once. Animations left out for the same reason are one
`animation_not_used` Issue, named `animations` when it sums up several, whose message gives their
count and the names of the first three, so a command with thousands of unused animations adds one
line to the report. `skipped` counts only the Issues of the Armour, not those of the animations.
Control values are held to 0 to 1. A facial animation whose rig cannot be read, or whose pose is
not finite, adds `face_animation_failed` when it has no track for a bone; the rig is read once.
Facial joints that cannot be placed by the DNA add `face_joints_failed`: the bones are then
appended to the skeleton without offsets, at the skin's own positions, and the head binds to them
all the same. An unknown NAME exits with code 2. `p4kx armour glb` takes the same options for the body and has the same
`animations` field. A CHF that holds no face DNA or whose DNA names an unknown head rig exits
with code 2, a file that is no CHF with code 3, a missing skeleton with code 3, an Archive key that
is missing or wrong with code 7, and an `--out` that is the Archive with code 2.

## `p4kx chf diff <a.chf> <b.chf> --json`

`--json` and `--format json` print the same one line:

```json
{"a": "…/a.chf", "b": "…/b.chf", "identical": false,
 "changes": [{"path": "dna.face_parts.neck.blends[2].value",
              "raw_path": "dna.face_parts.neck.blends[2].value", "old": "12002", "new": "12102"},
             {"path": "item_ports.head_itemport", "raw_path": "item_ports.0x4705ee39",
              "old": "item 1f0e…", "new": null}]}
```

`changes` is empty and `identical` true when the two files hold the same Character, in the same
order of every list, with the same header flags; the compressed frame and the padding of the
container are not compared. `identical` false is the exit code 1, true the exit code 0. Each change is
one field, in the order of the CHF JSON: `container.flags`, `versions`, `body_type`, `voice`,
`dna` (`gender_hash`, `variant_hash`, `blends_per_part`, `header_unknown`, `max_head_id`, then
`face_parts.<part>.blends[<n>].value` and `.head_id`), `item_port_tail`, `item_ports` (the root's
`port`, `item` and `tail_count`, then the children by port) and `materials.<attachment>`
(`base_material`, `flags`, `sub_materials.<name>` with `textures.<slot>.texture`,
`floats.<name>.value` and `colors.<name>.rgba`). `path` spells out every hash that a known name
matches (`--archive` adds names, as for `to-json`); `raw_path` keeps the hashes as `0x` and eight
hexadecimal digits and does not depend on the names known. A second item with the same hash in
one list gets `#1`, `#2` and so on behind its hash (for a texture behind its slot); equal items
are paired first, the rest in order, so `[A(x), A(y)]` against `[A(y)]` is one removal of `A(x)`.
`old` and `new` are text: numbers in decimal,
hashes and flags in hexadecimal, GUIDs as DataForge prints them, a colour as `#rrggbbaa`, a float
as written by Rust (a non-finite one with its bits). A field, item or sub-tree that only one file
has is one change with `old` or `new` `null`, holding `item <GUID>` and the number of its child
ports for a port, `base material <GUID>` and the number of sub-materials for a material, the
counts of textures, floats and colours for a sub-material, the value for a texture, float or
colour, and `present` for the DNA or a face part. Ports, materials, sub-materials, parameters and
textures (by slot) are paired by hash, so an insertion is one change, not a shift. Items that both
files hold in another order give one change `order` at their list (`item_ports.<port>.order`,
`materials.order`, `materials.<attachment>.sub_materials.order`, and `.textures.order`,
`.floats.order`, `.colors.order` below a sub-material) with the labels in the old and in the new
order; the order of face parts and blends is their meaning and is compared by position. The exit
code is 1 when the files differ (`identical` false) and 0 when not, as for `compare`; a file that
is no CHF exits with 3.

The report also carries `fieldOrigins` keyed by the exact report pointer and
`provenance` for both files. Each change's `old` and `new` uses identify the
original value or membership query on the corresponding side. Name lookups
used in the readable path and order text can carry Archive sources. The raw
hash path and comparison result derive from the CHF values.

## `p4kx chf to-json <file> --out <file> --json` and `p4kx chf from-json <file> --out <file> --json`

```json
{"input": "…", "output": "…", "versions": {"female": 2, "male": 7}, "size": 21934,
 "named": 14, "unresolved": 2, "unresolved_hashes": ["0x6d6de693"]}
```

`versions` are the inner versions of the CHF body; `size` is the number of bytes written, the
JSON text for `to-json` and always 4096 for `from-json`. Only `to-json` has `named` (the hashes
that got a label, counted where they occur), `unresolved` (those that got none) and
`unresolved_hashes` (the distinct unresolved hashes, ascending, as `0x` and eight hexadecimal
digits). Without `--out` the same counts are one line on stderr, and the plain text summary ends
with that line too. `to-json --json` needs `--out`, because
the converted JSON would otherwise share stdout with the summary; without it the exit code is 2.
An `--out` inside a Star Citizen installation, including a Channel's `user` folder, is refused with
exit code 2; copy a CHF file into `CustomCharacters` by hand.

The summaries add `originReceipt`, the companion basename, and immediate
`fieldOrigins` and `provenance`. The companion binds the unchanged artifact
bytes and addresses each emitted JSON datum or the opaque binary CHF.
See [Standalone field origins](source-origin.md).

### The CHF JSON

`to-json` writes, and `from-json` reads, one indented object. `from-json` of an unchanged
`to-json` output gives back the original file byte for byte (ADR-0014); any other value gives a
new file with a correct CRC-32C. Unknown fields and values out of range are rejected with exit
code 2; a JSON file larger than 1 MiB is not read (exit code 3). The hash fields are
CRC-32 or CRC-32C values of names; [docs/chf-hashes.md](chf-hashes.md) lists the function per field.

`to-json` writes the name of a hash it can resolve in a key beside it: `gender_hash_label`
(`character.dna`), `port_label` (item ports), `attachment_label` (materials) and `name_label`
(sub-materials, floats and colours), each right after the hash it names (ADR-0178). The labels
are output only: `from-json` accepts each of them in the one kind of object that carries it and
ignores its text, so the hash stays authoritative, an edited hash with an old label is written as
the hash, and a label anywhere else or any other unknown field is still refused. Errors point at
the line and column of the file as given. `--archive <Data.p4k>` adds the item ports of the items
the character wears and the sub-materials and parameters of the base materials it chooses to the
names the tool knows itself. A hash with no known name has no label. `BodyColor` and `EyeColor`
are mapped by hand: their hashes are not the CRC of those names.

| Field | Type | Meaning |
|-------|------|---------|
| `character.versions` | object | `female` (supported: 2) and `male` (supported: 7, 8); from 8 on the body ends with a zero `u32` |
| `character.body_type`, `character.voice` | string | GUIDs in the form DataForge prints them; the voice is all zeros when none is chosen |
| `character.dna` | object or `null` | `null` in a CHF that holds only materials, as the game's presets do; otherwise `gender_hash`, `variant_hash`, `blends_per_part`, `header_unknown`, `max_head_id` and `face_parts`: one object per face part in stored order, `part` naming it (`eyebrow_left`, `eyebrow_right`, `eye_left`, `eye_right`, `nose`, `ear_left`, `ear_right`, `cheek_left`, `cheek_right`, `mouth`, `jaw`, `crown`, `neck`, then `part_<n>`) and `blends` with exactly `blends_per_part` objects `{value, head_id}` |
| `character.item_ports` | object or `null` | `null` when the CHF holds no ports; otherwise the root port: `port` (name hash), `item` (GUID), `tail_count` (a count the layout keeps, as read) and `children`, the same objects, omitted when empty; at most 32 levels |
| `character.item_port_tail` | number | The `u32` the layout keeps behind the port count, in a CHF without ports the number of materials; omitted when 0 |
| `character.materials` | array | One object per attachment: `attachment` (name hash), `base_material` (GUID), `flags` and `sub_materials`, each with `name` (hash), `textures` (`slot`, `texture` GUID), `floats` (`name`, `value`) and `colors` (`name`, `rgba` as four numbers 0 to 255) |
| `container.flags` | number | The two header bytes behind the magic |
| `container.frame` | string | The original zstd frame as hexadecimal; reused while it decompresses to the body written from `character`, otherwise the body is compressed anew |
| `container.padding` | string | The bytes behind the frame as hexadecimal, without trailing zeros; written with the reused frame; with a body compressed anew only the last 8 bytes of the file are written back (ADR-0014), the rest is zeros, and a new frame that reaches into non-zero ones is an error (exit code 2) |

Hashes are unsigned 32-bit numbers. A float is a JSON number, or the string `0x` with its eight
hexadecimal digits when it is not finite (`"0x7fc00000"` for the usual NaN); every float is
written back with exactly the bits it was read with. `container` and each of its fields may be
left out; without `frame` the body is always compressed anew.

Wwise v154 effects, modulators and devices expose original type-specific
`source_fields`, including opaque plug-in byte ranges, typed references,
original property words and initial RTPCs. See [original effect, modulator
and audio-device metadata](audio-effect-metadata.md).
