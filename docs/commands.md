# Command line and MCP server reference

Every command, option and exit code of `p4kx`, and the tools of the MCP server `p4kx-mcp`. The
[README](../README.md) gives the overview; `p4kx --help` and `p4kx <command> --help` are the
authority on the exact options of the installed version. Terms follow [CONTEXT.md](../CONTEXT.md);
the JSON shapes are in [cli-json.md](cli-json.md).

## Commands

The first argument of nearly every command is the path of a `Data.p4k` Archive, written
`<archive>` below.

- `p4kx list <archive>` lists every Entry of an Archive: compression method, encryption flag,
  compressed size, size and path. Archives in the v1 (ZIP64) and the v2 (`Jiji` trailer) layout
  are recognised automatically; any other layout version is rejected with its number. A split
  texture (`.dds` with its parts and alpha tail) is one row with the size of the whole texture and
  ` (+N parts)` behind its path (`parts` in `--json`); `--parts` lists every part as an Entry of
  its own.
- `p4kx search <archive> <filter>` lists the Entries whose path matches a Filter, in the same
  columns as `list` but unfolded: every matching part is a row of its own. A Filter with `*` or `?` is a glob over the full path: `*` matches any run of
  characters, separators included, `?` exactly one character, and the glob must match the whole
  path (`*.xml`, `Data\Libs\*`, `*\defaultProfile.???`). Any other Filter matches when it occurs
  anywhere in the path (`Libs\Config`). Both ignore ASCII case and treat `\` and `/` alike; every
  other character, spaces and brackets included, is literal, and an empty Filter matches every
  Entry. No match is not an error. `--exclude <filter>`, repeatable, leaves out the Entries an
  exclusion Filter matches, with the same rules; a texture whose header it matches is left out with
  its parts. With `--exclude`, a search that finds nothing is invalid input (exit 2) and the error
  says whether the Filter matched none or the exclusion left out all it matched. `--json` lists the
  exclusion Filters as `exclude`. A Filter is at most 1024 bytes with at most 32 `*` and `?`; an
  empty or blank exclusion Filter is refused. Every command that takes a Filter refuses text beyond
  these limits as invalid input (exit 2), and the MCP tools answer with a tool error.
- `p4kx grep <archive> <text> [--in records,loc,entries] [--filter <filter>] [--exclude <filter>]
  [--lang <language>,…] [--limit <n>] [--per-item <n>] [--format table|json]` finds a text in the
  contents: in the string, localisation key and enum values of all Records (with the field path),
  in the texts of each language's `global.ini` and in the lines of text and CryXml Entries (CryXml
  converted to XML first). It matches part of a word and ignores case, so `Gladi` finds
  `AEGS_Gladius`. Nothing is indexed or stored: each search scans in parallel and streams, in a
  second or two on LIVE. Results are bounded (`--limit` per source, `--per-item` per Record or
  Entry) and say whether more exist; Entries with binary data, above `--max-entry-size` or
  encrypted without the Archive key are skipped and counted on stderr
  ([docs/cli-json.md](cli-json.md)). `--exclude`, repeatable, leaves out every Record,
  localisation key and Entry whose record path, key or path an exclusion Filter matches, with the
  rules of `search`; it also works without `--filter`. The window searches the same way in its
  *Search* tab.
- `p4kx verify <archive> [--filter <filter>] [--exclude <filter>]` checks the data of every Entry
  (or those the Filter matches, without those a repeatable `--exclude` matches, with the rules of
  `search`; a selection they leave empty is invalid input, exit 2) against the CRC-32C the Archive
  directory states, in parallel and without writing anything, and prints every mismatch with the
  stored and the computed value; an Entry that cannot be read is reported as failed. The checksum
  is computed while the data streams past, so a run needs one read and decode of the Archive (timings in [docs/verify.md](verify.md)).
  Exit code 1 when an Entry mismatches or fails. `p4kx extract --verify` does the same for each
  Entry while it is written: a mismatching Entry is not written, and the summary counts it
  (`mismatched` in `--json`). `p4kx socpak verify <archive> <container…>` and `socpak extract
  --verify` check a container with the plain CRC-32 of its directory. Checking is off unless
  asked for.
- `p4kx extract <archive> <entry paths…> [--out <folder>] [--conflict <policy>] [--convert [--textures <outputs>] [--mip <level>] [--audio <format>]]` writes the
  given Entries (stored, deflate or zstd, encrypted or not) into a new folder
  `SC-EXPORT-<Channel>-<Build version>-<timestamp>` in the current directory (the Channel is the
  name of the folder holding the Archive, an unknown Build is `UnknownBuild`, the timestamp is
  UTC), or into `--out`. Entry paths match with `\` or `/` and ignore case.
  `--conflict` decides what happens to a file that already exists in the target: `skip` (the
  default) keeps it, `overwrite` replaces it, and `if-changed` replaces it only when its size or
  CRC-32C differs from the Entry; kept files are reported as skipped. Entries are written in
  parallel, each to a temporary file that replaces the destination once complete. Entry paths
  that are absolute, carry a drive prefix, contain `..` or name a Windows device are rejected,
  and nothing is written outside the target. A corrupt Entry fails alone and is named with the
  reason. While stderr is a terminal a progress line shows the Entries done; Ctrl+C cancels the
  run promptly, leaves no partial file and reports the Entries not written as cancelled; a second
  Ctrl+C quits at once. The
  run ends with a summary of extracted, skipped, failed and cancelled Entries.
  `--convert` writes CryXml Entries (`CryXmlB`, `CryXml`, `CRY3SDK`, either byte order) as
  UTF-8 XML under their own path; text XML and every other Entry stay byte-identical. A CryXml
  Entry that cannot be converted fails alone with the reason, and `if-changed` compares an
  existing file with the converted XML. The summary then also counts the converted Entries.
  `--convert` also writes the DataForge Entry (`Data\Game2.dcb`, falling back to
  `Data\Game.dcb`) as one Record XML file per Record below its folder instead of the `.dcb`,
  e.g. `Data/libs/foundry/records/…`, each file under `--conflict`; Records with error values are
  still written, and the summary counts the Records with their error values.
  `--convert` also converts each texture (`<name>.dds`, its parts `<name>.dds.1` … `.N` and any alpha
  tail `<name>.dds.a` with `.1a` … `.Na`) once; the PNG carries the tail in its alpha channel, a
  high-range texture (BC6H or floating point) is exported as OpenEXR `<name>.exr` instead, keeping its range, and a
  cubemap as six faces `<name>_posx` … `<name>_negz`, a texture array as one PNG per element
  `<name>_layer0` … and a volume as one PNG per slice `<name>_slice0` …:
  `--textures` picks the outputs, by default `merge,png`: `merge` writes one standard DDS under
  the header's path, `png` the mip `--mip <level>` (0, the largest, by default; a texture with
  fewer levels gives its smallest) as `<name>.png`, and `raw` the Entries unchanged as well, with
  `merge` only the parts. With `merge` and no `png`, an alpha tail is merged like any split
  texture under its own path `<name>.dds.a`, so it is not lost. A selected header takes its
  parts from the Archive even when they are not selected. A texture that cannot be converted,
  e.g. a missing part or a format without a decoder, is reported as not converted with the
  reason, its raw Entries are written (the parts only when the merged DDS was written), and the
  Extraction continues with exit code 1. A part selected without its header is written
  unchanged.
  `--convert` also converts WEM sounds (`<name>.wem`) as `--audio` says, by default `ogg`: Wwise
  Vorbis is rebuilt as Ogg Vorbis and Opus repackaged as Ogg Opus, `<name>.ogg`, without
  re-encoding, and every other codec decoded to a 16-bit PCM WAV `<name>.wav`; `wav` decodes every
  sound to WAV, and `raw` keeps the WEMs. Each soundbank (`<name>.bnk`) is written unchanged and
  unpacked into the folder `<name>` next to it, each embedded WEM named by its ID and written in
  the same format. A sound whose codec cannot be decoded is reported as not converted, naming the
  codec, and written unchanged as `.wem`; plug-in media and the prefetched start of a streamed WEM
  are written unchanged without a report. Every file follows `--conflict`, `if-changed` comparing
  with the converted output. Sounds and soundbanks are converted on a few threads of their own,
  within a memory budget of 1 GiB.
  `p4kx extract <archive> --filter <filter>` extracts every Entry the Filter matches instead; a Filter that matches nothing is invalid input and the error says what to try, e.g. that a glob must match the whole path.
  `--exclude <filter>`, repeatable and only with `--filter`, leaves out every Entry an exclusion
  Filter matches, with the rules of the Filter: `--filter 'Data\Objects\*' --exclude '*_lod?.cgam'`
  extracts the models without their level-of-detail companions. A texture whose header an
  exclusion Filter matches is left out with its parts; an excluded part of a texture that stays is
  not written, and with `--convert`, which still reads it to build the texture, a note on stderr
  gives their number. A pair that selects nothing is invalid input (exit 2), and the error says
  whether the Filter matched none (`no Entry matches …`) or the exclusion left out all it matched
  (`the exclusion Filters leave out all N Entries that … matches`). A Filter longer than 1024 bytes
  or with more than 32 `*` and `?`, an empty or blank exclusion Filter and more than 64 of them are invalid
  input too.
  `--threads <n>` limits the worker threads (default one per logical processor); more than about four rarely help on one disk (docs/performance.md).
- `p4kx convert <folder> --out <folder> [--filter <filter>] [--exclude <filter>] [--textures <outputs>] [--mip <level>] [--audio <format>] [--conflict <policy>] [--copy-other] [--threads <n>]`
  converts a folder of files that were extracted earlier without conversion, as `p4kx extract
  --convert` converts the same Entries (ADR-0272): CryXml (by content), a texture header
  `<name>.dds` with its parts `<name>.dds.N` and alpha tail `<name>.dds.a`, `.wem` sounds, `.bnk`
  soundbanks and one DataForge file, which becomes one Record XML file per Record. The
  paths below the input folder act as Entry paths, the options are those of `extract`, the output
  names follow the rule of ADR-0248, and `--conflict` defaults to `skip`. A header whose parts are
  missing is reported as not converted and written unchanged. A file with nothing to convert is
  counted as not converted and left out; `--copy-other` writes it unchanged. The DataForge is the
  file at a path the game reads, `Data\Game2.dcb` before `Data\Game.dcb` (also `Game2.dcb` when
  the folder is the `Data` folder), because a stale `Game.dcb` next to the current one must not
  win by sorting order; without one, the first `.dcb` file by path that starts like a DataForge;
  without one, the first file of any name that does. Every other DataForge file the rule found
  is named on stderr as `left out` and in `dataforge_left_out` of the JSON report. `--filter` matches the relative paths with
  the rules of `p4kx search`; one that matches no file is invalid input. `--exclude`, repeatable
  and also without `--filter`, leaves out the files an exclusion Filter matches, with the texture
  rule of `extract`: a texture whose header it matches is left out with its parts, a part it
  matches alone is not written but still read to build the texture, which a note on stderr tells;
  a pair that selects no file is invalid input. Links (symbolic links and
  junctions) in the input folder are never followed and are listed as failed, as are names that
  are not UTF-8 or hold a separator. An output folder that is the input folder, lies inside it or
  contains it, or lies inside a Star Citizen installation is invalid input (exit 2) and nothing is
  written; an input that is no folder is too. Progress, Ctrl+C, `--json` and the exit codes are
  those of `extract`; the JSON report of [`p4kx extract`](cli-json.md#p4kx-extract--json) adds
  `not_converted`, the number of files left out, when it is not 0. The window offers **Convert
  folder…** in the Archive menu with progress and cancel.
- `p4kx info <archive>` prints the Channel, the Build (branch, version, change number, BuildId)
  from the `build_manifest.id` next to the Archive, and the number, encryption and sizes of its
  Entries, and the number of Entry issues (ADR-0025). An Archive without a readable manifest has
  an unknown Build and still opens. A malformed field of one directory record, such as a path
  that is not UTF-8, an encryption value other than 0 or 1 or v2 data outside the file, does not
  stop an Archive from opening: `info`, `list` and `extract` print up to ten `warning: Entry …`
  lines to stderr for the Entries concerned.
- **Input of every `p4kx dcb` command:** `<archive>` is a `Data.p4k` Archive or a DataForge file
  on disk, e.g. a `Game2.dcb` kept from an earlier patch (ADR-0249). A file is recognised by its
  content, a DataForge header, not by its name; the file is only read, never written. The limits
  are those of the Entry in an Archive: versions 6 and 8, and no more than 2 GiB. A file that is
  neither an Archive nor a DataForge, such as a zero-filled file, an executable or one shorter
  than the header, exits with code 3. `export` without `--out` names its default folder
  `SC-EXPORT-UnknownChannel-UnknownBuild-<timestamp>` for a file, as a file carries neither Channel
  nor build. A DataForge holds no localisation texts: `export --lang` on a file needs
  `--archive <Data.p4k>` for them and exits with code 2 without it. Tags and the other values
  resolve from the DataForge alone. The `archive` field of the JSON documents holds the path that
  was given.
- `p4kx dcb info <archive>` opens the DataForge of the Archive (`Data\Game2.dcb`, falling back to
  `Data\Game.dcb`) and prints the Entry it was read from, its version and its number of Records,
  structs and enums. DataForge versions 6 and 8 are read; any other version, version 5 and older included, is
  reported as not supported with exit code 5 (ADR-0136), and a DataForge Entry claiming more than 2 GiB is rejected before it is read.
- `p4kx dcb export <archive> [--format xml|json|ndjson] [--combined] [--filter <filter>]`
  `[--lang <language> [--archive <Data.p4k>]] [--tags] [--out <folder>] [--conflict …]` writes
  Records as Record XML
  ([docs/record-xml.md](record-xml.md)) or Record JSON
  ([docs/record-json.md](record-json.md)), one file per Record: `<record path>.xml`, or
  `<record path>/<Record name>.xml` for Records that share a record path (`.json` for JSON).
  `--format ndjson` writes one `records.ndjson` with one Record per line in record-path order.
  `--filter` selects Records by record path with the rules of `search`; `--out` and `--conflict`
  work as for `extract`. A value that cannot be read is written as an error at its field and the
  Record is still written; the summary counts these error values. The same Record always gives
  the same bytes. `--combined` writes every selected Record into one file in record-path order,
  named after the DataForge Entry (`Game2.xml`), under one `p4kx:records` root element.
  `--flat --format csv|ndjson` writes one row per leaf value instead
  ([docs/record-flat.md](record-flat.md)): `values.csv` (UTF-8 with byte order mark, formulas
  neutralised; `--csv-raw` for the exact values) or `values.ndjson`.
  `--lang` writes the text of every localisation key next to the key, or marks the key missing,
  in the chosen language; without it the output carries no texts. `--tags` writes the tag path
  (`Ship.Series.Avenger`) of the Tag next to every Reference to a Tag; without it the output carries
  only GUIDs.
- `p4kx dcb tags <archive> [<tag>] [--tree] [--format table|json]` prints the tag tree of the
  `TagDatabase`: every Tag with its tag path, GUID and legacy ID in tree order, or with `--tree` the
  names indented below their parent. `<tag>` is a tag path or GUID and limits the listing to that Tag
  and everything below it; with `--json` the document has the shape described in
  [docs/cli-json.md](cli-json.md). The Tags tab of the window shows the same tree.
- `p4kx dcb refs <archive> [<target>] --incoming|--outgoing|--files|--unresolved [--format table|json]`
  lists References. `--incoming` gives the Records that refer to the Record `<target>` (a GUID or a
  record path), with the field of each Reference; `--outgoing` the Records it refers to, with its
  References to a GUID no Record has listed apart; `--files` the record files that refer to other
  record files (the File graph), limited by `<target>` to the files that refer to its file or are
  referred to by it; `--unresolved` every Reference to a GUID no Record has.
- `p4kx dcb fields <archive> [<selection>] [--path <glob>] [--format table|csv|json] [--csv-raw]`
  lists every field path of the Records with how often it occurs, in how many Records, of which
  kinds its values are and the smallest and largest number. `<selection>` is a type name or a Query
  ([docs/query.md](query.md); its `select` clause is ignored) and limits the Records counted;
  `--path` keeps the field paths that match, with the rules of `search`. The shape is described in
  [docs/cli-json.md](cli-json.md).
- `p4kx dcb query <archive> "<query>"` runs a Query ([docs/query.md](query.md)) over the
  Records and prints a table: the record path first, then one column per path of `select`, one
  row per Record in record-path order, e.g.
  `p4kx dcb query Data.p4k 'from EntityClassDefinition at "*/spaceships/*"'`. `from <type>`
  selects a type and every type deriving from it, `at "<glob>"` selects by record path with the
  rules of `search`. An invalid Query exits with code 2 and marks its position. `--format csv`
  writes RFC 4180 CSV with a header row and `--format json` (or `--json`) one line of JSON, an
  array of objects keyed by column with typed values; `--out <file>` writes the result to a file
  instead of stdout and prints a summary.
- `p4kx entity loadout <archive> <entity>` prints the default Loadout tree of an Entity named by
  class name (`AEGS_Avenger_Titan`), GUID or record path: every port with the Entity attached to
  it, its size and helper, and recursively the ports of each attached Entity. Ports that cannot be
  resolved, such as a dangling Reference, are listed with the reason. A Loadout generated at
  random from loadout assortments, as a creature or an NPC has it, resolves to the first
  alternative of each group (ADR-0289). For a ship it also reads the
  vehicle implementation XML with the Modification its Record selects: ports the implementation
  hides are marked `(invisible)`, and the treads, wheels and landing gear are listed. With
  `--json` the tree has the shape described in [docs/cli-json.md](cli-json.md). A docked
  vehicle, such as the Merlin in a Constellation's docking collar, is listed as
  `[docked vehicle]`; `--stop-at-docked-vehicles` leaves out its own ports. `--exclude <port>`
  drops a branch of the Loadout and `--replace <port>=<entity>` attaches another Entity, with its
  own Loadout, to a port; a port is its path of port names joined by `/`, and an Entity that does
  not fit the port's item types and sizes is attached all the same and warned about on stderr.
  Both can be given several times, here and for `entity glb`.
- `p4kx entity glb <archive> <entity> --out <file>` exports an Entity as one GLB: its geometry and
  that of every Entity of its default Loadout, each placed at its port as a node named
  `<port>/<entity>` below the node of the Entity it is attached to. `--lod <n>` applies to every
  model and `--materials colours` writes plain colours, with the material library an Entity's
  Record names replacing the one its model names; `--materials textures` also embeds the
  textures of every part as PNG, each image once, at the mip `--texture-mip <n>` picks for the
  whole Assembly. The tint palette of each part (its Entity's, else the default of its material
  library) gives the colours of the materials that take one, so liveries look right. Parts
  below the Entity whose Entity or geometry cannot be resolved are skipped and listed, and so are
  palettes and textures that cannot be resolved. When the geometry of the Entity itself is not in
  the Archive (exit code 2) or cannot be read, also for want of its `.chrm` companion (exit code
  3), the command fails naming the file and writes nothing; `entity package` does the same. With
  `--json` the report has the shape described in [docs/cli-json.md](cli-json.md). A docked
  vehicle such as a Merlin or an Archimedes is
  placed as one part, without the parts of its own Loadout, unless `--expand-docked-vehicles`
  places those parts too. With `--animations` a creature whose skins are bound to its skeleton also
  gets the clips its animation database names, as glTF animations of the bones; without it the
  GLB is static.
  `--interior` adds the ship's rooms:
  the socpak containers the Records of the ship and of its attached Entities name, each at its
  bone of the Entity that names it, below an `Interior` node of that Entity; a docked vehicle
  brings its containers only with `--expand-docked-vehicles`. What cannot be resolved is listed,
  and without the flag only the hull is written.
- `p4kx entity package <archive> <entity> --out <folder>` writes an Entity as a Blender package,
  a folder that does not depend on Blender's file format: `scene.json` (format version, source
  Channel and Build, the node hierarchy with names and transforms), one GLB per distinct mesh
  and the textures of the materials as PNG, each written once. `--lod`, `--texture-mip`,
  `--exclude` and `--replace` work as for `entity glb`. The folder is written beside the target
  and moved into place when complete; it must not exist or must hold an earlier package. The
  format is described in [docs/blender-package.md](blender-package.md), the JSON Schema of
  `scene.json` is [schemas/blender-package.schema.json](../schemas/blender-package.schema.json).
- `p4kx character package <archive> --out <folder>` writes a Character as a Blender package: the
  head of a CHF file (`--chf`), armour and clothing (`--piece`, `--loadout`) on the default body
  (`--body`, optional when the Loadout has a head), as one skinned GLB with the skeleton, the
  head's expressions as morph targets and the animations (`--anim`, `--anim-from`,
  `--anim-folder`), listed in `scene.json`; `--materials` and `--texture-mip` pick plain colours or
  textures. A `--loadout` of a story character such as Miles Eckhart exports his outfit and his
  own head, hair and eyes; with `--chf` the CHF's head is kept instead. The Blender add-on imports
  it as an armature with shape keys and actions.
- `p4kx interior glb <archive> <container>... --out <file>` exports the Interior of a socpak
  container, a ship's rooms or a room on its own, as one GLB: a node per container, per streamed
  `.soc` file and per placed model, containers nested in it at their position and rotation.
  `--lod <n>`, `--materials colours` and `--no-vertex-colours` apply to every model. Containers nested in themselves,
  and containers, files and models that cannot be resolved, are listed; with `--json` the report
  has the shape described in [docs/cli-json.md](cli-json.md).
- `p4kx interior package <archive> <container>... --out <folder>` writes the Interior of a socpak
  container on its own as a Blender package: the same container, object and light nodes, props
  and lights as `entity package --interior`, without an Entity around them. `--lod`,
  `--texture-mip` and `--no-vertex-colours` work as for `entity package`; the folder is written
  beside the target and moved into place when complete. The Blender add-on imports it.
- `p4kx prefab list <archive> [<library>]` lists the prefab libraries below `Data\Prefabs` or the
  prefabs of one; `p4kx prefab dump <archive> <library> <prefab>` prints a prefab with its nested
  prefabs expanded: every object with type, class, geometry file and its transform to the prefab's
  space, the parents' rotation applied. A prefab that is missing, contains itself or is nested more
  than 16 deep stays unexpanded and is listed; `--json` has the shape in
  [docs/cli-json.md](cli-json.md).
- `p4kx armour glb <archive> --piece <entity> --body male|female --out <file>` exports pieces of
  armour and clothing worn on the default body of a variant as one skinned GLB: the body's
  skeleton once as bone nodes, one skin over them, the skin of the body and, for every `--piece`
  (repeatable; class name, GUID or record path), the geometry its Record names for the variant's
  tag, with its bones matched to the skeleton by name. `--loadout <loadout>` takes the pieces from
  a Loadout instead: a file on disk of that name is tried first and read as an XML Loadout,
  otherwise an Entity's default Loadout by class name, GUID or record path, or an XML Loadout
  (`Scripts/Loadouts/…xml`) by its path in the Archive; its armour and clothing (`Char_Armor_*`,
  `Char_Clothing_*`) are worn. A `--piece` then takes the place of the Loadout's pieces of the
  same item type, or is added. An Entity whose default Loadout is an outfit, an NPC such as Miles
  Eckhart, gives its base Loadout and its clothing; its `Char_Head` is worn with its own skins
  instead of the generic head and decides the body, so `--body` is optional (it overrides).
  `--materials textures` embeds the textures as PNG at the mip `--texture-mip <n>` picks (default
  `colours`), `--anim <name>` (or `all`) and `--anim-from <entry>` add animations, looked for first
  in the character's own folder `Animations/pu/<body>/<name>`, and `--anim-folder <folder>`
  names the only folders to look in. A story character's own head that has a `.RigLogic` file
  (ADR-0281) gets its expressions as morph targets, read from the blendshapes of its geometry at
  LOD 0 and carried to the vertices of a lower level (ADR-0293): all of them, `--expression <name>` (repeatable) for some, `--no-expressions` for none;
  its facial joints take the poses of its skeleton file and the face animations of its folder
  move them. The zones of the body that the worn clothing covers
  (`SCItemClothingParams`, ADR-0140) are left out of the body skin, as the game does not draw
  them, and listed as `body_zones_hidden`; with `--materials textures`, layered materials such as the `LayerBlend_V2`
  clothing are drawn from the diffuse and normal maps of their layers blended by the blend mask
  (ADR-0147), and those whose layers have no diffuse map are drawn in their layer colours and
  listed as `layers_without_diffuse`. A piece with no geometry for the variant, one that cannot
  be resolved, or whose model is missing or unreadable, is skipped and listed; with `--json` the report has
  the shape described in [docs/cli-json.md](cli-json.md). `p4kx armour bodies` lists the
  variants.
- `p4kx wwise resolve <archive> <trigger>` lists every sound variant of an audio trigger with
  its soundbank, WEM ID, source and container path, e.g. `Play_QDrive_Spool in SSQD_QDrive.bnk:
  WEM 98393519 embedded, not located; event 2178236557 > action 98720655 > blend 228967944 > …`;
  broken links are listed with the object where the chain breaks, and `--json` gives the shape in
  [docs/cli-json.md](cli-json.md). A streamed WEM is located as its loose WEM Entry, and
  localised sounds are resolved for `--lang <language>` with `--fallback-lang` (`English(US)` by
  default), e.g. `WEM 8000 streamed localised German, at Data\Sounds\wwise\Media\German\8000.wem`.
- `p4kx wwise events <archive> [--search <text>]` searches the audio triggers and the Wwise events
  they post by name (a substring or a glob such as `Play_*_Spool`, ignoring case) and lists each
  trigger with its events, their soundbanks and the Entities that name it, in definition order;
  `--limit` and `--offset` page through large results, `--entities` bounds the Entities listed
  per trigger, `--no-entities` skips the DataForge (when it cannot be read otherwise, the command
  warns and exits with 1), and `--json` gives the shape in
  [docs/cli-json.md](cli-json.md).
- `p4kx entity audio <archive> <entity>` lists the audio triggers an Entity names, each with the
  Wwise events of its ATL definition and the soundbanks holding them, e.g.
  `Play_QDrive_Spool: Play_QDrive_Spool in SSQD_QDrive.bnk`; triggers that reach no soundbank are
  listed with the reason, and `--json` gives the shape in [docs/cli-json.md](cli-json.md);
  `--sounds` lists the sounds of every trigger with their source and location, in `--lang`.
- `p4kx wwise extract <archive> <trigger> --out <folder>` writes the sounds of an audio trigger as
  `<trigger>[_<WEM ID>].ogg|wav` (`--format ogg|wav|raw`, `--conflict skip|overwrite|if-changed`,
  `--lang`); a sound that cannot be decoded is reported and written as `.wem`. Names follow the
  portable file-name rule of ADR-0248 (forbidden characters, `#` and a leading space become `%`
  and two hex digits, an empty name becomes `%`, a name taken under the case fold of the rule gets
  `_2`, `_3`, …), and a sound whose path exceeds its limit is reported as failed while the others
  are written. `--txtp` also writes one vgmstream `<event>.txtp` text file per Wwise event of the
  trigger next to the sounds (ADR-0254); P4KX writes the text only and does not run vgmstream.
  `p4kx entity audio --extract <folder>` does the same for every trigger of an Entity.
