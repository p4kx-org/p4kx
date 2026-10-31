# P4KX

Opens, browses, compares and extracts the game archives of Star Citizen and turns their internal
formats into readable or importable data for people and for other tools.

## Language

### The archive

**Channel**:
One installed release line of Star Citizen (LIVE, PTU, EPTU, HOTFIX, TECH-PREVIEW), each with its
own Archive.
_Avoid_: branch, environment, build

**Library folder**:
A folder holding Channel folders, e.g. `C:\Program Files\Roberts Space Industries\StarCitizen`; the
RSI Launcher installs every Channel into one.
_Avoid_: install folder, game folder

**Installation**:
The folders of Star Citizen the tool never writes into: the Library folders and the folder of an
opened Archive that holds the game's `Bin64` folder (ADR-0036).
_Avoid_: game folder

**Manual entry**:
A Channel folder, Library folder or Archive the user added by hand, searched in addition to the
detected places.
_Avoid_: custom path, bookmark

**Recent Archive**:
One of the last Archives opened in the window, the latest first.
_Avoid_: history, MRU

**Build**:
The game version an Archive belongs to, as stated in the Channel's build manifest (branch, version,
change number).
_Avoid_: patch, release, version (alone)

**Archive**:
The `Data.p4k` file of one Channel, holding every game file as an Entry.
_Avoid_: pak, package, zip

**Entry**:
One file inside an Archive, identified by its path in the Archive.
_Avoid_: file, asset, member

**Archive key**:
The AES-128 key that decrypts the encrypted Entries of an Archive. The user enters it; the tool
does not contain it, and it is stored only in the user's profile (ADR-0027).
_Avoid_: password, secret, built-in key

**Entry issue**:
A defect of one Entry found while the Archive's directory is read, such as data outside the file,
an encryption field other than 0 or 1, or a path that is not UTF-8. The Archive still opens and the
Entry stays listed (ADR-0025).
_Avoid_: warning, error (an error stops the open)

**Folder**:
A path prefix that Entries share, e.g. `Data\Libs`; spellings that differ only by ASCII case or
separator kind are one folder, named as its first Entry in path order spells it (ADR-0023).
_Avoid_: directory, Record folder (a Record folder groups Records)

