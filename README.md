<p align="center">
  <img src="docs/design/p4kx-logo.svg" alt="P4KX logo: data brackets around an X" width="128" height="128">
</p>

# P4KX

P4KX opens, browses, searches, compares and extracts the `Data.p4k` Archives of Star Citizen, and
turns their internal formats into readable or importable data: XML, JSON, PNG, OpenEXR, Ogg, WAV,
glTF and Blender packages. It comes as a Windows window (`p4kx-gui`), a command line (`p4kx`), a
read-only [MCP server](#mcp-server) (`p4kx-mcp`) and a [Blender add-on](#blender-add-on).

The current version is **[0.4.2](https://github.com/p4kx-org/p4kx/releases/tag/v0.4.2)**. P4KX is written in Rust and licensed under either of the
[Apache License, Version 2.0](LICENSE-APACHE) or the [MIT licence](LICENSE-MIT) at your option.
The source lives at [github.com/p4kx-org/p4kx](https://github.com/p4kx-org/p4kx).

P4KX is an unofficial Star Citizen fan project, not affiliated with the Cloud Imperium
group of companies and not endorsed by it. It ships no game data and reads only your own Star
Citizen installation. You are responsible for complying with the Star Citizen End User Licence
Agreement (EULA) and Terms of Service when you use it.

All content not authored by the P4KX contributors or its users is the property of its
respective owners. Star Citizen®, Roberts Space Industries® and Cloud Imperium® are registered
trademarks of Cloud Imperium Rights LLC.

Copyright (c) 2026 the P4KX contributors

## What P4KX does

- **Archive.** Reads the Archive layouts v1 (ZIP64) and v2, with stored, deflate and Zstandard
  Entries and encrypted Entries (with [your own key](#first-run-the-archive-key)). Finds the
  installed Channels (`LIVE`, `PTU`, `EPTU`, `HOTFIX`, `TECH-PREVIEW`) and their Build. An Archive
  is only ever read, and an updated Archive is noticed while it is open. `--recover` rebuilds one
  whose directory is damaged.
- **Search and verification.** Filters Entries by path with globs and leaves out others with a
  repeatable `--exclude`, finds a text in the Records, the localisation and the text Entries
  (`grep`), and checks every Entry against the CRC-32C of the Archive directory (`verify`).
- **Extraction.** Writes Entries in parallel into a new `SC-EXPORT-<Channel>-<Build>-<time>`
  folder, with conflict policies, cancellation and path-traversal protection, and only with file
  names valid on both Linux and Windows. `--convert` turns CryXml into XML, the DataForge into one
  XML file per Record, textures into PNG, DDS or OpenEXR and sounds into Ogg or WAV on the way;
  `p4kx convert` does the same for a folder extracted earlier.
- **DataForge and Records.** Reads DataForge versions 6 and 8, from an Archive or from a
  `Game2.dcb` file on disk, as a canonical Record tree, exports it as Record XML, Record JSON,
  NDJSON or flat CSV, and answers Queries, References, tag trees and field statistics over it.
- **Comparison.** Compares two Archives, for example an older and a newer Build: Entries by size
  and checksum, Records field by field, the DataForge schema, and the line diff of one Entry.
- **Textures and localisation.** Merges split DDS textures, decodes BC1 to BC7 and the
  uncompressed formats to PNG (high-range textures to OpenEXR), and reads the `global.ini` of every language.
- **Audio.** Converts Wwise WEM sounds and soundbanks to Ogg or WAV without re-encoding where the
  codec allows, resolves an audio trigger through its events and containers to the sounds, prints
  the object hierarchy of a soundbank and writes vgmstream `.txtp` files for resolved events.
- **Models, Assemblies and Interiors.** Reads CryEngine chunk files and models with materials,
  textures, skeletons and animations, and exports them as glTF (GLB). An Assembly places a ship, a
  vehicle, a weapon or a creature with its whole Loadout, palettes, landing gear, bone attachments
  and skin ports; the Loadout can be changed port by port, and another alternative of a generated
  Loadout can be picked (`--loadout-pick`). The Interior of a ship comes from its socpak
  containers, with props and lights.
- **Characters and armour.** Builds a Character from a CHF head or a story character's Loadout,
  the default body, armour and clothing, with hair, brows and beard bound to the face, face
  expressions as morph targets at every level of detail and animations; worn clothing hides what
  its Record hides. Converts CHF files to JSON and back byte for byte.
- **3D viewer.** The window draws models, Assemblies and Characters with wgpu (Direct3D 12 first on
  Windows): orbit, level of detail, material switch, skeleton and animation playback.
- **UI movies, ship screens and holograms.** Lists and draws GFx and SWF movies, renders the MFD
  screens of a ship from their Records as PNG or SVG, also powered on and page by page
  (`--var`), and draws the hologram of a vehicle.
- **Blender package.** Writes an Entity (a creature as one skinned rig), an Interior or a Character
  as a folder (`scene.json`, GLB parts, textures) that the Blender add-on imports with node-tree
  materials for layered surfaces, glass, stencil decals and liveries.
- **Basetool catalog and ship base data.** Writes the versioned JSON catalog of manufacturers,
  items, ships, commodities, blueprints, loadouts, paints, shops, the starmap, contracts with their
  templates, broker entries, spawns and hauling orders, reputation, mining, refining and loot that
  the Basetool imports, and the base data of every ship.

## The window

`p4kx-gui` has the tabs **Browse**, **Comparison**, **Query**, **Localisation**, **Tags**,
**Search**, **Entities**, **Sounds**, **Characters** and **UI**, a sidebar with the detected
Channels and an Export menu. It previews text, hex, textures, models, sounds and Records, runs
Queries, exports Records and the Basetool catalog, converts a folder of extracted files, opens a
`Game2.dcb` on its own, and fits down to a window of 640 by 400 points. On Windows its 3D view
opens Direct3D 12 first and falls back to the default backends; `WGPU_BACKEND` overrides that.
The look is P4KX's own dark theme, and the language is German or British English. See
[docs/user-help.md](docs/user-help.md).

## The command line

`p4kx <command> --help` documents every command and option; [docs/commands.md](docs/commands.md)
describes them in full, the JSON shapes are in [docs/cli-json.md](docs/cli-json.md), and the exit
codes are in `p4kx --help`. Every command takes the path of a `Data.p4k` as `<archive>`.

| Area | Commands |
|---|---|
| Archive | `list`, `search`, `grep`, `info`, `verify`, `extract`, `convert`, `channels`, `socpak`, `key` |
| DataForge | `dcb info`, `dcb export`, `dcb query`, `dcb refs`, `dcb tags`, `dcb fields`, `prefab` |
| Exports | `catalog`, `ships`, `actionmap export`, `loc`, `coverage` |
| Comparison | `compare` (`--entries`, `--records`, `--schema`, `--diff`) |
| Formats | `cryxml`, `texture`, `audio`, `bnk`, `chunks`, `model`, `ui assets\|extract\|render` |
| 3D and Blender | `entity loadout\|glb\|package`, `interior glb\|package`, `armour`, `character package`, `chf`, `ui hologram\|screens` |
| Audio resolution | `wwise resolve\|events\|extract`, `entity audio` |

## MCP server

`p4kx-mcp` is a read-only [Model Context Protocol](https://modelcontextprotocol.io) server over
stdio with 24 tools, so an AI client can look into the Archives: entries, Records, Queries, chunk
files, models, textures, sounds, soundbank objects and Loadouts. It writes no file. See
[docs/commands.md](docs/commands.md#mcp-server) for the tools and how to add it to Claude Code or
Claude Desktop.

## Blender add-on

The add-on `p4kx_importer` imports the packages written by `entity package`,
`interior package` and `character package` (package format 1.8) into Blender 5.2 LTS (File >
Import > P4KX Package (scene.json)). See [blender_addon/README.md](blender_addon/README.md) and
[docs/blender-package.md](docs/blender-package.md).

## Install

Windows on 64-bit x86 (x86_64). Each [GitHub Release](https://github.com/p4kx-org/p4kx/releases)
offers:

- `P4KX-<version>-x86_64.msi`: installs for the current user, without administrator rights, into
  `%LOCALAPPDATA%\Programs\P4KX`, adds the Start menu entry and puts the folder on your `PATH`.
- `P4KX-<version>-x86_64-portable.zip`: `p4kx-gui.exe`, `p4kx.exe` and `p4kx-mcp.exe` to run from
  any folder.
- `p4kx_importer-<version>.zip`: the Blender add-on (Edit > Preferences > Get Extensions >
  Install from Disk).
- `SHA256SUMS.txt` and `SHA3-512SUMS.txt`: the checksums of the other files, to check a download
  (see [CONTRIBUTING.md](CONTRIBUTING.md)).

Settings live in `%APPDATA%\P4KX`. The window can check GitHub once at start for a newer Release
if you switch that on in the Help menu; it is off by default, and nothing else touches the network.

## First run: the Archive key

Start `p4kx-gui` and pick a Channel in the sidebar, drop a `Data.p4k` on the window, or use
**Archive > Open Archive…**. Browsing, searching and comparing work at once. Some Entries are
encrypted; P4KX does not contain the key, so you enter it yourself the first time one is needed:
the window asks for 32 hexadecimal digits and stores them in your profile after checking them. On
the command line:

```powershell
p4kx key set
```

reads the key from the first line of stdin, or use `--key <HEX>` or the environment variable
`P4KX_ARCHIVE_KEY` for one run. `p4kx key show` and `p4kx key remove` manage the stored key.

## Quick start

```powershell
$archive = 'C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Data.p4k'
$older = 'C:\Program Files\Roberts Space Industries\StarCitizen\PTU\Data.p4k'

p4kx channels
p4kx info $archive
p4kx search $archive '*.mtl'
p4kx extract $archive --filter 'Data\Libs\Config\*' --convert --out .\export
p4kx dcb query $archive 'from EntityClassDefinition at "*/spaceships/*"'
p4kx entity glb $archive AEGS_Avenger_Titan --materials textures --out .\titan.glb
p4kx entity package $archive AEGS_Avenger_Titan --out .\titan-package
p4kx compare $older $archive --entries --records --report .\changes.json
```

P4KX only reads the game folders and refuses to write into a Star Citizen installation.

## Documentation

| Topic | Document |
|---|---|
| Using the window | [docs/user-help.md](docs/user-help.md) |
| Commands and the MCP server | [docs/commands.md](docs/commands.md), [docs/cli-json.md](docs/cli-json.md) |
| Queries | [docs/query.md](docs/query.md) |
| Record formats | [docs/record-xml.md](docs/record-xml.md), [docs/record-json.md](docs/record-json.md), [docs/record-flat.md](docs/record-flat.md) |
| Basetool catalog and ship base data | [docs/basetool-catalog.md](docs/basetool-catalog.md), [docs/ship-base-data.md](docs/ship-base-data.md) |
| Blender package | [docs/blender-package.md](docs/blender-package.md) |
| Plan and decisions | [docs/ROADMAP.md](docs/ROADMAP.md), [docs/adr/](docs/adr/) |
| The terms used | [CONTEXT.md](CONTEXT.md) |
| Release notes | [CHANGELOG.md](CHANGELOG.md) |

## Build

Requires the stable Rust toolchain (1.99 or newer).

```powershell
cargo build --release
.\target\release\p4kx.exe list "C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Data.p4k"
```

The release build produces `p4kx-gui.exe`, `p4kx.exe` and `p4kx-mcp.exe`. The code is platform
neutral and CI also builds it on Linux, but only Windows is shipped.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) first: the checks, the tests, the real-Archive suite, the
Contributor License Agreement and the sign-off of every commit. Security issues are reported as
described in [.github/SECURITY.md](.github/SECURITY.md). Everyone taking part is bound by the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Third-party licences

The licence texts of the Rust crates P4KX uses are in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), which ships with every Release. The Vorbis codebook
libraries embedded in `p4kx-audio` come from [ww2ogg](https://github.com/hcs64/ww2ogg), and the
Opus decoder [opus-pure](https://github.com/stephenberry/opus-pure) is a port of libopus; both
are licensed under the BSD 3-Clause licence.
