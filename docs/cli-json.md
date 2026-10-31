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

`filter` is the Filter as given; `entries` holds the matching Entries in Archive order and is empty
when nothing matches.

## `p4kx info <archive> --json`

```json
{"archive": "…", "channel": "LIVE", "build": Build, "entry_count": 4, "encrypted_count": 1,
 "issue_count": 0, "compressed_size": 35, "size": 118}
```

`issue_count` is the number of Entry issues found in the directory of the Archive (ADR-0025); `info`,
`list`, `extract` and `verify` print the first ten to stderr as `warning: Entry <path>: <issue>`.
With the global `--recover` flag an Archive whose end records or central directory are damaged is
rebuilt by a scan for directory records; `issue_count` then counts one more issue, the recovery
itself (two when a scan limit cut the scan short), which every command prints once on opening as
`warning: <archive>: <issue>`. The `key` commands ignore `--recover`.

`channel` is the name of the folder holding the Archive, or `null` when there is none; `build` is
`null` when the Build is unknown.

## `p4kx dcb info <archive> --json`

```json
{"archive": "…", "entry": "Data\\Game2.dcb", "version": 8, "record_count": 117709,
 "struct_count": 6696, "enum_count": 774}
```

`entry` is the path of the DataForge Entry as the Archive stores it.

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
 "empty": 29, "unresolved": 0, "docked_vehicles": 0}
```

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

An **Error** has `kind`, a stable code, `reason`, text for people, and for an unresolved port the
target: `reference` (the GUID) or `class_name`, the other `null`. The kinds are:

- at a port: `dangling_reference`, `invalid_reference`, `unknown_class`, `not_an_entity`,
  `unreadable_record`, `cycle` (the Entity is already attached further up the branch) and
  `too_deep` (more than 32 attached Entities below the root);
- among the `issues` of an Entity: `too_many_ports` (the tree reached 65,536 ports, so this
  Entity's further ports are left out; the tree never holds more), `ambiguous_class` (several
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
 "placements": [Placement, …], "animations": [Animation, …], "issues": [Issue, …]}
```

`unresolved_layers` counts the layer files of the material libraries of all parts, their bone
attachments and the Interior that the Archive lacks, that cannot be read or that exceed the load
budget ([ADR-0291](adr/0291-layer-files-are-read-once-per-model-set-within-a-byte-budget.md)); the
materials that name them keep the colours of their own file. A library lists a file once per
`Submtl` it takes, as the Export as GLB summary counts it. Across libraries, spellings of one path
that differ in case, in `\` and `/`, in a leading `Data/` or in the `.mtl` extension are one file,
which counts as often as its most frequent library lists it. Each library lists at most 256 files, so the number is a lower bound.
`first_unresolved_layer` is the first such path, as the layer names it; it is absent when
`unresolved_layers` is 0 (the example shows the case without any).

`<entity>` is named as for `p4kx entity loadout`, with the same exit codes. `animations` is absent
without any; for a creature it lists each clip the GLB writes or leaves out, shaped as in `p4kx
model glb`: `name`, `duration_seconds`, `additive`, `channels`, `keys` and `skipped_tracks`. A
creature whose skins are bound to its skeleton gets, with `--animations`, the clips its animation database names
(`animDatabase3P` of `SAnimatedCharacterComponentParams`, a Mannequin `.adb` with the databases
nested in it), looked for by file name without extension in the animation folders (`#filepath`) and
the track databases of its skeleton's parameter file ([ADR-0307](adr/0307-the-clips-of-a-creature-are-those-its-animation-database-names-and-are-written-on-the-bones-of-its-skeleton.md));
`--animations` asks for them, without it the GLB is static. Names no clip has, such as blend spaces, are skipped, additive
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
(ADR-0297). Otherwise the part sits at the helper its port names in the parent's model, else at
that bone of the root's skeleton when the parent is a skin part, or, for a landing gear, at the
node of its Entity's model named like the gear's bone; at the node of the parent's model named
like the port, or at the parent's origin, which is also where a port with no helper and no node of
its name lands, without an Issue, and where a landing gear lands whose Entity has no model. `helper`
(the helper's name, or the bone of a landing gear; only for `helper`), `modelled`
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
a character definition, `.cdf`: its skin, else its first attachment, is the part's model, its bone
attachments are drawn as nodes below the part, and the files of the other attachments, or of a bone
attachment that cannot be read, are not drawn; reported once per definition, `skipped` is false),
`skeleton_missing` and `skeleton_unreadable` (the skeleton file, `Model File`, of a character
definition is not in the Archive, is larger than 16 MiB or cannot be read as a model; the skin
is still the part's model; reported once per definition, `skipped` is false),
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
was reached). New kinds may be added in later versions.