- `p4kx loc languages <archive>` lists the languages the Archive holds a `global.ini` for, with
  their codes (`en`, `de`, `fr`, `it`, `es-ES`, `es-419`, `pt-BR`, `ja`, `ko`, `zh-Hans`,
  `zh-Hant`); `p4kx loc get <archive> <key> [--lang <language>] [--fallback <language>]` prints the
  text of a localisation key, with or without the leading `@`, ignoring case, in `--lang` (English
  by default) and otherwise in `--fallback`, with `\n` decoded; a key with no text in either is
  reported as missing and exits with code 2, a placeholder key such as `@LOC_EMPTY` prints
  nothing and succeeds. `p4kx loc export <archive> [--lang <language>,…] [--format json|csv]
  [--out <file>]` writes every key with its text per language, sorted by key. A `--lang` is a
  code, a folder name such as `german_(germany)` or a name such as `german` where one language has
  it. UTF-8 with or without a byte order mark and UTF-16 files are read; invalid bytes become
  U+FFFD and lines without `=` are skipped, and a `note:` line on stderr names each such Entry
  (also for `catalog`). `loc export --format csv` writes a UTF-8 byte order mark and CRLF line
  ends. `--fallback en` is the usual choice; there is no default. `--json` gives the shapes in [docs/cli-json.md](cli-json.md).
