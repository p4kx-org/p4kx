# Layer files are read once per model set, within a byte budget

Decided for issue #561 (follow-ups of #547). It amends ADR-0097 (layered materials), whose reading
rule fetched at most 256 layer files per library and kept each parsed library for the load.

## Decision

**Surfaces, not libraries.** `MaterialStatus::load` keeps, per layer file, only the surfaces
(`LayerSurface`: diffuse, specular, texture slots) of its sub-materials; the parsed file is dropped
once they are taken. A `Submtl` is picked by one rule, `MaterialLibrary::layer_position`, which the
reader and the Blender package share.

**Bounded work.** One load is charged for at most `MAX_LAYER_FILES` (256) distinct files and stops
fetching once `MAX_LAYER_BYTES` (64 MiB) of them are reached; a layer whose file the budget does not
admit is unresolved and listed in `MaterialLibrary::unresolved_layers`, as a missing file is. A file
is charged once per load, whether the cache held it or not, so which layers resolve does not depend
on what the cache holds or on the order of the loads. The limits of the `.mtl` reader of #158 still
apply to each file.

**One cache.** A `LayerCache` holds the surfaces of the layer files read so far and is shared by
the loads of several libraries, from several threads (`MaterialStatus::load_with`). It takes `&self`:
its lock guards only the table of files, and each file is fetched and parsed outside it, once per
key, through a `OnceLock`, so concurrent loads of different files do not wait for each other.
`ArchiveModels` owns one for the Assembly, the GUI exports and the Blender package; it lives as long
as that provider, one job; it also owns the one `TextureVerdicts`, the colour verdicts of the
textures, under the same rule. The cache starts over when it holds 1,024 files or 64 MiB of layer files:
the surfaces are copied from the file bytes, so the bytes bound its memory, and the file count
bounds the table. A file skipped for the budget is never cached, so the next load tries it again.
`LayerLimits` sets all four limits, for tests.

**Only drawing resolves layers.** `MaterialReading` says how far a library is read: `Skipped` reads
nothing, `Plain` reads the library without the layer files and the diffuse maps (`p4kx model info`
and the MCP model tools), `Drawn` reads them (every drawing caller, and the default `load` and
`Model::read*`).

## Consequences

The Blender package read each layer file as a full material through the model provider (ADR-0301 replaces that),
because it describes the layer's whole material in the manifest, which a `LayerSurface` does not
hold; only the `Submtl` pick is shared. Follow-up: #959. The CLI JSON and GUI summaries of
Assemblies do not list `unresolved_layers` yet (the Assembly reports them as issues); follow-up:
#960.