## `p4kx entity package <archive> <entity> --out <folder> --json`

```json
{"archive": "…", "entity": "AEGS_Gladius", "guid": "…", "out": "…", "lod": 0, "texture_mip": 0,
 "nodes": 83, "drawn_nodes": 56, "containers": 0, "objects": 0, "lights": 0, "meshes": 31,
 "shared_meshes": 25, "materials": 48, "palettes": 3, "textures": 120,
 "texture_pixels": 98304000, "texture_failures": 0, "files": 152, "bytes": 123456789,
 "vertices": 744831, "triangles": 818426, "lod_fallbacks": 0, "skipped": 0, "issues": [Issue, …]}
```

`<entity>` is named as for `p4kx entity loadout`, with the same exit codes. `lod_fallbacks` counts
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
given and `null` without `--chf`; `rig` is the head rig and `null` without a CHF. `loadout` is
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
or wrong exits with code 7.

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

`containers` are the Entry paths of the chain as for `p4kx socpak list`, with the same exit codes
(an Entry the Archive lacks is 2, a container that cannot be opened 3). `parts` counts the root
container, the `.soc` files, the nested containers, the placed models, the props and the lights;
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
`container_missing`, `container_unreadable`, `container_cycle` (the container is nested in
itself and not entered again), `file_missing`, `file_unreadable`, `invalid_placement`,
`objects_left_out`, `unsupported_objects`, `entities_left_out` (props or lights without a usable
placement), `light_unsupported` (ambient, planar or unknown lights, which glTF cannot express; the
`name` is the `.soc` file and the message gives the type and count), `geometry_missing`,
`geometry_unreadable`, `material_library`, `palette` (a tint palette that cannot be used; the
part keeps the next choice), `too_many_parts`, `too_many_containers`,
`too_many_vertices` and `too_deep`
([ADR-0081](adr/0081-an-interior-is-the-socpaks-soc-files-placed-in-nested-containers.md)). The
file is written even when parts are skipped.

## `p4kx interior package <archive> <container>... --out <folder> --json`

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
 "issues": [Issue, …]}
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
| `sounds` | object | With `--sounds` only: `variants` and `breaks` as `p4kx wwise resolve` lists them |

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

## `p4kx wwise extract <archive> <trigger> --out <folder> --json`

```json
{"archive": "…", "language": {…}, "trigger": {…}, "breaks": 0,
 "target": "…", "format": "ogg", "written": 3, "kept": 0, "unconverted": 1, "failed": 1,
 "cancelled": false,
 "files": [{"trigger": "Play_Multi", "wem_id": 100, "file": "…\\Play_Multi_100.ogg",
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
`failures` lists the sounds that got no file: source plug-ins and external sources (`wem_id` null),
WEMs no soundbank or loose Entry holds, and sounds whose absolute file path exceeds the limit of
ADR-0248 (260 UTF-16 units for the file, 248 for its folder), each with its reason; the other
sounds are still written. `cancelled` is true when Ctrl+C stopped the export before it handled
every sound; no partial file is left. Exit code 1 when anything is in `failures` or `unconverted`,
or `cancelled` is true.

With `--txtp`, the object has a `txtp` member, absent without it:
`{"written": 1, "kept": 0, "skipped": 1, "files": [{"trigger": "Play_Multi", "event": "Play_Multi",
"file": "…\\Play_Multi.txtp", "size": 120, "kept": false, "breaks": 0}, …], "skips": [{"trigger": "Play_Multi",
"event": "Play_Other", "reason": "plug-in source 9 has no file"}, …]}`. One `.txtp` file in
vgmstream's TXTP syntax is written per Wwise event that leads to sounds, named after the event by
the rule of the sound files (`event` for a name too long for a file) and holding the names of the exported files, which lie in the same
folder. An Event's actions, blend containers, music segments and music tracks are layers (`group =
-L<n> #@layer-v`), sequence containers and music playlists segments (`-S<n>`), and a random or
switch container or music switch a choice that the file settles on the first child (`-R<n>>1`);
the loop of a sound's WEM is written as `#I <start> <end>`. An event with a sound that has no
whole file (a source plug-in or external source, a WEM nothing holds, only the prefetched start of
a streamed WEM) is listed in `skips` and gets no file, as does an event whose file cannot be
written, e.g. because its path exceeds the limit of ADR-0248. An event with broken links is written
with the header comment `incomplete: <n> broken links`, and `breaks` of its file is `n`. A `group`
line carries only the macro; the comment on the object is the line above it. `files` and `skips` do
not change the exit code; the sounds without a file are in `failures` already. Not written when
Ctrl+C cancelled the export.

