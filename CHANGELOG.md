# Changelog

Notable changes of P4KX, one section per Release. The section of a version becomes the
notes of its GitHub Release; the workspace version names the next Release, and `cargo xtask`
fails while its section is missing.

New changes are not written here but as fragments in `changelog.d/` (see its `README.md`), which
`cargo xtask changelog --release <version>` folds into the section of the Release together with
the entries under `## [Unreleased]`.

## [Unreleased]

## [0.4.2] - 2026-11-01

A 3D, character, audio and catalog update.
- **Exports:** ships, creatures and characters export more completely:
  - creatures are skinned, with opt-in animations (`--animations`);
  - story characters get their own heads, and hair follows the head's joints and expressions;
  - stencil decals take the livery's primary decal colour, and glass is see-through;
  - worn parts hide what they cover;
  - `--loadout-pick` chooses a loadout alternative.
- **Blender package:** format 1.8. It states the layer shares and the creature rigs.
- **Audio:** vgmstream `.txtp` files and the object hierarchy of a soundbank.
- **Extraction:**
  - `p4kx convert` converts a folder extracted earlier;
  - `--exclude` Filters;
  - file names that are valid on Linux and Windows;
  - DataForge files open without an Archive.
- **Basetool catalog:** the new kinds `contractTemplates`, `missionBrokerEntries`,
  `cargoManifests`, `contractNpcSpawns` and `contractEntitySpawns`, and the values of the contract
  properties. All are additive, so the catalog stays `schemaVersion` 2.
- **Graphics:** on Windows the window opens Direct3D 12 first and falls back to the other backends.

### Added

- Audio: `p4kx wwise extract --txtp` also writes one vgmstream `.txtp` text file per Wwise event of
  the trigger next to the sounds. It layers an Event's actions and blend containers, chains
  sequence containers, selects the first child of a random or switch container and carries the
  loop of each sound; an event with broken links is marked incomplete, and an event with a sound
  that has no whole file is listed instead. P4KX writes the text only; it does not run vgmstream.
- UI screens: `p4kx ui render --screen` takes `--var NAME=VALUE` (repeatable) to set game
  variables before the bindings are evaluated, e.g. `--var powerstate=1` for a ship that is
  powered on. The report lists the variables in force and the names no canvas of the screen
  reads. The UI tab offers the state "At rest" or "Powered on" and a field for more variables.
- Story characters: the head of a character with a `.RigLogic` file of its own, such as Miles
  Eckhart, now has expressions and a face animation. `p4kx armour glb --loadout` and
  `p4kx character package` write the blendshapes of the head's geometry as morph targets
  (`--expression NAME`, `--no-expressions`), place the facial joints by the head's own skeleton
  file and animate them from the face clips of the character's folder.
- `p4kx-character`: `RigLogic` reads the joint delta records of a rig (`joint_records`,
  `joint_step`, `FaceRig::from_logic`) and `SkinBlendshapes` reads the `BShapesGpu` chunk of a
  head. `RigLogic` no longer implements `Eq`, as it holds the scales of the records as `f64`.
- Assembly: the bone attachments of a character definition, such as the housing of a thruster, are
  drawn at their bone of the skeleton, offset by their position and rotation, as nodes below the
  part in the GLB, the Blender package and the viewer. Their vertices count against the budget.
  An attachment whose file is missing or whose bone is unknown is reported.
- Basetool catalog: the new kind `contractTemplates` lists the `ContractTemplate` Records that
  `contracts[].templateGuid` names: display flags, class settings, deadline, mission properties,
  ship spawn and location links, hauling orders, partial reward payouts, modifiers, owner, mission
  flow, comms and the objective tokens with their handlers, reward contributions, properties and
  texts (game texts, flagged `x-gameText`). A stored name the catalog does not know is `null` and
  counted in the new warning `unknown_template_values`; a template that nests its phases or
  conditions too deep is a failure of the build.
- Basetool catalog: the new kind `missionBrokerEntries` lists the `MissionBrokerEntry` Records, the
  offers of the mission board, with their texts (game texts, flagged `x-gameText`), links,
  availability, schedule, cooldowns, instance limits, reward, deadline, reputation requirements and
  results, mission properties, ship spawn and location links, hauling orders, objectives, partial
  payout, modifiers and mission flow. Contracts name their entry in the new field
  `brokerEntryGuid`. The modifiers `factionHostility`, `hostileMission`,
  `ignoreMissionPlayerCriminality` and `lawLicense` and the flow conditions `anyTokensState` and
  `onMissionStart` are variants of the contract templates too (`anyTokensState` occurs three times
  and `onMissionStart` once on LIVE). A modifier, mission flow condition or action of a struct the
  catalog does not know fails the Record, in a template as in a broker entry, and is listed in the
  failures of the build; the unions of `contractTemplates` changed before the kind was first
  released.
- Basetool catalog: the new kind `cargoManifests` lists the cargo manifests of the Archive (how the
  game fills the cargo grid of a ship it spawns), and every contract entry gains `haulingOrders`,
  what the contract has to haul: commodities in SCU, entities and mission items, with the
  alternatives of an `or`. Schema version 2 keeps its number (ADR-0265).
- Basetool catalog: the new kinds `contractNpcSpawns` and `contractEntitySpawns` list the distinct
  NPC and entity spawn descriptions of the contracts, templates and broker entries with every field
  the Archive states (groups, options, spawn settings, tags, inventory items, the embedded NPC names
  as game texts flagged `x-gameText`). Contracts, contract templates and mission broker entries link
  to them in the new fields `npcSpawns` and `entitySpawns`. The value of an objective property is
  now `npcSpawnDescriptions`, `entitySpawnDescriptions` or `organization` where it was `other`; a
  value of any other struct fails the Record and is listed in the failures of the build. The
  `other` variant of `contractTemplates` and `missionBrokerEntries` was not part of a release.
- Extraction: `p4kx extract --filter` and `p4kx search` take a repeatable `--exclude <FILTER>` that
  leaves out the Entries it matches, with the rules of the Filter, e.g. `--filter 'Data\Objects\*'
  --exclude '*_lod?.cgam'` for all models without their level-of-detail companions. A texture whose
  header is excluded is left out with its parts; with `--convert`, an excluded part of a texture
  that stays is still read to build the texture, and a note says so. A pair that selects nothing is
  invalid input and names the side that removed everything. The Extraction dialog has a **Leave
  out** field with one Filter per line. Filter text is limited to 1024 bytes and 32 wildcards.
- Entity export: the skeleton file a character definition (`.cdf`) names is checked. A missing or
  unreadable skeleton is listed as the notes `skeleton_missing` and `skeleton_unreadable` in
  `entity glb` and `entity package`; the skin is still written. A real-Archive census lists which
  Entities with a `.cdf`, `.chr` or `.skin` outside the human rigs draw.
- Soundbanks: `p4kx bnk objects` prints the object hierarchy of a bank in bank order (type, ID,
  parent, children and, for music objects, segment durations and markers, track clips and playlist
  items with their loop counts), filtered by `--type` or `--id` and paged with `--limit` and
  `--offset`; objects the reader cannot decode are listed and counted. `--json` gives the same
  data.
- DataForge: every `p4kx dcb` command, the DataForge tools of the MCP server and the window
  (Archive menu, **Open DataForge file…**) read a `Game2.dcb` file on disk in place of an Archive.
  The file is recognised by its content; `dcb export --lang` takes the texts from `--archive`.
- `p4kx convert <folder> --out <folder>` converts a folder of files that were extracted earlier
  without conversion: CryXml, split textures, sounds, soundbanks and a DataForge file are written
  as `extract --convert` writes them. The window offers **Convert folder…** with progress and
  cancel.
- MCP: `list_soundbank_objects` lists the `HIRC` objects of a soundbank in bank order, paged with a
  cursor and narrowed by `type` and `id`, in the shape of `p4kx bnk objects --json`. Objects the
  reader does not decode are listed and counted, not skipped.
- GUI: the Extraction summary (with Conversion on) and the Record export summary list the Records
  whose file does not follow their name or record path, with GUID, record path and reason. A
  renamed Record does not count as failed.
- Exclusion Filters: `p4kx grep`, `p4kx verify`, `p4kx convert`, `p4kx socpak verify` and `p4kx
  socpak extract` take a repeatable `--exclude` with the rules of `p4kx search`, and the
  Extraction dialog of the window offers the *Leave out* field for the changes of a Comparison
  side too, with the same texture rule as for the Archive.
- Filter text from outside is bounded everywhere: more than 1024 bytes or 32 wildcards is invalid
  input in every command that takes a Filter, an error of the MCP tools, and a red field in the
  window.
- Library: `convert_folder` of `p4kx-extraction` takes the `FolderSelection` of
  `FolderSource::select` instead of a Filter, and `FolderSource::count_matching` is removed.
- Basetool catalog: a contract lists the payout it states itself in `fixedRewards` (currency,
  amount, maximum and bonus flag as stored). The UEC reward of most contracts is calculated by the
  game and is not part of the Archive, so it stays out of the catalog.
- Basetool catalog: the item `stats` gain the blocks `miningLaser` (throttle, modifiers and filter
  of a mining laser, ship and FPS) and `miningModifier` (activation, charges, lifetime, laser, rock
  and filter modifiers and damage multipliers of a mining module or gadget), and every fire mode
  gains `hitType`, `fullDamageRange` and `zeroDamageRange`. `schemaVersion` stays 2.
- Basetool catalog: `contracts` lists the time to complete, the buy-in, whether a contract can be
  shared or is illegal, once-only, the players per instance, availability in prison, the
  re-accept rules, hidden in mobiGlas and the intro contracts a contract requires. Each value
  that several levels can state names the level (sub contract, contract, handler or template) it
  comes from. Schema version 2 keeps its number.
- Basetool catalog: contracts list the ship spawn descriptions and location queries they state
  (`shipSpawns`, `locationQueries`, `locationPrerequisites`), with the new kinds
  `contractShipSpawns`, `contractLocationQueries` and `missionLocationTemplates`. The queries are
  exported as stored, never resolved to places.
- Basetool catalog: every `contracts` entry has a `description` game text (`x-gameText`, not to be
  passed on to third parties), resolved like the title.
- UI screens: the canvas a canvas widget holds is now the one a style entry or a boolean binding
  picks, so a powered MFD enters the canvases it swaps in (the Avenger Titan MFD with
  `--var powerstate=1`: 206 widgets in 18 canvases instead of 48 in 6). A setting can name the
  canvas it is meant for, `--var mapdisplaymaster::isradar=1`. The UI tab keeps the game state
  and the typed variables across sessions.
- Basetool catalog: every contract entry lists all places it is offered at (`offeredAt`: the
  localities and location objects its prerequisites name, without the five-place cap of
  `pickupPlaces`), the faction it is stated for with the place that names it (`giverFaction`: the
  handler, a positive reputation result or a reputation prerequisite; a rival with a negative amount
  does not count) and the blueprint pools it hands out on success (`blueprintRewards`, per entry,
  so a consumer can tell which sub contract gives a blueprint). The fields are additive in schema
  version 2.
- Entity export: `entity glb --animations` writes the clips of a creature as animations of its
  skeleton. They are the clips the creature's animation database names, found in the animation
  folders of its skeleton's parameter file; additive clips and clips that move no bone are left
  out and listed. Without the option the GLB stays static (ADR-0307).
- UI screens: state tags that a canvas binds to its widgets, the text a text field holds
  (`Type Text` selectors) and animations that hold their last frame are evaluated, so the
  Avenger Titan MFD with `--var powerstate=1` draws its bars, frames and some texts (52 shapes
  and 4 texts instead of none). The report lists every texture the Archive lacks per screen
  (`missing_textures`, and `texture not in the Archive: <path>` on stderr), and the asset
  listing of a screen follows the canvases its styles swap in.
- Entity export: `--loadout-pick <group>=<index>` on `p4kx entity loadout`, `entity glb` and
  `entity package`, the `loadout_pick` parameter of the MCP tool `entity_loadout` and a field in
  the GUI's Entity export choose another alternative than the first of a group of a generated
  Loadout (a creature's or an NPC's). A bare index applies to every group that has it; an index
  beyond a named group is reported as a Loadout issue and the group keeps its first alternative.
  Without a pick nothing changes.
- Entity, armour and Interior exports: the JSON report of `p4kx entity glb`, `armour glb` and
  `interior glb` carries `unresolved_layers`, the number of layer files of the material libraries
  that are missing, unreadable or over the load budget, and `first_unresolved_layer`. The Export
  of an Entity as GLB shows them in its summary, in German and English.
- UI screens: `--var landscapecanvasguid=<record path or GUID>` puts a page in the powered MFD
  (`portraitcanvasguid` for the portrait view), and `--var mfdviewname=<name>` names it in the
  tab bar, which now draws its chevrons and its name on every page. The pages of the Avenger Titan
  MFD (power, weapons, scanning, diagnostics, shields, configuration, communications and self
  status) can be rendered one by one and compared with the game.
- MCP: the `entity_loadout` tool states the `notes` of an Entity next to its `issues`, in the
  shape of the CLI JSON: what the Archive cannot decide, such as an `assortment_requirement`.
- Basetool catalog: the `contractProperties` of `contractTemplates` and the `missionProperties` of
  `missionBrokerEntries` state their value in the new field `value`, the same union as the
  properties of an objective, with the types `missionItem`, `aiName`, `combinedDataSetEntries`,
  `haulingOrders`, `reward`, `timeTrialRace` and `starMapLocation` added to it. `valueType` stays.
  A value of any other struct fails the Record.

### Changed

- Armour and Character export: a head at `--lod 1` and up now has its expressions as morph targets,
  carried from level 0 over its surface, so the face animation drives them at every level. By
  default all expressions are written at every level; `--expression` and `--no-expressions`
  apply at every level. The expressions make the file of a distant level about four times
  larger, so `--no-expressions` shrinks the files of distant levels.
- Layered materials: the layer files of one ship are fetched and parsed once for all its material
  libraries, and one load reads at most 64 MiB of them. `p4kx model info` and the MCP model tools no
  longer read layer files. The Export as GLB summary lists how many layer files are missing, as
  `p4kx model glb` does.
- Armour: a worn piece whose Record hides a port (`HiddenParts`) leaves out the piece at that port:
  a hat hides the hair, an undersuit the clothing under it, a jacket the shirt. The piece is
  reported as `part_hidden`, and the body shows where it covered it. A clothing chunk whose
  condition names no port and no tag now covers its zone; a chunk conditional on ports or tags
  still covers nothing and is reported as `clothing_conditions_undecided` when it names a zone of
  the body. A garment partly covered by another is still drawn whole (ADR-0279).
- Characters: the hair, brows, lashes and beard of a head are bound to the head's facial joints
  through the skin weights of the head vertices around them, so a facial animation moves the beard
  with the jaw and the lashes with the eyelids (ADR-0299).
- Releases: the checksum files are now `SHA256SUMS.txt` and `SHA3-512SUMS.txt`, so they open in
  any text editor with a double click. The release notes and the verification guide use the new
  names.
- Entity export: `entity glb` and `entity package` fail when the main geometry of the Entity
  itself is not in the Archive (exit code 2) or cannot be read (exit code 3), naming the file, and
  write nothing; before they wrote a GLB or package without meshes. A part below the Entity that
  cannot be drawn is still listed and skipped.
- Extraction and Record export write only file names that are valid on both Linux and Windows,
  whatever the platform. An Entry with a forbidden character, a Windows device name, a trailing dot
  or space, a part over 255 bytes or a full path over 260 characters is reported with the rule it
  breaks and the run goes on. Entries that differ only in case count as a collision. A Record with
  such a name is written under its GUID, a record path with such a part is escaped (`a|b` becomes
  `a%7Cb`), and the report lists each of them.
- Window: a DataForge file opens as its DataForge only, with no Entry table, no folder tree and no
  Entry counts in the status bar, and the Search tab is hidden for it. `p4kx dcb`, the MCP server
  and the window read a DataForge file through one shared source type.
- Audio: the sound files of `p4kx wwise extract` and `p4kx entity audio --extract` and the `.txtp`
  files take their names from the portable file-name rule of the extraction. A forbidden
  character becomes `%` and two hex digits instead of `_`, letters beyond ASCII and spaces are
  kept, and names that differ only in case stay distinct. A sound whose path is too long is
  reported as failed instead of aborting in the file system.
- Entity export: `entity glb` writes a creature, an Entity whose root geometry is a character
  file with a skeleton, with its skins bound to that skeleton: the bones as nodes, one glTF skin and
  joints and weights on the skinned meshes, so it can be posed in Blender or a viewer. Bones the
  skeleton lacks follow their nearest parent and are listed as `bones_not_in_skeleton`; a creature
  whose skeleton is missing or unreadable stays static. The JSON report counts `bones` and
  `skinned_meshes`.
- Record JSON, Record XML and Query JSON: a Reference to a Record that does not exist keeps its
  error value and also carries the GUID of the missing Record, as `"guid"` next to `"error"` in
  JSON and as the `guid` attribute of `p4kx:error` in XML. The Record JSON schema accepts it, and
  exports without it still validate.
- Game texts: leading and trailing blanks and line breaks are removed from the texts of the Basetool
  catalog and from the text the Localisation tab copies, e.g. `Antium Arms Moss Camo ` is now
  `Antium Arms Moss Camo`. Record XML and JSON, `p4kx loc` and `dcb export --lang` keep the text
  as the `global.ini` stores it.
- Entity export: the `--help` text of `entity glb` and `entity package` and the exit-code section
  say that a missing (exit code 2) or unreadable (exit code 3) geometry of the Entity itself
  fails the run and writes nothing.
- Basetool catalog: the `names` of a `missionLocationTemplates` entry carry the `displayName` game
  text (`x-gameText`) again, with its English and German text, instead of only the `nameKey`.
  Where the German file lacks a key, `de` is `null` and the Basetool falls back to `en`.
- Blender package: `entity package` of a creature whose skeleton the Archive holds writes one
  skinned GLB with the skeleton, the skins and the parts on its bones, and its `scene.json` says
  so in the mesh's `rig`. The Blender add-on imports it as an armature with skinned meshes. A
  creature without its skeleton stays a set of static meshes. Package format 1.8: `rig.body` is
  optional and the part origin `skin` is new (ADR-0316).
- Entity assembly: a worn Entity whose port another worn Entity hides by its `HiddenParts` (a
  hat hides the hair, an undersuit the clothing under it, a jacket the shirt) is left out, as in
  the Armour, and reported as `part_hidden`. It covers no zone of the body, so the body shows
  where the hidden garment covered it (ADR-0304).
- Screens: bound sizes, anchors, pivots, scales, positions, paddings and margins are evaluated at
  rest, and the text of a text field takes the colour of an entry that selects it by its parent.
  The powered MFD of the Avenger Titan now draws its header and counters.
- Entity export: the Requirement of a loadout assortment and of its groups is now read. A group
  that names no tag always applies. A group that depends on tags of the spawn still applies and is
  noted as `assortment_requirement` in the new `notes` of `entity loadout`; it is not an issue and
  does not appear in the catalog's loadout `issues`.
- Window: on Windows the graphics instance opens Direct3D 12 only, so the Vulkan loader, the
  Vulkan drivers and the overlay layers injected into them are no longer loaded. A machine where
  Direct3D 12 yields no adapter or device within a few seconds falls back to the default backends,
  and `WGPU_BACKEND` still overrides the choice.

### Fixed

- Blender add-on: a layered material without a blend map, mask map or vertex colours shows its
  layers by position and `WearBlendBase` as the GLB does, instead of only the first layer; a layer
  that is the only one with weight is shown. The package (format 1.5) states the shares per layer
  (`layer_shares`). A texture that cannot be loaded falls back to the material's colours instead
  of a black image, and decals with a stencil map take their `StencilDiffuseColor` and
  `StencilOpacity`.
- Screens: a canvas with the coordinate method `aspectOverridesWidth` or `aspectOverridesHeight`
  is laid out in the whole box of its canvas widget instead of a box of the aspect of its own
  size. The footer and header of every MFD page now span the screen. ADR-0106 records which layout
  guesses the cockpit screenshots of the Gladius and the Constellation confirm, refute or leave open.
- Characters: the body of a CHF character takes the base material and skin tone the character
  chose for it, so body and head match at the collar instead of a pink body under a dark head.
  The strands of a short beard that hang below the chin are the beard's own data and stay.
- Assembly: the face parts of a head (eyelashes, teeth, eyes, brows) take the sub-geometry tagged
  with the head's flags before the one tagged for the body, in `entity glb` and `entity package`
  as in `armour glb`. Miles Eckhart's eyelashes are his own again instead of the generic male ones.
- Armour: the natural colour of a hair material clamps its tint and dye to 0..1 and ignores a
  tint or dye with fewer than three components, so a bad value no longer reaches the exported
  base colour.
- Blender add-on: an `Illum` material with a specular map now multiplies that map by its `Specular`
  colour, as the GLB writer does. The Cutlass Black's landing gear, engine pipes and nozzles come
  out dark metal instead of light, almost white, metal. A specular map on a material without a
  `Specular` colour (default 0.04), including materials of 1.0 packages, no longer gives metal,
  matching the GLB writer.
- Assembly: the body of an NPC is found by its `Char_Body` attach type and the folder of the
  variant's body skin instead of an exact skin file name, so a body that names another skin file of
  the same variant is cut too. A cut that finds no body part, several, or one of another variant
  is not made and is reported as `body_zones_unknown`.
- Blender add-on: glass with the generation mask `PER_VERTEX_TINT`, such as the lamp lenses of the
  Cutlass Black, takes its colour from the vertex colours instead of being plain white glass.
- Blender add-on: glass is see-through in EEVEE and the viewport's Material Preview. Its alpha
  follows the glass colour (clear glass 0.15, darker tints more opaque) and the palette colour
  follows `GlassTintStrength`. In Cycles the same alpha makes glass lighter and less tinted.