**Path order**:
The order of Entries in folder listings: by path ignoring ASCII case with `\` as `/`, then byte by
byte.
_Avoid_: Archive order (the order of the Archive's directory), sort order

**Container**:
An Entry that is itself a plain ZIP archive, e.g. a `.socpak`, opened in place and read like an
Archive; it may hold further containers.
_Avoid_: nested archive, sub-archive, pak

**Filter**:
Text that selects Entries by path: a glob over the full path when it holds `*` or `?`, otherwise
a substring anywhere in the path; case and separator kind do not matter.
_Avoid_: query (a Query selects Records), search pattern

**Exclusion Filter**:
A Filter that leaves Entries out of a selection: an Entry is selected when the include Filter
matches it and no exclusion Filter does. A split texture is left out with its parts when its header
is; an excluded part of a texture that stays is not written, and a Conversion still reads it
(ADR-0256).
_Avoid_: negative filter, blacklist, ignore pattern

**Content search**:
A search for a text, as part of a longer one and ignoring case, in the string, localisation key and
enum values of Records, in the texts of each language's `global.ini` and in the lines of text and
CryXml Entries; a **hit** names the place and carries a short excerpt. It scans on demand and keeps
no index (ADR-0044); Entries it cannot read, such as binary ones, are skipped and counted.
_Avoid_: full-text index, grep (only the command), Filter (a Filter selects Entries by path)

**Layout**:
The on-disk structure of an Archive's directory: v1 (ZIP64 central directory with local headers)
or v2 (fixed 204-byte records and a name table behind a trailer ending in `Jiji`).
_Avoid_: format version, Archive version

**Preview**:
The start of one Entry shown in the window as text or as hexadecimal bytes, or a texture shown as
an image one mip at a time, or a Model drawn in 3D that the user turns, pans and zooms, read without
an Extraction.
_Avoid_: viewer, inspector

### Game data

**DataForge**:
The game's typed object database, stored in the Entry `Data\Game.dcb` (or `Game2.dcb`).
_Avoid_: DataCore, dcb, Game database

**Record**:
One typed object in DataForge, identified by its GUID and addressed by its record path.
_Avoid_: dcb entry, item (an item is one kind of Record)

**Record path**:
The file a Record is defined in, without `.xml`, e.g. `libs/foundry/records/entities/spaceships/aegs_avenger`;
several Records may share one, so only the GUID is unique.
_Avoid_: file name, record name (the name is `EntityClassDefinition.AEGS_Avenger`)

**Record order**:
The one order of Records in every listing and streamed output: by record path ignoring ASCII case
with `/` and `\` alike, then by Record name ignoring ASCII case, then by definition order in
DataForge.
_Avoid_: path order, sort order

**Record folder**:
A record path that several Records share, shown in the Record tree as a folder holding those Records.
_Avoid_: record group, file node

**Record tag**:
A label DataForge version 8 stores with every Record, e.g. `SystemsDesign` or `Unknown`; version 6
Records have none.
_Avoid_: category, kind

**DataForge issue**:
A deviation from the known DataForge layout that the reader tolerates and reports instead of
failing: conversion flags above the low byte of a property, or a Record without name or tag.
Distinct from an error value, which marks one value that cannot be read.
_Avoid_: warning

**Reference**:
A link from one Record to another by GUID; it is never expanded into the referring Record. A weak
pointer, the stored data type, becomes a Reference into its own Record, with the location of the
instance it points to.
_Avoid_: pointer (alone); weak pointer for the link itself

**Incoming Reference**:
A Reference seen from its target: the Reference that a Record is the target of, with the Record that holds
it and its field and array indices. The Reference index answers which Records refer to a Record, how
often and from how many Records; `p4kx dcb refs` and the Referenced by tab list them.
_Avoid_: backlink, reverse Reference

**File graph**:
The record files of a DataForge with an edge from one file to every other file that a Record of it
refers to.

**Unresolved Reference**:
A null or dangling link that a Query path follows with `->`; it reaches null and is counted
 once per row in the Query result, however many paths of the row meet it. The Reference index also
 keeps every Reference to a GUID no Record has and counts it.
_Avoid_: broken link, missing Reference

**Field path**:
The field names from a Record down to a value, joined by `/`, with no array positions; the
target of a strong pointer adds the name of its actual struct as a segment. Not a Query path.
_Avoid_: JSON path, XPath

**Field statistics**:
For every field path of some Records: how many values it has, in how many Records, of which kinds,
how many are empty or distinct, and the smallest and largest number; `p4kx dcb fields` lists them
(ADR-0049).
_Avoid_: schema (the schema lists what fields may exist, the statistics what the Records hold)

**Query**:
A path expression that selects Records or values inside Records.
_Avoid_: search (searching is over Entry paths), filter (a Filter selects Entries)

**Saved Query**:
A Query kept under a unique name in the settings file, rerun from the Query tab with one click.
_Avoid_: favourite, bookmark, preset

**Condition**:
The `where` part of a Query: comparisons of a Record's values joined by `and`, `or` and `not`, which
a Record has to meet to be selected.
_Avoid_: filter (a Filter selects Entries)

**Component selector**:
The `[TypeName]` segment of a Query path: it selects the elements of a struct array whose struct
is that type or derives from it, e.g. `Components[VehicleComponentParams]`.
_Avoid_: type filter, cast

**CryXml**:
The binary XML encoding CryEngine uses for many Entries outside DataForge.
_Avoid_: CryXmlB, binary XML

**Localisation key**:
The name a Record gives a text instead of the text itself, e.g. `@manufacturer_NameACAS`; it
resolves to one text per language in that language's `global.ini` Entry
(`Data\Localization\<folder>\global.ini`), of which a Build holds eleven. Code follows the game's
spelling `Localization`.
_Avoid_: loc string, text id

**Tag**:
A Record of the struct `Tag` that labels game content, e.g. `Ship.Series.Avenger`. Fields such as
`Tags`, `RequiredTags` and `PortTags` refer to Tags by GUID (a Reference). Not the Record tag.
_Avoid_: label, keyword, category

**Tag tree**:
The Tags in the tree the `TagDatabase` Record lists: its root Tags, each with its children. One
tree per DataForge, read by `TagTree`; a Tag no `TagDatabase` reaches becomes a root of its own,
unlisted. `p4kx dcb tags` and the Tags tab show it.
_Avoid_: tag database (that is the Record), tag hierarchy

**Tag path**:
The names of a Tag and its ancestors from the root down, joined by `.`, e.g. `Weapon.Gun`. Several
Tags may share one path, so only the GUID is unique. Record XML and Record JSON write it next to a
Reference to a Tag with `--tags`, like the text of a Localisation key.
_Avoid_: tag name (that is the last name alone), tag key

### Assets

**Entity**:
A Record that describes a placeable game object such as a ship, vehicle, weapon or item.
_Avoid_: object, actor

**Loadout**:
The tree of Entities attached to an Entity's ports, e.g. a ship's components and weapons.
_Avoid_: fitting, equipment

**Class name**:
The name an Entity is named by in Loadouts, its Record name after `EntityClassDefinition.`, e.g.
`AEGS_Avenger_Titan`; matched ignoring case.
_Avoid_: item name, entity name

**Port**:
A named place on an Entity where another Entity attaches, e.g. `hardpoint_weapon_nose`. Its
**port definition** in the Entity's Record gives the sizes and types it takes and a default
item; its **port transform** is the helper in the Entity's geometry it sits at, with an offset.
_Avoid_: hardpoint (fine as part of a name), slot, socket

**Loadout entry**:
One port of a Loadout as a Record or XML Loadout states it: the port name, the Entity to attach
by Reference or Class name, and optionally a Loadout for that Entity.
_Avoid_: loadout item, fitting

**Outfit**:
The default Loadout of an Entity that is made of a base Loadout and the Loadouts of its outfits,
laid over each other per Port; a story character's body, head, hair and clothing (ADR-0109).
Every outfit without name tags is worn; when all have name tags, the first is (ADR-0181).
_Avoid_: costume

**Bespoke head**:
The head skins a story character has in the folder of its own head's material library, used in
place of the generic Protos skins its Record names (ADR-0109). Its face comes from its own
RigLogic file, skeleton file and head geometry (ADR-0281).
_Avoid_: custom head, NPC head

**Override**:
The Loadout a Loadout entry gives the Entity it attaches, which replaces the Entity's own entries
port by port (ADR-0012).
_Avoid_: patch, customisation

**Loadout changes**:
An own configuration of a Loadout: **exclusions** leave a Port's branch out and **replacements**
attach another Entity, with its own Loadout, to a Port, if it fits the Port's types and sizes
(ADR-0076).
_Avoid_: override (that is the entry's Loadout), mod, swap

**XML Loadout**:
A Loadout stored as an XML or CryXml Entry, e.g. `Scripts/Loadouts/…`, that a Record names by
path instead of listing its entries; it may include further XML Loadouts.
_Avoid_: loadout file, script loadout

**Vehicle implementation**:
The XML file a ship's `VehicleComponentParams.vehicleDefinition` names, with the parts of the
hull, their item ports and flags such as `invisible`, treads and wheels. Its **Modification**
(`VehicleComponentParams.modification`) selects a variant of it: a patch file and attribute
overrides applied before the parts are read (ADR-0050).
_Avoid_: vehicle definition (that is the Record's field), vehicle XML

**Ship base data**:
The facts of a ship that no catalog text holds: hull mass and hull points summed over the parts of
its vehicle implementation, the cargo capacity of the **cargo grids** (Entities of item type
`CargoGrid`) in its default Loadout, its `AttachDef.Size` and the default Loadout as class names
per port path, each with the file and field it was read from (ADR-0061).
_Avoid_: ship stats, ship specs

**Assembly**:
An Entity combined with its Loadout, geometry, materials and textures into one exportable model.
_Avoid_: scene, build (a Build is a game version)

**Part**:
An Entity placed in an Assembly: the root Entity or one attached to a port, named
`<port>/<entity>`, with the model its Record names, a transform relative to its parent part and
the material library its Entity chooses (ADR-0040).
_Avoid_: component, child

**Blender package**:
A folder that describes an Assembly without depending on Blender's file format: `scene.json` (format
version, source Channel and Build, node hierarchy), one GLB file per distinct mesh and the textures
of the materials as PNG (ADR-0083).
_Avoid_: .blend file, scene export

**Interior**:
The rooms and props of an Entity, stored in socpak containers inside the Archive.
_Avoid_: room, level

**Object container**:
A socpak container the Record of an Entity names for its Interior; it is placed at a bone of the
Entity and may hold further containers. Counts of an Entity's Interior are of its object
containers, counts of a container exported on its own are of the containers opened, nested ones
included.
_Avoid_: room container

**Prefab**:
A reusable group of placed objects (models, entities, markers) in a prefab library, an XML file
below `Data\Prefabs`. An object of type `Prefab` instances another prefab; resolving a prefab
expands those into one list of objects with transforms to the prefab's space. Not a UI Template.
_Avoid_: template, blueprint

**WEM**:
A Wwise sound file, loose as an Entry or embedded in a soundbank, holding one audio stream in one
codec (PCM, IMA ADPCM, Wwise Vorbis, Opus, PTADPCM, …).
_Avoid_: audio file, sound file

**Loop**:
The part of a looping WEM that repeats, from its start frame up to, not including, its end frame;
read from the WEM's `smpl` chunk and kept as a `smpl` chunk in WAV and as `LOOPSTART` and
`LOOPLENGTH` comments in Ogg, while the sound itself is exported once.
_Avoid_: repeat, cycle

**Codebook library**:
The Vorbis codebooks a Wwise Vorbis WEM refers to by id instead of storing them: the default
(libvorbis) or the aoTuV library; needed to rebuild the WEM as Ogg Vorbis.
_Avoid_: codebook file, packed codebooks

**Soundbank**:
A Wwise BNK file holding sections such as `BKHD`, `HIRC` (sounds, events and actions) and `DIDX`
with `DATA` (the embedded WEMs, each named by its ID); a streamed WEM may be embedded only as its
prefetched start.
_Avoid_: sound package, audio archive

**HIRC object**:
One entry of a Soundbank's `HIRC` section: a Sound, Action, Event, container, music segment, track,
switch or playlist and so on, with a short ID and, for a node, a parent and children. Listed by
`p4kx bnk objects`, which marks an object the reader does not decode (undecoded), decodes only in
other bank versions (unsupported) or finds malformed.
_Avoid_: bank node, wwise object

**Audio trigger**:
A named sound action an Entity's Record refers to, e.g. `Play_QDrive_Spool`; its ATL definition
maps it to Wwise events.
_Avoid_: sound, audio event

**ATL definition**:
An `ATLConfig` Entry below `Data\Libs\GameAudio\`, text XML or CryXml, that defines audio triggers
with the Wwise events each posts and the soundbanks it preloads.
_Avoid_: audio XML, game audio file

**Wwise event**:
An event object in a Soundbank's `HIRC` section that leads through actions to sounds, identified
by its Wwise short ID, the 32-bit FNV-1 hash of its name in lower case.
_Avoid_: trigger (an Audio trigger is the game's name for it)

**Audio index**:
The Audio triggers of every ATL definition and the Wwise events of every Soundbank of one
Archive, read once while the Archive is open and joined by Wwise short ID.
_Avoid_: sound database, audio cache

**Sound variant**:
One Sound object an Audio trigger may play, reached from a Wwise event through Play actions and
random, sequence, switch and blend containers; it has a WEM ID, a source (embedded, prefetched,
streamed or a plug-in) and a container path from the Event object to the Sound object.
_Avoid_: sound file, variation

**Broken link**:
A place where the way from a Wwise event to a WEM ends without a sound: an object the soundbank
lacks, cannot read or does not follow, a cycle, a limit of the walk, or a localised sound without
a WEM in the Audio language asked for and its fallback; named by the object where it breaks.
_Avoid_: missing sound, unresolved trigger

**Loose WEM**:
A WEM Entry named `<id>.wem` below `Data\Sounds\wwise\` (usually in its `Media` folder), the media
of a streamed or prefetched Sound variant; found by the ID of the WEM, never by a path a soundbank
states.
_Avoid_: external WEM, stream file

**Audio language**:
A language folder below `Data\Sounds\wwise\`, e.g. `English(US)`, that holds localised soundbanks
and Loose WEMs; read from the Archive, not a fixed list, and unrelated to the languages of the
Localisation keys. A localised Sound variant is resolved in the Audio language asked for, else in
the fallback language.
_Avoid_: voice language, locale

**Character**:
A human body with head (CHF/DNA), armour and clothing, assembled like an Entity.
_Avoid_: avatar, player model

**Body variant**:
The build a Character's body comes in, male or female: its skeleton, its default body skin and
the tag (`Male`, `Female`) an Entity's Record gives the geometry worn on it.
_Avoid_: gender, body type (the CHF's field)

**Body zone**:
A named part of the body skin (`torso01_zone`, `l_arm03_zone`) that worn clothing covers on a
layer above the body; a Piece's Record lists the zones it covers.
_Avoid_: body part, region, mask

**Piece**:
One skin worn on a Character's body: a piece of armour or clothing from an Entity, or a head from
a CHF, skinned to the body's skeleton by bone name.
_Avoid_: item (an Entity), attachment

**Look**:
The material choices a CHF makes for one Piece of its head: a base material, and colours and numbers
that replace parameters of its shaders (skin tone, iris colour, hair melanin and dye). The export
draws them as one colour per material.
_Avoid_: skin (a skin is a model; "skin" also names the game's port attachment type, whose parts are
drawn in the bind pose, ADR-0224), theme

**CHF**:
A 4096-byte character file holding a Character's body type, face DNA, worn item ports and
material choices, saved by the character customiser or shipped in the Archive.
_Avoid_: character preset, head file

**DNA**:
The face of a Character in a CHF: for every face part, weighted blends of stock heads.
_Avoid_: face data, morph values

**DNA rig**:
The `.dna` file beside a head skin: the stock heads of a head family with, per head and face part,
the morphs that turn the base head into that head. A Character's DNA mixes them into its neutral
face.
_Avoid_: face rig, RigLogic (a different file)

**RigLogic file**:
The `.RigLogic` file of a head in `Objects/Characters/RigLogic`, named like the head without
`_head`: the controls and correctives of its rig, the joints it moves and the delta records that
move them, for that one head. A story character's bespoke head has it in place of a DNA rig
(ADR-0281); the `RigLogic` chunk of a head skin has the same layout.
_Avoid_: DNA rig, face rig

**Expression**:
One facial movement of a head rig, such as a blink or a lip corner pull: a control named in the
`RigLogic` chunk of the head skin or in the RigLogic file, or a corrective that fires when some
controls are active together. The DNA rig holds its moves per stock head; a Character's DNA mixes
them, and the head is exported with them as named morph targets. A bespoke head has them in the
`BShapesGpu` chunk of its geometry file.
_Avoid_: face morph, shape key

**Cubemap face**:
One of the six images of a cubemap texture, in DDS order +X, −X, +Y, −Y, +Z, −Z; exported as
`posx` … `negz`.
_Avoid_: side, surface

**Layer**:
One element of a texture array, or one slice of a volume texture; exported as one PNG
`<name>_layerN` or `<name>_sliceN`, and previewed tiled on one sheet.
_Avoid_: frame, page

**Chunk file**:
A CryEngine file made of typed chunks in the `#ivo` or the older `CrCh` container, e.g. a model
(`.cgf`, `.cga`, `.skin`, `.chr`), its geometry companion, or an animation (`.caf`, `.dba`).
_Avoid_: chunked file, CGF (one extension)

