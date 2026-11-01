# The Assembly places parts at port helpers and chooses their material libraries

Decided for issue #124. Evidence: `p4kx entity glb` over AEGS_Gladius, AEGS_Avenger_Titan,
ANVL_Hornet_F7C, RSI_Constellation_Andromeda and TMBL_Cyclone of LIVE
(`crates/p4kx-assembly/tests/real_archive/real_assembly.rs`), where 96% to 99% of the parts found their
helper or a node named like their port and 0 to 5 parts were skipped.

Amended by ADR-0321 for the rotation convention and by ADR-0150: it names ADR-0028 for the level-of-detail fallback, reads character
definitions, and corrects the sentences on equal materials, the Loadout's libraries, the waves and
the `.cdf` limit below.

## Layers

`p4kx-assembly` builds the Assembly and depends on `p4kx-extraction` (the Archive and model
readers) and `p4kx-gltf`. `p4kx-gltf` does not know the Assembly: it writes a `Scene`, a tree of
named nodes with a transform, each optionally holding a model and the material library to draw it
with. The Assembly turns its parts into such a scene. The name is glTF's; the glossary's
**Assembly** stays the Entity with its parts. Scenes are what #126 (shared meshes and textures)
and #127 (textures, palettes) extend; the writer already writes equal materials once, comparing
the glTF material including its name, so two libraries with the same colours cost one material
only when their materials carry the same name.

## Placement

A part sits in the space of its parent part. Its transform is the world transform of the node of
the parent's model that the port's helper names (`SItemPortDef` helper, matched ignoring case),
followed by the port's offset: the position in metres and the rotation in degrees, applied about X
first, then Y, then Z. Without a helper, or when the parent's model lacks it, the node named like
the port is used (the vehicle XML ports of #289 have no definition, and the names of their parts
are the node names); failing that the part sits at the parent's origin, and a helper that was
named but not found is an issue. An offset helper in the part's own model is undone.

ADR-0321 validates the Ang3 convention above against the ports of the LIVE ships and pins it with
multi-axis tests; a real ship whose weapons sit wrongly is the signal to revisit it, in
`placement.rs` only.

The glTF node of a part is a child of the node of its parent part, not of the helper node inside
the parent's model: the helper's transform is folded into the part's own, so that the hierarchy
is the Loadout tree (`<port>/<entity>` below the parent) and a part can be hidden or replaced
(#128) without touching the parent's model.

## Material libraries

The Entity's geometry (`SGeometryResourceParams`) names the model and, usually, a material
library. That library **replaces** the one the model's `MtlName` chunk names: 46 of 150 sampled
skins find no library of their own. A named library that is missing is an issue and the part is
drawn with the default material; the model's own library does not take over. Models are read
without their library (`Model::read_without_materials`, `MaterialStatus::Skipped`), and each
distinct library is read once per Assembly, a bare name relative to the model's folder, and shared
by every part that uses it. A `Part` holds its own `MaterialStatus`. A Loadout entry has no field
that names a library (ADR-0150).

## Limits and failures

Everything comes from the Archive and is untrusted. An Assembly holds at most 16,384 parts, at most
2^25 vertices counted per part, and is built no deeper than 2 × 32 + 2 levels; the Loadout's own
32-level and 65,536-port limits apply before. Models are read in file order in waves of distinct
files in parallel, and no further wave is started once the models read so far hold the vertex
budget (ADR-0150); the parts are then taken in order, so the same Archive always gives the same
Assembly. A part over the
limit is left without a model and listed. An Archive key that is missing or wrong is not a part's
problem: it stops the build (exit code 7) instead of listing every part as unreadable.

A model that lacks the requested level of detail is read at the nearest lower level, and the
export counts those models. This is an exception to ADR-0028, which rejects a fallback from a
missing level; a model with no level at or below the request is read at its lowest (ADR-0150).

## Not decided here

Docked vehicles (#305), the vehicle implementation XML (#289) and exclusions and replacements
(#128) are not read or applied. Character definitions (`.cdf`) are read as ADR-0150 says.