- Materials: stencil decals (`MeshDecal` with `STENCIL_MAP`, such as the branding of the ANVL
  Paladin) are cut out by their stencil map in the GLB export and in the Blender add-on instead of
  being drawn as solid quads. The motif takes `StencilDiffuseColor` and `StencilOpacity`, a
  `StencilTintOverride` takes the primary palette colour, and a decal whose motif the game fills in
  at run time (`$TintPaletteDecal`) is left out. Blender packages are format 1.6: the stencil map
  has the texture usage `stencil` and each stencil decal states `stencil`.
- Entity export: the head, teeth and eyes of a story character such as Miles Eckhart are his own
  skins, as in the armour export, instead of the generic head skins.
- GLB export: `COLOR_0` is written only on the primitives whose material reads the vertex colours
  (glass with `PER_VERTEX_TINT`, layered surfaces without a blend map). Glass such as the hull
  windows of the Cutlass Black is no longer tinted and faded by mask data in glTF viewers. Blender
  packages are format 1.4: each material description states `reads_vertex_colours`, and the add-on
  builds its vertex-colour nodes from it.
- Assembly: an item port of a creature's skin, such as the collar of a Kopion, is placed at the
  bone of the creature's skeleton its helper names instead of at the origin with the note
  `helper_not_found`. A helper no skeleton holds stays a listed note.
- Entity export: creatures whose Loadout is generated at random from loadout assortments, the
  Quasigrazers, now draw their body skin and eggs; P4KX takes the first alternative of each group.
  A creature whose Loadout file the Archive lacks, such as the Aloprat, lists
  `loadout_unreadable` with the reason instead of exporting an empty scene silently.
- Entity Assembly: the mobiGlas of a human NPC and the items its body holds, such as the tumbler
  of Miles Eckhart or the weapons in the right hand, hang on the body bone their port names
  instead of sitting at the origin between the feet. A rigid prop on a skin port is placed like a
  bone attachment; skins, clothing and other character items keep their bind pose. A helper that
  the skeleton of a human root holds is no longer listed as `helper_not_found`.
- Blender add-on: dark tinted glass is no longer nearly opaque in EEVEE. Glass tints the scene
  behind it by its colour, as the game's glass does, and EEVEE and Cycles now agree.
- Catalog: the cooldown of a contract or sub contract that overrides it through a parameter
  override (`PersonalCooldownTime` and similar) is exported with the override's value and level;
  `cooldownLayer` is now `handler`, `contract` or `subContract`, and the new `cooldownLayers` names
  the level of each cooldown field.
- Basetool catalog schema: the closed value sets `FactLayer`, `StandingLevel`, `SpawnLevel`,
  `HaulingSource`, `HaulingItemKind`, `LocationCount`, `CargoFillType` and `CargoSpawnKind` are
  strings with their known values in `x-values`, so a catalog with a new value still validates.
  The catalog data does not change.
- Hologram render on the software adapter: on Windows the software adapter now opens a Direct3D 12
  instance only, so the Vulkan drivers and the capture overlays injected into them are no longer
  loaded. That removes the multi-second start-up stalls and the rare crash (exit code 2173) seen
  under load.
- Viewer: releasing the graphics device no longer blocks `p4kx ui hologram` or the window for
  longer than a time limit. The command waits for the queue to go idle and drops the device on a
  worker thread, and prints a warning that names the stage the driver stopped in when it did not
  finish in time; the window ends the process with status 4 ten seconds after it has saved its
  settings and leaves a note in the settings folder.
- Characters: at `--lod 1` and up the hair, brows, lashes and beard follow the expressions of
  the head like at LOD 0 (ADR-0305). The GLB of a dressed character grows by about three and a half
  times with expressions at every level; `--no-expressions` writes the small file.
- Entity Assembly: the clothing and armour of a generic NPC no longer appears as a cardboard box or
  armour crate at the origin of the character. When the head names no body, the body Entity
  decides, the clothing takes the `Male` or `Female` models of the body, and the body skin is cut
  under it. A worn item whose only geometry is a world prop is left out and listed as
  `world_prop_not_worn`.
- UI screens: the constant of an integer comparison in a screen (`seatdashboard/currentmode`
  equal to 4, say) was read as zero, so the powered MFD drew every mode of its self-status page at
  once, with two large light-blue blocks at the lower corners. A runtime image such as the ship
  diagram no longer fills its box with a colour, a notification of the power page is no longer
  drawn at rest, and the arrows of the tab bar take their colour.
- Materials: a stencil decal with a `StencilTintOverride` of 2, such as the logos of the RSI Ursa
  Medivac, paints the red, green and blue masks of its stencil map with the livery's decal colours
  in the GLB export and in the Blender add-on, instead of one grey from the primary palette
  colour. Blender packages are format 1.7: a palette states its `decal` colours and such a decal
  has the `palette` value `decal`.
- Characters: an NPC with the generic head (most NPC archetypes) now takes its body from its
  `Char_Body` Entity in `p4kx armour glb --loadout`, the character window and `p4kx entity glb`,
  so a female NPC is no longer built on the male body. `body` of the `p4kx entity glb` report gains
  `named_by` and `named_by_head`.
- Ursa Medivac and every other layered hull: a GLB no longer multiplies the layer shares stored in
  the vertex colours into the hull colour, so the Respite livery is dark grey and black instead
  of purple. The Blender package is unchanged.

## [0.4.1] - 2026-10-28

A catalog and release-process update. The Basetool catalog gains these kinds and fields; all are
additive, so the catalog stays `schemaVersion` 2:
- the scan signature of mineable rocks;
- the crafting stations with their queue limits, crafting-time modifiers and the ships that mount
  them;
- contract cooldowns;
- the older loot Records;
- a more precise `playability`, checked against the RSI ship matrix.

Every Release from now on carries a VirusTotal report, build provenance with attestation bundles,
CycloneDX SBOMs and SHA-256 and SHA3-512 checksums.

The third-party notices and P4KX's own licences ship as Markdown. The README and the documentation
are brought up to date, and every GitHub Action is pinned to a commit. There are no breaking
changes.

### Added

- Basetool catalog: the new warning `harvest_locations_without_object` counts the harvestable
  locations that have no starmap object although one is recorded above their container, with one
  line per reason: the object is not in the starmap objects, 64 or more mining claim markers lie
  above, or a marker has no object above it.
- Basetool catalog: the kinds `legacyLootTables` and `legacyLootArchetypes` with the older loot
  Records, so that `lootConfig.lootTableGuid` of a slot resolves. The loot container scan sums all
  `.soc` files of a container and warns about placed containers the Archive lacks and about entity
  files that name several containers or multi configs.
- Releases: every Release now carries `SHA256SUMS` and `SHA3-512SUMS`, a CycloneDX SBOM per
  binary, Sigstore attestation bundles for offline verification with `gh attestation verify`, a
  VirusTotal report of the MSI and the portable ZIP, and GitHub's list of merged pull requests.
- Basetool catalog: the new kind `mineableEntities` lists the mineable rocks and deposits with
  their composition, mining parameters, radar contact type and scan signature. The slot of the
  scan signature is the one the radar parameters name; the raw `signatures` are exported too.
- Basetool catalog: the new kind `crafters` lists every crafting station with `maxJobsInHistory`
  and, per queue, `maxJobsInProgress` and `maxJobsWaiting`. The Archive holds no per-player
  blueprint cap; these are its only crafting limits.
- Basetool catalog: each contract carries its cooldowns (`hasPersonalCooldown`,
  `personalCooldownTime`, `personalCooldownTimeVariation`, `abandonedCooldownTime`,
  `abandonedCooldownTimeVariation`) and `cooldownLayer`, the level that states them. The numbers
  are as stored; the Archive does not state their unit.
- Basetool catalog: the `crafters` kind carries `baseCraftingTime` and the crafting `modifiers`
  (type, `timeMultiplier` and the blueprint categories a modifier applies to), per queue the
  accepted blueprint categories and process types, and the `type`, `subType` and `size` of the
  crafter, which a ship's crafter port is matched against. A modifier type, condition or queue
  rule the export does not know is kept by name and reported in `issues`.

### Changed

- The third-party notices are now `THIRD-PARTY-NOTICES.md`, a Markdown document with a table of
  every Rust crate (version, licence, link) and the licence texts in fenced blocks. P4KX's own
  licences also ship as `LICENSE-MIT.md` and `LICENSE-APACHE.md`. The portable ZIP and the MSI
  carry the Markdown files next to the plain licence files.

### Fixed

- Catalog: the `AI_Super` ships of Anvil Hornet F7C Mk II and RSI Perseus are `ai` instead of
  `player`, and those of Drake Corsair, RSI Polaris and MISC Fury are `ai` instead of `null`. The
  `playability` rule reads the Record tag `SystemsDesign` as an AI marker.
- Catalog: Basher, Sabre Raven EX, Skylark, Railen, S-65 Stingray, Tyilui and Constellation Mk V
  Centaurus are `player` instead of `null`; an exploding hull no longer keeps a `ReadyToInclude`
  ship out of `player`.

## [0.4.0] - 2026-10-28

The fourth Release brings the 3D side of the tool:
- whole ships and vehicles as Assemblies, with Loadouts, palettes, textures and landing gear, in the
  GLB export and the 3D viewer;
- interiors, characters and armour with their faces and animations;
- the Blender package and its add-on;
- MFD screens and holograms;
- Wwise sound resolution.

The Basetool catalog adds:
- ship insurance and the item recovery rules;
- mining compositions and harvestable locations;
- factions, reputation, mission types and localities;
- loot tables and placed loot containers;
- a playability class for ships and items.

The GUI follows P4KX's own theme and fits the smallest window without sideways scrolling.
Differential tests compare P4KX with the output of unp4k, unforge and StarBreaker.

Breaking changes:
- The repository moved to `p4kx-org/p4kx`, which changes the Record XML namespace and the Record
  JSON schema `$id`.
- The Basetool catalog and the ship base data are now `schemaVersion` 2.
- The default CSV of `dcb query` and `dcb fields` changed.
- Several library types changed; see Changed.

### Added

- MCP server: the tools `list_chunks`, `get_model_info` and `list_model_items` list the chunks of a
  chunk file Entry and summarise a model Entry (nodes, meshes, levels of detail, materials, bones,
  animations), paged and without returning chunk data. Unknown chunk types are marked, and broken
  or missing files give clear tool errors.
- Assembly export: `p4kx entity glb --materials textures` embeds the diffuse, normal and
  metallic-roughness textures of every part, each image once however many parts use it (liveries
  share it unless a specular map bakes the palette colours in), at the mip that `--texture-mip` picks for the whole Assembly. Textures that cannot be read
  keep the plain colour and are listed in the report.
- Assembly export: the tint palette of each part, the one its Entity names or else the default of
  its material library, gives the colours of the materials that take one, so liveries look right.
  A palette that cannot be resolved is listed (`palette` issue); the part falls back to the
  colours of its library.
- Entity: `p4kx entity loadout` and `p4kx entity glb` take `--exclude <port>` and
  `--replace <port>=<entity>`, each repeatable, to export a ship without its weapons or with
  other components. A replacement is checked against the port's item types and sizes, from its
  definition in the Record or from the vehicle implementation, and rejected with the reason
  (exit code 2); the replacing Entity brings its own Loadout. The Loadout tree marks the ports
  `excluded` and `replaced`.
- GUI: the new Entities tab lists the ships, vehicles and weapons of the open Archive, searchable
  by class name and record path and filterable by kind. Selecting an Entity shows its Loadout as a
  tree of ports and attached Entities, where a branch can be left out and an Entity replaced by
  another one that fits the port's item types and sizes. "Export as GLB…" writes the Entity with
  those changes as one GLB with the level of detail, material (plain colours or textures, with
  the tint palettes of the Entities) and texture size choices, which are remembered. The export
  runs in the background with progress and cancel and ends with a report of the parts that could
  not be resolved and the textures that are missing.
- Assembly: `PortNode::constraints` returns the port's constraints built once, `LoadoutChanges`
  can be edited in place (`set_excluded`, `set_replacement`, `remove`, `action_for`), and
  `EntityCatalogue` lists every Entity with its kind and item facts for pickers.
- GUI: the Entities tab shows the selected Entity with its Loadout in a 3D view above the tree.
  The view draws the same Assembly the GLB export writes, at the chosen level of detail (0 to 3)
  and with plain colours or the diffuse textures, with the tint palettes of the parts. Leaving a
  port out or replacing an Entity builds the view again; "Hide" and "Show" in the tree hide a
  branch in the view only. Parts that share a model are uploaded once and drawn as instances,
  parts outside the view or smaller than a pixel are not drawn, and the Assembly is built in the
  background with progress and a stop button. "Export as GLB…" starts with the detail and
  materials of the view.
- 3D viewer: a model with bones shows a bar under the view with a Skeleton checkbox that draws its
  bones over the model. A skinned model also lists the animations that move its bones, and plays,
  pauses, loops and scrubs the chosen one on a timeline; the vertices are skinned on the graphics
  device. The preview panel opens taller for models to make room.
- Interior: `p4kx interior glb <archive> <container>... --out <file>` exports a socpak container as
  one GLB: the models its `.soc` files place, at their transforms, and the containers nested in it
  at their position and rotation, recursively. Cycles, missing containers, files and models are
  listed. `p4kx-model` reads object container descriptions and the `IncludedObjects` of `.soc`
  files.
- Interior: `p4kx interior glb` also places the props of the `.soc` entities and writes their lights
  with `KHR_lights_punctual`: point, spot and directional lights with colour, intensity and range
  (the intensity is the raw engine value and the colour an approximation).
  Ambient and planar lights, which glTF cannot express, are listed. The JSON report gains `props`
  and `lights`. `p4kx-model` reads the entity chunk of a `.soc` file (`SocEntities`).
- Interior: `p4kx entity glb <archive> <entity> --interior --out <file>` adds the rooms of a ship
  to its GLB. The socpak containers the Entity's Record names (`VehicleComponentParams` and
  `SObjectContainerComponentParams`) are placed at their bone of the hull with the Record's
  offset, below an `Interior` node of the Entity, with the containers nested in them. Containers
  and parts that cannot be resolved are listed; without the flag only the hull is written.
- Entities tab: the checkbox Interior in the 3D view builds the rooms and props of the Entity's
  object containers in the background, with progress and Stop, and shows them with the hull;
  switching it off hides them without building again. The Entity export has the same choice,
  remembered with the other options, and a socpak Entry in the Archive offers Export Interior as
  GLB with level of detail, materials, progress and a report.
- Library: `Interior::build_watched` and `EntityInterior::build_watched` can be stopped and report
  their progress, and `Interior::write_glb_with` embeds textures.
- CLI: `p4kx wwise extract <archive> <trigger> --out <folder>` and `p4kx entity audio --extract
  <folder>` write the sounds of an audio trigger or an Entity as Ogg, WAV or raw WEM files named
  `<trigger>[_<WEM ID>].<ext>`, safe for the file system and unique. A sound that cannot be
  decoded is reported and written unchanged; `--conflict` decides about existing files.
- GUI: the new Sounds tab lists the audio triggers of an Entity (by class name, GUID, record path
  or the selected Record) and searches triggers and Wwise events by name. Each trigger shows its
  sounds with soundbank, WEM ID, source kind and language; a sound plays with the audio player,
  and the sounds of a trigger or of the whole list are exported as Ogg, WAV or raw WEM with the
  trigger name in the file names, with progress and cancel. The audio is indexed off the UI thread.
- Armour: `p4kx armour glb <archive> --piece <entity> --body male|female --out <file>` writes
  pieces of armour and clothing worn on the default body of a variant as one skinned GLB: the
  body's skeleton once as bone nodes, the skin of the body, and for every piece the geometry its
  Record names for the variant's tag, skinned to the skeleton by bone name. `--piece` can be given
  several times; a piece with no geometry for the variant or whose model cannot be read is
  skipped and listed. `p4kx armour bodies` lists the variants.
- Armour: `p4kx armour glb --loadout <loadout>` takes the pieces from a Loadout instead of a list:
  an Entity's default Loadout by class name, GUID or record path, or an XML Loadout by its path in
  the Archive or as a file on disk. The armour and clothing anywhere in its tree are worn, other
  items such as weapons are not; entries that cannot be resolved are listed. `--piece` can be
  added: it takes the place of the Loadout's piece of the same item type, or is added.
- Characters: `p4kx chf glb <archive> <file> --out <file>` writes the head of a CHF character as
  one skinned GLB: the head skin of the character's head rig, shaped into the character's
  neutral face by its face DNA, worn on the default body of the character's body variant. The
  DNA rig reader (`p4kx-character`) reads the neutral and expression morphs, joint and region
  weights and delta records of the rig. Hair, eyes, teeth and expressions follow later.
- Characters: `p4kx chf glb` now dresses the head. The teeth, eyes, hair, brows, lashes and beard the
  character wears are in the GLB: the eyes and teeth shaped by the face DNA, the rest following
  the shaped head. The character's material choices are applied (its head material, skin tone,
  iris colour, hair melanin and dye) and the textures of the skin, eyes and hair are embedded
  (`--materials`, `--texture-mip`); hair cards are cut out by their opacity maps. Items,
  materials and textures the Archive does not have are listed in the report.
- Characters: `p4kx chf glb` writes the expressions of the head rig as named glTF morph targets of
  the head (399 for the male and 406 for the female default character, e.g. `CTL_expressions
  blinkR`), mixed by the character's DNA so that they move the character's own face, and stored as
  sparse accessors (3.3 MB for a head). `--expression NAME` (repeatable) limits them and
  `--no-expressions` leaves them out; only LOD 0 has expressions. `p4kx-gltf` writes morph targets
  (`RigPiece::with_morphs`), `p4kx-character` reads the `RigLogic` names and blends the
  expressions, and the GLB checker of the testkit validates sparse accessors and morph targets.
- Characters: `p4kx chf glb` and `p4kx armour glb` take `--anim NAME` and `--anim-from ENTRY` and
  write body animations on the body skeleton. For a CHF character, face animations are evaluated
  through the rig logic of the head into the poses of the facial joints and the weights of the
  expression morph targets. The facial joints of the head skin are now part of the skeleton, at
  the character's own joint positions, and the head is skinned to them.
- Characters tab: composes a Character from a body, a head (a CHF file of the Archive or one you
  pick), pieces of armour and clothing and a Loadout, shows it in the 3D view with skeleton overlay,
  previews the expressions of the head, lists the pieces that could not be fitted, and exports it as
  GLB with progress and Stop.
- Blender package: `p4kx entity package <archive> <entity> --out <folder>` writes an Entity as a
  folder that does not depend on Blender's file format: `scene.json` (format version, source
  Channel and Build, node hierarchy with names and transforms), one GLB per distinct mesh, shared
  by every node that draws it, and the textures of the materials as PNG, each written once. The
  folder is written beside the target and moved into place when complete, and the JSON Schema of
  `scene.json` is checked in. See `docs/blender-package.md`.
- Blender package format 1.1: `scene.json` describes every material of the meshes and of their
  layers as the game defines it (shader, generation mask, texture slots with their UV modifiers,
  layers with tints, tiling and palette entries, shader parameters kept as name and value), lists
  each tint palette once, and the package holds the textures of every slot. `p4kx entity package`
  reports the materials and palettes. Packages of format 1.0 stay readable.
- Blender: the new add-on in `blender_addon/` imports a Blender package with File > Import > P4KX
  Package. It keeps the hierarchy, names and transforms, imports shared meshes once as linked
  instances, loads the textures and gives the materials a basic node setup. A package with a
  newer format version is refused with a message to update the add-on. It targets Blender 5.2 LTS.
- Blender add-on: material node trees from the package's material descriptions (format 1.1):
  layered surfaces blended by their masks, palette tints, wear, glass, decals, holograms and
  alpha modes, with shared node groups. Unknown shaders get a basic material and a warning.
- Blender package: `p4kx entity package --interior` writes the Entity's Interior into the package
  (format 1.2): containers, models and props as nodes and lights as nodes with type, colour, raw
  intensity, range and cone angles. The schema and `docs/blender-package.md` describe them. The
  Blender add-on builds the containers and instances and creates Blender point, spot and sun
  lights; the import option *Light strength* scales their energy.
- Blender package: `p4kx character package <archive> --out <folder>` writes a Character, the head
  of a CHF file and the armour and clothing of pieces or a Loadout on a body, as a package of
  format 1.3 with one skinned GLB that holds the skeleton, the head's expressions as morph targets
  and the animations. The Blender add-on imports it as an armature with shape keys and actions
  that can be posed and animated. `p4kx_blender::write_character_package` is the entry point for
  front-ends.
- Library: `Mesh` and `PackageSummary` of `p4kx-blender` no longer implement `Eq`, because they
  hold floating-point values now.
- Desktop window: the export of an Entity offers "Blender package" next to "GLB file". The package goes
  into a folder you pick, which may be new, empty or hold an earlier package that is replaced as a
  whole. The export shows its progress with Cancel and ends with a report of what the package holds.
- Release: every GitHub Release now attaches the Blender add-on as `p4kx_importer-<version>.zip`
  next to the MSI and the portable ZIP. Its version always equals the Release version; `cargo
  xtask` fails when the add-on manifest and the workspace version differ.
- CLI: `p4kx ui screens <archive> <entity>` lists the `BuildingBlocks` screens of an Entity and of
  its Loadout (MFDs, radar, annunciators, HUD) with the canvases, textures, vector graphics, movies
  and localisation keys each uses and the widgets by element type; `p4kx ui render <archive>
  --screen <canvas>` renders such a canvas as a PNG or SVG still through the GFx renderer: the
  layout and the boxes, circles and polygons in the colours of the style cascade. Texts, textures
  and the other widget types without a renderer are left out and listed (exit code 1).