- `p4kx actionmap export <archive> [--format json|csv] [--csv-raw] [--language <language>]
  [--out <file>]` writes the action maps of `Data\Libs\Config\defaultProfile.xml`: every action with the
  localised label of its key and its default bindings for the keyboard, mouse, gamepad and
  joystick. The CSV has one row per action and a cell per device, bindings joined with ` | `; the
  JSON lists every binding with its device, input and activation mode ([ADR-0180](adr/0180-the-action-map-export-lists-every-default-binding-of-the-default-profile.md)).
  A label whose key has no text in `--language` (English by default) stays empty.
- `p4kx catalog <archive> --out <file> [--filter <filter>] [--exported-at <seconds>]` writes the
  Basetool catalog for the basetool's P4K import ([docs/basetool-catalog.md](basetool-catalog.md)):
  schema version (2), Channel and Build, completeness per kind and the kinds of the catalog:
  manufacturers, items (with their item class), ships and ground vehicles, commodities, crafting
  blueprints, loadouts, paints, shops, the starmap, contracts (with their ship spawns and location
  queries), factions and reputation, mining,
  refining and loot, each ordered by GUID, with test, template, dummy,
  debug, deprecated and dev Records left out and counted as skipped, and names and descriptions
  resolved in English and German. `--filter` marks the catalog partial. It prints the counts per
  kind, the item classes read and the unresolved texts per language and warns when the catalog
  nears or exceeds the 64 MB upload limit. An `--out` name ending in `.zst` writes the catalog as
  one zstd frame (level 19, content size declared, plain `zstd -d` decodes it), a fraction of the size, which the Basetool accepts as
  it does plain JSON. A selected Record that cannot be read is left out,
  listed on stderr with its GUID and reason and counted as failed; the catalog is still written
  and the exit code is 1. `p4kx catalog schema` prints the catalog's JSON Schema.
