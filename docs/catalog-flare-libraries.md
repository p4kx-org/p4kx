# Lens flare libraries

`definitions.flareLibraries` lists the physical XML or CryXml libraries in
`Data/Libs/Flares/` with the unqualified root `LensFlareLibrary`. Each library
has its physical Entry identity and a preorder list of elements. A node's
`authored.parent` is the zero-based index of its parent in that list. Root
nodes have an explicit null parent.

The authored node retains its original qualified name, physical position,
attribute and namespace declaration order, and direct text. Values that differ
from their XML lexical spelling retain that spelling too. Repeated item
identities each retain a distinct physical occurrence. Unrecognised elements
and declarations are available in the same authored tree.

## Typed fields

A node's sourced `role` distinguishes the library, its direct items, nested
components, their `Params` containers, parameter groups, and other elements.
Each supported attribute has a `fields` entry with its exact name, sourced
typed value, and optional texture link. The typed states distinguish absence,
invalid syntax, and an explicit empty value. Finite scalar, integer, pair and
triple values retain their numerical shape. The authored tree provides their
exact original text. Parameter interpretation uses the original group and
attribute names.

| Authoring group | Fields with a texture link |
| --- | --- |
| CameraOrbs | LensTexture, OrbTexture |
| ChromaticRing | GradientTexture |
| ImageSpaceShafts | GoboTex |
| IrisShafts | BaseTex, GradientTex |
| LensGhost | Texture |
| MultiGhost | Texture |
| Streaks | GradientTex |

Texture links use exact Archive-relative paths and the optional `Data/`
prefix. Supported declarations use the `textures`, `objects`, `EngineAssets`
or `Engine` directory and the `.dds` or `.tif` extension. The link gives its
directory decision and matching physical Entries. The decision's origin records
the declaration and directory facts it used. The source inventory describes
the matching Entries' stored encryption. The lookup reads directory metadata.

Use the separate `p4kx-model` SOC source API for light declarations.
Call `SocEntity::authored_lights()` to find Entity light components or
`SocEntity::authored_light_groups()` to find groups. A group's `lights()` and a
baked light's `components()` return the group-owned light components.
`SocLightSource::element()` and `SocLightMode::element()` expose each original
element, including nested reference declarations and exact source positions.
Runtime rendering depends on separate engine behaviour.

## Rust SDK

`catalog_queries::flares::read_flare_libraries` uses the same selector and reader
as the Catalog. Select all libraries, a physical root Entry index, or an exact
Archive-relative path with either separator and ASCII-insensitive matching.
The snapshot includes source-family availability, observed Entry outcomes,
the selected rows, and their original source inventory. The query reads the
selected XML Entries and directory metadata without opening DataForge.