- UI renders: the radar screens of ships (`p4kx ui render --screen`, for example
  `components/map/mapdisplaymaster`) list what the game fills at run time by its reason (maps, 3D
  models and windows, stalk lines) instead of as widgets without a renderer, and separators draw
  the canvas of the ModularKit that holds their look. A custom shape without a graphic draws the
  box its style gives it and is listed as a graphic the game sets at run time.
- CLI: `p4kx ui hologram <archive> <entity> --out <file.png>` draws the hologram of a vehicle as a
  PNG: its Assembly with the Loadout, as a transparent tinted shell with a glowing edge, seen
  through the camera of the game's `UIHoloVehicle_Config` Record (field of view, view angle and
  distance factor, each overridable). `--size`, `--background` (`transparent` or a colour with
  alpha), `--view-angle`, `--elevation`, `--tint`, `--opacity`, `--glow` and `--software` shape
  the picture. It is drawn offscreen without a window, by the renderer the 3D viewer uses.
- GUI: a UI tab browses the GFx and SWF files of the Archive and the screens of every ship or
  vehicle, draws the picked one off the UI thread, lists what the drawing left out, and exports
  it as PNG or SVG like `p4kx ui render`.
- Keyboard: the lists of the Extraction summary and the summary of the DataForge tab take the focus
  with Tab and scroll with the arrow keys, Page Up, Page Down, Home and End.
- Textures: array and volume textures decode per image. The Extraction and `p4kx texture png`
  write one PNG per array element (`<name>_layerN`) or volume slice (`<name>_sliceN`), and the
  Image tab tiles the images on one sheet and names the number of layers. `R16G16_FLOAT` and
  `R32_UINT` textures decode too.
- Library: `p4kx-cryxml` gains `compare_trees` and `TreeComparison`, which count where the child
  table of a CryXml file differs from the tree its parent indices give. The reader itself is
  unchanged; on LIVE the child table never changes the output (ADR-0142).
- Record export: the flat value export writes one row per leaf value of the Records, as CSV
  (`values.csv`, for spreadsheets) or NDJSON (`values.ndjson`), with the Record GUID, name and
  path, the field path, the array indices, the kind, the value and the target path of a Reference
  (`docs/record-flat.md`). `p4kx dcb export --flat --format csv|ndjson` writes it, `--lang` adds
  the resolved text of localisation keys and `--csv-raw` writes the exact values without byte order
  mark and without the leading `'` that keeps a spreadsheet from running a text as a formula; the
  Record export dialog of the desktop app offers the three flat forms. There is no SQLite output
  (ADR-0112).
- Wwise: the soundbank reader decodes the music segments, tracks, switch containers and
  playlists, the node base parameters of sounds and the soundbank a Play action names. Trigger
  resolution follows them, and PlayEvent actions, to sounds; a Play action that names another
  soundbank is a broken link of the new kind `other_soundbank`. Entities also name audio
  triggers through string fields whose name contains `audioTrigger`, e.g. the audio trigger name
  of a melee weapon.
- Library: `HircValue` and `HircObject` of `p4kx-audio` no longer implement `Eq` and `Hash`,
  because the music objects hold floating-point values now. `SoundSource.parent_id` and
  `ActionTarget.bank` are new public fields, and `BreakReason` has the new variant
  `OtherSoundbank`.
- Textures: floating-point textures (`R16G16B16A16_FLOAT`, `R32G32B32A32_FLOAT`, `R16_FLOAT`,
  `R32_FLOAT` and the others) are high range like BC6H. The Extraction and `p4kx texture png`
  write them as OpenEXR with 32-bit float RGB, one file per image, and 8-bit output is
  tone-mapped (ADR-0010, ADR-0123). Textures in `R11G11B10_FLOAT`, `R9G9B9E5_SHAREDEXP`,
  `R32G32_FLOAT`, `R32G32B32_FLOAT`, `R8_SNORM`, `R8G8_SNORM` and `R8G8B8A8_SNORM` decode too.
- Models: a mesh that stores no normals gets smooth normals computed from its triangles
  (`Mesh::normals_computed`); quantised positions can be read with the model box
  (`Model::read_with_box`); `LodSelection::OnlyOrBase` reads a missing level from the base meshes
  and reports an issue instead of failing; nodes keep the scale, geometry type and `key=value`
  properties of the node table and the model its list of material indices. `p4kx model info`
  shows them (ADR-0119).
- Record XML and Record JSON: a Record now carries its name and, in a version 8 DataForge, its
  tag. Record JSON has `$name` and `$tag` after `$path`; Record XML has the root attributes
  `p4kx:name` and `p4kx:tag` after `path`, and the root of every document declares `xmlns:p4kx`.
  Version 6 and Records without a tag have no tag. The Comparison reports the tag next to the
  name and treats a different tag as a change of the Record; the Comparison window shows it as a
  row of its own. The change is additive; the Record
  JSON Schema and the Comparison report schema are updated (ADR-0124).
- MCP server: `list_entries`, `search_entries`, `read_entry_text` and `read_entry_bytes` read
  inside socpak containers with paths like `outer.socpak::inner.socpak::path`. New tools:
  `image_preview` returns a DDS texture as a PNG image block (chosen mip, cubemap face),
  `mtl_summary` summarises a `.mtl` material library, and `entity_loadout` lists the Loadout
  tree of an Entity with the reason for every unresolved port. All are read-only, paged where they
  list, and bounded in size.
- MCP server: the audio tools `search_audio_triggers`, `get_entity_sounds`, `resolve_audio_trigger`,
  `list_soundbank_wems` and `get_wem_info` follow an Entity's audio from trigger through Wwise
  events and soundbanks to WEMs, paged and with metadata only. `get_audio_excerpt` returns a
  short decoded WAV excerpt of a WEM, at most 10 seconds and 1 MiB.
- CHF: `p4kx chf to-json` writes the name of a hash beside it (`port_label`, `attachment_label`,
  `name_label`, `gender_hash_label`) for the item ports, material slots, sub-materials and
  parameters it can resolve, and counts the hashes it cannot, on stderr, after the conversion line
  and in the `--json` summary. `--archive` adds the names the Archive knows. `from-json` ignores
  the labels, so the byte-exact round trip still holds.
- CLI: `p4kx extract --threads <n>` limits the worker threads of an Extraction. A Filter that
  matches no Entry now also says what to try, for example that a glob must match the whole path.
- Archive: `Archive::open_with` and `OpenOptions::recover` rebuild an Archive whose end of central
  directory is missing or whose central directory is damaged by scanning the last 4 GiB for
  directory records; `EntryIssue::Recovered` says so. The command line offers it as the global
  `--recover` flag. Without it nothing changes.
- Soundbanks: `p4kx bnk list` and the Audio tab classify every embedded media item as WEM, MIDI,
  plug-in data or unknown, and `bnk list --json` carries the `class`. Sounds whose source is a
  plug-in or the External Source plug-in are marked as having no file, so the audio index does not
  report them as missing media.
- Chunk files: `p4kx chunks list` names the chunk types of `.soc` files (`CryXmlB`, `AreaShape`,
  `IncludedObjects`, `ModelPaths` and others) instead of marking them unknown, and
  `p4kx chunks dump --text` writes a `CryXmlB` chunk as XML and a `Json` or `ModelPaths` chunk as
  text.
- CLI: `p4kx prefab list <archive> [<library>]` lists the prefab libraries below `Data\Prefabs` or
  the prefabs of one, and `p4kx prefab dump <archive> <library> <prefab>` prints a prefab with
  every nested prefab expanded: each object with its type, class, geometry file and transform to
  the prefab's space, the rotation and scale of the parents applied. Nested prefabs are found by
  their GUID or name; one that is missing, contains itself or is nested deeper than 16 levels
  stays unexpanded and is listed. `--json` gives the same as data.
- Assembly: `PrefabSet` reads the prefab libraries of an Archive and resolves a prefab to its
  objects with full transforms, with cycle and depth limits; `PrefabLibrary` in `p4kx-model`
  reads one library file.
- CLI: `p4kx actionmap export` writes the action maps of the default profile as JSON or CSV:
  every action with its localised label (`--language`) and its default keyboard, mouse, gamepad
  and joystick bindings. Library: `read_action_maps` and `write_action_maps` in `p4kx-export`.
- CLI: `p4kx chf diff <a.chf> <b.chf> [--format text|json]` lists the fields in which two CHF
  character files differ, by path with old and new value: header, face DNA by part and blend, item
  ports and materials down to colour and float parameters. Hashes are shown as names where a name
  is known.
- Basetool catalog: the kinds `factionReputations`, `reputationScopes`, `reputationStandings`,
  `reputationContexts`, `missionTypes`, `missionLocalities` and `contractScenarios`, and the contract
  fields `minStandingGuid`, `maxStandingGuid` and `standingRequirements`. A faction is lawful by the
  `entityLawful` property of its Record, not by its class name. On LIVE 3,976 of 5,175 contract
  entries have a standing range.
- Basetool catalog: the kinds `mineableCompositions`, `miningGlobalParams`,
  `harvestableProviderPresets`, `subHarvestableMultiConfigs` and `harvestableLocations`. A
  provider preset links each harvestable to its entity and mineable composition; a location names
  the starmap object a container with a provider preset hangs below (19 of 49 presets on LIVE).
- Basetool catalog: the kinds `lootTables`, `lootArchetypes` and `lootPoolFilters` with the weights
  of the V3 loot Records, `lootContainers` with the containers placed with a multi config per
  location, and the field `lootConfig` of the sub harvestables of a multi config. On LIVE 3,004
  containers name 41 multi configs.
- Basetool catalog: every ship has the new field `insurance` with the claim time
  (`baseWaitTimeMinutes`), the mandatory wait (`mandatoryWaitTimeMinutes`), the expediting fee
  (`expediteFee`), `restoreOnClaim` and `shipEntityClassName` of its Record, or `null` when
  the Record has none. This belongs to the unreleased catalog version 2.
- Basetool catalog: the new object `itemRecovery` holds the item recovery multipliers of the game
  (`claimCostMultiplier`, `maxExpeditedClaimCostMultiplier`, the delivery rates and the override
  groups with the GUIDs of their entity classes), so the Basetool can work out claim fees itself.
  The catalog states no fee for an insured Loadout; the game computes it at run time.
- `model info` prints bone names sanitised, shows the bone count of a level of detail that carries a
  skeleton of its own, and counts the vertices per mesh whose texture coordinates were not finite
  numbers (`Bad UVs`, `skeleton_bones` and `non_finite_uvs` in JSON).
- `model glb --max-influences N` keeps the strongest N bones of a vertex and scales their weights
  to add up to 1; 4 writes the first influence set only, for viewers such as Three.js.
- `entity glb --json` reports `placements`: how each part was placed.
- Models: `p4kx_model::ModelError::Unsupported` with the new `UnsupportedLayout` (`BigEndian`,
  `StreamedMesh`, `MeshVersion`, `PhysicsVersion`, `PhysicsPrimitive`) and
  `ModelIssue::PhysicsSlotsIgnored`. This is breaking for a `match` on `ModelError` that has no
  wildcard arm.
- Search tab: opening an Entry hit scrolls the Text or XML preview to the line of the hit (#387).
- `p4kx entity glb --expand-docked-vehicles` places the Loadout parts of docked vehicles too; the
  text summary of `p4kx entity loadout` shows the number of docked vehicles.
- Basetool catalog window: the summary lists the warnings of the game data, in German and
  British English, as `p4kx catalog` prints them, and the progress window shows the step that
  reads the crafting data. `p4kx catalog` shows the same step on its status line.
- Basetool catalog: the new kind `shipStoreSlugs` maps the slugs of the RSI pledge store to the
  ships of the catalog (205 of 256 slugs on LIVE), with retired slugs flagged. It is curated data
  that ships with the tool (ADR-0117); `p4kx catalog` prints the slugs whose mapping went stale,
  and a real-Archive test fails while there are any.
- Window: the Entity picker of the Entities tab folds cosmetic variants under their base, the
  groups of `variantOf` (ADR-0066). The base shows the number of its variants and a chevron that
  lists or folds them, also from the keyboard; a search finds a variant and lists it under its
  expanded base. The Record list and tree are unchanged.
- 3D viewer: the doors, turrets and landing gear of a static model with helper bones now play
  their animations in the viewer.
- Loadout reader: `p4kx entity loadout` lists the base Loadout and the outfits of an NPC actor,
  with their name tags and which of them are worn, as `outfit` lines and as `outfit` in the JSON
  output; `armour glb --loadout` and `character package --loadout` report the same under
  `loadout`. Outfits without name tags all apply, and the first one applies when all have name
  tags (ADR-0181).
- Sounds tab: a row of language buttons beside the actions chooses the language folder in which
  localised sounds are resolved, like `--lang` on the command line. Choosing one resolves the
  selected trigger again and applies to the sound export.
- Interiors: entities that only name the Record of their class, such as doors, panels and
  cabinets, are placed as props with the model, material library and tint palette of that Record
  (`interior glb`, `interior package`, `entity glb --interior`, `entity package --interior` and the
  window). Models and props are drawn with the tint palettes the `.soc` file or the Record names,
  else the default palette of their material library.
- CLI: `p4kx ui render --screen` draws the texts of `BuildingBlocks` screens (`WidgetText`,
  `WidgetTextField`) as glyph outlines of the fonts in `fonts_en.gfx`, in the font, size, colour,
  alignment, case and wrapping the widget and the style cascade set; localisation keys resolve in
  the new `--lang` (English by default), texts the component parameters of a nested canvas fill
  are included, and every text left out or approximated (run-time texts, missing keys or glyphs,
  default fonts) is listed. The report counts the texts drawn and left out. Widgets now paint in
  the order of their `layer`.
- CLI: `p4kx ui render --screen` draws the textures and vector graphics of `BuildingBlocks`
  screens: image widgets with their scaling method (fill, contain, cover), flips and colour, icons
  and custom shapes from their SVG files (paths, basic shapes, transforms, solid fills and
  strokes, colour overlays, palette slots named in element ids, nine-slices), in PNG and SVG
  output. A texture or SVG file that is missing, unreadable or beyond a limit is listed and its
  widget skipped. The report counts the images drawn and left out; the textures and the paths of
  one screen are bounded by `Limits`.
- Models, Entities, interiors, armour and characters: with `--materials textures`, layered
  materials such as the `LayerBlend_V2` clothing and the `HardSurface` hulls are drawn from the
  diffuse and normal maps of their layers, tiled and blended by the material's blend map, instead of
  one flat colour; the viewer draws the same base colour image. `layers_without_diffuse` now lists
  only the layered materials none of whose layers has a diffuse map (ADR-0147).
- Blender package: `p4kx interior package <archive> <container>... --out <folder>` writes the
  Interior of a socpak container on its own as a package of format 1.3, with the same container,
  object and light nodes, props and lights as `entity package --interior`, and `--lod`,
  `--texture-mip` and `--no-vertex-colours`. The Blender add-on imports it.
  `p4kx_blender::write_interior_package` is the entry point for front-ends.
- Desktop window: the export of an Interior container and of the Character of the Characters tab
  offers a Blender package next to the GLB file, with the folder checks, progress, Cancel and
  summary of the Entity package. The package of a Character carries the animations of its
  animation folders unless "None" is chosen. The Characters export button is renamed to "Export…" and opens a
  setup; the Interior export button and window are renamed to "Export Interior…".
- Characters: `p4kx armour glb --loadout <npc entity>` and `character package --loadout` export a story
  character such as Miles Eckhart in one command. The Entity's outfit Loadout (base Loadout and
  clothing) is resolved, the head with its own skins (head, teeth, eyes, brows, lashes) replaces
  the generic Protos head, and the hair takes the material of its colour item. The body follows
  the head, and `--anim` looks in the character's own animation folder first; `--anim-folder`
  names the folders to look in. `armour glb` takes `--materials textures` and `--texture-mip`.
- Extraction: `p4kx_dataforge::FLAT_TAG_COLUMN` and its re-export `p4kx_extraction::FLAT_TAG_COLUMN` name the last column of the flat value export.
- Basetool catalog: modifier ranges of a blueprint with a struct type the reader does not know are
  counted and reported as the crafting warning `unknown_modifier_ranges` (a `count` and the
  `details` types) in `p4kx catalog --json`, on stderr and in the catalog windows of the GUI, in
  German and British English. A Build without such a range has no warning.
- Library: `CompiledQuery::shared_prefixes` and `SharedPrefix` return the groups of columns
  that walk their first path steps once per Record.
- Record export: the setup window offers to write the tag path next to every tag Reference, for
  the JSON and NDJSON forms, as `p4kx dcb export --tags` does.
- Record preview: a tag Reference shows its tag path next to the link.
- Basetool catalog: the new kind `qualityQuantizations` lists the quantised quality steps of the
  game (`bands` with `start`, `end` and `mappedValue`) and links each to the `ResourceType`s that
  use it by `resourceTypeGuids`. A Record that no `ResourceType` uses is kept with an empty list; a
  Record that cannot be read is listed as failed. `p4kx catalog` prints and `--json` reports a
  `quantizations` summary with the unused Records and the links that reach no quantization; it
  does not change the exit code. This belongs to the unreleased catalog version 2.
  `read_crafting` returns them as `Crafting::quality_quantizations`.

### Changed

- Assembly export: parts that use the same model, level of detail and material library are
  written as one mesh that their nodes share, so ships with repeated parts give smaller files
  (the Gladius shrinks from 55.8 MB to 44.8 MB). `p4kx entity glb` reports `shared_meshes`,
  `shared_materials` and `shared_textures`.
- Models: a `HairPBR` material whose `TexSlot1` is an opacity map (`_opac`), such as hair cards,
  is now written alpha-masked with that map as the alpha of a white base colour image, in
  `entity glb`, `model glb` and `chf glb`, so the cards are cut out instead of opaque.
- Textures: floating-point textures are written as OpenEXR `<name>.exr` instead of `<name>.png`,
  so their file names change; the `format` of `p4kx texture png --json` is `exr` instead of `png`
  for them, and four-channel ones keep their alpha in the file. The 8-bit preview and
  `--tone-map` output of these textures is tone-mapped instead of clamped to 0..1.
- Extraction remembers the target folders it has made, still checking each is a real folder
  before it writes there, which makes a repeated Extraction of 76,000 Entries about 3 times
  faster on LIVE. Selecting Entries by path or by Filter now runs on all processors (about 30
  times faster on 32 of them).
- `p4kx bnk list` gains a `Class` column. The text of `p4kx wwise` names a source without a file
  as a plug-in or external source and counts it as without a file. MIDI media is extracted as
  `.mid` instead of `.wem`.
- DataForge version 5 and older are not supported (ADR-0136): they are reported as an unsupported
  version, with exit code 5 on the command line and a named message in the Records tab, and
  the documentation lists versions 6 and 8 as the supported ones.
- Models: the four-byte second normal stream of the meshes is read as their second texture
  coordinate set (`Mesh::second_uvs`) instead of as a last-resort normal source, and the
  unverified 16-byte tangent layout, which no Archive holds, is no longer read (ADR-0114).
- Smallest window: the work surface now fits a window of 640 × 400 without scrolling sideways, so
  the Search, Localisation and Tags buttons are in view. Toolbars of the Browse, Record search and
  Characters tabs wrap their actions into a line of their own.
- Legal notices: the source link in the About window is a hyperlink and the window scrolls in a
  very small window. `p4kx -V` and `p4kx-mcp -V` show the legal notice like `--version`. The MCP
  tools that list Entries, chunks, Records, structs and enums say that their content may be
  forwarded and must not be redistributed. The test-channel notice calls the Channel one that
  carries test builds, and the Comparison tab uses the plural form for two test Channels. The notice for EPTU adds
  that its content may be covered by a non-disclosure agreement.
  `docs/cli-json.md`, the README and the user help describe the notice.
- GLB export: the inverse bind matrices of a skeleton are composed from the relative bone poses, so
  they agree with the node transforms even for a file whose stored world poses are inconsistent.
- Loadout: ports that only the vehicle implementation declares are listed in the tree with the
  new `source` value `empty`, so `--replace` can attach an Entity to them. A script that checks
  `source` must accept new values.
- Models: a `CrCh` model with a layout the reader does not implement (a mesh in data streams, a
  big-endian chunk, another chunk version or physics primitive) now exits with code 1 instead of
  3, as unsupported rather than corrupt. A `Mesh` chunk of another version than `0x801` that a
  node refers to used to give a node without a mesh and exit code 0; `p4kx model info` and
  `p4kx model glb` now exit with code 1. `p4kx model info` lists a `CrCh` mesh that names
  physics data in further slots, which are not read.
- CLI: `p4kx wwise events` now exits with 1 when the Entity lookup fails (DataForge, key or
  Archive read error); it exited with 0 before. The triggers are still listed, and
  `--no-entities` still exits with 0.
- Model info: `p4kx model info` lists the animations in the model's folders from their headers
  without decoding any key, and finding the animation files of a model no longer compares every
  path of the Archive on one thread (about 110 ms down to about 5 ms on LIVE). An animation
  in a model's folder whose key format the reader refuses is now listed in `animations` instead of
  `unread_animations`; the files `--anim-from` names are still decoded in full, so there it stays
  unread and an unreadable file still exits with 3. The `reason` of an unreadable file starts with
  the Entry path or file, then the cause, without repeating the file name inside the cause.
- CSV: breaking for scripts that read or compare the default CSV of `dcb query`, `dcb fields` and
  `loc export`; `--csv-raw` gives the old bytes. The default CSV of all three now starts with a
  byte order mark (`dcb query` and `dcb fields` wrote none before; `loc export` already did) and
  puts a `'` before a text that a spreadsheet would run as a formula (`=`, `+`, `-`, `@`, tab,
  carriage return); numbers stay unchanged. The Query export of the desktop app follows the same
  rules.
