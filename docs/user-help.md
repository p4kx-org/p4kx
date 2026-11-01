# P4KX help

How to use the P4KX window. Terms follow [CONTEXT.md](../CONTEXT.md). The command line and the
MCP server are described in [commands.md](commands.md).

## Preview

Click an Entry's name in the table to preview it below the table. The Entry is read straight
from the Archive in the background, whatever its compression and whether it is encrypted, so
the window stays responsive while it loads.

- **Text** shows the Entry as text, line by line. UTF-8 is assumed; a UTF-8 or UTF-16 byte order
  mark is honoured. Bytes that are not valid text and control characters other than tabs and
  line breaks show as a middle dot `·`.
- **Hex** shows 16 bytes per row: the offset, the bytes in hexadecimal and the printable ASCII
  characters, with a dot for every other byte.
- **Image** appears for a texture's header `.dds` and an alpha tail `.dds.a`, and opens when you
  select one. Above the image a facts bar gives the format, the size of the largest mip, the
  number of mips and the cubemap faces. The texture's split parts are read and decoded in the
  background; the first mip shown is the largest that fits the preview, at one texture pixel per
  screen pixel. The **Mip** buttons, labelled with each mip's longer side, switch to another mip;
  a mip larger than the preview scrolls. A cubemap shows its six faces as a cross, the elements
  of a texture array and the slices of a volume are tiled on one sheet, row by row from the top
  left (the facts bar then gives their number as **Layers**), an alpha tail is merged into the
  alpha channel, and BC6H and floating-point formats are tone-mapped. A format the preview cannot decode is
  named; the Text and Hex tabs still show the Entry's data.
- **Info** appears next to Model for a model file and lists what the file holds: the nodes, the
  meshes and levels of detail with their vertices and triangles, the geometry file, the material
  library and the material IDs the submeshes use, the bones, and the chunks, counted as read,
  skipped and unknown, with the chunks the model did not read listed by file, type and type ID.
  Animations show the number of animations in the `.caf` files and `.dba` databases next to the
  model and in the folder at the same place below `Data\Animations` that an export writes by
  default (additive animations are not counted); they read "Reading…" while the files are read
  after the model, and "Not read" when they could not be read or are more than 64 files or
  64 MiB. A model without bones reads none. The tab also works for a model the 3D view cannot
  show, such as a skeleton file.
- **Export as GLB…**, at the right end of the tab bar for a model file and in the Export menu,
  opens a dialog with the level of detail (each with its triangles), the material mode (plain
  colours, or with textures at a chosen size), whether to write the vertex colours, for a model
  with animations whether to write them (off by default), and the file to write. The file cannot
  be inside a Star Citizen
  installation. The export runs in the background with a progress bar and **Cancel**, writes the
  file only when it is complete, and ends with a summary of what the file holds, the chunks it
  skipped, the materials that fell back to the default material, the textures the Archive
  lacks, the animations and channels written and the animations it left out (additive ones,
  files that could not be read, and animations none of whose tracks matches a bone of the
  model). A failure shows a message
  and the rest of the window keeps working. A model that needs the Archive key opens the key
  dialog; export again once the key is entered. The choices are remembered.
- **Model** appears for a model file (`.cgf`, `.cga`, `.skin`, `.chr`) and opens when you select one. The
  model and its geometry companion are read in the background and drawn in 3D, lit and framed to
  fit, at its most detailed level, without materials. A model with bones can show its **Skeleton**,
and its animations play with **Play**. Above the view a facts bar gives the
  vertices, the triangles and the size in metres. Drag to orbit, drag with the right mouse button
  (or Shift) to pan, and use the wheel to zoom; once the view has the keyboard focus, the arrow
  keys orbit, Shift and the arrow keys pan, `+` and `-` zoom and Home resets. **Reset view** frames
  the model again. A model with more than 4,000,000 vertices or 8,000,000 triangles, or a model
  file or companion above 256 MiB, is not shown. If the graphics device cannot draw the view or
  is lost, the tab says so and the rest of the window keeps working. Enlarge the preview by
  dragging the divider above it to see more of the model.
- **Copy** puts the open tab on the clipboard: the text, or the hex rows as shown.

### Size limits