- `p4kx ships <archive> --out <file> [--format json|ndjson] [--filter <filter>]
  [--exported-at <seconds>]` writes the base data of every ship
  ([docs/ship-base-data.md](ship-base-data.md)): hull mass and hit points summed from the
  vehicle implementation XML with its Modification applied, the cargo capacity of the cargo grids
  of the default Loadout, `AttachDef.Size` and the Loadout as class names per port path, each
  value with the file and field it was read from. Docked vehicles are marked and not counted. A
  ship that cannot be read is listed on stderr and the exit code is 1.
- `p4kx coverage <archive> [--format table|json]` checks the game data a companion app reads
  ([docs/coverage.md](coverage.md)) against the Archive: whether each listed Record type,
  `EntityClassDefinition` component and vehicle folder exists, how many it has and whether it
  reads, exports and converts. Exit code 1 when something listed fails.
- `p4kx channels` lists the installed Channels with Build version, source and Archive path. It
  looks below `C:\Program Files\Roberts Space Industries\StarCitizen`, below the library folders
  named in the RSI Launcher log (`%APPDATA%\rsilauncher\logs`, see
  [ADR-0005](adr/0005-library-folders-from-the-launcher-log.md)) and in manually added
  folders and Archives. `p4kx channels add <path>` and `p4kx channels remove <path>` manage the
  manual entries in `settings.toml` in `%APPDATA%\P4KX` (the platform config directory
  elsewhere). `--config-dir`, `--launcher-dir` and `--default-library` (or `P4KX_CONFIG_DIR`,
  `P4KX_LAUNCHER_DIR`, `P4KX_DEFAULT_LIBRARY`) override these locations. Launcher credential
  files and the launcher's settings store are never opened.
- `p4kx cryxml <file> [--out <file>]` converts a CryXml file on disk, e.g. an Entry extracted
  earlier, to UTF-8 XML on stdout or in `--out`. All three signatures (`CryXmlB`, `CryXml`,
  `CRY3SDK`) and both byte orders are read; the XML is indented by two spaces with LF line
  endings, and the same input always gives the same bytes. The output is always well-formed:
  characters a name cannot hold, `:` included, become `_`, a repeated attribute name gets the
  first free suffix `_2`, `_3`, …, and bytes that are not UTF-8 read as `?`. A file that already
  is text XML is written unchanged. A truncated or corrupt file is an error and writes nothing.