- Field statistics: a Reference now has a distinct count, a Record with too many values adds
  nothing to the counts, and the memory of a count is bounded by the number of threads. A field
  path can be turned into a Query path, and a type name with a backtick is rejected.
- Public patch: the RSI Launcher log is read once per detection or command and by `p4kx-channels`
  only. Public patches come from the last 16 MiB of each log file and lines up to 64 KiB, and the
  most recent 4096 changes are kept; library folders are still read from the whole file, with lines up to 1 MiB. The
  comparison and schema drift reports and the MCP `list_channels` and `archive_info` now state the
  public patch of each Build; the report schemas list `public_patch` as optional. The MCP server
  keeps the patches until a log file changes.
- Log sources: `p4kx channels` reads the launcher folder of `P4KX_LAUNCHER_DIR`; `info`, `catalog`,
  `ships`, `compare` and the packages read `P4KX_LAUNCHER_LOGS` or the launcher's own log folder.
- Library API: `Build::with_public_patch` now takes the value and reads nothing,
  `Build::with_public_patch_from` is removed, `schema_report` takes the public patches as a fourth
  argument, and `CatalogOptions`, `ShipBaseOptions` and `CompareOptions` have a new public field
  `public_patches`.
- Basetool catalog: a blueprint with a `CraftingCost_Select` nested deeper than 32 levels is no
  longer exported with its cost tree cut off; it is left out of `blueprints` and counted as a
  failure (stderr, `failed` in `--json`, the summary), so `p4kx catalog` exits with code 1 for such
  a DataForge. This belongs to the unreleased catalog version 2; no blueprint of LIVE is that deep.
- Vehicle implementations: an item port now tells its types with their sub-types, its display name
  and where each of its values comes from; the Modification's applied `Elem` overrides and the file
  a value was read from can be asked. New API: `ElemOverride`, `VehicleItemType`,
  `VehicleImplementation::{elems, elem_overriding, source_path}` and
  `VehicleItemPort::{item_types, display_name, origin_of}`.
- A patch with several children of one name (two `Parts`) keeps all of them, the first replacing
  the base's first child of that name (further base children of the name stay); before, the last
  one won. A choice not tested on real data.
- Text XML nested deeper than 128 elements is refused by the material, object container, `.soc`
  entity, audio trigger, shop and star map readers, as it already was for vehicle implementations
  and Loadout files. The material, object container and `.soc` readers return the new `TooDeep`
  error variants, the audio reader gives a reason string, the shop and star map readers treat the
  file as unreadable. New API: `p4kx_cryxml::{check_text_depth, MAX_TEXT_DEPTH, TextTooDeep}`.
- Breaking: `PartKind` and `PortVisibility` are `#[non_exhaustive]`; matches on them need a wildcard.
- Record JSON Schema: the `tag` key is allowed only on a Reference of type `Tag`.
- Model preview: the animations next to a model are read after the model, so the 3D view appears
  without waiting for them. The Info tab shows "Reading…" until they are counted, reads at most
  64 files or 64 MiB of them ("Not read" beyond that), stops reading when another Entry is
  selected, and reads none for a model without bones. Export as GLB lists an animation none of
  whose tracks matches a bone of the model under "Animations not written".
- Docked vehicles: the `NOITEM_Vehicle` marker of an item type ignores case, and the Records
  below a docked vehicle are no longer read ahead when docked vehicles are not expanded.
- Basetool catalog (breaking for the Basetool, part of `schemaVersion` 2): ships whose record path
  or name holds the word `temp`, `display` or `hijacked` are left out and counted as skipped, 21
  ships on LIVE, 15 of them `player` (ADR-0167). The other kinds are not affected. The Basetool
  must accept version 2 before it reads the new catalogs.
- `p4kx ships` and `build_ship_base` apply the same rule, which is a breaking change of the ship
  base data: its `schemaVersion` is now 2 and the 21 ships are no longer listed
  (`docs/ship-base-data.md`).
- Basetool catalog: the summary window shows how many items and ships are `player`, `ai`, `mission`
  and `special`, and how many are unclassified, in German and English.
- Basetool catalog: the rule that assigns `categoryGuid`, `parentGuid` and `typeGuid` changes, a
  breaking change covered by the `schemaVersion` 2 bump (ADR-0054). A blueprint whose category is a
  left-out leftover or unreadable now has a `null` `categoryGuid`, and a group names a parent or a
  `ResourceType` only through the groups the `resourceGroups` kind selects, so no reference points at
  an entry the catalog lacks. No value changes on LIVE. The documentation states that the groups form
  a forest of unfixed depth.
- Basetool catalog: the reward sources of a blueprint pool come from one scan of every Record
  instead of the reference index (ADR-0154). Nothing is capped any more. The warning
  `UnreadableReferenceRecords` became `UnreadableRewardRecords`, which counts only unreadable
  Records of a type that holds a reward source elsewhere, or every unreadable Record when no source
  is found; an unrelated unreadable Record no longer makes `p4kx catalog` exit 1. In
  `p4kx catalog --json` this is a breaking change of `warnings`: the kind
  `unreadable_reference_records` is now `unreadable_reward_records`.
- `p4kx-export`: `CatalogProgress` has a `phase` (`CatalogPhase::Crafting` or `Records`), and
  `CraftingWarning` has `kind()`, `count()` and `details()`. `p4kx-dataforge`: the
  `ReferenceIndex` constructors with a limit or a stop are replaced by
  `build_with(forge, limit, stop)`.
- Basetool catalog: a gameplay property whose name is a placeholder key keeps the key in `name`,
  like items and blueprints. None of the 29 properties of LIVE has one, so no catalog of LIVE
  changes.
- Basetool catalog: `schemaVersion` is now 2, a breaking change for the Basetool import. A paint
  lists only the ships whose port and Record tags offer the tags the paint itself requires, so
  `paints[].shipGuids` has 40 fewer entries on LIVE 4.7 (ADR-0161); the Basetool must accept
  version 2 before it reads the new catalogs.
- Library: `p4kx_export::Kind::rule()` returns `Option<KindRule>`, `None` for the derived kinds
  `Paints`, `Loadouts` and `Shops`; `CatalogSummary::skipped()` no longer adds the leftovers of the
  derived kinds twice.
- Ships: `p4kx ships` lists numbers of which only a prefix was read (`12,5x` as 12) in `partial`
  next to `unreadable`, names the XML Loadout file of a ship's default Loadout in `loadoutSource`,
  gives each Loadout port the Record or XML Loadout file that supplied it in `suppliedBy`, and
  rounds the `health` of hull thresholds like their `damage`. It checks `--out` before the
  Archive is read. Invisible format characters of the whole Unicode category Cf are dropped from
  numbers.
- Basetool catalog: the summary states the size as "{compressed} compressed (of the upload limit
  of {limit}), {size} uncompressed". The progress window shows its bar only while Records are
  checked, not at 100 % during the long write. `p4kx catalog --out <file>.gz` is refused with exit
  code 2 instead of silently writing plain JSON. The user help says that the file name, not the
  file type of the save dialog, decides between `.json.zst` and plain `.json`.
- Catalog: the build reads the vehicle implementation of each ship once, takes the paint ports
  from it and shares one Entity index between the ship readers and the Loadout readers, which makes
  it faster. `ShipReader::with_entities` and `LoadoutResolver::with_entities` share an index;
  `LoadoutResolver::entities` is no longer a `const fn`; `VehicleKind` and `MassBasis` are
  `#[non_exhaustive]`.
- Basetool catalog window: the summary lists the loadouts with their ports, unresolved
  attachments, docked vehicles and Entities without an entry in items or ships.
- Basetool catalog: a ship whose own loadout cannot be resolved stays in `ships` and is listed
  under the failures with a reason that starts with `loadout: `.
- Catalog: the starmap positions read every reachable container once, in parallel, instead of
  reopening a container for each reference. A container without the main XML name and a second
  child list in one element now show up in the starmap diagnostics. `StarmapReport` has accessor
  methods instead of public fields.
- MCP server: `list_chunks`, `get_model_info` and `list_model_items` read at most two chunk files or
  models at a time, so concurrent calls cannot exhaust memory. `list_model_items` builds only the
  section it lists, and `get_model_info` no longer binds every animation. `list_chunks` accepts
  chunk files up to the same size as the CLI, and a missing Entry is named by its own path.
- GUI: changing the Loadout of an Entity in the Entities tab no longer reads the DataForge and
  indexes its Entities again for every click. One worker per Loadout does that once, resolves one
  change after the other and stops a resolution a newer change supersedes. Cancel now also takes
  effect while an Export of an Entity is still resolving its Loadout. The progress of that export
  shows a bar that fills only for the Interior, the one step that counts its work, and a sweeping
  block for the steps without a measure instead of invented fractions. The Loadout and export
  messages for a Record the DataForge lacks are translated.
- Assembly: `LoadoutChanges::set_replacement` refuses a port below an excluded one with
  `ChangeError::Conflict`, `LoadoutResolver::with_stop` lets `resolve_with` end with the new
  `ChangeError::Cancelled`, and the `EntityKind` documentation says that the item type counts
  for a weapon.
- 3D viewer: the preview panel of a model keeps its height on every tab and cannot be dragged so
  small that the playback bar covers the view. Play, the position slider and Loop are dimmed while
  nothing can play, and a graphics device that cannot skin a model no longer shows playback
  controls that do nothing.
- Entity Assembly: the Interior now includes the object containers of attached Entities, each
  placed relative to the Entity that names it, with an `Interior` node below that Entity. A docked
  vehicle brings its containers only when docked vehicles are expanded. The Interior of an Assembly
  without parts is written at the top of the scene. `EntityInterior::build` returns the new
  `AssemblyError::TreeMismatch` for a tree other than the Assembly's own. The `helper_not_found`
  and `offset_differs` texts name the Entity's model instead of the parent's or the hull.
- Interiors: with the DataForge, every model and prop without a palette of its own is drawn with
  the default tint palette of its material library, as the parts of an Assembly are, so existing
  Interior exports and the 3D view change their colours. Interior lights are brighter, because the
  stored colour is linear and is no longer converted a second time.
- Library: `SocEntities::other` no longer counts the entities that name a class Record; they are
  listed in the new `SocEntities::classed`.
- Entities: the 3D view of an Assembly samples textures from mip levels, so minified textures no
  longer shimmer, and draws glass from the far part to the near one. Changing the Loadout or
  the detail again reuses the models already read. "Show all" no longer counts hidden ports that
  were left out, and a replaced, hidden port shows both chips.
- Screens: `p4kx ui render --screen` lays out clones, lists and library templates, stack, grid and
  wrapping flex containers, anchor, pivot, rotation, scale, clipping, minimum and maximum sizes,
  automatic sizes from the content and rounded and chamfered corners. Raw canvases count fixed
  values in pixels of their own size, world-space canvases such as HUDs fit their root widget, and
  the aspect modes and scaled canvas widgets are applied. Lists draw their template once per row
  of a static array; rows that the game fills at run time are listed. What is still approximated
  is listed per widget type in the report (ADR-0106).
- Screens: `p4kx ui render --screen` evaluates the `Child` and `Count` selectors and the break
  conditions of `Ancestor` selectors in style entries, and applies the margins, flex growth and
  shrink, orientation offsets, width and height behaviours and flex settings that style entries
  set. A screen is drawn in its state at rest: the bindings of each canvas, calculated with every
  game variable at zero and the parameters its canvas widget hands it, decide which widgets are
  instantiated and active and what their alpha is, so the radar of a ship no longer shows every
  state at once. The `stylesheetOverride` of a canvas widget styles the canvas it holds. A widget
  that a style entry hides no longer leaves its room in the layout. Bound fields that are not
  evaluated, and root widgets that a binding takes out of the screen, are listed in the report as
  `Binding`, so many screens now exit with code 1 (ADR-0110).
- Characters: the hair, brows, lashes and beard of a head now follow its expressions. `p4kx chf
  glb` gives them a morph target for each expression that moves them (on the first mesh of each,
  at LOD 0 only), and a facial animation sets their weights with the head's; its budget counts
  these weights too, so a facial clip near the limit can now be refused. The GLB grows by some 25
  to 45 MiB with all expressions; `--no-expressions` or `--expression` keeps it small. The Blender
  package carries the hair targets too.
- Breaking: the repository moved to `p4kx-org/p4kx`. The Record XML namespace is now
  `https://github.com/p4kx-org/p4kx/record-xml`, and the Record JSON schema `$id` is now
  `https://github.com/p4kx-org/p4kx/schemas/record-json.schema.json`. Links, the update check, the
  About text, the installer and the Blender add-on name the new repository.
- Face animations: long clips are smaller. The sample reduction tolerates 0.5 mm of a joint
  translation, 0.001 per quaternion component of a joint rotation (about 0.1 to 0.2 degrees) and
  0.01 of a morph weight instead of 0.1 mm, 0.0001 and 0.001, so a 151-second idle clip drops from
  34.3 MB to 20.7 MB. A head skin whose stored control hashes do not match the control names is
  refused.
- GUI: every second row of a table is now striped in a lighter grey that reads across a wide
  table, and the tiles of the Comparison summary take the darker bar fill so they stand out from
  the panel.
- CLI: the hologram of `p4kx ui hologram` is now clearly see-through. Every surface of the model is
  drawn, weaker the deeper it lies, so the inner structure and the far side show through the hull
  and the edge glow stays on the silhouette; overlapping surfaces no longer make bright spots. The
  default `--opacity` is 0.25 (it was 0.22) and, with `--glow`, caps how opaque the shell can get.
- Loadouts: a Loadout with a `Char_Head` Entity now exports that head instead of the generic
  Protos head, and `--body` is no longer required when the Loadout names a head.
- Screens: `p4kx ui render --screen` names the Record field (`imageSource`, `svgPath`,
  `urlOptional`) that would name the graphic the game picks at run time, for the widgets it lists
  as filled in at run time.