| Limit | Value | What happens |
|---|---|---|
| Largest Entry previewed | 256 MiB | A larger Entry is not read; the preview says so. Extract it to look inside. |
| Bytes shown | 1 MiB | Only the start of a longer Entry is read and shown, with a note giving the bytes shown and the Entry's full size. Copy takes only what is shown. |

The limits are fixed.

## Containers

A socpak container (`.socpak`) is an Entry that holds Entries of its own. In the path tree it
follows the subfolders of its folder and expands like a folder: expanding or selecting it opens
it in the background, with a spinner and *Opening…* on its row, and shows its folders and the
containers inside it, which expand the same way. Selecting a container or one of its folders lists
its Entries in the table; the search, the checkboxes, **Extract selected…**, **Extract shown…** and
the preview then work on the Entries of that container, which are written at their paths inside
it. **Extract all…** always extracts the whole Archive, and a container is not opened by it.

Each container is opened once and kept while the Archive is open. The open containers may hold
1 GiB of memory together; when another one needs room, containers whose rows are collapsed and
whose Entries are not shown are closed first, and they open again when expanded. A container that
cannot be opened says so on its row, *Cannot open* for a corrupt one, *Not opened: memory limit*
when the open containers need their memory; hovering the row gives the reason. Collapsing a
container while it opens cancels the opening. When the Archive changes on disk, its containers
are closed until you reload it.

## Extracting

**Extract selected…**, **Extract shown…** and **Extract all…** open the dialog **Extract Entries**
with the scope, the target (a new folder `SC-EXPORT-<Channel>-<Build>-<time>` inside the chosen
folder, or an existing folder) and what to do with files that already exist. The target cannot be
inside a Star Citizen installation. The field **Leave out** takes one Filter per line, with the
rules of the search, and skips the Entries they match: `*_lod?.cgam` leaves out the level-of-detail
companions of models. A texture whose header is matched is left out with its parts; with the
Conversion on, an excluded part of a texture that stays is still read to build the texture, and the
summary says so. The dialog refuses Filters longer than 1024 bytes or with more than 32 wildcards,
blank lines and a selection that leaves nothing. The same field is in the dialog of a Comparison
side.

Only file names valid on both Linux and Windows are written. The summary of an Extraction with
Conversion and of a Record export lists the Records whose file does not follow their name or record
path, with GUID, record path and reason (*Renamed Records*); they are written and do not count as
failed.

**Archive › Convert folder…** converts a folder of files that were extracted earlier without
conversion (CryXml, split textures, sounds, soundbanks and a DataForge file) into a new folder, as
the Conversion of an Extraction does. It runs in the background with progress and **Cancel**, and
ends with a summary. `p4kx convert` does the same on the command line.

## Comparing two Archives

The **Comparison** tab next to **Browse** compares two Archives, for example an older against a newer Build.

1. Pick the **Old Archive** and the **New Archive**. Each picker lists the detected Channels with
   their Build, the Recent Archives, and **Choose file…** for any other `Data.p4k`.
2. Switch on **Entries**, **Records** or both. The Record level reads the DataForge of each
   Archive and fails with the reason when one has none. The list shows the Entry changes; the
   Record changes go into the saved report.
3. Press **Compare**. Both Archives are opened and their central directories joined in the
   background, without decompressing anything; a progress bar shows the Entries joined, and
   **Cancel** stops at once.

The result shows both sides with Channel and Build, old first, so the direction of every change
is clear. An Entry is added when only the new Archive has it, removed when only the old one has
it, and changed when its size or CRC-32C differs; paths match ignoring case and separator kind.
The tiles count each kind, and **By directory** counts them per top-level directory.

The list below can be filtered with the rules of the search (a glob with `*` or `?`, otherwise
any part of the path) and sorted by any column; the counts follow the Filter. **Save report…**
writes the JSON report of what is shown, the same file `p4kx compare --report` writes with that
Filter (see [cli-json.md](cli-json.md)).

## DataForge

When the Archive holds DataForge (`Data\Game2.dcb`, or `Data\Game.dcb` in older Archives), a
**DataForge** node follows the folders of the path tree. Expanding or selecting it loads DataForge
in the background; a spinner and *Loading…* show on the node meanwhile, and the window stays usable.
The Records then appear as folders by record path: a Record alone at its record path is named after
the last part of its record path; several Records sharing one record path sit in a record folder
named after that part.
Hovering a Record shows its name, type and GUID.