- `p4kx texture info <archive> <entry>` prints the format, size, depth, mip count, array size,
  cubemap faces and number of mip parts of a DDS texture; `p4kx texture merge <archive> <entry>
  --out <file>` merges the header Entry and its mip parts `<entry>.1` … `<entry>.N` into one
  standard DDS file that common tools open. Both also take a header file on disk instead of an
  Archive and an Entry, with its parts next to it. Legacy and DX10 headers are read; a missing,
  truncated or oversized part is named in the error and nothing is written. A split cubemap or
  texture array, whose parts hold one mip of every face, merges face by face into a valid
  cubemap or array. `info` also names the alpha or smoothness tail `<entry>.a` with its parts
  `<entry>.1a` … `<entry>.Na`; `merge <archive> <entry>.a` merges the tail on its own.
- `p4kx texture png <archive> <entry> --out <file>` decodes the largest mip of a texture and
  writes it as an 8-bit RGBA PNG; `--mip <level>` picks a smaller one, 0 being the largest. It
  takes a header file on disk like `info` and `merge`. BC1 to BC7 and the uncompressed formats in
  use decode with pure-Rust decoders; any other format fails with a message naming it and exit
  code 1. An alpha or smoothness tail is decoded and merged into the alpha channel. A cubemap is
  written as six face files `<stem>_posx.png` … `<stem>_negz.png` next to `<file>`, or with
  `--cross` as one horizontal cross in `<file>`. A texture array is written as one file per
  element `<stem>_layer0.png` …, a cubemap array per element and face `<stem>_layer0_posx.png` …
  (with `--cross` one cross per element) and a volume as one file per slice of the mip
  `<stem>_slice0.png` …; the numbers are padded to the width of the last. A high-range BC6H or
  floating-point texture is written as OpenEXR
  with 32-bit float RGB instead, with the extension `.exr`, so its range is not clipped;
  `--tone-map` writes an 8-bit PNG instead, Reinhard tone-mapped and sRGB-encoded (ADR-0010). The
  message names both the requested and the written files, and `--tone-map` on any other texture
  only prints a note.