- Characters: animations left out for the same reason are one `animation_not_used` issue with
  their count and the names of the first three, instead of one issue each. `scene.json` of a
  Character package with `--anim all` no longer grows with the number of animations. The
  `skipped animation … of <file>` lines of the CLI are summed up the same way per reason (#584).
- Wwise: trigger resolution follows Play and PlayEvent actions into the soundbank they name or
  the one that holds the Event, chosen by the audio language, and follows Trigger actions to the
  segments of music stingers. A walk reads at most one other soundbank, and at most 8 PlayEvent
  and Trigger actions may follow one another (new break kind `chain_too_long`). On LIVE 22 more
  events resolve to sounds, 5 982 variants lead through music, and `other_soundbank` breaks fall
  from 238 to 2.
- Library: `BreakReason` has the new variant `ChainTooLong`; `p4kx-assembly` exports
  `MAX_ACTION_CHAIN` and `MAX_STINGERS`; `p4kx-testkit` has
  `TestHircObject::music_segment_stinging`.
- `p4kx-export`: `CraftingWarning` is no longer `Copy`, and `UnreadableVehicleFiles` and
  `UnreadableShopInventories` are struct variants with `count` and `details` instead of tuple
  variants, which breaks a `match` on them.
- Record export: the flat value export carries the Record tag of a version 8 DataForge. The CSV
  gets a last column `record_tag`, so its header changes (every existing column keeps its
  position; the tag is the ninth column, or the tenth after `text` with `--lang`); NDJSON rows get
  an optional `record_tag` key (`docs/record-flat.md`, ADR-0112).
- MCP: the containers kept open per Archive have a byte budget (256 MiB, counting the decoded
  bytes they keep in memory; none above 128 MiB is kept), a container is decoded outside the cache
  lock under its own permit, calls for the same container share one decode and its error, and a
  path inside a container is found through its index. `entity_loadout` resolves the Loadout of an
  Entity once while its pages are read, within a budget of ports.
- Archive: `Archive::resident_len` returns the decoded bytes an Archive or container keeps in
  memory.
- MCP: `get_wem_info` pages its `locations` with `cursor` and `limit` instead of stopping at 50,
  and a WEM selected by `path` reports its language folder. `get_entity_sounds` and
  `get_wem_info` reject an out-of-range cursor before they build any item (#625).
- Models with textures: the textures decoded at once are bounded to 2^27 pixels by their headers
  inside the glTF writer, whatever the loader allows per texture, so a hull with many large
  materials holds at most about 1 GiB of them instead of about 3.5 GiB. Cancelling a GUI export also stops
  the texture header reads (#633).
- Catalog: where several parts of a vehicle file share a port name, the first part now gives the
  port its sizes, types and flags, as in the Loadout tree. On LIVE the `Radar_Helper` port of eight
  Ursa ships no longer lists `start_hidden`. Part of the `schemaVersion` 2 change.
- Browser: clicking a column header on an Archive with about a million Entries redraws the table
  faster, because the sort no longer recounts every Entry or reallocates the row list.
- Basetool catalog: every `ResourceType` that a quality quantization links to is listed as a
  commodity, so each `resourceTypeGuids` entry resolves to a `subtypeGuid`. This belongs to the
  unreleased catalog version 2.
- Loadout replacement: a replacement that does not fit the port's item types, sub-types or sizes
  is no longer refused. `--replace` of the `entity` and `ui hologram` commands attaches it, warns
  on stderr, lists it as `incompatible` in the JSON and exits 0; the replacement chooser of the
  Entities tab lists every Entity, the fitting ones first and the others marked "Does not fit",
  and the Loadout panel and the export summary of the Entities tab warn about a replacement that
  does not fit. The data of LIVE does not keep its own port definitions, so a strict check
  refused items the game ships. A port whose largest size is 0 beside a smallest size of 0 or
  none states no limit, and an item of size 0 counts as an item without a size.
- Entities: with the Interior on, changing the Loadout or the detail again reuses the Interior's
  models already read instead of reading them from the Archive once more.
- Assembly: 538 of the 1144 ships and ground vehicles of the LIVE Archive now draw the right-hand
  model on their right ports (Gladius, Hornet, Idris, Constellation and more). This changes
  `entity glb`, the packages, the viewer and the holograms. The Hammerhead pairs its main thrusters
  diagonally because its data says so, and the NPC Miles Eckhart gains one head part with a
  model in `entity glb`.
- Basetool catalog: an unreadable `ItemRecoveryConfigurationParams` Record is now listed in the
  failures of the build instead of being skipped silently, so `itemRecovery` being `null` with a
  failure differs from `null` without one. The Record is selected by the path and type rule of a
  kind, under `entitlementpolicies/`. The numbers of `itemRecovery` and the wait times of
  `insurance` also read integer and double values. The documentation marks the meanings taken
  from field names as inferred and states that the catalog holds no claim fee.

### Removed

- Models: `CorruptChunk::BigEndian`, `CorruptChunk::StreamedMesh` and
  `CorruptChunk::PhysicsPrimitive` in `p4kx-model`; the layouts are now
  `ModelError::Unsupported`. This is breaking for an exhaustive `match` on `CorruptChunk`.
- `p4kx catalog --json`: the warning kind `reward_sources_truncated` no longer exists, because the
  reward sources are no longer capped (ADR-0154). `p4kx-export`: `read_crafting_limited` and
  `CraftingWarning::RewardSourcesTruncated` are removed.
- Record writers: `RecordXml::write_with`, `RecordJson::write_with`,
  `RecordXml::write_element_with` and `RecordXml::write_wrapped_element_with` in `p4kx-dataforge`
  (also re-exported by `p4kx-extraction`). Call `write_annotated(tree, texts.into(), out)`,
  `write_element_annotated(tree, texts.into(), depth, out)` or
  `write_wrapped_element_annotated(tree, texts.into(), depth, out)` instead; `texts` is the same
  `Option<&dyn TextResolver>` and the output is unchanged.
- Loadout changes: `ChangeError::Incompatible` in `p4kx-assembly`; a replacement that does not
  fit its port is applied and returned by `ChangedLoadout::mismatches`. This is breaking for an
  exhaustive `match` on `ChangeError`. It is also breaking for scripts: `--replace` of `entity
  loadout`, `entity glb`, `entity package` and `ui hologram` with an Entity that does not fit the
  port used to exit with code 2 and print no JSON, and now exits with code 0 and lists the
  mismatch in `incompatible`.

### Fixed

- Verification: a checksum mismatch of the data is no longer replaced by a read error of the
  stored bytes that follow it. A hash block (`0x5003`) of another length than 32 bytes is reported
  as an Entry issue.
- Read-only guard: the refusal names the installation as `C:\Games\StarCitizen` and no longer with
  the `\\?\` prefix. Library and Channel folders added by hand to the settings are protected in the
  window and on the command line when they hold the game's `Bin64` folder or a child that does; a
  folder without it stays writable. A manually added Archive counts as an installation when its
  folder holds `Bin64`. The running-game notice says that the RSI Launcher updates the Archive.
- Entity export: thrusters and other parts whose geometry is a character definition (`.cdf`) are
  drawn with their skin instead of being reported unreadable, and the other attachments are
  listed as `attachments_left_out`. A model that has only levels above the requested one is
  drawn at its lowest level. A part below a parent without a model no longer reports a missing
  helper, and the vertex budget holds within a wave of models.
- Model textures: the image budget of a GLB is checked from the texture headers before anything is
  decoded, each source texture is decoded once though two images use it, and a normal map whose
  alpha tail has fewer mips is exported at the tail's last mip instead of being lost.
  `p4kx model glb --materials textures --json` for a model file without an Archive reports one
  `textures_note` instead of a fallback per texture.
- CLI: `p4kx wwise events` gives the reason as `entities_skipped` in the JSON when the Entities
  cannot be looked up, so that `entities: null` differs from `--no-entities`. It warns about Entity
  Records that cannot be read (`unreadable_records` in the JSON), skips reading the DataForge for
  an empty page, and, like `p4kx entity audio`, ends quietly on a closed pipe also while it
  prints the warnings.
- Content search: a search started right after a cancelled one waits until the cancelled one has
  ended instead of running beside it, and cancelling stops the read of DataForge. The memory
  budget of 256 MiB now counts the tables parsed from a `CryXml` Entry and the decoding buffer of
  UTF-16, so a search over all text Entries of LIVE peaks at about 140 MiB of heap instead of 390
  MiB. A `CryXml` file whose tables overlap is refused as corrupt. A lone carriage return ends a line, as it does in the preview. `i` finds `İ`, and the final
  sigma `ς` matches `σ`. Separators, zero-width characters and the byte order mark are replaced in
  excerpts, and `p4kx grep` aligns wide characters in its table (#387).
- `ui render`: two imports of one font name from different movies keep their own fonts, and a
  movie's own export of that name no longer replaces an imported one. The import budget counts a
  movie by the uncompressed length it declares and refuses an oversized one before decompressing
  it. `--lang` also picks the localisation folder for imported fonts, and whitespace in a text
  field no longer draws boxes when its font has no outline for it.
- Vehicle implementations: a file nested exactly one level deeper than the 128 allowed was read when
  it was CryXml but refused when it was text; both now read 128 levels and refuse 129.
- Tags tab: reading the tag tree stops when the Archive changes on disk, is closed or another one
  is opened, instead of running on in the background. While a search filters the tree no row shows
  a chevron, because it could not expand anything. The search finds tags whose paths hold umlauts
  and other non-ASCII letters whatever their case. The tab lays out its texts and rows once instead
  of every frame, which takes about a third of its allocations away.
- `p4kx dcb tags`: the last line reads "4 of 4 tags" instead of "4 tags of 4 tags".
- Vehicle implementations: the memory a vehicle XML and its patch may use now counts the size of every element and attribute record, so a small hostile file can no longer expand to hundreds of megabytes.
- Basetool catalog under a `--filter`: a blueprint no longer links a dismantle process the filter
  left out, and a Record outside the filter that cannot be read is no longer listed as failed.
- Basetool catalog: a `ResourceType` that only a cost of a later tier uses becomes a commodity too.
- Catalog: a paint no longer lists ships whose port and Record tags lack the tags the paint itself
  requires (40 wrong assignments on LIVE), and the leftover paints are counted as skipped in the
  `paints` line.
- Loadout changes: a replacement by the root Entity or by an Entity the port lies below is now an
  error (exit code 2) instead of a silently replaced port. Where docked vehicles are not expanded
  (`entity glb` by default, `entity package`, `ui hologram` and `entity loadout
  --stop-at-docked-vehicles`), a change to a port below one no longer fails; it is not applied,
  warned about on stderr and listed as `ignored` in the JSON. The sub-types of a vehicle's item
  ports are checked when an Entity replaces what a ship hardpoint holds.
- `p4kx wwise extract` and `p4kx entity audio --extract` stop on Ctrl+C without leaving `.part`
  files and report the cancelled run (`cancelled` in the JSON, exit code 1). `--conflict
  if-changed` streams the sound into a temporary file instead of holding it in memory. A WEM that
  several variants lead to is exported from its full copy, not from a prefetched start, and the
  WEMs of one soundbank are converted in parallel.
- Armour: the notes `not_skinned` and `bones_not_in_skeleton` describe the level of detail that is
  exported. A piece worn through its main geometry, which is not a skin, gets the note
  `main_geometry_worn`. Nodes below a skinned mesh keep the transforms above it in the GLB.
- Models, Assemblies and the viewer: a material painted with a dark tint palette colour and a
  bright specular colour keeps its tint instead of turning into a plain metal.
- Sounds tab: opening another Archive while the audio is still being indexed stops indexing the
  old one. Clicking through the embedded sounds of one soundbank reads the soundbank once. A
  sound that should play but has no Entry to read says so instead of showing nothing. The
  column dividers of the trigger table end above the list footer, and the status and language
  columns are wide enough for their longest German and British English texts.
- Interior: the models of `.soc` files whose object table has a record length other than 208 bytes
  (14 % of the tables in the Archive) are placed completely; records of kind 16 are skipped
  instead of ending the list, and a placement that is no rotation with a scale is counted as
  invalid. A `.soc` file with an object table layout that is not known gives a `FileUnreadable`
  issue. `Interior::add_to_scene` returns `None` when the Interior has no parts.
- Armour from a Loadout: an unresolved attachment below a weapon or another item that is not worn
  no longer counts as a failed armour piece. A Loadout that a worn item cannot read in full, such
  as a missing XML include, is now listed as `loadout_unreadable` under the item's name without
  counting as a skipped piece. In the JSON report a `piece_not_found` for a Reference that cannot
  be read is now named `<port>/unreadable` instead of `<port>/<path of ports>`.
- Desktop: in a window narrower than 1020 points the sidebar hides behind a narrow rail with a
  button that shows it, and the work tabs scroll instead of squeezing their controls into each
  other, down to the minimum window size of 640 x 400. The Browse list no longer overlaps the
  preview below it, its extract buttons move below the search field when they do not fit beside
  it, and the Entry table fits its columns again after the window got smaller.
- Blender package: a move into place that fails because another program holds the folder open is
  tried again for a short while and then reports that the folder is in use. Meshes, textures and
  the folders are flushed to disk before the move, a failed run removes the parent folders it
  created, and a NaN or infinite number from bad source data becomes an issue instead of a `null`
  in `scene.json`. `p4kx entity package` reports `lod_fallbacks` like `p4kx entity glb`, each mesh counted once.
- Characters: the tangents and normals of a head shaped by its DNA now turn with the new surface,
  so normal-mapped shading no longer drifts on moved areas. A CHF whose body type does not belong
  to the head rig of its DNA is listed as the issue `body_type_mismatch`; the head is still made.
- Interior lights: the colours the game stores are linear and were converted from sRGB a second
  time, which made the lights too dark; they are written as stored. A scene node that holds a light
  and a model or children is no longer rotated by the light's quarter turn: the light sits on a
  child node.
- Blender package: a layer file shared by several materials is followed again whenever it is
  reached at a shallower layer depth, together with the layers below it, so the package no longer
  depends on the order of the materials. The material indices in the mesh GLBs are written by the
  GLB writer (`GlbOptions::material_indices`), so a package mesh keeps the key order and numbers
  of `entity glb`.
- Characters: a hair material choice that only sets the redness of the melanin is now applied
  instead of being ignored.
- Models, Assemblies and the 3D view: materials with `MatLayers` (the hull materials of ships) are
  drawn in the colour their layers give instead of black or white. The layer files are read, and
  the layers are blended by their tint or palette entry and the wear share of the material; a
  missing layer file is reported and the material keeps its own colours.
- Entities: with the Interior switched off, the extent, the mesh count and the issue count of the
  3D view leave it out, as the parts and triangles already did. Stop ends reading the Interior
  within a model file instead of after a wave of 256, and the model count of the progress ends at
  its total.
- Export: the export dialog keeps its top edge below the header when its height changes, instead
  of jumping between the GLB and the Blender package form. The progress and summary windows of an
  export open in the middle below the header and stay clear of the status bar; their body scrolls
  in a small window. The setup of an Interior container fits the default window without a scroll
  bar.
- Export: the summary of an Entity export counts its object containers, the summary of a
  container exported on its own the containers opened, and each says so; `p4kx entity glb` and
  `p4kx entity package` follow with "object containers".
- Characters: an expression on a head without a skeleton now moves the face the right way. The
  facts bar over a 3D view cuts a value that does not fit instead of running out of the bar, and the
  status row above the view only takes room while the build runs, is stopped or failed.
- Screens: two font styles that share a paint file but differ in name, weight or bold flag no
  longer share one font choice, and a text of automatic size is shaped once instead of twice.
- Eyes: the ambient occlusion and specular overlay materials of the eye shader are no longer
  drawn as opaque shells, which showed the eyes red with a tiny pupil.
- Armour and characters: the body no longer shows through worn clothing. The body zones that the
  clothing's Records cover (`SCItemClothingParams`) are left out of the body skin in `armour glb`,
  `character package` and the viewer, and listed as `body_zones_hidden`; a body skin that does not
  fit the zone table stays whole (`body_zones_unknown`). With `--materials textures`, layered
  clothing materials without a diffuse map are listed as `layers_without_diffuse` and drawn in
  their layer colours (ADR-0140).
- `p4kx catalog`: the warnings `unreadable_vehicle_files` and `unreadable_shop_inventories` name
  the files and why they cannot be read (`details` in the JSON summary). A vehicle implementation
  the Archive lacks is no longer counted as an unreadable vehicle file.
- Textures: the plain normal map that a layered material keeps as a fallback is no longer made,
  and no longer takes one of the 256 images, when the layered normal image was made. Exports near
  the cap give the slots to the materials that need them.
- Rendering: the offscreen renderer no longer waits forever for the graphics driver. An adapter or device request or a picture read-back that stays unanswered for 60 seconds fails with an error, and the test runs stop a render test that is still running after three minutes.
- Layered materials: the blend mask of a `LayerBlend_V2` material takes its channels by
  precedence (red over green over blue), so armour such as the guard's ORC-mkV is black with
  yellow stripes instead of half white. In the Blender add-on a painted layer's specular map no
  longer makes metal, the wear from vertex colours is no longer inverted, a packed detail map
  tints only through its diffuse channel, and the vertex colours no longer tint Character
  materials. A scripted Character import without a window context now brings in the armature.
- Assembly: a part takes the sub-geometry that its port's flags name (`left`, `right`), so the
  side doors of the Cutlass Black are drawn and the right thrusters of the Prospector sit on the
  right instead of repeating the left model. When that sub-geometry is not in the Archive, the part
  keeps the main geometry of its Entity and the new note `sub_geometry_missing` names the file.
- Armour: worn pieces are drawn with their tint palette, the one their geometry names or the
  default of their material library, so that layers that take a palette entry, such as the plates
  of the Defiance Core Tactical, are no longer white. `armour glb`, `character package`, `chf glb`
  and the Characters tab are affected.
- Shading: a palette entry with a bright specular colour paints a metal again, so the MISC
  Prospector is brushed silver and not glossy black; usual paints such as the Cutlass Black stay
  dark paint, in the glTF export and in the Blender add-on.
- Models and the Blender add-on: decals that draw no colour are no longer drawn as opaque white
  panels and lines. A mesh decal without any texture and a relief-only decal with a diffuse opacity
  of 0 are hidden, so the Cutlass Black's engine pods and the ANVL Arrow's panels show their
  surface again.
- Character package: a worn backpack without bone weights now hangs on the bone of its port on the
  back of the armour and follows it, instead of lying at the Character's origin between the feet.
- Assembly: the landing gear of a ship are written as parts (`landing_gear/<bone>`) with their
  model placed at the bone of the ship's model, in GLB, package and viewer, and deployed: posed at
  the end of the extend clip of the gear's skeleton. A gear without such a clip keeps the pose of
  its model and is listed as `gear_not_deployed`; a gear whose bone the model lacks sits at the
  origin and is listed. In a package they are nodes of the kind `object`.
- Characters: the scalp and stubble shells of hair and beards are no longer drawn as opaque black
  skins in GLB exports and Blender packages. They are blended by their density map, taken from the
  character's own body. Without the map they are transparent, and a GLB export lists the missing
  file as a texture fallback.
- Entity: items on ports the vehicle implementation or the port definition marks `invisible` are
  no longer drawn in the GLB, the package and the viewer (the Prospector's components no longer
  stick out of the cabin walls or hang below the hull). They stay in the Loadout tree and in the
  part list, marked hidden, with no model and no object containers; `entity glb --json` reports
  `hidden_parts` and `hidden` per placement. A Modification that clears the flag draws the item
  again.
- Contract change: a port whose definition carries the flag `invisible` and that the vehicle
  implementation does not name now reports `invisible`; it used to be `unspecified` (absent in
  JSON). This applies to the catalog's `loadout.ports[].visibility`, to `p4kx entity` (text and
  JSON) and to the MCP loadout.
- Entity models: an item port of a model with a skeleton, such as the drill arm of the Prospector,
  is anchored to its helper when that is a bone of the skeleton. The mining laser now hangs at the
  tip of the arm instead of inside the nose, and the part is no longer listed as `helper_not_found`.
- Entity: an attached part that names no tint palette of its own (side doors, ramp, landing gear)
  now takes the palette of the ship it hangs on, so the Cutlass Black's doors and the gear of the
  Cutlass Black and the Arrow are no longer light grey or white. Parts with their own palette keep
  it, and the default palette of a material library applies only where no ancestor has one.
- Entity assembly: the clothing of a character or NPC takes the sub-geometry of its body (`Male`,
  `Female`), as armour does. `entity glb` and `entity package` of Miles Eckhart no longer draw the
  jacket as a placeholder box and now draw his shirt, pants, boots, hair and teeth. Ships and
  vehicles are unchanged (ADR-0219).
- Models and the Blender add-on: the Hammerhead's hull and thrusters are no longer purple. A packed
  detail map in a material's own diffuse slot (`aegs_ext_detail_01.tif`) is a detail map, not a
  diffuse map. White relief decals (a `MeshDecal` with parallax occlusion mapping, a displacement
  map and no tint) are hidden, so the white contour lines on the Hammerhead and the Gladius are
  gone.
- Characters: the eyelashes are no longer white in `p4kx character package` and `p4kx chf glb`. A
  hair material the Character made no choice for, such as the lashes, takes the colour of its own
  shader parameters (ADR-0226).
- Query: the Record count below the result table no longer paints over its last rows in a window
  of medium height.
- Entities: in a narrow window the name of the Loadout and its changes no longer overlap; the
  changes move to a second line when they do not fit beside the name.
- Entity assembly: the body of an NPC no longer shows through its clothing. `entity glb` and
  `entity package` hide the body zones that the worn clothing covers, by the rule of the character
  package, and report them as `body_zones_hidden` (ADR-0228).
- Entity assembly: clothing of a character that takes a body's sub-geometry no longer gets the
  material library or tint palette of the placeholder box; a sub-geometry file the Archive lacks
  is listed as `body_geometry_missing`, and a port-flag choice that equals the main geometry is no
  longer overridden by the body. `entity glb` reports the body and the head that names it.
- Assembly: a part on a skin port, such as an NPC's clothing, is drawn in the body's bind pose.
  Its helper and offset no longer move it, so Miles Eckhart's pants are centred on the body, and
  it gets no `helper_not_found` note. The part reports the new placement `skin`.
- Models and the Blender add-on: white relief decals are hidden only while their diffuse map is
  grey. Decals with a coloured map (switch buttons, damage impacts, trim, headlight glows) are
  drawn again. The Blender add-on no longer repeats the rule.
- Models and the Blender add-on: decals the game fills at runtime (`$RenderToTexture`: registration
  marks, name plates, serial numbers) are hidden instead of showing as white strips. Layers that
  have a tint but no palette entry draw as grey metal with detail instead of white.
- Assembly: a skin port without a helper node is now recognised as a skin, so its part is drawn
  in the bind pose and reports the placement `skin`.
- GUI: the Localization and Tags tables no longer run under the footer in a short window.

## [0.3.0] - 2026-10-24

The third Release, and the first as P4KX: the tool and its repository are renamed (formerly
P4K-Extractor, now `greluc/P4KX`) and get their own logo, and the settings folder moves to `P4KX`
with a one-time copy.
The Basetool catalog becomes a full game-data export: ship fields, loadouts, paints, item stats
including armour, clothing, radar, tractor beams and salvage, variant groups, the starmap with
world positions, contracts, mineable elements and refining processes, and shops with their item
lists; the catalog is written as `.json.zst` for the upload. New tools: `p4kx coverage` and the
schema drift of `p4kx compare --schema`.

Two breaking changes: the Archive key is no longer part of the tool and is entered once, and the
Record XML namespace and the Record JSON schema `$id` now use `https://github.com/greluc/p4kx/`.

### Added

- P4KX has its own logo, data brackets around an X: the window and taskbar icon, `p4kx-gui.exe`,
  `p4kx.exe`, the installer and its Start-menu shortcut show it, and the header shows its mark
  before the wordmark. The logo files and rules are in `docs/design/` (ADR-0075).
- Streamed and localised sounds. `p4kx wwise resolve` locates a streamed or prefetched WEM as the
  loose WEM Entry named by its ID below `Data\Sounds\wwise\` and resolves localised events,
  soundbanks and WEMs for `--lang <language>` with `--fallback-lang` (`English(US)` by default),
  both language folders read from the Archive; each variant states whether it is localised, the
  language it was resolved in and whether that is the fallback, and a localised sound without a
  WEM in either language is listed as a `no_localisation` broken link. `p4kx entity audio
  --sounds [--lang] [--fallback-lang]` lists the variants of every trigger with source and
  location. `ArchiveAudio::resolve_trigger_in`, `LanguageChoice`, `AudioIndex::languages`,
  `AudioIndex::loose_wem`, `WemLocation::Entry` in `p4kx-assembly`,
  `SoundSource::is_language_specific` in `p4kx-audio` (ADR-0033).
- Models: levels of detail, tangents and vertex colours. `p4kx model info` lists every LOD with
  its vertex and triangle counts and whether its meshes have normals, tangents and colours;
  `p4kx model glb --lod <n>` exports the level of detail `n` (0 by default; a level the model
  lacks exits with code 2) from the geometry companion `<stem>_lod<n>.<extension>m`, with the
  nodes of the base model file. The GLB now carries `TANGENT` (with handedness) and `COLOR_0` for
  the meshes that have them; `--no-vertex-colours` leaves the colours out. `Model::read_with`,
  `LodSelection`, `Mesh::tangents`, `Mesh::colours` in `p4kx-model`, `GlbOptions::vertex_colours`
  in `p4kx-gltf`. Real-Archive tests check the tangents against the texture axes, the LODs of 360
  models and the node chain against the `MeshInfo` box (docs/model-lods-and-vertex-attributes.md,
  ADR-0028). The vertex colours are read as BGRA; models whose names say `blue` are mostly blue
  and those named `red` or `orange` mostly red under that order, which a real-Archive test checks.
- Entry checksums: `p4kx verify <archive> [--filter <filter>]` checks the data of all or the
  filtered Entries against the CRC-32C of the Archive directory in parallel and reports every
  mismatch (exit code 1, `--json` lists them as `mismatched`); `p4kx extract --verify` and
  `p4kx socpak extract --verify` check each Entry while it is written, do not write a mismatching
  Entry and count it in the summary; `p4kx socpak verify` does the same with the plain CRC-32 of
  a container. The checksum is computed while the data streams, without a second pass, and
  reading stays unchecked unless asked for. `Archive::open_entry_verified` and
  `ExtractOptions::verify` are the library switches.
- socpak containers in the path tree of the window: a container expands like a folder, opened in
  the background with a spinner and cancelled by collapsing it, and nested containers expand too.
  Selecting a container lists its Entries for search, preview and the extraction of the selected
  or shown Entries. Opened containers are kept per Archive within 1 GiB of memory, closing the
  least recently used collapsed ones first; a corrupt container says so on its row, and a change
  of the Archive on disk closes them.
- Audio Conversion during Extraction: with `p4kx extract --convert` and the Conversion switch of
  the window, WEM sounds are written as Ogg (Wwise Vorbis and Opus without re-encoding, WAV for
  other codecs), as WAV or unchanged (`--audio ogg|wav|raw`, the same choice in the Extraction
  setup, remembered in the settings file). Each soundbank is written unchanged and unpacked into
  a folder of the same name, its WEMs named by ID and converted alike. A sound whose codec cannot
  be decoded is reported with the codec and written unchanged; the Extraction continues. Progress,
  cancellation and Conflict policies cover every converted file, a cancelled run lists the sounds
  of a soundbank it did not unpack, a soundbank folder exists only when a sound is written into
  it, and sounds are converted on their own threads within a memory budget.
- `p4kx ui render <archive> <entry> --out <file.png> [--size <W>x<H>|<W>] [--background
  transparent|movie|#RRGGBB[AA]]`: renders the first frame of a UI movie with its sprites as a
  PNG, drawn with tiny-skia. Shapes with solid, gradient and bitmap fills (external images and
  sub-images included) and strokes are drawn with the transforms, colour transforms and masks
  of their placements, morph shapes at their ratio. Texts, buttons and videos are skipped,
  filters and blend modes are not applied, and every element left out is listed with its
  reason, plain or with `--json` (exit code 1). Placements, paths, shape conversions, masks,
  gradient stops, sprite nesting, the canvas size and the area the fills cover (64 times the
  canvas) are bounded. Also for movie files on disk.
- `p4kx-mcp` DataForge tools: `get_record` returns a Record by GUID, or every Record of a record
  path, as Record JSON; `search_records` finds Records by record path and type name with Filters,
  in Record order; `list_structs` and `list_enums` list the struct definitions with parent and
  fields (data type, array, target struct or enum) and the enums with their values; `run_query`
  runs a Query and returns its typed columns and one page of rows as `p4kx dcb query --format
  json` writes them, the failed Records and the unresolved References, and an invalid Query as an
  error with its byte span. Every result pages with a cursor and holds at most 256 KiB of items;
  a single Record or row larger than that is an error. The DataForge is read once per open
  Archive, kept open with its indexes and read again when the Archive file changes. A Query is
  limited to 64 columns and 128 comparisons, selecting its Records stops after 30 seconds or when
  the client cancels, and the selection is kept for the pages that follow.
- `p4kx wwise resolve <archive> <trigger> [--json]`: resolves an audio trigger to its sound
  variants. Each Wwise event is followed through its Play actions and random, sequence, switch and
  blend containers to the Sound objects; every variant is listed with its soundbank, WEM ID,
  source (embedded, prefetched, streamed or plug-in) and container path, and an embedded WEM is
  located in the soundbank that holds it, with its offset and length. Broken links (a missing,
  unreadable or unfollowed object, a cycle, or a limit of depth, fan-out or size) are listed with
  the object where the chain breaks. Soundbank containers of version 154 are now decoded to their
  children.
- Query tab: completion of struct and field names from the DataForge schema while typing, after
  `from`, `.`, `->` and in `[…]`, with the field types; and saved Queries that are kept in the
  settings, run with one click, renamed and deleted.
- Looping sounds: `audio info` reports the loop of a WEM, `audio wav` keeps it in a `smpl` chunk
  and `audio ogg` in `LOOPSTART` and `LOOPLENGTH` comments.
- Soundbanks in the Audio tab: select a BNK Entry to see the WEMs it embeds as a list of ID,
  codec, duration and size, and select one to see its facts and play it with the controls of a
  loose WEM, without unpacking the bank. A WEM the bank holds only the prefetched start of and
  plug-in media are marked and explained instead of played; a corrupt soundbank shows a message
  and the rest of the window keeps working. The preview opens taller for soundbanks, and the list
  scrolls with the keyboard.
- Models: materials as plain colours. A model's `.mtl` file (text XML or CryXml) is read into a
  `MaterialLibrary` of sub-materials with shader, surface type, flags, colours, opacity, glow and
  texture slots; the material ID of a submesh is the position of its sub-material, and a file
  without sub-materials serves every ID. `p4kx model glb --materials colours` (the default)
  writes one glTF material per sub-material with base colour, opacity, roughness, metalness and
  emissive colour and no textures; alpha-tested materials mask, translucent ones, decals and
  glass blend, `MtlFlags` two-sided draws both faces, and submeshes whose material draws nothing
  (the `NoDraw` flag or shader, opacity 0) are left out. A library the model names with a bare
  name is looked for beside the model first. A missing, unreadable or unsafe `.mtl` path is
  reported by `model info` and `model glb` (text and `--json`) while the model exports with a
  grey default material, as does a material ID beyond the sub-materials or an entry of the
  sub-material list that is no material. `model info` lists the materials. `MaterialLibrary`,
  `MaterialStatus`, `Model::materials` in `p4kx-model`, `MaterialMode` in `p4kx-gltf`
  (docs/model-materials.md, ADR-0032).
- Models: textured PBR materials. `p4kx model glb --materials textures` embeds the textures of
  the materials in the GLB as PNG images, found through the Archive's path index (a `.tif` name in
  the `.mtl` file is the `.dds` Entry, with its alpha tail): the diffuse map (`TexSlot1`) as base
  colour, the normal map (`TexSlot2` or `TexSlot3`, `_ddna`/`_ddn`) with its blue channel
  reconstructed and its green channel flipped to glTF's up, and a metallic-roughness image whose
  roughness is the inverse of the normal map's smoothness (its alpha tail) and whose metalness
  comes from the specular map (`TexSlot4`); a metal's colour is its specular colour, so with a
  specular map the base colour holds the tinted diffuse and specular colours. `--texture-mip <n>`
  picks the mip level, 0 (the largest) by default. Masked materials use the texture's alpha.
  An image several materials use is embedded once; at most 256 images and 2^29 pixels per file.
  A texture that is missing, has an unsafe path, cannot be decoded or does not fit leaves the
  material's plain colour and is listed in the summary (`textures`, `textured_materials`,
  `texture_pixels` and `texture_fallbacks` in `--json`), names sanitised for the terminal.
  `write_glb_with` and `TextureLoader` in `p4kx-gltf`, `Texture::from_index` and
  `ModelSource::write_glb_textured` in `p4kx-extraction` (docs/model-materials.md, ADR-0042).
- Models: skeletons and skinning. `.skin` and `.chr` files are read with their bones (parent,
  bind pose relative to the parent and in the model's space, controller ID) and, for skins, the
  weights of up to eight bones per vertex; the `CompiledBones` form of static models with animated
  parts is read too. `p4kx model glb` writes a skin with its skeleton as a glTF skin: the bones as
  nodes, `JOINTS_0`/`WEIGHTS_0` (and a second set for vertices with more than four bones), inverse
  bind matrices, all in Y up and metres like the mesh; a `.chr` is written as its bone nodes. A level
  of detail whose own `_lod<n>` file has a different skeleton is weighted to that one. `model info`
  lists the bones with parent, node and bind position, and marks skinned meshes (new `Skinned`
  column; `--json` has `bones` with both poses and `skinned` per mesh); `model glb` reports the bones
  and skinned meshes. `Bone`, `BoneTransform`, `Influences`, `Mesh::influences`, `Lod::bones` and
  `Model::bones_of` in `p4kx-model` (docs/model-skeletons-and-skinning.md, ADR-0039).
- Models: `p4kx model glb --anim <name>` writes the animations of `.caf` files and `.dba`
  databases as glTF animations on the bone nodes of a skinned model or skeleton, or on the nodes a
  static model's helper bones drive (a door, a turret). `--anim all` takes every animation found in
  the model's folders, `--anim-from` adds files to look in. `p4kx model info` lists the animations
  with their duration. Tracks for bones the skeleton lacks are left out and reported. Additive animations (offsets
  to a base animation) are left out and listed as skipped unless named.
- Chunk files: `ChunkFile::chunk_data_to_end` returns the bytes from a chunk to the end of the file.
- Models: the older `CrCh` container. `p4kx model info|glb` reads nodes (name, hierarchy, parent
  relative transform), the physics mesh a `Mesh` chunk refers to as LOD0 with one submesh, and
  the material library from the `MtlName` chunk of a `CrCh` model, and reports unknown chunks as
  for `#ivo`. The Archives hold one such model, a Carrack collision proxy; only its chunk layouts
  are read (docs/crch-models.md, ADR-0041). `p4kx model` prints node and material names with
  control characters replaced.
- Window: a model file (`.cgf`, `.cga`, `.skin`, `.chr`) gets an Info tab next to Model that lists
  what the file holds: nodes, meshes, levels of detail with their vertices and triangles, the
  geometry file, the material library and the material IDs in use, the bones, the animations of the
  `.caf` files and `.dba` databases next to it, and the chunks counted as read, skipped and unknown
  with the ones the model did not read listed by file, type and type ID. It works for a model the 3D
  view cannot draw, such as a skeleton file. Export as GLB…
  (at the right end of the preview tab bar and in the Export menu) writes the selected model as one
  GLB file: a dialog chooses the level of detail, the material mode, the vertex colours, whether to
  write the animations of a model that has some (additive ones are left out) and the file,
  which cannot lie inside a Star Citizen installation. The export runs in the background with a
  progress bar and Cancel, writes the file only when it is complete, and ends with a summary of what
  the file holds, the animations written and the ones left out, the chunks it skipped, the materials
  that fell back to the default material and, with textures
  (choose "With textures" and a texture size), the textures the Archive lacks;
  a failure shows a message and the rest of the window keeps working, and a missing Archive key
  opens the key dialog. The choices are remembered in the settings file. `ModelSource::write_glb_observed`
  in `p4kx-extraction` writes with the animations, progress and cancellation
  (`ConversionError::Cancelled`) (ADR-0047).
- `p4kx entity glb <archive> <entity> --out <file>`: exports an Entity as one GLB, its geometry
  and the geometry of every Entity of its default Loadout placed at its port. Each part is a node
  named `<port>/<entity>` below the node of the Entity it is attached to, placed at the port's
  helper in the parent's model with the port's offset. `--lod <n>` applies to every model (a model
  without that level is written at the nearest lower one) and `--materials colours` writes plain
  colours, with the material library an Entity's Record names replacing the one its model names.
  Parts whose Entity or geometry cannot be resolved are skipped and listed in the report, as text
  or with `--json`. Models are read once and in parallel.
- `p4kx-assembly` builds an `Assembly`, a scene graph of placed models, from a Loadout tree, and
  `p4kx-gltf` writes a `Scene` of placed models as one GLB, writing equal materials once.
- 3D viewer tracer: selecting a model Entry (`.cgf`, `.cga`, `.skin`) opens the new Model tab of the
  Preview, which draws the model's most detailed level with wgpu inside the window: lit, framed
  to fit and shown beside a facts bar (vertices, triangles, size in metres). Drag orbits, a right
  or Shift drag pans, the wheel zooms; with the keyboard focus the arrow keys orbit, Shift and the
  arrow keys pan, `+` and `-` zoom and Home resets; **Reset view** frames the model again. The
  model is read and flattened on a background thread (at most 4,000,000 vertices and 8,000,000
  triangles, files up to 256 MiB), its buffers are uploaded once and replaced when another model
  is selected, and a device that cannot draw the view or is lost gets a message while the rest of
  the window keeps working. Materials, LOD and skeleton switches follow (ADR-0031, design
  decision D.35).
- `p4kx wwise events [--search <text>]` searches the audio triggers and Wwise events of the ATL
  definitions by name (substring or glob, ignoring case) and lists for each result the trigger,
  its events, the soundbanks that hold them and the Entities that name the trigger. The output is
  plain or `--json`, in definition order and paged with `--limit` and `--offset`; `--entities`
  bounds the Entities listed per trigger and `--no-entities` skips the DataForge.
- `p4kx ui render … --svg`: writes the first frame of a UI movie as an SVG document that scales
  without loss, for the movie file or an Entry. Shapes become `<g>` elements placed with `<use>`,
  gradients, strokes, colour transforms and masks become the matching SVG elements, and bitmaps
  are embedded as PNG data; the output is deterministic, streamed to the file, and bounded by the
  limits of the PNG, so both end at the same element, and at most 1 GiB. The title is the Entry
  name, or the file name without its directory, escaped. A bitmap fill that does not repeat is
  drawn once, without continuing its edge pixels, and a stroke painted with a non-repeating
  bitmap tiles it. The `--json` report of `ui render` names the written `format`.
- `p4kx ui render`: static texts (`DefineText`) and text fields (`DefineEditText`) are drawn from
  the glyph outlines of the embedded fonts, with their size, colour and alignment, in the PNG and
  as paths in the SVG. Text fields honour the Flash HTML subset (`<p align>`, `<font face size
  color letterSpacing kerning>`, `<br>`), word wrap, margins, indent, leading and password
  fields. A character whose font or glyph is missing is drawn as an empty box and listed in the
  report (`missing_font`, `missing_glyph`, `missing_glyph_index`); no system font is used. With
  `--lang <language>` and an Archive, localisation keys (`@key`, with `,P=` parameters) resolve in
  that language's `global.ini`; an unknown key is drawn as it is and listed (`unlocalised`). A
  text field shows at most 4096 characters and a frame lays out at most 131072; the rest is
  listed (`text_truncated`, `too_many_glyphs`).
- Incoming References: `p4kx dcb refs <archive> <guid|record path> --incoming|--outgoing [--format table|json]`
  lists the Records that refer to a Record, or those it refers to, with the field and the array
  indices of every Reference, the References to a GUID no Record has apart; `--files` lists the
  edges between record files, `--unresolved` every Reference to a GUID no Record has. The window
  shows a *Referenced by* tab for the selected Record, a Query selects Records with `referencing
  "<GUID or record path>"`. One index over all Records, built in parallel on first use and kept
  per Archive, serves all of them.
- Content search: `p4kx grep <archive> <text> [--in records,loc,entries] [--filter <filter>]
  [--lang <language>,…] [--limit <n>] [--per-item <n>] [--format table|json]` finds a text, ignoring
  case and as part of a word (`Gladi` finds `AEGS_Gladius`), in the string, localisation key and
  enum values of all Records, in the localisation texts of each language and in the lines of text
  and CryXml Entries. A hit names the Record and its field path, the language and key, or the
  Entry and its line, with a short excerpt. Binary and oversized Entries and encrypted Entries
  without the Archive key are skipped and counted. The window gets a *Search* tab with the same
  three sources: a click on a hit selects the Record, opens the localisation key or selects the
  Entry. Each search scans in parallel without an index (ADR-0044); on LIVE it takes about a second
  for all Records, 0.15 s for all localisation texts and two seconds for all text Entries.
- Field statistics: `p4kx dcb fields <archive> [<type or Query>] [--path <glob>] [--format table|csv|json]`
  lists every field path of the Records, e.g.
  `Components/SCItemWeaponComponentParams/fireActions/SWeaponActionFireRapidParams/fireRate`, with
  how many values it has, in how many Records, of which kinds, how many are empty or distinct, and
  the smallest and largest number, to find the field a Query needs. The target of a strong pointer
  adds its actual struct to the path. One parallel pass counts all Records of LIVE in about two
  seconds.
- CLI: `p4kx coverage <archive>` checks a reference list of the game data a companion app reads
  against the Archive: for each Record type, component of an `EntityClassDefinition` and vehicle
  definition folder whether it exists, how many Records, instances or Entries it has, and whether
  they read, export and convert. The list is data in the repository; the result is a table or
  JSON ([docs/coverage.md](docs/coverage.md)).
- `p4kx ui render`: fonts that a UI movie imports from another movie (`ImportAssets`,
  `ImportAssets2`), which most text fields of the game name, are resolved in the Archive and the
  texts are drawn with their glyphs instead of boxes. The exporting movie is found from the import
  URL relative to the importing movie, with `.swf` and `.gfx` as alternatives and the UI and
  English localisation folders as fallbacks, read once per render, and followed through chains of
  imports up to 4 levels deep; the report lists the movies read and the fonts not found
  (`imports`). On the LIVE Archive 98.6 % of the text fields with imported fonts resolve, and the
  first frames of the movies with text lose 1388 of 1424 `missing_font` entries.
- `p4kx ui assets`: lists the imports (`imports`), symbol classes, nine-slice grids, the frame index
  of every frame label (`frame_labels`) and the known tags whose content is not read
  (`unread_tags`).
- Entity: `p4kx entity loadout` reads the vehicle implementation XML of a ship with the
  Modification its Record selects. The tree shows per port whether the implementation hides it
  (`invisible`), and lists the ship's treads, wheels and landing gear; the JSON has `vehicle`,
  `landing_gear` and a port `visibility`. `p4kx-assembly` offers `VehicleImplementation::read`
  for exports.
- `PortNode::nested` in `p4kx-assembly`: the ports of the nested Loadout of an empty port.
- Loadout: docked vehicles (attached Entities whose item type starts with `NOITEM_Vehicle`, such
  as snub craft) are marked in `p4kx entity loadout`, in the text and as `docked_vehicle`,
  `expanded` and `docked_vehicles` in the JSON. `--stop-at-docked-vehicles` leaves their ports
  out; `LoadoutResolver::stopping_at_docked_vehicles` does the same for library users.
- Entry SHA-256: every Entry of the Archives measured states a SHA-256 of its stored bytes (the
  `0x5003` block of the v1 directory), which `Entry::sha256` returns and `p4kx verify` and
  `--verify` check in addition to the CRC-32C, while the data streams. It covers the bytes before
  decryption and decompression, so it needs no key; a mismatch is reported with the kind `sha256`
  and 64 hex digits (ADR-0034, docs/verify.md). The 32 bytes of a v2 record are read and returned
  by `Entry::sha256` but not checked until a real v2 Archive confirms the layout. `Checksum::hex`
  was added in `p4kx-archive`.
- DataForge: the tag tree. Fields such as `Tags`, `RequiredTags` and `PortTags` refer to Tags by
  GUID; `TagTree` in `p4kx-dataforge` reads the `TagDatabase` into the tree of Tags with their tag
  paths (`Ship.Series.Avenger`), parents and children, and answers tag GUID to tag path. A Tag the
  `TagDatabase` does not reach is a root of its own; Tags listed twice, dangling children and
  cycles are counted and never fail (ADR-0052). The tree is reachable from `p4kx-extraction` and
  `p4kx-export` for the Basetool catalog kinds that match tags.
- `p4kx dcb tags <archive> [<tag>] [--tree] [--format table|json]` prints the tag tree, or one Tag
  and everything below it; `--tree` indents the names or nests the JSON. `p4kx dcb export --tags`
  writes the tag path next to every Reference to a Tag, as the `tag` attribute of Record XML and
  the `tag` key of Record JSON (`RecordExportOptions::tags`, `Annotations`); without it the output
  is unchanged (docs/record-xml.md, docs/record-json.md, docs/cli-json.md).
- The Tags tab of the window shows the tag tree with GUID and legacy ID, expands and collapses
  Tags, searches the tag paths and copies the path or GUID of the selected Tag.
- Comparison: `p4kx compare <old> <new> --schema` lists the structs, properties (data type, conversion type, target), enums and enum options that were added, removed or changed between the DataForges of two Builds, with the number of Records each concerns; `--json` prints the report, whose JSON Schema is `schemas/schema-drift-report.schema.json`. The Comparison tab shows the same drift in a new Schema list once the Record level was compared.
- Ship base data: `p4kx ships <archive> --out <file>` writes the hull mass, the hull points with
  the destruction and detach thresholds of the parts, the cargo capacity of the cargo grids of the
  default Loadout, `AttachDef.Size` and the default Loadout as class names per port path of every
  ship as JSON or NDJSON, each value with the file and field it was read from (the Record, the
  vehicle implementation, the Modification's patch file or its `Elem`). Docked vehicles are marked
  and not counted. `build_ship_base` and `ShipReader` in `p4kx-export`;
  `VehicleImplementation::totals`, `ValueOrigin` per part value, `VehiclePart::destroy_ratio`,
  `EntityNode::loadout`, `EntityNode::attach_type` and `EntityNode::inventory_container` in
  `p4kx-assembly` (docs/ship-base-data.md, ADR-0061).
- Basetool catalog: every weapon, shield, quantum drive, power plant, cooler and missile in `items`
  carries typed stat blocks in a new `stats` field, as raw Archive values without derived
  figures (ADR-0065). The catalog stays at version 1 (ADR-0054).
- Basetool catalog: the new kind `starmapObjects` lists the starmap objects (star systems, planets,
  moons, stations, jump points) with `type`, `parentGuid`, the localised `name` and the world
  `position` in metres. The position composes the `pos` and `rot` of every `Child` element of the
  system containers, with the full rotations of the containers above it. Additive; `schemaVersion`
  stays 1 (ADR-0054). `p4kx catalog` prints the placement counts and warns about containers it
  cannot read (exit code 1).
- The Build carries the public patch version (for example `4.7.2`) when the RSI Launcher log has a
  line with the Build's change number. `p4kx info`, `p4kx channels --json` and the Basetool
  catalog's `source.build.publicPatch` show it; it is `null` when the log has no such line.
- Basetool catalog: every ship and item carries `variantOf`, the GUID of the base of its cosmetic
  variant group, or `null` (ADR-0066). `p4kx catalog` counts the groups and variants per kind. The
  catalog stays at version 1 (ADR-0054).
- Legal notices: README, the new window "About P4KX" under Help, `p4kx --version` and
  `--help`, the `p4kx-mcp` help and instructions, and the notes of every Release state that
  P4KX is an unofficial Star Citizen fan project, not affiliated with or endorsed by the
  Cloud Imperium group of companies, ships no game data, reads only your own installation, and
  that you are responsible for complying with the Star Citizen EULA and Terms of Service. README
  and the About window also carry the trademark notice of the Fankit (text only; in the English
  wording in German too).
- MCP server: the README, the server instructions and the descriptions of `read_entry_text`,
  `read_entry_bytes`, `get_record` and `run_query` say that returned content may be forwarded by
  the MCP client to third parties such as a cloud model provider, and that game texts and media
  must not be redistributed.
- Read-only use: Extraction, Export and every other write refuse a target folder inside a Star
  Citizen installation (the folder holding the opened Archive and the Library folders), however
  the path is spelled. While `StarCitizen.exe` runs, the window and the command line warn that the
  tool should be used with the game closed; reading continues. Only the process list is read.
  The whole installation counts, including a Channel's `user` folder, so `p4kx chf from-json` no
  longer writes into `CustomCharacters`: copy the file in by hand.
- Archives of test Channels (PTU, EPTU, TECH-PREVIEW, HOTFIX): the window shows a persistent
  warning notice, `p4kx` prints one line on stderr and `p4kx-mcp` adds a note to its responses that
  the material of test builds is confidential under the Star Citizen Terms of Service and must not
  be shared or published. Opening, browsing, extracting and comparing work as before (ADR-0038).
- Basetool catalog: every ship and item carries `playability` (`player`, `ai`, `mission`,
  `special` or `null`), decided from crew manifests, tags, the inclusion mode and the shop display
  of the Records, never from names. `p4kx catalog` prints the count per value and `--json` lists
  them under `playability`. The catalog stays at `schemaVersion` 1.
- Basetool catalog: the kind `loadouts` lists, for every ship and every item that has ports, the
  ports with size, item types, flags, tags and visibility, and the Entity the Default Loadout puts
  into each as an `itemGuid` into `items` or `ships`, with the ports of the attached Entity nested
  below it (the Loadout's overrides applied). Docked vehicles are marked and not expanded. The kind
  is additive, `schemaVersion` stays 1 (ADR-0054, ADR-0064). `p4kx catalog` and `--json` count the
  ports, the attached Entities and those without an entry in `items` or `ships`; the window lists
  the kind (docs/basetool-catalog.md).
- Basetool catalog: the kinds `resourceGroups` (the `ResourceTypeGroup` Records, with the group that
  lists each as `parentGuid`) and `blueprintCategories` (the `BlueprintCategoryRecord` Records).
  Every commodity `typeGuid` and every blueprint `categoryGuid` now resolves to an entry. The
  kinds are additive and keep `schemaVersion` 1 (ADR-0054). `p4kx catalog` and the Basetool
  catalog summary of the desktop app list their counts.
- Basetool catalog: every ship carries `mass`, `massBasis`, `hullHealth`, `cargoScu`,
  `cargoGridsWithIssue`, `size` and `vehicleKind` (spaceship or ground vehicle) from the ship base
  data. Unknown values are `null`, never 0. `p4kx ships` states the `vehicleKind` too.
- Basetool catalog: every blueprint lists all its `tiers` with the selections ("choose n", with
  `count` and `options`), the quantity multiplier per cost line and the quality modifiers, next to
  the unchanged first-tier fields `craftTimeSeconds` and `ingredients`; it also states
  `defaultAvailable` and its `dismantle` link. The new kinds `gameplayProperties`,
  `qualityDistributions` (with their location overrides), `blueprintPools` (with the places that
  reward them and their chance) and `dismantleProcesses`, and `craftingGlobal` with the resources
  and items a dismantle excludes, complete the crafting data. All additive; `schemaVersion` stays 1
  (ADR-0054). `p4kx catalog` lists the counts and warns when the reference index is truncated or
  holds several dismantle processes (exit code 1).
- Crafting library: `Crafting::warnings` reports a truncated reference index, the Records it could
  not read and several dismantle processes instead of dropping reward sources silently;
  `dismantle_return()` links only the single dismantle process and exposes the tier and
  `excluded_resources`. The catalog and the library select blueprints by one rule.
- Basetool catalog: every commodity carries `densityGramsPerCubicCentimetre` and
  `refinedSubtypeGuid` from its `ResourceType` (additive in schema version 1). The DataForge holds
  no yields; see `docs/commodity-refining.md`.
- Basetool catalog: the kind `paints` lists every paint (an item of the type `Paints`) with the
  ships it fits, `shipGuids`, found structurally: the paint port of a ship (`SItemPortDef` of its
  Record or `ItemPort` of its vehicle implementation, with the Modification applied) requires tags
  that the paint's `Tags` carry or lie below in the tag tree. The kind is additive, `schemaVersion`
  stays 1 (ADR-0054). `p4kx catalog` and `--json` count the paints that fit a ship and those that
  fit none; the window lists the kind (docs/basetool-catalog.md).
- Basetool catalog: an output name ending in `.zst`, for example `p4kx catalog <archive> --out
  basetool-catalog.json.zst`, writes the catalog as one zstd frame (level 19, content size and
  checksum declared, so a plain `zstd -d` decodes it), compressed while it is serialised. The desktop save dialog offers the
  second file type, and the summary shows the compressed and the uncompressed size. The size
  warning compares the compressed file with the 64 MB upload limit and the decompressed catalog
  with the 1 GB limit of the Basetool; `p4kx catalog --json` gains `compressed_size`,
  `decompressed_limit` and `decompressed_warning`. Plain `.json` stays the default.
- Basetool catalog: the new kind `contracts` lists every contract and sub contract with its
  resolved title, mission type and the reputation it grants on success. Version 1 stays.
- Basetool catalog: the new kinds `mineableElements` and `refiningProcesses`. A mineable element
  carries `resourceTypeGuid` (a commodity's `subtypeGuid` only when the catalog has that commodity;
  20 of 41 on LIVE), `instability` and the other plain numbers of the `MineableElement` Record;
  several elements may share one `ResourceType`, so none is picked per commodity. A refining process carries `processName`, `refiningSpeed` and
  `refiningQuality`; the DataForge holds no yields. Additive; `schemaVersion` stays 1 (ADR-0054).
- Basetool catalog: the new kind `shops` lists the shops of the registry `LayerBackups.xml` (Stanton
  and Pyro) with their identity (`key`), internal names, the container chain, `body` and `location`,
  `kind` and `brand` from the shop containers, a link to the starmap object of the location
  (`starmapObjectGuid`) and a flag for commodity exchanges. It holds no prices, stock or item list;
  the inventory files cannot be assigned to shops (ADR-0069). Additive; `schemaVersion` stays 1
  (ADR-0054). `p4kx catalog` prints the shop counts.
- Basetool catalog: a shop carries `inventory` and `itemGuids` when its shop containers name an
  inventory file in their `.soc` (ADR-0071); 331 of the 1,184 shops on LIVE get a list. It holds the
  item GUIDs only, no price, stock or rental data. `p4kx catalog` counts the shops with and without
  a list and warns about inventory files it cannot read.
- Basetool catalog: `stats` of an item now also covers suit armour (`armor`), clothing and
  armour worn values (`clothing`), radars (`radar`), tractor beams (`tractorBeam`) and the salvage
  and tractor multipliers of attachments (`weaponModifier`), and every fire mode has a
  `damagePerSecond` (the power of a mining laser) and a `spread`. The document states, for each
  gameplay property a blueprint modifies, the `stats` field it changes or why there is none
  (ADR-0072). The catalog stays at version 1.

### Changed

- The window, the installer and the documentation no longer name an organisation. The look is
  unchanged, and the earlier P4KX glyph gives way to the new logo. The copyright reads "Copyright (c) 2026 the P4KX contributors", the installer manufacturer is "P4KX
  contributors", and the installer writes `HKCU\Software\P4KX` instead of a key below the earlier
  organisation path; an installed version upgrades and its old key is removed.
- **Breaking:** the Archive key is no longer part of the tool; you enter it
  ([ADR-0027](docs/adr/0027-archive-key-comes-from-the-user.md)). The constant `ARCHIVE_KEY` of
  `p4kx-foundation` is gone: `ArchiveKey` is a value that `Archive::open_with_key` or
  `Archive::set_key` give an Archive, and `p4kx_testkit` encrypts its Archives with a test key
  of its own. Without a key an Archive still opens, lists, searches and compares, and reads every
  unencrypted Entry; an encrypted Entry fails with `ArchiveError::KeyRequired`, and a key that
  does not decrypt the Archive with `ArchiveError::WrongKey`, found by decrypting one encrypted
  Entry and comparing its CRC-32C. `p4kx --key <hex>`, the environment variable
  `P4KX_ARCHIVE_KEY` and the key stored with `p4kx key set|show|remove` in your profile give
  the key, in that order; the MCP server takes the same three (`--key`, `P4KX_ARCHIVE_KEY`, the
  stored key), and the window asks in a dialog when an encrypted Entry is first needed, and from
  **Archive key…** in the Archive menu. A missing or wrong key exits `p4kx` with the new code 7.
  The key is stored in `archive.key` next to the settings, written atomically and readable by
  your user alone where the platform allows it, and never appears in logs, error messages or JSON.
  `cargo xtask keys` fails when a tracked file holds a key-like literal named as a key, or the
  real key while `P4KX_ARCHIVE_KEY` is set. Real-Archive tests need `P4KX_ARCHIVE_KEY`.
- Models: `p4kx_model::ModelFile` is `#[non_exhaustive]` and has the new variant
  `LodGeometry(u32)` for the geometry companion of a level of detail from 1 on (`lod<n>-geometry`
  in the `chunks` of `model info`), so a `match` on it needs a wildcard arm. The table of
  `p4kx model info` has the columns Tangents and Colours, and its chunk table is wider for the
  `lod<n>-geometry` names.
- GUI: the window follows the measured palette: the house orange is `#E77E23` again, the canvas `#0F0F0F`, the panel `#282C30`, the
  bar `#1A1D21`, the control `#373E43` (hover `#434B51`), the hairline `#36373B`, the count badge
  `#2F394C`, the text `#D1D2D3` and the muted text `#A2A5A9`; the selected tint is 26 % orange,
  every orange tint and glow follows, tooltips fill with the control colour, and the glyph and
  logo SVGs use the house orange, tree rows hover in 2.5 % white and the drop target fills with
  6 % orange as the CSS of the design sets them. The earlier values were sampled from HDR screenshots and were
  too light.
- GUI: the window follows the design after an adaptation to a website palette: house orange `#FB8A28`, the canvas `#202020`
  with panel `#35393D`, bar `#2A2D31` and control `#3F474C` fills, the lighter text and border
  greys, the new text tints, and the `radius-sm` corner of 4 points on buttons, fields, segments,
  chips, alerts, modals, dropdowns, tiles, the list panel and the preview. Header, status bar and
  tab bar use the bar surface, table rows hover in `control-bg` and tree rows in 3 % white, the
  ghost button hovers in `color-accent-light`, tooltips are square on near-black with the orange
  glow, focus rings follow the radius of their control, and the range thumb is round. The empty
  state no longer hides the work tabs, the dialogs' foot no longer stretches the dialog to the
  window height, and read-only and text fields draw the rounded input frame. The desktop spec and
  its JSON follow.
- Folders whose names differ only by ASCII case, or whose Entries use `/` and `\` alike, are one
  folder, in the window's tree as in `list_entries` of `p4kx-mcp`, named as its first Entry in
  path order spells it (ADR-0023). `list_entries` lists the Entries of a folder in path order and
  pages in time proportional to the page; the window and the server share one folder index.
- DataForge: a format drift that no known database has is read and reported instead of failing the
  Record or the file. Bits above the low byte of a property's conversion type are ignored, so a
  flag no longer turns a single value into an array slot and breaks the struct layout; a Record
  whose name offset is `0xFFFFFFFF` reads with an empty name and one whose tag offset is
  `0xFFFFFFFF` without a tag. `DataForge::issues` lists these deviations (one per property or
  Record, at most 1024 listed, all counted by `issue_count`), and `DataForge::survey` counts the
  format details no tool interprets (docs/dataforge-format.md). A real-Archive canary test
  checks them for `P4KX_DATA_P4K` and `P4KX_DATA_P4K_B`.
- Three `#ivo` chunk types change their name in `p4kx chunks list` and `p4kx model info`, also in
  the `type` field of their `--json` output: `0x2B7ECF9F` is `PositionBonemap` (was
  `BoundingData`), `0x58DE1772` `StatObjPhysics` (was `LodMeshData`) and `0xBE5E493E`
  `ExportFlags` (was `AssetMetadata`); `0x57A38888` is now `BShapesGpu` and `0xF5C6EB5B`
  `BShapes`.
- Models: the `TANGENT` attribute of a GLB has the handedness `w` negated, so that the bitangent
  points up the image as glTF's does; the engine's points down it, and the normal maps of
  textured materials have their green channel flipped to match (ADR-0042, which settles the open
  question of ADR-0028).
- GUI: alerts fill the width of their panel like the theme's `.alert`, and the tabs of
  the preview scroll sideways with the mouse wheel when they do not fit next to Back, Forward and
  Copy, like `.tab-nav` with `overflow-x: auto`.
- `p4kx ui render`: a font without any glyph outline is a missing font (`missing_font`, drawn as
  boxes) instead of drawing nothing, and fonts of other movies are found by name only, never by
  their own ID.
- `p4kx entity glb` places a docked vehicle, such as the Merlin of a Constellation or an
  Archimedes, as one part without the parts of its own Loadout, so nothing is counted twice.
- `p4kx-archive`: `Checksum::value` now returns `Option<u32>`, `None` for a SHA-256 digest, which
  breaks library users that relied on a plain `u32`.
- Changelog fragments: a user-visible change adds a file `changelog.d/<issue>.md` instead of an
  entry in `CHANGELOG.md`, so parallel pull requests no longer conflict there. `cargo xtask
  changelog` checks the fragments, and `cargo xtask changelog --release <version>` folds them and
  the entries under `## [Unreleased]` into the section of the version (ADR-0035).
- Basetool catalog: new kinds and fields may be added to version 1 without raising `schemaVersion`
  (ADR-0054), and a new value in a closed value set is additive. The JSON Schema now allows unknown
  properties, types `itemClass` as a string, and marks every game-text field with `x-gameText`. The
  documentation lists the guarantees for readers and says that a production import takes `LIVE`
  catalogs only.
- Basetool catalog: the window suggests `basetool-catalog.json.zst` as the file name and lists the
  compressed form first. Plain `.json` stays selectable but is over the 64 MB upload limit; a
  warning advises `.json.zst`.
- Breaking: the Record XML namespace URI is now `https://github.com/greluc/p4kx/record-xml` and
  the `$id` of the Record JSON schema is
  `https://github.com/greluc/p4kx/schemas/record-json.schema.json`. These replace the
  identifiers of earlier versions in one step. A consumer that checks the old
  namespace or schema id must switch to the new one. The update check, the About text and the
  installer link point to the new repository.
- The tool is now called P4KX, and its repository is `greluc/P4KX`. The window, the About text,
  the help, the installer and the artefacts (`P4KX-<version>-x86_64.msi` and the portable ZIP)
  carry the new name. The MSI keeps its upgrade code, so an installed P4K-Extractor upgrades to
  P4KX; its install folder, Start menu shortcut and `PATH` entry move to the new name.
- Converted audio carries `P4KX` in the Ogg and Opus vendor string, so the bytes of converted
  files differ from earlier versions. The generator of exported glTF files reads `P4KX <version>`
  and the update check identifies itself as `P4KX/<version>`.
- The settings folder is now `P4KX` in the platform config directory (`%APPDATA%\P4KX` on
  Windows). On first start, when it does not exist yet, the content of the old `P4K-Extractor`
  folder (the settings and the stored Archive key) is copied into it; the old folder stays as a
  backup, and the copy never overwrites a file. When the copy fails, the old folder stays in use
  and the next start tries again.
- Basetool catalog: shops get their item list from shop components in every container and from the
  shop prefabs, not only from the shop folders, and the components that later positions of a
  container file hold are found by object name or `CryGUID` (ADR-0073). For the shops that name the
  placeholder inventories `temp_outpost_shopinventory` and `inv_cryastro_dummy`, `inventory` and
  `itemGuids` are now `null` instead of the placeholder list. On LIVE 294 of 1,184 shops have a
  list, 64 are unscanned instead of 144. The shape of the catalog does not
  change; `p4kx catalog --json` counts the new cases in `shops`.

### Removed

- `ModelError::UnsupportedContainer` in `p4kx-model`: `CrCh` models are read.

### Fixed

- Archives with a malformed field in the directory record of a single Entry now open instead of
  failing as a whole, and the Entry is reported as an Entry issue (ADR-0025): an Entry path that
  is not UTF-8 is listed with its invalid bytes replaced and extracted under a name that spells
  them as `%XX`, so Entries with equal replaced paths no longer overwrite one another; an
  encryption field or 0x5002 block value other than 0 or 1 is read as encrypted, and a 0x5002
  block of another length than 6 bytes is reported; a v2 Entry whose data lies outside the file is
  reported when the directory is read and fails only when it is read. `p4kx info`, `list` and
  `extract` print the issues to stderr, `p4kx info` counts them, and `archive_info` of `p4kx-mcp`
  lists them; at most 1024 issues are listed and all are counted. A ZIP64 locator that does not
  stand right before the end record is found by searching backwards. Extraction now also rejects
  the Windows device names `COM¹` to `COM³` and `LPT¹` to `LPT³` as Entry paths.
- CHF: a file whose body changes keeps the last 8 bytes of the original file (the game writes
  `efbeadde efbeadde` there; all 41 CHF files of the LIVE Build have it) instead of zeroing them,
  with a correct CRC-32C. A new frame that would reach into non-zero ones is an error, not an
  overwrite. An unchanged JSON still gives the original file byte for byte (ADR-0014).
- Localisation: `@blank_space`, the filler Records use for an empty line (372 fields in the LIVE
  Build), is a placeholder key like `@LOC_EMPTY`: it has no text, so `p4kx loc get` reports
  `placeholder` instead of the single space of the English `global.ini`, and `p4kx dcb export
  --lang` no longer annotates it. A `global.ini` that holds both `key=` and `key,P=` keeps both
  lines; a lookup prefers the plain key (`GlobalIni::resolve_suffixed` returns the other). The
  Basetool catalog of the LIVE Build is unchanged.
- Soundbanks of a version above 154 are no longer misdecoded: their layouts differ (an Event
  starts with other fields), so a Sound, Action, Event or container of such a bank is kept raw and
  reported as an object whose layout is not known (`docs/audio-bank-versions.md`). `HircKind::of`
  maps the type numbers of `HIRC` objects by bank version, which differ up to version 126. Star
  Citizen's banks are all version 154 and decode as before.
- A Sound object of a version 150 soundbank is decoded correctly: it has no `cacheID`, which
  versions from 151 on write before the in-memory media size, so the media size was read one field
  late. Star Citizen's version 154 banks were not affected.
- A WEM with the format tag `0x0002` is no longer always taken for IMA ADPCM: a `fmt` chunk that
  holds DSP coefficients (`12 + 46 × channels` bytes after the common fields) is DSP ADPCM, which
  is reported as unsupported, and one with a block alignment of 260 bytes per channel is PTADPCM
  and decodes. These are console variants; every Star Citizen WEM is Wwise Vorbis.
- Soundbanks with a quirk that leaves the rest readable are no longer rejected whole: bytes after
  the last `HIRC` object, a repeated `DIDX` WEM ID (the first entry is kept), `DIDX` without
  `DATA` and a non-UTF-8 name in `STID` are worked around and listed by `Soundbank::warnings`.
  The audio index reads soundbanks up to 32 MiB instead of 16 MiB, and `Wem::chunks` and
  `Wem::chunk` list every RIFF chunk of a WEM, `smpl`, `cue ` and `LIST` after `data` included.
  No soundbank or WEM of the LIVE, PTU and TECH-PREVIEW Archives has any of these quirks
  (`docs/audio-survey.md`).
- Mesh dequantisation: a survey of LIVE, PTU, TECH-PREVIEW and EPTU compares the two boxes of
  the mesh chunks (static models only; skins store floats). Of the about 30,000 `.cgfm` chunks
  and about 31,000 `.cgam` chunks of each Archive whose boxes differ, the face normals can decide
  about 6,000 in LIVE, and 99 % of those choose the second, scaling box; the rest are axis-aligned
  and undecidable. The reader keeps the second box for every mesh, documents the evidence and its
  limits (docs/mesh-bounding-boxes.md) and tests it with a differing model box; no exported model
  changes.
- Mesh streams: both normal tags now take 4-byte packed and 12-byte float vectors, and a
  16-byte tangent stream (two quaternions) gives normals, so a mesh that stores only such a
  stream keeps its normals; no real file has the 16-byte layout, so reading the normal from its
  first quaternion is unverified. The packed vectors of the second normal tag rank last: in LIVE they
  disagree with the tangent frames of the 17,902 meshes that hold both, so no exported model
  changes.
- Mesh streams: a mesh chunk that holds one of the streams a model reads (indices, vertices,
  normals, tangents) twice is reported as corrupt (`RepeatedStream`) instead of the later stream
  silently replacing the first. No mesh chunk of LIVE, PTU, TECH-PREVIEW or EPTU does.
- The `#ivo` chunk type table follows a survey of LIVE, PTU and TECH-PREVIEW
  (docs/chunk-types.md): twelve types that `p4kx chunks list` reported as unknown are named
  (`BShapes`, `BShapesGpu`, `RigLogic`, `RigInfo`, `ProtosInfo`, `PhysicalHierarchy`, `VisAreas`,
  `Skeleton` and four more), seven stay unknown, and the table entries that occur in no measured
  Archive are marked unverified.
- A WEM whose odd-sized chunk is not followed by a pad byte, as Wwise writes it, no longer
  misaligns the chunks after it: the reader takes the position right after the chunk when it
  holds a valid chunk header, and skips the pad byte of plain RIFF otherwise. No WEM of the LIVE,
  PTU and TECH-PREVIEW Archives has an odd-sized chunk before its last.
- Models: a skin with a texture coordinate that is NaN or infinite (14 `.skinm` of LIVE, helmets
  among them) no longer fails to open; the coordinate reads as 0.
- Models: a model whose geometry companion holds only visibility areas (a `VisAreas` chunk and no
  mesh, as in the interior visarea corridors of ships) reads as helper nodes without meshes
  instead of failing with a missing mesh; `model info` lists the chunk as skipped.
- Loadout: an entry without an Entity (an XML `<Item>` with an empty `itemName`, or a DataForge
  manual entry without target) that carries nested entries no longer drops them. The port stays
  empty and the nested entries appear below it as `PortNode::nested` in `p4kx-assembly`. This
  changes the output of `p4kx entity loadout`: the text lists them indented below the empty port,
  the JSON gives every Port a `nested` array, and `ports`, `attached` and `empty` count them. On
  LIVE it affects the two human body Entities (`Body_ItemPort` with 10 nested items) and
  no ship or vehicle.
- `THIRD-PARTY-NOTICES` now carries the licence text and copyright notice of every Rust crate
  linked into `p4kx`, `p4kx-gui` and `p4kx-mcp` (ISC, Unicode, MPL-2.0, Zlib, OFL and the other
  licences that were missing), plus the Lato fonts. CI fails when a dependency change leaves the
  file out of date (`cargo xtask notices`).
- Vehicle implementations: a number with an invisible Unicode format character inside it, such as
  the U+202C in the Carrack's `mass`, is read in full instead of up to the character.
- Query: a completion request made right after the Archive changed on disk is no longer ignored
  while the build of the schema for the previous DataForge is still running.
- `p4kx coverage`: `SolarSystem` is counted as the `StarMapObject` Records of that
  `StarMapObjectType` (new `starmap-type` kind and `starmap_types` JSON array) instead of as a
  missing Record type, so the exit code no longer reports a gap that does not exist.
- Catalog: ships whose vehicle definition differs only in case or path separator now fall into
  one variant group. Items with weak pointers into different instances of their Record no longer
  compare equal, and a repeated GUID in a malformed Archive no longer counts as a group without
  variants.
- Catalog: contracts whose `ContractTemplate` Record lies outside `contracts/contracttemplates`
  now take title and mission type from it. An organisation token without a variant is resolved
  only for an organisation with a single string variant. A cancelled build stops while the
  contract records are read, and a Filter that matches no generator no longer reads them.

## [0.2.0]

The second Release: readable game data. CryXml and DataForge Records as XML and JSON, the
Basetool catalog, the Comparison of two Archives, textures and localisation, and first commands
for Queries, audio, models, UI movies, socpak containers, CHF files and the MCP server.

### Added

- Verification of textures and localisation against all installed Channels, recorded in
  `docs/m5-verification.md`. A texture header Entry that ends with the 8-byte CryEngine trailer
  `CExtCEnd` after its last mip (`cm_inventory_probe_cm.dds`, in every Channel) is no longer
  refused with "not a whole number of mips": the trailer is left out of the merged DDS and of
  the mips read. Opening a texture only checks the trailer's size; `merge` and `read_mip` of a
  mip from the header Entry's tail verify its bytes (`TextureError::UnknownTrailer`), `read_mip`
  of a mip from a part does not.
- GUI: the menu "Record texts" chooses the language of the texts the Record preview shows next
  to localisation keys, in the Record tab and in the Record XML. It lists the languages whose
  `global.ini` the open Archive holds plus "Keys only", follows the interface language until
  one is chosen, and stores the choice in the settings (`record_language`). A key without a text
  in the language is marked "Missing"; control, bidirectional and line feed characters of the
  texts are shown as escapes. A language is read, indexed and copied once per Archive; switching
  it re-renders the open Record from the DataForge already held, without reading it again.
  `LanguageTexts::owned` resolves with an `OwnedTextSet`.
- GUI: a Localisation tab next to Query. It reads the `global.ini` of every language of the open
  Archive off the UI thread behind a loading indicator and shows one row per localisation key
  and one column per language, with missing translations marked. The search matches keys and
  texts of all languages ignoring case, runs off the UI thread while typing and keeps the
  window responsive on a full table; control and bidirectional characters in texts are shown as
  escapes. The key or the text of the selected cell is copied with two actions or Ctrl+C, and
  the table scrolls with the keyboard. All texts are in the German and British English interface
  files.
- Verification of the Basetool catalog against all installed Channels: Records with the words
  `debug`, `deprecated` or `dev` in their path are left out as development leftovers, and the
  measured sizes (at most 21.1 MB against the 64 MB upload limit) are recorded in
  `docs/basetool-catalog.md`.
- An unreadable selected Record no longer aborts `p4kx catalog` and the desktop app's Basetool
  catalog: it is left out and counted as a failure, the catalog is still written, `p4kx catalog`
  lists the failed Records on stderr and under `failed` in `--json` and exits with code 1, and the
  summary window lists them.
- `p4kx dcb export --lang <language>`: Record XML, Record JSON and NDJSON with the text of every
  localisation key next to the key, in the `global.ini` of that language; a key without a text is
  marked missing, a placeholder key stays alone. Additive: without `--lang` the output is byte
  for byte the same as before, and the export report names the `language` only with it.
  `RecordXml::write_with`, `RecordJson::write_with` and `TextResolver` in `p4kx-dataforge`,
  `LanguageTexts` and `RecordExportOptions::texts` in `p4kx-extraction`; the Record JSON Schema
  allows the new `{"key", "text"|"missing"}` object. `RecordFileFormat::write_file` takes the
  texts as an extra parameter.
- `p4kx model info|glb <archive> <entry>`: static `#ivo` models (`.cgf` with their `.cgfm`
  geometry companion) shown with their nodes, LODs, meshes, materials, bones and chunks, and
  exported as one glTF 2.0 binary file (GLB) that Blender opens upright and in metres: one node
  per model node with its transform, one primitive per submesh with positions, normals and
  texture coordinates, and one material per material ID. Unknown chunks are skipped and listed.
  Both commands also take a model file on disk with its companion next to it.
- `p4kx entity audio <archive> <entity> [--json]`: the audio triggers an Entity names, each with
  the Wwise events its ATL definition posts and the soundbanks whose `HIRC` section holds them,
  matched by the events' Wwise short IDs (FNV-1). Triggers without definition, without event or
  in no soundbank are listed with the reason; ATL definitions and soundbanks that cannot be read
  are warned about. The audio index reads every ATL definition and soundbank of the Archive once,
  in parallel.
- `p4kx ui assets <archive> <entry> [--json]` and `p4kx ui extract <archive> <entry> --out
  <folder>`: list the tags, shapes, morph shapes, bitmaps, fonts and sprites of a GFx or SWF UI
  movie with their IDs and export names, and write its bitmaps as PNG and its shapes as JSON in
  twips. Compressed (zlib, LZMA) and uncompressed movies are read, unknown tags are kept and
  counted, a broken tag is reported without failing the movie (exit code 1), and GFx external
  images are decoded from the DDS textures next to the movie, on disk only inside the movie's
  `Data` folder. Also for movie files on disk.
- `p4kx socpak list <archive> <container…>` and `p4kx socpak extract <archive> <container…>`:
  socpak containers, also nested ones, are read from the Archive without extracting them first,
  listed like Archives and extracted with `--filter`, `--out`, `--conflict` and `--convert`.
  Nesting depth, Entry count, decoded size, compression ratio and overlapping Entry data are
  bounded; a corrupt container fails with exit code 3 and an error naming its Entry.
- `p4kx chf to-json <file> [--out <file>]` and `p4kx chf from-json <file> --out <file>`: CHF
  character files as JSON and back. The new `p4kx-character` crate checks magic, size and
  CRC-32C, decompresses the zstd body and reads its inner versions (7 and 8), body type, face DNA,
  item ports and material choices; an unchanged JSON gives back the original file byte for byte,
  an edited one a new file with a correct CRC-32C. Unknown versions and CRC-32C mismatches are
  errors that name them.
- `p4kx-mcp`: a read-only MCP server over stdio for AI clients, with the tools `list_channels`,
  `archive_info`, `list_entries` and `search_entries`. An Archive is named by Channel or by
  absolute path, per call or with `--archive`; listings page with a cursor, at most 1000 items a
  page. `p4kx-mcp.exe` ships in the portable ZIP and the MSI next to `p4kx.exe`.
- `p4kx-mcp` tools `read_entry_text` and `read_entry_bytes`: read an Entry in ranges from a byte
  offset, as text (CryXml converted to XML, UTF-16 by its byte order mark, otherwise UTF-8;
  binary Entries are an error) or as base64. A range is at most 256 KiB of text or 192 KiB of
  bytes; a larger `max_bytes` is clamped and the result says so, and `next_offset` continues.
  Stored Entries, encrypted or not, are read from the offset directly; compressed ones decode at
  most 1 GiB to reach it. At most four Entries are decoded whole for text at a time.
- The Audio tab of the preview: select a WEM to see its codec, channels, sample rate and
  duration, and play it without extracting, with play, pause, seek and volume. PCM, IMA ADPCM,
  PTADPCM and Wwise Vorbis sounds play; another codec, a missing audio device or a sound too long
  to hold is named instead.
- The Query tab of the window: run Queries over the DataForge of the open Archive in the
  background with progress and cancel, see an invalid Query explained with its line and column
  and underlined in the editor, sort the result table by any column, page through large results,
  and export the result as CSV or JSON exactly as `p4kx dcb query --out` writes it.
- The Record changes of a Comparison in the window: a Records tab lists every added, removed,
  moved, changed and unreadable Record with its record path and type, sorted and filtered like the
  Entries, and a selected Record shows its field changes with old and new value, links by the
  record path of their target and unreadable values with their reason.
- The Export menu of the window with **Record export…**: the Records selected in the Record tree
  or matching a record-path filter as Record JSON, one file per Record or one NDJSON file, into a
  new or an existing folder with a Conflict policy, in the background with progress, cancel and a
  summary of the Records written and those with error values. DataForge and the planned files are
  read once per open Archive and reused by every later export.
- **Export › Basetool catalog…** in the window: the Basetool catalog of the open Archive into a
  file chosen in the save dialog, in the background with progress and cancel, writing no partial
  file, and a summary of the entities and skipped Records per kind, the resolved and missing texts
  per language and the size with a warning near or over the basetool upload limit. The Record
  export and the catalog share the DataForge read once per open Archive.
- `p4kx_export::build_catalog_from`: builds the Basetool catalog from a DataForge Entry already
  read.
- `p4kx_export::write_planned_record_export` with `RecordFilePlans`, and
  `p4kx_extraction::export_planned_record_files` with `RecordFilePlan`: Record exports that plan
  the files of a DataForge's Records once; `RecordExportOptions::progress` reports every Record
  as it ends.
- `p4kx entity loadout <archive> <entity> [--json]`: the default Loadout tree of an Entity found
  by class name, GUID or record path, resolved recursively through nested and overriding
  Loadouts, XML Loadouts and port default items, with each port's sizes, types and helper; a port
  without Entity or with a dangling Reference, an unknown class name or a cycle is listed with the
  reason instead of failing the tree.
- `p4kx dcb query <archive> "<query>"`: Queries over the Records of the DataForge, defined in
  `docs/query.md`: select Records by type (subtypes included) and record-path glob, and list
  scalar fields by dot path as columns of a table, the record path first. Errors name the problem
  and mark its position in the Query, with a suggestion for a mistyped type or field name.
- `p4kx dcb query --format table|csv|json [--out <file>]`: Query results as RFC 4180 CSV with a
  header row or as JSON, an array of objects keyed by column with values typed as booleans,
  numbers and strings, written page by page to stdout or to a file; the columns of the table
  fit every row.
- `where` conditions in Queries: comparisons with `=`, `!=`, `<`, `<=`, `>`, `>=` on numbers,
  strings, GUIDs, enums by name and booleans, string matching with `contains`, `starts with` and
  `like` ignoring case, joined by `and`, `or`, `not` and parentheses. A comparison of
  incompatible types is marked as an error, and a missing value never matches.
- Paths into nested data in Queries: dot paths through inline structs and strong pointers, `[n]`
  for one array element (null when out of range), `[*]` for every element and `[TypeName]` for
  the elements of a polymorphic array of that type, e.g.
  `Components[VehicleComponentParams].crewSize`. A path through `[*]` or `[TypeName]` gives a
  list per cell, a JSON array in CSV and JSON, and a comparison over it holds when one of its
  values holds.
- `->` in Query paths follows a Reference to the Record it links to, or a weak pointer to its
  instance, in `select` and `where`, chained and behind array elements, e.g.
  `AttachDef.Manufacturer->Code` or `escorts[*]->name`. A null or dangling Reference gives null
  and is counted as unresolved in the result summary; `->` where no Reference stands is marked
  as an error.
- `p4kx catalog <archive> --out <file>`: the Basetool catalog in its new, versioned format with
  Channel, Build and completeness per kind, starting with the manufacturers; test, template,
  dummy, debug, deprecated and dev Records are left out and counted, `--filter` makes it partial,
  and a warning comes before the 64 MB upload limit. `p4kx catalog schema` prints its JSON Schema, checked in as
  `schemas/basetool-catalog.schema.json`.
- English and German names and descriptions in the Basetool catalog, resolved from the Archive's
  `global.ini` files next to their localisation keys, with `\n` as a line feed; a key or
  language the Archive lacks or an empty text leaves that text `null`, the placeholder keys
  `@LOC_EMPTY`, `@LOC_PLACEHOLDER` and `@LOC_UNINITIALIZED` give no text, and `p4kx catalog`
  counts resolved, unresolved and placeholder texts per language. A corrupt `global.ini` ends
  `p4kx catalog` with exit code 3.
- `p4kx texture info` and `p4kx texture merge`: read a DDS texture split into a header and mip
  parts, from an Archive or from files on disk, and merge it into one standard DDS file.
- `p4kx texture png`: decode a mip of a BC1, BC2, BC3 or uncompressed texture with pure-Rust
  decoders and write it as an 8-bit PNG; the largest mip by default, another with `--mip`.
- Cubemaps and alpha tails: `p4kx texture merge` merges split cubemaps and texture arrays face by
  face, `p4kx texture png` writes a cubemap as six face files or with `--cross` as one cross, an
  alpha or smoothness tail `.dds.a` is merged into the alpha channel of the PNG, and
  `p4kx texture info` reports it.
- High-range BC6H textures, signed and unsigned: `p4kx texture png` decodes them with a pure-Rust
  decoder to floating-point RGB and writes OpenEXR (`.exr`, 32-bit float RGB) instead of a PNG, so
  their range is not clipped, cubemaps as six faces or a cross; `--tone-map` writes a Reinhard
  tone-mapped 8-bit PNG instead.
- `p4kx compare <old> <new> [--entries] [--filter] [--report <file>]`: the Entries added, removed
  and changed between two Archives, from their central directories without decompressing, with
  counts per top-level directory, a JSON report with a checked-in schema, and exit code 1 when the
  Archives differ.
- The Comparison tab of the window: pick an old and a new Archive from the detected Channels, the
  Recent Archives or any file, compare their Entries, their Records or both off the UI thread with
  progress and cancel, and see both Builds, the counts per change kind and per top-level directory,
  and the sortable list of added, removed and changed Entries, filtered with the rules of the
  search. **Save report…** writes the same JSON as `p4kx compare --report` with the current
  levels and Filter, the Record level included.
- `p4kx compare --records`: the Records of both DataForges joined by GUID and listed as added,
  removed, moved or changed, with each changed scalar field and its old and new value, in the
  summary counts and the JSON report; a Record that cannot be read is listed as unreadable with
  the reason. Without `--entries` or `--records` both levels run.
- `p4kx compare --records` compares the whole Record tree: changes inside nested structs and
  arrays carry their location, e.g. `hull.plates[1].armor`, array elements added or removed at the
  end are listed as such, and changed References show the old and new target's GUID and record
  path; weak pointers compare by their location inside the Record as well.
- `p4kx compare <old> <new> --diff <entry>`: the line diff of one text, XML or CryXml Entry as a
  unified diff, or as JSON hunks with `--json`; CryXml is converted to XML first, an added or
  removed Entry is diffed against an empty side, and a binary Entry is reported as changed or not.
- `p4kx audio info` and `p4kx audio wav`: describe a Wwise WEM sound from an Archive or from disk
  and write PCM WEMs as WAV files; every Wwise codec is detected.
- `p4kx audio ogg`: rebuild a Wwise Vorbis WEM as a standard Ogg Vorbis file without re-encoding,
  from an Archive or from disk. The headers are rebuilt with the default codebooks and checked
  with a Vorbis decoder, falling back to the aoTuV codebooks; the file plays for the sound's exact
  duration. The codebook libraries come from ww2ogg under the BSD 3-Clause licence, whose notice
  ships as `THIRD-PARTY-NOTICES` in the ZIP and the MSI.
- `p4kx audio wav` and `p4kx bnk extract --format wav` decode Wwise IMA ADPCM and PTADPCM WEMs,
  mono and multi-channel, to 16-bit WAV. IMA ADPCM durations count 64 frames per 36-byte block
  per channel, as Wwise writes them, instead of 65.
- `p4kx audio wav` and `p4kx bnk extract --format wav` decode Wwise Vorbis WEMs to 16-bit WAV for
  the sound's exact duration, with the codebooks the Ogg export picks; the channels keep the
  order and speaker mask of the WEM. A sound that cannot be decoded fails with an error naming
  the WEM and the reason.
- `p4kx audio ogg` and `p4kx bnk extract --format ogg` repackage Opus WEMs (Wwise Opus, Ogg Opus
  in a WEM and Switch Opus) as standard Ogg Opus files without re-encoding, with the pre-skip,
  the channel mapping of the Wwise layout and the sound's exact duration; `p4kx audio wav` and
  `p4kx bnk extract --format wav` decode them at 48 kHz to 16-bit WAV in the speaker order of the
  WEM, with the pure-Rust decoder opus-pure under the BSD 3-Clause licence, whose notice ships in
  `THIRD-PARTY-NOTICES`. The `OggReport` of `p4kx-audio` gains a `codec` field and its
  `codebooks` becomes an `Option`, `None` for Opus; `p4kx audio ogg --json` reports `codec` and a
  `null` `codebooks` for Opus.
- `p4kx bnk list` and `p4kx bnk extract`: list the WEMs a Wwise BNK soundbank from an Archive or
  from disk embeds, with ID, codec and duration, and write them as `<id>.wem`, converted to WAV or
  rebuilt as Ogg Vorbis; the `BKHD`, `DIDX`, `DATA`, `HIRC` and `STID` sections are parsed, and a
  corrupt soundbank fails with an error naming the section.
- `p4kx chunks list` and `p4kx chunks dump`: list the chunks of a CryEngine chunk file (`.cgf`,
  `.cga`, `.skin`, `.chr`, their geometry companions, `.caf`, `.dba`) from an Archive or from disk,
  in the `#ivo` (`0x900`) or the `CrCh` (`0x746`) container, with type, type ID, version, chunk
  ID, byte order, offset and size, unknown types marked; and write one chunk's bytes to a file.
- `p4kx cryxml`: convert a CryXml file on disk (`CryXmlB`, `CryXml`, `CRY3SDK`, either byte
  order) to well-formed UTF-8 XML; text XML is written unchanged.
- Conversion during Extraction: `p4kx extract --convert` and the Conversion switch of the
  Extraction window write CryXml Entries as UTF-8 XML; text XML and every other Entry stay
  unchanged. The switch is remembered with the other Extraction settings, and the summaries and
  `--json` count the converted Entries.
- Texture Conversion during Extraction: with `p4kx extract --convert` and the Conversion switch,
  each texture is merged into one DDS file and its largest mip exported as PNG once, not once per
  part, with its alpha tail in the PNG's alpha channel, a high-range texture as OpenEXR and a
  cubemap as six face files; `--textures merge,png,raw` and `--mip <level>`, and the same choices in the Extraction
  window, pick the outputs and keep the raw parts. A texture that cannot be converted is listed
  with its reason, its raw Entries are written and the Extraction continues. The choices are
  remembered with the other Extraction settings.
- `p4kx dcb info`: the version and the number of Records, structs and enums of the DataForge
  (`Data\Game2.dcb`, falling back to `Data\Game.dcb`); versions 6 and 8 are read, so it works on
  every current Channel, and any other version is reported as not supported.
- `p4kx dcb export`: write Records as Record XML, one file per Record under its record path, with
  owned structs nested, arrays in stored order, References and weak pointers as links (GUID,
  record path, type and, inside the Record, the location) and explicit nulls. Unreadable values
  such as dangling References are reported per value while the rest of the Record is written.
- `p4kx dcb export --format json|ndjson`: write Records as Record JSON (`docs/record-json.md`,
  with a checked-in JSON Schema), one file per Record under its record path or one
  `records.ndjson` with one Record per line in record-path order, with the structure and field
  order of the Record XML, References as objects with `guid`, `path` and `type`, and errors with
  their reason. The new service crate `p4kx-export` writes the Record export in every form.
- `p4kx dcb export --combined`: every selected Record in one Record XML file in record-path
  order, named after the DataForge Entry, built in parallel and streamed to disk, byte-identical
  across runs.
- Conversion of DataForge during Extraction: with `p4kx extract --convert` or the Conversion
  switch of the Extraction window, the DataForge Entry is written as one Record XML file per
  Record below its folder instead of the `.dcb`, with progress and cancellation per Record; the
  summaries and `--json` count the Records and their error values.
- DataForge in the window: a DataForge node below the folders of the path tree shows the Records
  as folders by record path, with record folders where several Records share one. It loads in the
  background the first time it is opened, with a loading indicator, and is kept until the Archive
  is reloaded or changes on disk; an unsupported version or a load error is named on the node. The
  new DataForge tab of the preview shows the version, the numbers of Records, structs and enums and
  the first Records.
- Archive menu entries "Reload" (F5) and "Extract selected…" (Ctrl+E), with the same keyboard
  shortcuts working anywhere in the window.
- Texture groups in the window: a split texture (`.dds` with `.dds.N` parts and an Alpha tail)
  is one row with its total size, its parts one level in below it. Checking a row checks all its
  Entries, a search that matches a part shows the whole texture, sorting by size uses the total
  size, and "Selected" and "Shown" extract every part. Parts without a header stay single rows.
- The long views of the preview and the Comparison scroll with the keyboard: Tab focuses the Text,
  Hex and Image previews, the line diff, the field list of a Record change and the field list of
  the Record preview, and the arrow keys, Page Up, Page Down, Home and End scroll them like the XML
  view.
- The Extraction summary lists the Records that were not written with their record path, GUID and
  reason, as it lists failed Entries.

### Changed

- `p4kx list` folds the parts of a split texture into the row of its header: sizes of the whole
  texture, a ` (+N parts)` suffix on the path and a `parts` field in `--json`. `--parts` restores
  one row per Entry.
- `p4kx extract --filter` and `p4kx socpak extract --filter` now also take every Entry of a
  texture whose header, part or Alpha tail matches.
- The window follows the P4KX design and its desktop spec: buttons, toggles,
  tabs, tables, tree, modals and alerts, Lato in three weights, the P4K-Extractor glyph as window
  icon and header mark, an empty state with an "Open Archive" action.

## [0.1.0]

The first Release: open, browse and extract Star Citizen Archives.

### Added

- Reading Archive layouts v1 and v2: ZIP64 central directory, Store, Deflate and Zstandard
  Entries, AES-128-CBC encrypted Entries.
- Detection of the installed Channels and the Build of each, plus manually added folders and
  Archives.
- The `p4kx` command line: `list`, `search` and `extract` with JSON output and documented exit
  codes.
- The P4K-Extractor window: Channel picker, drag and drop, recent Archives, path tree, sortable
  Entry table, search with quick-filter chips, text and hex preview, and Extraction of selected,
  shown or all Entries with progress, cancellation and conflict policies.
- A notice with a reload offer when the open Archive changes on disk.
- German and British English interface, dark look.
- A portable ZIP and a per-user MSI installer.
- An optional check for a newer Release at start, off by default and switchable in the Help menu.