DataForge is loaded once per open Archive and kept until you reload the Archive or it changes on
disk. A DataForge version this P4KX does not read (only versions 6 and 8 are read), or a database it cannot read, is named
on the node; the rest of the window keeps working.

The **DataForge** tab of the preview shows the version, the numbers of Records, structs and enums,
and the first 10 Records in the order DataForge stores them.

The **Referenced by** tab, next to the **Record** tab while a Record is selected, lists the Records that
refer to the selected Record: one line per Reference with the field that holds it, e.g.
`Components[3].maker`, and a link to the Record that holds it; a click selects that Record and stays in
the tab. A count line gives the References and the Records they come from, and a list is cut after
10,000 lines. The first time the tab is opened for an Archive, every Record is read once to build the
index of References, which takes a moment and runs in the background; later Records and the
`referencing` clause of a Query use the same index. `p4kx dcb refs` lists the same References on the
command line.

### Tags

Many fields of Records (`Tags`, `RequiredTags`, `PortTags`, …) refer to a Tag by its GUID. The **Tags**
tab shows the tag tree the `TagDatabase` Record lists: each Tag with its GUID and its legacy ID, the
children below their parent. The tree starts collapsed at its roots; the chevron, or a double click on
the row, opens and closes a Tag. **Search** matches the tag path, e.g. `Weapon.Gun`, ignoring case, and
shows the matches with the Tags above them, however the tree is opened. Select a row and press **Copy
tag path** or **Copy GUID** to put it on the clipboard; the tooltip of a row gives its full tag path.
The table keeps the width of its columns and scrolls sideways in a narrow window; the legacy ID is
a number and stands right-aligned. The tab reads the tags in the background when it is first opened for an Archive and shares the
DataForge with the other views. A note above the tree says when it differs from what the
`TagDatabase` lists, for example Tags that no `TagDatabase` reaches, which stand as roots of their
own. `p4kx dcb tags` prints the same tree; `p4kx dcb export --tags` writes the tag path next to every
Reference to a Tag in the Records.

### Searching Records