**Model**:
One chunk file read as nodes with their hierarchy and transforms, meshes per level of detail
(LOD) and submeshes that each draw with one material ID of a material library.
_Avoid_: mesh (one node's triangles), object, asset

**Geometry companion**:
The chunk file next to a model that holds its mesh data, named like the model with an `m`
appended, e.g. `box.cgfm` for `box.cgf`.
_Avoid_: mesh file, streaming file

**LOD companion**:
The geometry companion of a level of detail from 1 on, named `<stem>_lod<n>.<extension>m`, e.g.
`box_lod1.cgfm`. Its meshes belong to the nodes of the base model file.
_Avoid_: LOD file, lod mesh

**Skeleton**:
The bones of a model, read from the `CompiledBones2` chunk of a skin or a `.chr` file, or from
the `CompiledBones` chunk of a static model with animated parts. Each bone has a parent, a bind
pose relative to its parent and one in the model's space. A level of detail of a skin may have a
skeleton of its own in its `<stem>_lod<n>.<extension>` file.
_Avoid_: rig (the face rig of a head is another chunk), armature, joints (the glTF word for the
bones a skin refers to)

**Bone weights**:
The bones one vertex of a skinned mesh is weighted to: up to four bones, or eight in the heads,
each with a weight; the weights of a vertex add up to 255. Stored by the bone-map stream of the
geometry companion and read as `Influences`.
_Avoid_: skin weights, skinning data, bone map (the stream, not its content)

**Animation**:
The rotation and translation keys of the bones of a skeleton over frames, read from a `.caf` file
or from one entry of a `.dba` animation database. A track holds the keys of one controller, which
a bone is addressed by (`controller_id`, the CRC-32 of its name); keys are absolute and relative
to the parent bone. Exported as one glTF animation.
_Avoid_: clip, motion, take

**Pose**:
The matrices that move every bone of a skeleton from its bind pose to where an animation puts it
at one moment, one per bone. The 3D viewer computes it once per frame while an animation plays and
the GPU uses it to move the vertices of a skinned mesh and to draw the skeleton over the model.
_Avoid_: frame (a frame of an animation is a key time), snapshot

**Material library**:
The `.mtl` file a model names with its `MtlName` chunk: one material that serves every material
ID, or a list of sub-materials whose position is the material ID. Each material has a shader,
colours, an opacity and texture slots.
_Avoid_: material file, material set

**Texture slot**:
An entry `TexSlot<n>` of a material that names a texture. The slot and the end of the file name
(`_diff`, `_ddna`, `_spec`) say what the texture is for; a `.tif` name in a material is the `.dds`
Entry of the Archive. The textures of a model's materials are embedded in its GLB as PNG images.
_Avoid_: texture channel, map slot

**Texture group**:
A `.dds` header Entry with its parts `.dds.1` … `.dds.N` and its Alpha tail, listed as one item
with the sizes of the whole texture. A part or tail without a header Entry is in no group and
stays an Entry of its own.
_Avoid_: texture set, texture bundle

**Alpha tail**:
A second split texture `.dds.a` with parts `.dds.1a` … `.dds.Na` that holds a texture's alpha or
smoothness channel, merged into the alpha channel on export.
_Avoid_: attached alpha, gloss map, smoothness map

**UI movie**:
A Scaleform GFx (`GFX`, `CFX`) or Flash SWF (`FWS`, `CWS`, `ZWS`) file of the game's user
interface: a stream of tags that define shapes, bitmaps, fonts and sprites and place them on
timelines.
_Avoid_: Flash file, SWF (alone), GFx asset

**Definition**:
A shape, morph shape, bitmap, font or sprite a UI movie defines under a numeric ID, which
placements and other definitions refer to; an export name may name it.
_Avoid_: character (a Character is a human body), symbol

**External image**:
A bitmap of a GFx UI movie stored outside it, as a texture file named relative to the movie; the
game ships it as DDS even where the movie names a TIFF or TGA.
_Avoid_: linked image, imported bitmap

**Imported asset**:
A definition a UI movie takes from another UI movie (`ImportAssets`) under a local ID, by the
export name the other movie gives it; its font is found by that name, never by an ID of the other
movie.
_Avoid_: linked asset, shared font

**Placement**:
A definition put on a timeline of a UI movie at a depth, with a transform, a colour transform
and, for a mask, the depths it clips; the placements of a frame form its display list.
_Avoid_: instance

**Scene**:
A frame of a UI movie composed into drawing instructions independent of the output: each shape
once, its placements with transforms, colour transforms and masks, and what was skipped. It is
rasterised to PNG or written as SVG. Text is part of it as glyph shapes, one per glyph and colour.
_Avoid_: render tree, stage (the stage is the area of the movie)

**Screen**:
A `BuildingBlocks_Canvas` Record that an Entity's Record refers to, e.g. the MFD, the radar or the
HUD of a ship: widgets laid out inside a canvas of its own size, nesting further canvases through
canvas widgets. `p4kx ui screens` lists them, `p4kx ui render --screen` draws one into a Scene
(ADR-0089).
_Avoid_: UI movie (a Screen is DataForge data, not a movie), display (that is the Entity that shows it)

**Widget**:
One node of a Screen's `scene` array, a `BuildingBlocks_DisplayWidget` or a type derived from it
(card, canvas, text, image, circle, ...), with sizing, margin, padding, a flex, stack or grid
layout policy, an anchor, a pivot, a turn, tags, a background and a border; the tree of widgets is
given by their `parent` links.
_Avoid_: element (the renderer's report names widgets by element type), node

**Template**:
A widget in the `library` array of a Screen's canvas, with the widgets whose `parent` it is. It is
not drawn where it stands: a clone draws it once and a list once per row of its static array
(ADR-0106).
_Avoid_: prefab, component (a component is the Entity's UI component)

**Style cascade**:
The rules that give a widget its colours, geometry and visibility at rest: the palette slots of a
`BuildingBlocks_Style`, and style entries whose selectors (type, tag, parent, ancestor, child,
count and their combinations) pick widgets and whose modifiers set fill, stroke, background,
border colour, alpha, activity, sizes, margins, padding, anchor, pivot, scale and flex settings.
State tags bound by a canvas are tags of their widget, and an animation that holds its last frame
is applied at that frame (ADR-0287); an entry that names no type and selects by the parent also
colours the text of a text field (ADR-0296); transitions are not evaluated (ADR-0089, ADR-0110).
_Avoid_: CSS, theme

**State at rest**:
What a Screen shows before the game sets anything: every game variable has its zero value, and
the `operations` of each canvas, evaluated on that, decide which widgets are instantiated and
active, their alpha, the number fields of their layout (size, anchor, pivot, scale, position,
padding, margin, bounds) and the parameters of nested canvases (ADR-0110, ADR-0296). A caller may
set game variables, among them the one that names the page of an MFD (ADR-0259, ADR-0306).
_Avoid_: default state, idle (the game has no such state)

**Text field**:
A definition of a UI movie that shows a text (`DefineEditText`), with a font, size, colour,
alignment and optionally HTML markup or a localisation key (`@key`); a static text (`DefineText`)
places glyphs by index instead.
_Avoid_: label, string

### Operations

**Extraction**:
Writing selected Entries or Records from an Archive into a folder on disk.
_Avoid_: unpack, export (an Export is something else)

**Conflict policy**:
What an Extraction does with a file already in the target folder: skip it, overwrite it, or
overwrite it only if changed (its size or the Entry's checksum differs from the Entry, or its size or
CRC-32C from the converted output).
_Avoid_: collision mode, overwrite mode

**Verification**:
Checking the decoded data of an Entry against the CRC-32C its directory states, and its stored bytes
against the SHA-256 it states (v1 Archives only), while the data is read: `p4kx verify` for a whole Archive or a
Filter, `--verify` during an Extraction. Off unless asked for.
_Avoid_: validation, integrity scan

**Conversion**:
Turning an Entry or Record from its game-internal format into a readable one, e.g. CryXml or a
Record into XML.
_Avoid_: decoding, transformation

**Comparison**:
The set of Entries and Records that were added, removed or changed between two Archives.
_Avoid_: diff (fine in code), delta

**Release**:
A published version of P4KX itself: a `v*` tag with its portable ZIP, its MSI and its
notes from `CHANGELOG.md`.
_Avoid_: build (a Build is a game version), update

**Update check**:
The optional, user-switched question to GitHub whether a newer Release than the running one exists;
it only links to that Release, never downloads it.
_Avoid_: updater, auto-update

**Basetool catalog**:
The JSON Export of manufacturers, items, ships, commodities and blueprints that the basetool's P4K
import consumes.
_Avoid_: p4k-catalog, basetool export

**Kind**:
One group of entities in the Basetool catalog, e.g. manufacturers or items, selected by record-path
prefix and type and complete or partial on its own.
_Avoid_: category, entity type

**Export**:
A structured file derived from DataForge for another tool, as opposed to an Extraction of Entries.
_Avoid_: dump

**Record export**:
The generic Export of selected Records as Record XML or JSON, one file per Record or one combined
file.
_Avoid_: JSON dump, DataForge dump