- `p4kx compare <old> <new> [--entries] [--records] [--filter <filter>] [--report <file>]`
  compares two Archives, e.g. an older against a newer Build. The Entry level joins both central directories
  by path (ignoring case and separator kind) and lists Entries as added, removed, or changed when
  their size or CRC-32C differs; nothing is decompressed, so it finishes in seconds. The Record
  level joins the Records of both DataForges by GUID and lists them as added, removed, moved (same
  GUID at another record path) or changed, with every changed scalar field and its old and new
  value; enum fields show the option names, and a value that cannot be read is a change with the
  reason. A Record that cannot be read at all is listed as unreadable with the reason, and the
  other Records are still compared. It prints both sides (Channel and Build) and the counts per
  change kind, for Entries also per top-level directory. `--filter` selects Entries by path and Records by record path
  with the rules of `search`; `--report` writes the full Comparison as JSON (see
  [docs/cli-json.md](cli-json.md#p4kx-compare-old-new---json)). Every level is compared when
  none is given; the Record level needs a DataForge in both Archives (exit code 5 otherwise). The
  exit code is 0 without and 1 with differences.
- `p4kx compare <old> <new> --schema` compares the schema of both DataForges instead of their
  contents: the structs, properties (data type, conversion type, target), enums and enum options
  added, removed or changed, each struct and enum with the number of Records it concerns on both
  sides. `--json` prints the report (see
  [docs/cli-json.md](cli-json.md#p4kx-compare-old-new---schema---json)); the Comparison tab
  of the window shows it in the Schema list once the Record level was compared. The exit code is 0
  without and 1 with differences.
- `p4kx compare <old> <new> --diff <entry>` prints the line diff of one Entry between both Archives
  as a unified diff: CryXml is converted to XML first, UTF-16 with a byte order mark is decoded,
  other text and text XML are compared byte for byte, and an Entry on one side only is diffed
  against an empty side. A binary Entry, or one larger than 128 MiB or 4 194 304 lines, gets a note
  saying whether it changed. `--json` prints the hunks as JSON (see
  [docs/cli-json.md](cli-json.md#p4kx-compare-old-new---diff-entry---json)). The exit code is
  0 when the Entry is unchanged and 1 when it was added, removed or changed, also when its lines are
  equal after conversion.
- `p4kx audio info <archive> <entry>` prints the codec, channels, sample rate and duration of a
  Wwise WEM sound; `p4kx audio wav <archive> <entry> --out <file>` writes a PCM, IMA ADPCM,
  PTADPCM, Wwise Vorbis or Opus WEM as a 16-bit WAV file, and
  `p4kx audio ogg <archive> <entry> --out <file>` rebuilds a Wwise Vorbis WEM as a standard Ogg
  Vorbis file and an Opus WEM as a standard Ogg Opus file, both without re-encoding. All three
  also take a WEM file on disk instead of an Archive
  and an Entry. PCM, IMA ADPCM, Wwise Vorbis, Opus, PTADPCM and the console codecs are
  detected; a conversion the codec does not support yet fails with an error naming it, and a
  corrupt or truncated WEM fails with the reason.
- `p4kx bnk list <archive> <entry>` prints the ID, codec and duration of every WEM a Wwise BNK
  soundbank embeds; `p4kx bnk extract <archive> <entry> --out <folder>` writes them as
  `<id>.wem`, with `--format wav` converted to WAV, or with `--format ogg` rebuilt as Ogg Vorbis or Ogg
  Opus.
  Both also take a soundbank file on disk. A corrupt soundbank fails with an error naming the
  section.
- `p4kx bnk objects <archive> <entry> [--type <type>] [--id <id>] [--limit <n>] [--offset <n>]`
  prints the objects of the `HIRC` section in bank order: type, ID, parent and children, and for
  music objects a segment's duration, markers and stingers, a track's clips, a switch container's
  groups and targets and a playlist's items with their nesting and loop counts. `--type` keeps
  one object type, `--id` one object (decimal or `0x` hexadecimal); `--limit` (default 100) and
  `--offset` page the result, and the summary line gives the next offset. Objects of a type the
  reader does not decode, of a bank version it does not support or with a malformed body are
  marked and counted in the summary line, not skipped. It prints the structure only; `p4kx wwise
  resolve` resolves a trigger to its sounds.
- `p4kx ui assets <archive> <entry>` lists what a GFx or SWF UI movie holds: its header, every
  tag code with its count (unknown tags kept and counted), and the shapes, morph shapes, bitmaps
  (lossless, JPEG, PNG, GFx external and sub-images), fonts and sprites with their IDs and export
  names; a tag that cannot be read is listed instead of failing the movie, with exit code 1.
  `p4kx ui extract <archive> <entry> --out <folder>` writes every bitmap as `bitmap_<id>.png`
  and every shape as `shape_<id>.json` (morph shapes as `morph_shape_<id>.json`) in twips as the
  movie stores them; an external image is decoded from the DDS texture it names next to the
  movie (on disk only inside the movie's `Data` folder), a sub-image is cut from its bitmap.
  Both also take a movie file on disk, compressed (`CWS`, `CFX`, `ZWS`) or not (`FWS`,
  `GFX`).
- `p4kx ui render <archive> <entry> --out <file.png> [--size <W>x<H>|<W>] [--background
  transparent|movie|#RRGGBB[AA]]` renders the first frame of a UI movie, with its sprites, as a
  PNG: shapes with solid, gradient and bitmap fills (external images included) and strokes, with
  the transforms, colour transforms and masks of their placements, and morph shapes at their
  ratio. `--size` with both sides fits the stage centred into them, with one side keeps its
  aspect ratio; the default is the stage size. Static texts and text fields are drawn from the glyph
  outlines of the fonts of the movie, with their size, colour and alignment; a character whose font
  or glyph is missing is drawn as an empty box and listed, since no system font is used. Buttons and
  videos are skipped, filters and blend modes are not applied, and every element left out is listed
  (exit code 1). A movie is
  untrusted: placements, paths, conversions, masks, the canvas and the area the fills cover are
  bounded, and what lies beyond a limit is listed instead of drawn.
- `p4kx ui render … --lang <language>` resolves the localisation keys (`@key`, with `,P=`
  parameters) of text fields in the `global.ini` of that language of the Archive; it needs an ENTRY.
  A key the language does not hold is drawn as it is and listed. The language is also the
  localisation folder in which imported fonts are looked for after the folders of the UI.
- `p4kx ui render … --svg` writes the frame as an SVG document instead, which scales without loss:
  shapes are written once and placed with their transforms, gradients, strokes, colour transforms
  and masks become SVG elements, and bitmaps are embedded as PNG data. The same input gives the
  same bytes, `--size` and `--background` set the size and the backdrop of the document, and the
  same limits end the frame at the same element as for the PNG. A bitmap fill that does not repeat
  is drawn once without continuing its edge pixels, and a stroke painted with a non-repeating
  bitmap tiles it. Text is written as paths, never as `<text>` elements. A document is at most 1 GiB;
  a larger one is an error and no file is left.
- `p4kx ui hologram <archive> <entity> --out <file.png> [--size <W>x<H>|<side>] [--background
  <colour>] [--view-angle <degrees>] [--elevation <degrees>] [--fov <degrees>] [--distance-scaler
  <factor>] [--tint <colour>] [--opacity <0..1>] [--glow <0..4>] [--lod <n>] [--software]
  [--exclude <port>] [--replace <port>=<entity>]` draws the hologram of a vehicle: the Assembly of
  the Entity with its Loadout, as a see-through tinted shell, every layer drawn, that glows along its edges, seen
  through the narrow lens of the game's `UIHoloVehicle_Config` Record (field of view, view angle
  and distance factor, 10, 60 and 1.5 in the current game, each overridable). The picture is
  drawn offscreen without a window, by the same wgpu renderer as the 3D viewer, on the graphics
  card or with `--software` on the software adapter (WARP, lavapipe), which gives a picture that
  does not depend on the graphics card. The background is `transparent` or `#RRGGBB[AA]`; the PNG
  has straight alpha. Textures and interiors are not drawn.
- `p4kx ui screens <archive> <entity>` lists the `BuildingBlocks` screens of an Entity and of
  everything in its default Loadout, e.g. the MFDs, the radar and the HUD of a ship: per screen the
  port, the Entity, the canvas Record and its size, the canvases, textures, vector graphics, movies
  and localisation keys it uses, and its widgets by element type with whether they are drawn; the
  UI movies the Entities name are listed after them. `--json` carries the same.
- `p4kx ui render <archive> --screen <canvas> --out <file.png> [--svg] [--size …] [--background …]
  [--brand <code>] [--var <name>=<value>]…` renders such a canvas, named by record path or GUID,
  as a still through the
  GFx renderer: the layout of its widgets (sizes, margins, padding, flex containers, raw units)
  is computed from the Records and the boxes, circles and polygons are drawn in the colours of
  the style cascade (palette slots of the default and manufacturer style, style entries with type,
  tag, parent and ancestor selectors at rest). Texts, textures, vector graphics, lists, clones and
  every other widget type without a renderer are left out and listed, with the features the layout
  approximates (exit code 1); no ActionScript is run. Every game variable is zero, so the MFD of a
  cold ship is off; `--var powerstate=1` sets one (repeatable), and the report lists the variables
  in force and those the screen does not read. `--var CANVAS::NAME=VALUE` limits a setting to the
  canvases of that Record name, which are not told apart when several share it; `a::b::c` is
  refused (ADR-0277). The canvas a canvas widget holds is the one a style
  entry, a boolean binding or a game variable picks (ADR-0277, ADR-0306): the powered MFD
  `m_mfd_screen` shows the page named by `--var landscapecanvasguid=<record path or GUID>` (the
  `mc_s_*_master` Records below `mfdscreens/mc_mfdcomponents/screens`, among them `power`,
  `weapons`, `scanning`, `diag`, `shields`, `config`, `comms` and `self`), `portraitcanvasguid` in the portrait view, and its tab bar
  names the page by `--var mfdviewname=<name or @key>`, an `@key` being looked up in the
  language (`--lang`). The UI tab keeps its state and typed
  variables across sessions.
- `p4kx chf to-json <file> [--out <file>] [--archive <Data.p4k>]` converts a CHF character file, e.g. one saved by the
  character customiser, to indented JSON on stdout or in `--out`: its inner versions (7 and 8
  are read), body type, face DNA with every face part named, item ports and material choices.
  `p4kx chf from-json <file> --out <file>` writes the JSON back as a CHF file: unchanged, it
  gives the original file byte for byte; edited, a new file with a correct CRC-32C. The tool never
  writes into a Star Citizen installation, and that includes the `user` folder of a Channel, so
  `--out` cannot be `LIVE\user\client\0\CustomCharacters` (exit code 2): write the file elsewhere
  and copy it in by hand. A file with
  a wrong size, magic or CRC-32C, an unknown version or a corrupt body fails with the reason
  (exit code 3), as does invalid or unwritable JSON (exit code 2). The JSON is described in
  [docs/cli-json.md](cli-json.md#the-chf-json); which hash function names which field is in
  [docs/chf-hashes.md](chf-hashes.md). A hash that a known name matches gets that name in a
  `*_label` key beside it (`from-json` ignores the keys), `--archive` adds the names the Archive
  knows, and the hashes without a name are counted.
- `p4kx chf diff <a.chf> <b.chf> [--format text|json] [--archive <Data.p4k>]` lists the fields in
  which two CHF files differ, by path with the old and the new value: header, DNA by face part and
  blend, item port tree, and materials down to textures, float and colour parameters. Hashes that
  a known name matches are shown as names; the raw path keeps them. It finds which field a control
  of the character customiser changes. Exit code 1 when the files differ, 0 when not; the JSON is
  described in [docs/cli-json.md](cli-json.md#p4kx-chf-diff-achf-bchf---json).
- `p4kx chf glb <archive> <file> --out <file>` exports the head of a CHF character as one skinned
  GLB: the head skin of the character's head rig, its vertices moved by the character's face DNA
  into the character's neutral face (the neutral morphs of the DNA rig beside the skin, mixed by
  the DNA's blends: per face part up to the four strongest heads), worn on the default body of the
  character's body variant. The teeth and what the character wears on its head are included: the
  eyes, hair, brows, lashes and beard, with the materials the character chose (its head and body material,
  skin tone, iris colour, hair melanin and dye) and their textures (`--materials colours` leaves
  the textures out, `--texture-mip N` picks their size). The head rig's expressions (about 400 of
  its 495 blendshapes, e.g. `CTL_expressions blinkR`) are written as named morph targets of the
  head, mixed by the DNA so that they move the character's own face and stored sparse;
  `--expression NAME` (repeatable) limits them, `--no-expressions` leaves them out. A lower level carries the
  targets of LOD 0 to its own vertices (ADR-0293). Hair, brows, lashes and beard get morph targets of their own that follow the head's
  (on the first mesh of each, at every LOD, ADR-0305; they multiply the size of the GLB by about 3.5 at
  every level, `--no-expressions` writes the small file); eyes and teeth have none. `--lod N` picks
  the level of detail of head and body, `--no-vertex-colours` leaves the vertex colours out (they
  are written as `COLOR_0` only on primitives whose colour they are, ADR-0318). A head
  that cannot be shaped (its rig is missing, belongs to another rig or does not match the skin) is
  listed and left out of the GLB, which is still written, and the exit code is 1; an item or a
  material the Archive does not have is listed, and a character without face DNA exits with 2. The
  report is described in [docs/cli-json.md](cli-json.md).
- `p4kx socpak list <archive> <container…>` lists the Entries of a socpak container in the
  columns of `list`, read from the Archive without extracting the container first; each further
  path names a container inside the one before. `p4kx socpak extract <archive> <container…>
  [--filter <filter>] [--exclude <filter>] [--out <folder>] [--conflict <policy>] [--convert]`
  writes its Entries, or those the Filter matches, without those `--exclude` matches, with the
  rules of `extract`; `--exclude` alone leaves out of all Entries, and a pair that leaves nothing is
  invalid input. `p4kx socpak verify <archive> <container…> [--filter <filter>] [--exclude
  <filter>]` selects the same way. `if-changed` compares with the CRC-32 containers store.
  Containers nest at most 8 deep; a container whose Entries overlap, whose stored sizes disagree or
  which declares more than 2048 times its stored size for an Entry is refused, as are compressed
  containers that together decode to more than 512 MiB along one chain. A container that cannot be
  opened fails with exit code 3 and an error naming its Entry.
- `p4kx chunks list <archive> <entry>` prints the chunk table of a CryEngine chunk file (`.cgf`,
  `.cga`, `.skin`, `.chr`, their geometry companions, `.caf`, `.dba`) in the `#ivo` or `CrCh`
  container: index, type, type ID, version, chunk ID, byte order, offset and size, with unknown
  types marked; `p4kx chunks dump <archive> <entry> --chunk <index> --out <file>` writes one
  chunk's bytes unchanged, or with `--text` as text for the `CryXmlB`, `Json` and `ModelPaths`
  chunks of a `.soc` file. Both also take a chunk file on disk, e.g. an extracted `.soc`.
- `p4kx model info <archive> <entry>` shows an `#ivo` model (`.cgf` or `.skin` with its geometry
  companion `.cgfm` or `.skinm`, or a skeleton file `.chr`): its nodes with their hierarchy, its
  LODs and meshes with vertex and triangle counts, the material library and material IDs, its bones
  with parent and bind position, and every chunk with whether it was read, skipped or unknown, and
  the materials of its `.mtl` file;
  `p4kx model glb <archive> <entry> --out <file> [--lod <n>] [--materials colours|textures]
  [--texture-mip <n>] [--max-influences <n>]` writes it as one glTF 2.0 binary file (GLB), Y up and
  in metres (`--max-influences 4` keeps four bones per vertex, for viewers that read one set), with the
  normals, tangents and vertex
  colours the meshes have, as `COLOR_0` on the primitives whose colour they are (glass with
  `PER_VERTEX_TINT`; ADR-0318), `--no-vertex-colours` leaves
  the colours out, and lists the unknown
  chunks it skipped. `--lod` picks the level of detail, 0 by default; the LODs come from the
  companions `<name>_lod<n>.<extension>m`, and a level the model lacks exits with code 2.
  `--materials colours` (the default) gives every sub-material of the model's `.mtl` file a glTF
  material with base colour, opacity, roughness, metalness and emissive colour and no textures;
  submeshes whose material draws nothing (collision and helper geometry) are left out, and a
  missing or unreadable `.mtl` file is reported while the model exports with a grey default
  material. `--materials textures` also embeds the textures of the materials as PNG images: the
  diffuse map as base colour, the normal map (green flipped to glTF's up, the blue channel
  reconstructed), a metallic-roughness image from the normal map's smoothness and the specular
  map, shared images once; `--texture-mip` picks the mip level, 0 (the largest) by default. A
  texture that is missing or cannot be decoded leaves the plain colour and is listed. A skin
  (`.skin`) is written with its skeleton and bone weights as a glTF skin (up to eight bones per
  vertex, `JOINTS_0`/`WEIGHTS_0` and `_1`), a `.chr` skeleton file as bone nodes. Both also take a
  model file on disk with its companions next to it, which has no textures to read.
- Encrypted Entries need the Archive key, which you enter yourself; the tool does not contain it
  ([ADR-0027](adr/0027-archive-key-comes-from-the-user.md)). Without it an Archive still
  opens, lists, searches and compares, and reads every unencrypted Entry; an encrypted Entry fails
  with a message that says the key is missing, or that it does not fit. `--key <hex>` (32
  hexadecimal digits) or the environment variable `P4KX_ARCHIVE_KEY` gives it for one run, with
  the option before the variable before the stored key. `p4kx key set [<hex>] [--archive <archive>]`
  stores it in your profile (`%APPDATA%\P4KX\archive.key` on Windows, the platform
  config directory elsewhere, readable by your user alone where the platform allows it), after
  checking it against an encrypted Entry of the Archive, an installed Channel when none is named;
  without `<hex>` it reads the first line of stdin, which keeps the key out of the process list
  and the shell history. `p4kx key show` shows the stored key and the one in use with all but
  the last four digits hidden, `p4kx key remove` deletes the stored key. The key never appears in
  logs, error messages or JSON. `--key-file` (or `P4KX_KEY_FILE`) names another file for the stored
  key, for the command line, the window and the MCP server alike.
- `--recover` on any command rebuilds an Archive whose end of central directory is missing or
  whose central directory is damaged, by scanning its last 4 GiB for directory records. Entries
  whose records are lost are missing, and a warning says the Archive was recovered. Without the
  flag such an Archive does not open.
- `--json` on any command prints its result as one line of JSON instead of text; the shapes are
  documented in [docs/cli-json.md](cli-json.md).

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success: every requested Entry was extracted or already existed; `verify`: every Entry matched its checksum; `compare`: no differences |
| 1 | Partial failure: some Entries or Records failed or were cancelled, the data of an Entry does not match its checksum (`verify`, `extract --verify`), the target or the settings cannot be written, a texture, sound, soundbank or model cannot be read, converted or written, a tag or definition of a UI movie cannot be read or written, `ui render` leaves an element out (skipped, drawn without an effect, or beyond a limit; for `--screen` also a widget type without a renderer or an approximated layout), `chf glb` or `character package` could not shape the head by its DNA (the output is written without the head), `wwise events` could not look up the Entities (the triggers are still listed), or `coverage` finds a listed item failing; `compare`: the Archives or the `--diff` Entry differ; `chf diff`: the CHF files differ |
| 2 | Invalid input: bad arguments, an Entry path the Archive does not contain (for `compare --diff`: neither Archive contains), a Filter that matches no Entry or Record, a Channel path that does not exist or was never added, a texture, sound, soundbank, chunk file, UI movie, model or geometry companion file that does not exist, the geometry of the Entity that `entity glb` or `entity package` exports when it does not exist, a mip level, chunk or LOD the file does not have, an output that is one of its inputs or lies inside a Star Citizen installation (the tool never writes there), a package folder that holds something other than an earlier package, an invalid Query, an Entity that no Record or several Records match, a malformed `--exclude` or `--replace`, an audio trigger no ATL definition has, an audio or localisation language the Archive holds nothing for, a `ui render` canvas that is empty, larger than 16384 pixels a side or 2^25 pixels, or not named by exactly one canvas Record, `--json` without `--out` for `cryxml` or `chf to-json`, a catalog `--out` ending in `.gz`, CHF JSON that is invalid, has an unknown version or cannot be stored as CHF, and a CHF file for `chf glb` or `character package` that holds no face DNA or whose DNA belongs to an unknown head rig |
| 3 | The Archive, a socpak container in it or the input file cannot be read: missing, not an Archive, not CryXml, CHF, a chunk file or a UI movie, corrupt, a corrupt model or companion (for `entity glb` and `entity package` the geometry of the Entity itself, or its `.chrm` companion, that cannot be read), with a CHF CRC-32C mismatch or unknown CHF version, also a `compare --diff` Entry that cannot be read, does not hold its stated size or holds corrupt CryXml, a `global.ini` of `catalog` or `loc` that is over 64 MiB, holds more than 1,048,576 keys or cannot be read (invalid bytes and lines without `=` are no error), and a prefab library that cannot be read; a missing texture, sound, soundbank, chunk file, UI movie or model file is 2 |
| 4 | The Archive changed on disk while it was open, e.g. updated by the RSI Launcher; the Extraction stopped, and Entries written before may be stale |
| 5 | The DataForge cannot be read: the Archive holds none, its version is not supported or it is corrupt, or the Record of the Entity whose Loadout or audio is shown cannot be read |
| 6 | `compare`: the report or the output cannot be written |
| 7 | The Archive key is missing or wrong: an Entry is encrypted and no key was given with `--key`, `P4KX_ARCHIVE_KEY` or `p4kx key set`, or the key does not decrypt the Archive |
| 130 | Interrupted: a second Ctrl+C ended an Extraction at once; temporary `.p4kx-*.part` files may remain |

`p4kx --help` lists every case of each code.

## MCP server

`p4kx-mcp` is a read-only [Model Context Protocol](https://modelcontextprotocol.io) server over
stdio, so an AI client can look into the Archives. It is built with the official Rust MCP SDK
(`rmcp`) and speaks every protocol version the SDK negotiates, up to `2026-07-28`. Its tools never
write a file.

What the tools return is the content of the Star Citizen Archive. The AI client may forward it to
third parties, for example the cloud model provider behind the client. Game texts and media must
not be redistributed. You remain responsible for complying with the Star Citizen EULA and Terms of
Service.

A tool that reads an Archive of a test Channel (`PTU`, `EPTU`, `HOTFIX`, `TECH-PREVIEW`) adds
the test-build notice of [ADR-0038](adr/0038-test-build-archives-carry-a-notice.md) to its
result as one more text content block. The structured content and the output schemas do not
change. `p4kx-mcp --version` and `-V` show the legal notice.

| Tool | Result |
|------|--------|
| `list_channels` | The detected Channels with Archive path, source and Build, found like `p4kx channels` |
| `archive_info` | Channel, Build, number of Entries and encrypted Entries, sizes, `issue_count` and the first 50 `issues` (Entry path and defect) of the directory |
| `list_entries` | One folder of an Archive (`folder`, e.g. `Data/Libs`): its subfolders with their Entry count, then its Entries |
| `search_entries` | The Entries matching a Filter (`filter`), with the rules of `p4kx search` |
| `read_entry_text` | A range of an Entry as text (`path`, `offset`, `max_bytes`): CryXml as XML, UTF-16 by its byte order mark, otherwise UTF-8 |
| `read_entry_bytes` | A range of an Entry's data as base64 (`path`, `offset`, `max_bytes`) |
| `get_record` | A Record by `guid`, or every Record of a record `path`, as Record JSON |
| `search_records` | The Records whose record `path` and `type` name match Filters, in Record order |
| `list_structs` | The struct definitions matching a `name` Filter, with parent and declared fields (data type, array, target struct or enum) |
| `list_enums` | The enum definitions matching a `name` Filter, with their values |
| `run_query` | One page of a Query result (`query`): typed columns, rows as `p4kx dcb query --format json` writes them, failed Records, unresolved References |
| `list_chunks` | The chunks of a chunk file Entry (`path`) as `p4kx chunks list` shows them: index, type, type ID, known, version, chunk ID, byte order, offset and size; `type` (a name or `0x` ID) narrows it. No chunk data is returned |
| `get_model_info` | A model Entry (`path`) summarised as `p4kx model info --json`: geometry, material library and IDs, levels of detail, and the number of materials, nodes, meshes, bones, animations and chunks |
| `list_model_items` | One `section` of a model Entry, paged: `materials`, `nodes`, `meshes`, `bones`, `animations`, `unread_animations` or `chunks`, each item shaped as in `p4kx model info --json` |
| `search_audio_triggers` | The audio triggers whose name or Wwise event names match `search` (a substring or a glob), as `p4kx wwise events`: events, the soundbanks holding them, why a trigger leads to none, and, with `entities` above 0 (the default is 0, because the lookup reads every Entity), the Entities naming it |
| `get_entity_sounds` | The audio triggers an Entity (`entity`: GUID, record path or class name) names, each with its ATL definition, events and soundbanks, as `p4kx entity audio` |
| `resolve_audio_trigger` | The sound variants of a trigger (`trigger`, `lang`, `fallback_lang`) as `p4kx wwise resolve`: soundbank, WEM ID, source kind, where the WEM is and the object path; broken links with the first page |
| `list_soundbank_wems` | The WEMs a soundbank (`soundbank`) embeds, paged, as `p4kx bnk list`: ID, size, codec, channels, sample rate, frames, duration, prefetch. No audio |
| `list_soundbank_objects` | The `HIRC` objects of a soundbank (`soundbank`) in bank order, paged with a cursor, as `p4kx bnk objects --json` lists them: type, ID, parent, children, how far the reader decodes the object, and the fields of sounds, actions, containers and music objects. `type` (as `p4kx bnk objects --type`) and `id` narrow it; `matched` counts the objects that pass, `total` and `layouts` the whole bank. Objects the reader does not decode are listed and counted. No audio |
| `get_wem_info` | One WEM by `wem_id` or loose Entry `path`: every place that holds it and its codec, channels, sample rate, duration and loop. No audio |
| `get_audio_excerpt` | A short decoded excerpt of one WEM (`wem_id` or `path`, `start_seconds`, `duration_seconds`) as an `audio/wav` content block next to its metadata; at most 10 seconds and 1 MiB, see [ADR-0131](adr/0131-the-mcp-audio-excerpt-is-a-bounded-wav-and-the-only-tool-that-returns-audio.md) |
| `image_preview` | A DDS texture (`path`) as one PNG image content block of at most 1 MiB, with its format, size and mip count as structured content: the mip chosen by `mip` or the largest that fits `max_side` (512 by default, at most 2048), a cubemap as a cross or one `face`, an array or volume as a sheet |
| `mtl_summary` | A `.mtl` Entry (`path`), paged by sub-material: name, shader, surface type, flags, colours, opacity, glow, texture slots, shader parameters and layers |
| `entity_loadout` | The default Loadout of an Entity (`entity`: class name, record path or GUID) as a list of ports, parents first: path, depth, source, what the port takes, the Entity attached and for each unresolved port the reason; each Entity states its `issues` and, only when it has any, its `notes` (what the Archive cannot decide, such as an `assortment_requirement`, as in the CLI JSON); `max_depth` (16 by default, at most 32) and a cursor bound it |

`list_entries`, `search_entries` (with `container`), `read_entry_text`, `read_entry_bytes`,
`image_preview` and `mtl_summary` open socpak containers with `::`: `outer.socpak::inner.socpak::path`
reads an Entry of a container inside a container, and `outer.socpak::` lists a container's top level
([ADR-0129](adr/0129-container-paths-in-mcp-tools-join-containers-with-a-double-colon.md)).
The paths a listing returns carry their containers and work as they are in the other tools.

Every tool except `list_channels` takes `archive`: a Channel name such as `LIVE` (ignoring case)
or the absolute path of a `.p4k` file on a local drive; the DataForge tools (`get_record`,
`search_records`, `list_structs`, `list_enums`, `run_query`) also take a `.dcb` DataForge file
such as `Game2.dcb` (ADR-0249). UNC, device, relative and `..` paths, a
`:` after the drive (an alternate data stream) and a path that resolves to a network drive or
device are refused. Without it the tools use the Archive given with `--archive` when the server
was started. Opened Archives stay open between calls and are opened again when their file
changes. Listings page: `limit` sets the page size (100 by default, at most 1000), and a result
with `next_cursor` has a next page that `cursor` asks for; a cursor only continues the listing
that returned it, on the same version of the Archive. The DataForge tools read the Archive's DataForge once, keep it
open with its indexes while the Archive stays open, and hold at most 256 KiB of items in one result:
a larger page ends early with `next_cursor`, and a single Record or row larger than that is an
error. `list_entries` lists folders as
[ADR-0023](adr/0023-entry-folders-fold-case-and-separators.md) defines them. `--config-dir`, `--launcher-dir` and
`--default-library` (or `P4KX_CONFIG_DIR`, `P4KX_LAUNCHER_DIR`, `P4KX_DEFAULT_LIBRARY`) work as
for `p4kx channels`.

The server reads encrypted Entries with the Archive key from its `--key <hex>` option, the
`P4KX_ARCHIVE_KEY` variable of its configuration or the key stored with `p4kx key set` or in the
window, in that order; the stored key is read on each call, so one stored while the server runs
applies to the next call. A tool that reads an encrypted Entry without a key fails with an error
that says how to give it. `--key-file` (or `P4KX_KEY_FILE`) names another file for the stored key.

The MSI installs `p4kx-mcp.exe` next to `p4kx.exe` in
`%LOCALAPPDATA%\Programs\P4KX`, and the portable ZIP holds it too. Add it to the AI
client with its full path. Claude Code:

```powershell
claude mcp add p4kx -- "$env:LOCALAPPDATA\Programs\P4KX\p4kx-mcp.exe" --archive LIVE
```

Claude Desktop and other clients that read an `mcpServers` file, with `<user>` your Windows user
name:

```json
{
  "mcpServers": {
    "p4kx": {
      "command": "C:\\Users\\<user>\\AppData\\Local\\Programs\\P4KX\\p4kx-mcp.exe",
      "args": ["--archive", "LIVE"]
    }
  }
}
```

## Related documents

- [README](../README.md): what P4KX is, install, quick start.
- [user-help.md](user-help.md): using the window.
- [CONTRIBUTING.md](../CONTRIBUTING.md): building, testing and the real-Archive suite.