While the DataForge node or one of its Records is selected, the list panel searches Records instead
of Entries. Type into **Search Records** to find Records by record path or type name, with the
same rules as the Entry search: text with `*` or `?` is a glob over the whole record path or type
name, any other text matches anywhere in it; case and `/` or `\` do not matter. The Record's name
alone does not match. The search runs in the background and starts again with every change; the
matches list name, type and record path, sorted by record path, and their number shows next to the
field. Clicking a match, or pressing Enter on it, selects the Record in the tree, opens the folders
above it and shows the **DataForge** tab. Selecting a folder of the Archive returns to the Entries.

### Exporting Records

**Export › Record export…** writes Records as Record JSON (see [record-json.md](record-json.md)).
It is offered while the open Archive holds DataForge that did not fail to load, and while no other
Record export runs.

1. Choose the Records: **Selected in the Record tree** takes the Records at and below the node
   selected in the tree when the dialog opened (the DataForge node takes every Record), and
   **Record path filter** takes every Record whose record path matches the text, with the rules of
   the search: a glob with `*` or `?`, otherwise any part of the record path.
2. Choose the form: **JSON** writes one file per Record under its record path, Records sharing a
   record path into a folder of that name; **NDJSON** writes the one file `records.ndjson` with one
   Record per line in record-path order.
3. Choose the target like for an Extraction: a new folder `SC-EXPORT-<Channel>-<Build>-<time>`
   inside the chosen folder, or the chosen folder itself with what to do about files that already
   exist.
4. Press **Start export**.

The export runs in the background with a progress window; the first export of an Archive reads
DataForge first, and later ones reuse it. **Cancel** stops it at once. The summary gives the
Records written, skipped, failed and cancelled, the Records with error values and their number,
and the target, then lists failed Records with the reason. A cancelled NDJSON export keeps no
partial file.

### Creating the Basetool catalog

**Export › Basetool catalog…** writes the Basetool catalog of the open Archive (see
[basetool-catalog.md](basetool-catalog.md)), the same file as `p4kx catalog`. It is offered while
the open Archive holds DataForge that did not fail to load, and while no other catalog is being
created. Choose the file in the save dialog, `basetool-catalog.json.zst` by default; an existing
file is replaced. The open Archive itself is refused as the target. The file name decides the form,
not the file type selected in the dialog, which no system reports back: a name ending in `.zst`
writes the catalog as one zstd frame, a fraction of the size, which the Basetool accepts as it does
plain JSON; a name ending in `.json` writes plain JSON for inspection. The name is corrected after
the dialog: `foo.zst` becomes `foo.json.zst`, and a doubled `.json` that a dialog appended is
collapsed. A name ending in `.gz` is refused with a message, because gzip is not written. The summary
shows the compressed and the uncompressed size, so it tells which form was written.

The catalog is created in the background with a progress window; DataForge already read for the
Record tree, a Query or a Record export is reused, and the other way round. **Cancel** stops it at once and writes no file; an
existing file stays as it was. The summary lists per kind the entities written, the Records left
out as development leftovers and whether the kind is complete, per language the texts resolved,
missing and placeholders, the size against the basetool upload limit of 64 MB and the file. A
catalog close to the upload limit gets a warning, one over it an error note, because the basetool
rejects it. A selected Record that cannot be read does not stop the catalog: it is left out and
listed in the summary under "Records that could not be read and were left out", with its record
path, GUID and reason.

## Searching the contents

The **Search** tab finds a text inside the Archive, not only in paths and names. Type the text,
tick where to look (**Records**, **Localisation**, **Entries**) and press **Search** or Enter. The
text is found as part of a longer one and whatever its case: `Gladi` finds `AEGS_Gladius`.

- **Records**: the string, localisation key and enum values of every Record, with the field they
  stand in, e.g. `Components[3].AttachDef.Tags`.
- **Localisation**: the texts of every language's `global.ini`, with language and key.
- **Entries**: the lines of text and CryXml Entries (CryXml is converted to XML first, so the line
  numbers are those of the XML), by default of the extensions xml, mtl, cdf, adb, entxml,
  chrparams, animevents, ini, cfg, json, txt, lua, csv, eco and tbl.

The counts on the segments above the list say how many hits each source has; click a segment to
list its hits. Click a hit to open it: a Record is selected in the Record tree, an Entry in its
folder, a localisation text in the Localisation tab. Each source keeps at most 1,000 hits, a Record
or Entry shows at most three of its own, and a note says when more exist. Entries that hold binary
data, are above 64 MiB, cannot be read, or are encrypted and need the Archive key (see below) are
not searched; the note names how many. Nothing is indexed: each search scans the Archive again,
which takes about a second for all Records and two for all text Entries of LIVE. The same search
runs on the command line as `p4kx grep`.

## Running Queries

The **Query** tab runs Queries ([query.md](query.md)) over the DataForge of the open Archive, for
example `from AmmoParams select speed, lifetime where speed >= 1500`. Write the Query into the
editor and press **Run Query** or Ctrl+Enter. DataForge is read in the background the first time
and kept for the next Query of the same Archive; a spinner and then a progress bar show how far
the Query has come. **Cancel** shows the Query as cancelled at once; a DataForge that is still
being read is read to the end in the background and kept, so the next Query starts without
reading it again.

`referencing "<GUID or record path>"` selects the Records that refer to a Record, e.g.
`from Ship referencing "libs/makers/aegis"`.

A Query that cannot run is explained below the editor with its line and column, and the part of
the Query it concerns is underlined in red, e.g. a mistyped field with the name it probably meant.
The mark goes away as soon as you change the Query.

While you type, the editor lists the names that fit at the cursor from the DataForge schema: the
struct names after `from` and inside `[…]` (the struct of the array's elements and every struct
deriving from it), and the field names at the start of a path, after `.` and after `->`, each
with its type. The list starts with the names that begin with the word typed so far, ignoring
case; ↑ and ↓ choose a name, Enter, Tab or a click writes it (in backticks when it needs them),
Escape closes the list and Ctrl+Space opens it again. The names are read once per Archive in the
background when the editor first gets the focus.

**Save Query…** keeps the Query of the editor under a name in the settings, so it is there again
after a restart; saving under a name already taken replaces that Query. The saved Queries are
listed under the editor: a click on one puts it into the editor and runs it, **Rename** and
**Delete** change the list. A settings file larger than 16 MiB is not read, and saved Queries
in it that are damaged, nameless, empty, longer than 65,536 bytes or beyond the 200th are left
out.

The result is a table with the record path first and one column per path of `select`, in record
path order. Numbers stand right-aligned; a path through `[*]` or `[TypeName]` shows its list of
values in brackets, more than eight as the first eight and how many more; `null` is grey and a
value that cannot be read red. Click a column head to sort by it: numbers by value, text ignoring
case and lists by their first value and then their length; click again to reverse. Values that
cannot be read, nulls and missing values stay at the end either way. Large results are shown
500 rows per page; the number of Records and the page buttons are below the table.

**Export CSV…** and **Export JSON…** write the whole result in the order of `docs/query.md`,
byte for byte as `p4kx dcb query --format csv|json --out <file>` writes it, whatever the table is
sorted by. The file is written in the background and only replaces an existing file once
complete; **Cancel export** stops it and leaves any existing file as it was. The open Archive
itself is never overwritten.

## Looking up localisation

The **Localisation** tab lists every localisation key of the Archive with its text in each language
of its `global.ini` files. Type a key or a text into **Search** (`item_Name`, `Aurora`) to narrow the
list; **Copy key** and **Copy text** put the selected row on the clipboard. A language without a
text for a key shows *Missing*. A `global.ini` that was not read cleanly (invalid bytes, lines
without `=`) is named in a note above the table. **Copy text** copies the text with its line breaks
decoded and without leading and trailing blanks or line breaks; `p4kx loc` looks the same keys up on
the command line and keeps the text exactly as the file stores it.

## Entities and the 3D view

The **Entities** tab lists the ships, vehicles and weapons of the DataForge, filtered by **All**,
**Ships**, **Vehicles** or **Weapons** and searched by class name or record path (`AEGS_Gladius`).
A ship with variants shows a row for its base and a fold-out for the variants.

Selecting an Entity resolves its default Loadout, the tree of its ports with the Entity attached to
each. Per port, the **Include** checkbox leaves a branch out of what is built, **Replace…**
attaches another Entity (the dialog lists the Entities and says which ones fit the port's item
types and sizes), and **Restore** undoes it; **Reset changes** returns to the default Loadout. A docked vehicle is a
part of its own and is not expanded. Ports that cannot be resolved are listed with the reason.

Beside the Loadout the Assembly is drawn in 3D: all parts at their ports, with **Detail** (the
level of detail), **Materials** (plain **Colours**, or **Textures** read in the background), and
**Interior** to add the rooms of the ship's object containers. The view reports the parts, meshes
and triangles; parts can be hidden and shown, and parts the viewer could not resolve are counted
and listed. Drag to orbit, drag with the right mouse button (or Shift) to pan and use the wheel
to zoom, as for a single model. **Stop** ends a build that takes too long, **Build again** starts
it again.

**Export…** writes the Assembly as one **GLB file** or as a **Blender package** (a folder with
`scene.json`, one GLB per mesh and the textures, see [blender-package.md](blender-package.md)),
with the level of detail, the materials and the texture size of your choice. The Export menu
offers **Export Interior…** for a socpak container on its own. The dialog follows the same rules
as the model export: the target cannot be inside a Star Citizen installation, the file is written
only when complete, and the summary lists what could not be resolved, including the layer files
of the materials that are missing. For a creature or an NPC whose Loadout the game generates at
random, **Loadout alternatives** takes another alternative of a group (`Body=1, Eggs=2`; counting
from 0, a bare index applies to every group that has it). `p4kx entity glb`,
`entity package` and `interior glb|package` do the same on the command line.

## Characters

The **Characters** tab composes a Character and shows it in 3D. Choose the **Body** (male or
female; a head can decide it), the **Head** from the Archive or from a CHF character file saved by
the character customiser, and the **Armour and clothing**: a Loadout (an outfit of a character
Entity or an XML Loadout) and single pieces added to it. A piece takes the place of the Loadout's
piece of the same item type. **Expressions** loads the facial expressions of the head, each with a
strength slider.

**Export…** writes the Character as a GLB or a Blender package with the skeleton, the shape keys and
the materials, with textures when you choose them and the animations of your choice (none or all).
Pieces that could not be fitted are listed in the report. `p4kx armour glb`, `character package`
and `chf glb` do the same on the command line.

## Sounds

The **Sounds** tab shows the audio of an Entity. Enter a class name, GUID or record path of an
Entity (or use the selected Record) and press **Show sounds**, or search the audio triggers and
Wwise events of the whole Archive by name (`Play_*_Spool`; `*` and `?` allowed). Each trigger shows
its status (resolved, no definition, no event, no soundbank) and its Wwise events. Selecting a
trigger resolves its sounds: soundbank, WEM ID, source (embedded, prefetched, streamed, plug-in) and
language, and lists the places where the way to a sound breaks. Select a sound to play it; a
sound with no loose WEM Entry or only a prefetched start is explained instead. **Export sounds…**
writes the sounds of the selected trigger or of all triggers as Ogg, WAV or unchanged WEM files.
`p4kx wwise` and `p4kx entity audio` do the same on the command line.

## UI movies and ship screens

The **UI** tab draws pictures. **Files** lists the GFx and SWF movies of the Archive and draws the
first frame of the selected one, **Ships** lists the ships and vehicles and the screens (MFDs,
radar, HUD) of the selected one, and draws a screen from its Records. Choose the **Size**, the
**Background**, the **State** (*At rest* or *Powered on*, which sets `powerstate=1`), more game
variables as `NAME=VALUE` in **Variables** (`mfdviewname=…`, `landscapecanvasguid=…` to draw one
page of a powered MFD) and the **Format** (PNG or SVG) and press **Export…**. The state and the
variables are kept across sessions, and the report lists the variables no canvas of the screen
reads and the textures the Archive lacks. The tabs **Contents** and
**Left out** list what the movie holds and what the renderer left out (fonts it lacks, unsupported
widget types, limits). `p4kx ui` draws the same pictures on the command line, and `p4kx ui
hologram` draws the hologram of a vehicle, which the window does not offer.

## The window

- The tab bar has **Browse**, **Comparison**, **Query**, **Localisation**, **Tags**, **Search**,
  **Entities**, **Sounds**, **Characters** and **UI**. The sidebar lists the detected Channels
  (Archive menu: **Open Archive…**, **Open DataForge file…** for a `Game2.dcb` kept on its own,
  **Add folder…**, **Add Archive…**, **Convert folder…**, **Recent Archives**), or you
  drop a `Data.p4k` on the window. A DataForge file opens as its DataForge only: no Entry table,
  folder tree or Entry counts, and no Search tab.
- The **Export** menu holds **Record export…**, **Export as GLB…** for a model, **Export
  Interior…** and the **Basetool catalog…**.
- The smallest window is 640 by 400 points. Below 872 points of width the sidebar folds into a
  rail on the left edge; a button on it shows and hides the sidebar. A toolbar that does not fit
  puts its actions in a line of its own, and what still does not fit scrolls.
- The look is P4KX's own dark theme in the Lato typeface. **Language** in the menu switches
  between German and British English; the first start follows the system language.
- **Graphics.** On Windows the 3D view opens Direct3D 12 only, so the Vulkan drivers and the
  overlays injected into them are not loaded. A machine where Direct3D 12 yields no adapter or
  device within a few seconds falls back to the default backends. The environment variable
  `WGPU_BACKEND` (for example `vulkan`) overrides the choice. If the graphics driver does not
  release the device in time at exit, the window ends the process with status 4 ten seconds after
  it has saved its settings and leaves a note in the settings folder.

## The Archive key

Encrypted Entries can only be read with the key of the Archive, which the tool does not contain.
Everything else works without it: opening, listing, searching and comparing an Archive and
reading its unencrypted Entries.

The first time an encrypted Entry is needed, the window asks for the key: a dialog with a field for
32 hexadecimal digits, hidden unless you tick **Show the key**. **Save** checks the key against an
encrypted Entry of the open Archive; a key that does not decrypt it is refused and stored nowhere,
and a key that does is stored in your user profile and used from then on, and the Entry is read
again. Without an open Archive, or one with no encrypted Entry to check against, the key is stored
unchecked. **Cancel** closes the dialog, and the window does not ask again for this Archive; the
preview of an encrypted Entry then says that the Archive key is missing. If P4KX_ARCHIVE_KEY or the
stored key file holds no valid key, the dialog says so instead of ignoring it silently.

**Archive key…** in the Archive menu opens the dialog at any time to enter another key. It also
shows whether a key is stored, with all but its last four digits hidden, and **Remove stored key**
deletes it. The key is never shown in full, logged or put in an error message. On the command
line, `p4kx key` does the same and `--key` and `P4KX_ARCHIVE_KEY` give a key for one run.

## When the Archive changes

The RSI Launcher may update an Archive while it is open in the window. The window compares the
Archive's size, modification time and Build with the state it was opened in before every
Extraction and preview, and about once a second while it is open.

Once it changed, a notice with **Reload** appears below the menu. A running Extraction stops as
if cancelled, and its summary says that the Archive changed; files written before the change was
noticed may be stale. No Extraction or preview reads from the changed Archive until you reload
it, which opens it again as it is on disk now.

The command line behaves the same way: `p4kx extract` stops and exits with code 4.

## Installing and updating

Each Release on GitHub offers three downloads:

- **`P4KX-<version>-x86_64.msi`** installs for the current user only, without
  administrator rights, into `%LOCALAPPDATA%\Programs\P4KX`. It installs `p4kx-gui.exe`,
  `p4kx.exe` and `p4kx-mcp.exe`, adds P4KX to the Start menu and that folder to your `PATH`, so
  `p4kx` works in a new terminal. A newer MSI replaces the installed version in place; uninstall
  through **Settings › Apps**.
- **`P4KX-<version>-x86_64-portable.zip`** holds `p4kx-gui.exe`, `p4kx.exe` and `p4kx-mcp.exe`
  to run from any folder, without installing anything.
- **`p4kx_importer-<version>.zip`** is the Blender add-on that imports a Blender package. In
  Blender 5.2 LTS use Edit › Preferences › Get Extensions › Install from Disk. It needs no
  installer and has the same version as the Release.

Both keep their settings in `%APPDATA%\P4KX\settings.toml`, so switching between them or
uninstalling keeps them. Coming from P4K-Extractor, the first start copies its old settings
folder `%APPDATA%\P4K-Extractor` (settings and stored Archive key) into `%APPDATA%\P4KX` when that
does not exist yet; the old folder stays as a backup.

### About

**Help › About P4KX** shows the version, the licence, a link to the source code and the legal
notices: P4KX is an unofficial Star Citizen fan project, not affiliated with or endorsed by the
Cloud Imperium group of companies. It ships no game data and reads only your own installation, and
you are responsible for complying with the Star Citizen EULA and Terms of Service. In a very small
window the text scrolls. `p4kx --version` and `-V` show the same legal notice.

### Archives of test Channels

An Archive in a `PTU`, `EPTU`, `HOTFIX` or `TECH-PREVIEW` folder holds material of Star Citizen
test builds. Material of the community tests is confidential under the Star Citizen Terms of
Service, and `HOTFIX` builds are treated the same way. You can still open,
browse, extract and compare it, and the window says so with a warning below the header for as long as
such an Archive is open, in every tab. In the Comparison tab it names the test Channel or Channels
of the two Archives. Do not share or publish anything taken from it. For `EPTU`, the Evocati test universe, the notice adds that the content may also be
covered by a non-disclosure agreement and must not be shared. The command line prints a
`notice: …` line to stderr once per Archive and run, and the MCP server adds the notice to its results.

### Checking for a new version

**Help › Check for a new version at start** is off until you switch it on. While it is on, the
window asks the GitHub API once at start for the latest Release of P4KX; switching it on
also asks at once. When a newer Release exists, the status bar shows **Version … is available**,
which opens its page in your browser.

The request is a plain anonymous GET of
`https://api.github.com/repos/p4kx-org/p4kx/releases/latest`; it carries only the
program name and version as its user agent and nothing about you, your machine or your Archives.
It is the only network traffic of P4KX. A failed request shows nothing.