`p4kx entity audio --extract <folder>` writes the same files for every trigger of the Entity and
adds an `extraction` object with exactly `target`, `format`, `written`, `kept`, `unconverted`,
`failed`, `cancelled`, `files` and `failures`, as described here; `archive`, `language`, `trigger`, `breaks`
and `issues` are not repeated in it.

## `p4kx wwise resolve <archive> <trigger> --json`

```json
{"archive": "…", "language": {"requested": "German", "fallback": "English(US)"},
 "trigger": {"name": "Play_QDrive_Spool", "definition": "Data\\Libs\\GameAudio\\SC_SSQD_QDrive.xml",
             "events": [{"name": "Play_QDrive_Spool", "id": 2178236557, "localised": false}]},
 "variants": [Variant, …], "breaks": [Break, …],
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
target lies in another soundbank, the walk reads that one soundbank (at most one besides the one
holding the event, ADR-0158) and the variant's `soundbank` is the one holding the sound.
A **Break** has `event`, `soundbank` (or `null` when no soundbank holds the event), `object` (the
step where the chain breaks, the last of `path`), `target` (for `missing`, `other_soundbank` and
`cycle`, the ID the object refers to, otherwise `null`), `path`, `kind` and `message`. The kinds are
`no_soundbank`, `unreadable` (a soundbank cannot be read again; the Break's `soundbank` is the
one that cannot, which is the other soundbank when the walk was leaving the soundbank of the
event), `missing` (the object refers to an ID the soundbank does not hold; for a Trigger action,
a trigger no stinger names), `other_soundbank` (the target lies in a soundbank the walk does not
enter: the Archive holds no soundbank of the ID a Play action names, or the walk has already read
its one other soundbank), `chain_too_long` (more than 8 PlayEvent and Trigger actions in a row),
`malformed`, `unknown_layout` (an object of a bank version other than 154), `unsupported` (an
object kind that is not followed, e.g. an actor-mixer), `cycle`,
`too_deep` (a path of 32 objects), `too_many_children` (only the first 1024 children of a
container are followed), `too_many_objects` (at most 16,384 objects per event and soundbank) and
`no_localisation` (a localised event or streamed sound has no soundbank or WEM in `requested` nor
in the fallback language).
New kinds may be added in later versions.

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
References listed in `references` and `record_count` the number of distinct Records among their
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
holds the Reference. A DataForge that holds more than 8,388,608 References, or Records that
cannot be read, give a warning on stderr; the index is then incomplete. It keeps the first
8,388,608 References in the order of the Records.

## `p4kx dcb tags <archive> [<tag>] [--tree] [--format table|json] --json`

Lists the tag tree: the Tags of the `TagDatabase` with their tag paths. `<tag>` is a GUID or a tag
path (ignoring case; sibling Tags may share one, so it can name several); with it only that Tag and
everything below it is listed, without it every Tag. `--json` is `--format json`; with
`--format table` it is invalid input (exit code 2), as is a `<tag>` no Tag has.

```json
{"archive": "…", "tag_count": 18891, "shown_count": 3, "issues": {"unreadable": 0, "dangling": 0,
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
stderr: `unreadable` Tag Records left out, `dangling` listed Tags that are no Tag Record,
`repeated` Tags listed more than once (the first listing counts, which also cuts a cycle),
`too_deep` Tags left out, with everything below each, below 64 levels, `too_long` Tags left out, with
everything below each, whose name is over 256 bytes, whose path is over 1,024 bytes or that would
take the paths of the tree over 64 MiB, `duplicate` Tag Records left out because an earlier one has
their GUID, and `unlisted` Tags no `TagDatabase` reaches, which stand as roots of their own.

With `--tree` the document holds `"tree": true` and `tags` holds only the top of the listing, the
roots or the Tags `<tag>` names, each with its `children` nested as objects of the same shape; they
have no `parent` and `depth`, and `children` is no list of GUIDs:

```json
{"tree": true, "tags": [{"guid": "…", "name": "Weapon", "path": "Weapon", "legacy_id": 7,
 "children": [{"guid": "…", "name": "Gun", "path": "Weapon.Gun", "legacy_id": null,
 "children": []}]}]}
```

## `p4kx dcb fields <archive> [<type or Query>] [--path <glob>] [--format table|csv|json] --json`

The statistics per field path of the selected Records ([query.md](query.md#field-statistics)).
`--json` is `--format json`; with `--format table` or `--format csv` it is invalid input (exit
code 2).

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

## `p4kx catalog <archive> --out <file> --json`

```json
{"archive": "…", "out": "…", "schema_version": 2, "size": 131969336, "compressed_size": null,
 "upload_limit": 64000000, "decompressed_limit": 1000000000,
 "size_warning": "over_limit", "decompressed_warning": null, "kinds": [{"kind": "manufacturers", "count": 1201, "skipped": 2,
 "complete": true}, {"kind": "items", "count": 23280, "skipped": 1145, "complete": true},
 {"kind": "ships", "count": 1087, "skipped": 57, "complete": true},
 {"kind": "commodities", "count": 145, "skipped": 0, "complete": true},
 {"kind": "blueprints", "count": 1618, "skipped": 0, "complete": true},
 {"kind": "resourceGroups", "count": 19, "skipped": 0, "complete": true},
 {"kind": "blueprintCategories", "count": 21, "skipped": 0, "complete": true},
 {"kind": "gameplayProperties", "count": 29, "skipped": 0, "complete": true},
 {"kind": "qualityDistributions", "count": 21, "skipped": 1, "complete": true},
 {"kind": "qualityQuantizations", "count": 62, "skipped": 1, "complete": true},
 {"kind": "blueprintPools", "count": 160, "skipped": 0, "complete": true},
 {"kind": "dismantleProcesses", "count": 1, "skipped": 0, "complete": true},
 {"kind": "paints", "count": 1064, "skipped": 68, "complete": true},
 {"kind": "loadouts", "count": 10708, "skipped": 0, "complete": true},
 {"kind": "starmapObjects", "count": 2093, "skipped": 0, "complete": true},
 {"kind": "shops", "count": 1184, "skipped": 0, "complete": true}], "skipped": 1205,
 "failed": [{"guid": "…", "path": "libs/foundry/records/entities/scitem/odd", "error": "…"}],
 "warnings": [{"kind": "unreadable_reward_records", "count": 3}],
 "item_classes": {"classified": 347, "other": 408, "missing": 22525},
 "paints": {"paints": 1064, "assigned": 1039, "unassigned": 25, "ships": 1046, "unreadable_vehicles": 0},
 "loadouts": {"ships": 1087, "items": 9621, "ports": 293451, "attached": 152081,
 "docked_vehicles": 65, "unresolved": 505, "outside": 61, "with_issues": 193},
 "starmap": {"objects": 2093, "placed": 1239, "unplaced": 854, "ambiguous": 9, "systems": 3,
 "unreadable_containers": 0, "missing_containers": 2, "malformed_children": 0},
 "shops": {"shops": 1184, "registry_entries": 1184, "registry_missing": false, "registry_unreadable": false, "malformed": 0,
 "duplicate": 0, "containers_listed": 317, "containers_unreadable": 0, "containers_with_shop": 164,
 "other_containers_scanned": 8224, "other_containers_unreadable": 0, "other_containers_with_shop": 171,
 "prefabs_listed": 30, "prefabs_unreadable": 0, "prefab_entities": 8, "prefab_ambiguous": 1, "prefab_only_shops": 0,
 "with_kind": 590, "mixed_kind": 60, "without_container": 534, "with_items": 294, "inventory_ambiguous": 43,
 "inventory_placeholder": 399, "inventory_missing": 0, "inventory_unreadable": 1, "inventory_unscanned": 64,
 "soc_components": 320, "soc_paths_unattributed": 12, "inventories_unreadable": 1, "inventories_missing": 0, "inventories_named": 57,
 "inventories_in_archive": 119, "item_ids_dropped": 0, "commodity_exchanges": 16,
 "linked": 1000, "unlinked": 184, "ambiguous_link": 54, "unlinked_locations": ["…"]},
 "store_slugs": {"slugs": 256, "unmapped": 51, "mapped": 205, "resolved": 205, "retired": 0,
 "exported": 205, "stale": [], "stale_check_skipped": false},
 "playability": {"items": {"player": 7954, "ai": 0, "mission": 522, "special": 8570,
 "unclassified": 6234}, "ships": {"player": 324, "ai": 690, "mission": 25, "special": 35,
 "unclassified": 13}},
 "variants": {"items": {"groups": 1108, "variants": 4732},
 "ships": {"groups": 196, "variants": 850}},
 "quantizations": {"unused": 20, "unresolved_links": 0},
 "texts": [{"language": "en", "available": true, "resolved": 37963, "unresolved": 160,
 "placeholder": 23658}, {"language": "de", "available": true, "resolved": 25497,
 "unresolved": 12626, "placeholder": 23658}], "unresolved": 12786}
```

`quantizations` counts two facts of the game data that do not affect the exit code: `unused`, the
quality quantizations of the catalog that no `ResourceType` uses, and `unresolved_links`, the links
of `ResourceType` Records to a Record that is no readable quality quantization of the catalog (of
another type, failed or a development leftover; a Reference to a GUID no Record has reads as null).

`store_slugs` counts the curated RSI store slugs against the ships of the catalog and lists the
`stale` ones whose mapping no longer holds; `stale_check_skipped` is `true` and `stale` empty when slugs are
mapped but none resolves, as for an Archive that is not the game's ([basetool-catalog.md](basetool-catalog.md), "Ship store
slugs").

The catalog itself goes to `out` ([basetool-catalog.md](basetool-catalog.md)); a name ending in
`.zst` writes a zstd frame, and a name ending in `.gz` is refused with exit code 2 before anything is
written. `size` is the size of the JSON in bytes, uncompressed;
`compressed_size` is the size of the file written when it is compressed and `null` otherwise.
`size_warning` is `near_limit` from 90 % of `upload_limit`, `over_limit` above it, and `null`
otherwise; it compares the file written (`compressed_size`, else `size`). `decompressed_warning`
does the same for `size` against `decompressed_limit` (1,000,000,000). `kinds` lists every kind in catalog order with the entities written, the
development leftovers `skipped` and whether the kind is `complete` (no `--filter`); `skipped` sums
them. `failed` lists the selected Records that could not be read and are left out of the catalog,
with GUID, record path (empty when it cannot be read) and the reason and its causes joined by
`: `, ordered by GUID; the example shows one, a healthy Archive has none. The catalog is written
all the same, and the exit code is 1 when `failed` or `warnings` is not empty. `warnings` lists what
makes the crafting data incomplete or ambiguous although every Record was read, each as an object
with a `kind`: `unreadable_reward_records` with a `count` (Records that cannot be read although
their struct type is that of a Record with a reward source, so a source may be missing) and
`unknown_modifier_ranges` with a `count` (modifier ranges of blueprints whose struct type the reader does not know, so they are missing from the modifiers) and `details` (the struct types, ordered, without repeats), `unknown_template_values` with a `count` (stored names in the contract templates that the catalog does not know, a state, a category, a comms timing or reason or a kind of objective property, each `null` in the catalog) and `details` (the kind, the value and the number of each, ordered),
`ambiguous_dismantle` with a `count` (several dismantle processes, so no blueprint links one),
`unreadable_vehicle_files` with a `count` (ships whose vehicle file exists and cannot be read, so they have no paints; a file the Archive lacks is no such ship) and `details` (the path and the reason of each file, ordered, without repeats; the JSON lists every file, stderr the first eight) and
`unplaced_starmap_containers` with a `count` (unreadable containers, malformed or out-of-range Children and walk
limits that left starmap objects without a position), `unreadable_harvest_containers` with a `count` and `details` (system containers that cannot be opened or whose `.soc` cannot be read, so a provider preset may be missing from `harvestableLocations`; the path and the reason of each, ordered),
`unreadable_loot_containers` with a `count` and `details` (containers or entity files that cannot be read, a `.soc` that cannot be counted, a placed container the Archive lacks, and the containers beyond the limit of the walk, so a multi config may be missing from `lootContainers` or a container has no counts; as above),
`ambiguous_loot_entity_files` with a `count` and `details` (entity files of containers that name several different containers or multi configs, of which only the first of each is used; the container and file of each, ordered),
`unreadable_harvestables` with a `count` and `details` (`HarvestablePreset` or entity Records that are missing or cannot be built, so the harvestables that name them lack their entity and composition; the role, GUID and reason of each, ordered),
`harvest_locations_without_object` with a `count` and `details` (harvestable locations with no starmap object although one is recorded above their container: the object is not in the starmap objects, 64 or more mining claim markers lie above, or a marker has no object above it; one line per reason with its number; a container with no object recorded above is the normal case and is not counted),
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
least two members (`groups`) and the entities with a `variantOf` (`variants`). `texts` lists per language whether the Archive holds its `global.ini` (`available`) and
counts per text occurrence, so a key that several names or descriptions share counts once for
each: `resolved` texts, `unresolved` texts (a key the file lacks or holds empty, or every key in a
language that is not available; `null` in the catalog) and `placeholder` texts, whose key stands
for no text (the same in every language, neither resolved nor unresolved). `unresolved` sums the
unresolved texts of every language.

## `p4kx coverage <archive> [--format table|json] --json`

The coverage of the reference list in [coverage.md](coverage.md). The exit code is 1 when a
listed type (candidates excepted), component or folder is missing, unreadable or does not convert.

```json
{"archive": "…", "complete": false, "unreadable_entities": 0, "types": [Type, …], "components": [Component, …], "starmap_types": [StarMapType, …], "entries": [Folder, …]}
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

## `p4kx ships <archive> --out <file> --json`

```json
{"archive": "…", "out": "…", "format": "json", "ships": 1087, "skipped": 57, "mass": 1087,
 "hull": 1087, "cargo": 565, "issues": 3,
 "failed": [{"guid": "…", "path": "libs/foundry/records/entities/spaceships/odd", "error": "…"}]}
```

The ship data itself goes to `out` ([ship-base-data.md](ship-base-data.md)). `format` is `json` or
`ndjson`; `ships` counts the ships written, `skipped` the development leftovers; `mass`, `hull`
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

CSV cells are written as they are. A text that starts with `=`, `+`, `-` or `@` is read as a
formula by a spreadsheet program, so open a table of untrusted texts as text, not as formulas, or
use the JSON format.

With `--out` and `--json` the command prints `{"archive": "…", "out": "…", "format": "csv",
"languages": ["en"], "keys": 101234}`.

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
action's. An input that is empty or blank is no binding.

The CSV starts with a UTF-8 byte order mark, has CRLF line ends and the header
`action_map,action_map_label,action,label,activation_mode,keyboard,mouse,gamepad,joystick`. A
device cell joins its bindings with ` | `; a binding whose activation mode differs from the
action's reads `input (mode)`. Text cells follow the rules of ADR-0159: a cell that starts with
`=`, `+`, `-`, `@`, a tab or a carriage return gets a leading `'`. `--csv-raw` writes the exact
texts without the `'` and keeps the byte order mark; it is invalid input (exit code 2) with
`--format json`.

With `--out` and `--json` the command prints `{"archive": "…", "out": "…", "format": "csv",
"language": "en", "action_maps": 50, "actions": 1103, "unresolved_labels": 1}`;
`unresolved_labels` counts the label keys, of actions and of action maps, with no text in the
language.

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

## `p4kx texture merge <archive> <entry> --out <file> --json`

Also `p4kx texture merge <file> --out <file> --json` for a header file on disk.

```json
{"texture": "Data\\Textures\\hull_ddna.dds", "output": "…\\hull_ddna.dds", "parts": 4,
 "size": 2916}
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
## `p4kx audio info <archive> <entry> --json`

Also `p4kx audio info <file> --json` for a WEM file on disk.

```json
{"sound": "Data\\Sounds\\wwise\\Media\\100.wem", "codec": "pcm", "codec_name": "PCM",
 "format_tag": 65534, "channels": 2, "sample_rate": 48000, "sample_count": 1200,
 "duration_seconds": 0.025}
```

`sound` is the Entry path, or the path of the file on disk. `codec` is one of `pcm`,
`ima_adpcm`, `vorbis`, `opus`, `ptadpcm`, `xwma`, `xma2`, `aac`, `dsp`, `hevag`, `atrac9` or
`unknown`; `codec_name` is its readable name and `format_tag` the raw tag of the `fmt` chunk.
`sample_count` is the number of frames (samples per channel) and `duration_seconds` the playing
time; both are `null` when the header does not state them and the codec does not allow computing
them.

## `p4kx audio wav <archive> <entry> --out <file> --json`

Also `p4kx audio wav <file> --out <file> --json` for a WEM file on disk.

```json
{"sound": "Data\\Sounds\\wwise\\Media\\100.wem", "output": "…\\100.wav", "channels": 2,
 "sample_rate": 48000, "sample_count": 1200, "size": 4844}
```

`sample_count` is the number of frames written and `size` the size of the written WAV file. A
codec without a decoder prints no JSON; it fails with exit code 1 and an error naming the codec.

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

## `p4kx bnk list <archive> <entry> --json`

Also `p4kx bnk list <file> --json` for a soundbank file on disk.

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
the reader decodes them: `decoded` (the fields of the type were read; containers and music objects must use their whole
body, a sound, action or event is not checked for bytes after its fields), `undecoded` (a type the
reader has no decoder for, e.g. actor-mixers, buses and states; the body is skipped),
`unsupported` (a decoded type in a bank version whose layout is not known: Sound below version 150,
containers and music objects below 154, any decoded type above 154) and `malformed` (the body does
not hold the fields of its type). `matched` counts the objects that pass `--type` and `--id`;
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
IDs of its sources. `detail` depends on the type and is `null` for an event (its actions are the
children) and for undecoded, unsupported and malformed objects:

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
  and `targets` (the nodes its leaves play, in tree order).
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
failures are not repeated on stderr. The exit code is 1 when `failed` is not empty.

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

## `p4kx ui render <archive> <entry> --out <file.png> [--svg] [--size <W>x<H>|<W>] [--background …] [--lang <language>] --json`

Also `p4kx ui render <file> --out <file.png> --json` for a GFx or SWF file on disk.

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
defaults and the rest are clamped, as the camera does. `drawn_parts` is the number of parts with
geometry the picture shows, `triangles` the triangles they place, and `issues` the notes and
skipped parts of the Assembly as `entity glb` lists them. `milliseconds` splits the run into
reading and assembling, building the scene, drawing and reading the picture back, and writing
the file. An Entity the Archive lacks, a bad option, a `--out` that is the Archive or lies in a
Star Citizen installation and a `--size` that is empty or beyond what the device draws
print no JSON and fail with exit code 2. A replacement that does not fit its port is drawn all the
same and listed in `incompatible` (absent when empty), as for `entity loadout`; `--loadout-pick`
works as there. A machine without
a graphics adapter fails with `no graphics adapter` on stderr and exit code 1.

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

## `p4kx ui render <archive> --screen <canvas> --out <file.png> [--svg] [--size <W>x<H>|<W>] [--background …] [--brand <code>] [--var <name>=<value>]… [--lang <language>] --json`

Renders a `BuildingBlocks` canvas, by record path or GUID, instead of a movie; an ENTRY excludes
`--screen`. Texts are drawn from the fonts of the movie the `UIElement` Record
`uielements/buildingblocks` names (`fonts_en.gfx`) and their localisation keys resolve in `--lang`,
English by default; when the font movie or the language cannot be read, a warning goes to stderr and
the texts are left out.

`--var <name>=<value>` (repeatable, at most 64, only with `--screen`) sets a game variable before
the bindings are evaluated (ADR-0259); every other variable stays at zero, the state at rest
(ADR-0110). `p4kx ui render … --screen <MFD> --var powerstate=1` draws the MFD of a powered ship.
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
overlay` for spacer containers, `list rows come from run-time data: none drawn at rest`, `list
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
the file gives them. `nodes` keeps the order of the file;
`parent` is the index of the parent node or `null` for a root, and `transform` the 3×4 row-major
matrix from the node's space to its parent's, translation last, in CryEngine axes (Z up) and
metres. A model without a `NodeMeshCombo` chunk has one root node named after the file. `scale` is
the scale the node's record stores (1 on every axis when it stores none), `geometry_type` the
record's geometry type (0 for geometry, 2 or 3 for a helper; `null` without a `NodeMeshCombo`
chunk), `helper` whether it is a helper, `properties` the `key=value` lines of the node's entry in
the chunk's property table in order, e.g. `class=AnimatedJoint`, and `node_material_indices` the
chunk's list of material indices. A transform or scale whose values are all below 1e-5 is the
identity or 1. `issues` lists in words what a read worked around: `model info` reads
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
`chunks` lists every chunk of the model file and of its
geometry companion (`file` is `model`, `geometry` or `lod<n>-geometry`) with `usage` `read` when
the model was read from it, `skipped` for a known type the model does not read and `unknown` for
an unknown type.
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
material reads the vertex colours, ADR-0253);
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

## `p4kx compare <old> <new> --json`

The Comparison report, the same document `--report <file>` writes (there pretty-printed). Its
JSON Schema (draft 2020-12) is generated from the report types and checked in as
[`schemas/comparison-report.schema.json`](../schemas/comparison-report.schema.json); a test fails
when the two drift apart, and `P4KX_BLESS=1 cargo test -p p4kx-comparison --test report_schema`
regenerates it. The output is deterministic: Entries are ordered by path and Records by record
path, both ignoring case and separator kind, Records with the same record path by GUID. The exit
code is 0 without and 1 with differences on any compared level.

| Field | Type | Meaning |
|-------|------|---------|
| `format_version` | number | Version of the report shape, now `1` |
| `source.old`, `source.new` | object | `archive` (path), `channel` (string or `null`) and `build` (a [Build](#build) or `null`) of each side |
| `levels` | array | The levels compared, e.g. `["entries", "records"]` |
| `filter` | string or `null` | The Filter that selected the Entries by path and the Records by the record path of either side |
| `entries.counts` | object | `added`, `removed` and `changed` Entry counts |
| `entries.directories` | array | The same counts per top-level directory (`directory`, empty for the root), ordered ignoring case |
| `entries.changes` | array | One object per Entry: `path` (as the new side stores it, the old side for a removed Entry), `change` (`added`, `removed` or `changed`), and `old` and `new` with `size` and the checksum for the sides that have it: `crc32c` for an Archive Entry, `crc32` for an Entry of a plain ZIP archive (a SHA-256 would be `sha256` as 64 hex digits; no Entry states one) |
| `records.counts` | object | `added`, `removed`, `moved`, `changed` and `unreadable` Record counts |
| `records.changes` | array | One object per Record: `guid`, `change` (`added`, `removed`, `moved`, `changed` or `unreadable`), `old` and `new` with `record_path`, `name`, `tag` (version 8 only, absent without one) and `type` for the sides that have it and can be read, `fields`, and `errors` for an unreadable Record |
| `records.changes[].fields` | array | One object per changed field or array element, nested ones included: `path` (its location, e.g. `hull.plates[1].armor`), and `old` and `new` values for the sides that have it; empty for an added, removed or unreadable Record |
| `records.changes[].errors` | array | One object per side whose Record cannot be read: `side` (`old` or `new`) and `reason`; absent for every other Record |

`entries` is absent when the Entry level was not compared, `records` when the Record level was
not compared.

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
| `counts` | object | `structs`, `properties`, `enums` and `options`, each with `added`, `removed` and `changed` counts; `properties` counts the property changes of the changed structs and `options` the option changes of the changed enums |
| `structs` | array | One object per added, removed or changed struct: `name`, `change` (`added`, `removed` or `changed`), `records`, `parent` and `properties` |
| `structs[].records` | object | `old` and `new`: the Records on each side whose struct is this struct or inherits from it, `0` on a side that lacks the struct; instances that are no Record, such as the targets of strong pointers, are not counted |
| `structs[].parent` | object | `old` and `new` parent struct name (`null` for none); present only when a changed struct inherits from another struct than before |
| `structs[].properties` | array | The properties the struct declares itself that differ, ordered by name, empty for an added or removed struct: `name`, `change`, `aspects`, and `old` and `new` for the sides that have it |
| `structs[].properties[].aspects` | array | What differs in a changed property, in this order: `data_type`, `conversion`, `target`; empty for an added or removed property |
| `structs[].properties[].old`, `.new` | object | `data_type` (e.g. `Reference`, `String`, `Class`; `Unknown(0x…)` for an unknown code), `conversion` (`0` for one value, any other code for an array) and `target` (the struct or enum name, or `null`) |
| `enums` | array | One object per added, removed or changed enum: `name`, `change`, `records` and `options` |
| `enums[].records` | object | `old` and `new`: the Records on each side whose struct declares or inherits a property of the enum, each Record counted once |
| `enums[].options` | array | The options added or removed from a changed enum, ordered by name: `name` and `change` (`added` or `removed`) |
| `unreadable` | object | `old` and `new`: the structs, enums, properties, options, parents and Records left out on each side because a name or struct cannot be read, or because a struct, enum or property name is defined twice (the first definition wins) |

A property is joined by name within its struct, and differs in its data type, conversion type or
target. A struct also differs in its parent. Records are counted by the struct they are instances
of (GUID-addressed Records only), then rolled up to the parents, at most 64 levels and without
following a cycle.

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
hold the size its central directory states, or holds corrupt CryXml. Memory stays bounded: each
side holds at most 128 MiB of text and about 9 bytes of bookkeeping per line.

## `p4kx cryxml <file> --out <file> --json`

```json
{"input": "…", "output": "…", "format": "CryXmlB", "byte_order": "little"}
```

`format` is the signature, `CryXmlB`, `CryXml` or `CRY3SDK`, or `text` for text XML written
unchanged; `byte_order` is `little`, `big`, or `null` for text XML. `--json` needs `--out`,
because the XML would otherwise share stdout with the JSON; without it the exit code is 2.

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
