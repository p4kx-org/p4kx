# The layer cache keeps parsed layer libraries and the Blender package reads them from it

Decided for issue #959, the follow-up of #561 that ADR-0291 left open. It amends ADR-0291
("Surfaces, not libraries") and keeps ADR-0100 (the add-on builds the node trees from the package
descriptions).

## Decision

**The manifest keeps the whole layer material.** The add-on builds the node tree of a layer from
its description like that of any material: shader, flags, public parameters, texture slots with
their roles, and the layer's own layers (`material_layers.py`, `context.usable`). A `LayerSurface`
(diffuse, specular, texture slots) holds none of the rest, and shrinking the manifest to it would
change the package format and the add-on. The format stays as it is: no version bump.

**The cache keeps libraries, not only surfaces.** `LayerCache` stores, per layer file, the parsed
`MaterialStatus` (the library, or why there is none), read without the file's own layers and
diffuse maps. `LayerCache::surface` takes the `LayerSurface` of the picked sub-material from it, so
the surface loads and the Blender package fetch and parse a file once between them, per key until
the cache starts over. `LayerCache::library` returns the status to any caller. The Submtl pick and
the per-load budget (counted in file bytes) are unchanged.

**The cache is bounded by the estimated size of what it holds.** A parsed library takes more
memory than its file. `MaterialLibrary::estimated_bytes` counts struct sizes and text, and
`LayerLimits::cached_bytes` (64 MiB by default) limits that estimate, not the file bytes. Measured
on the ship material libraries of LIVE (`real_layer_cache_size.rs`): 918 distinct layer files of
1.2 MB hold an estimated 3.1 MB parsed, a factor of 2.6; the largest library names 50 of them.
The test fails above a factor of 8.

**One spelling.** The library path of a layer file is its lower-cased request, whichever spelling
asked first, so the manifest's `library` field of a layer does not depend on the order of the
reads. The add-on does not read that field.

**One reader for the package.** `ModelProvider::layer_library(path, budget)` returns the status;
`ArchiveModels` answers it from its cache, the wrappers forward it, and the default, for a provider
without a shared cache, reads the file as a library and applies no budget. The provider also
supplies the budget (`ModelProvider::layer_budget`, the limits of its cache). The package holds one
budget, so a ship that names more layer files than a load may read reports the rest as
`layer_material` issues ("the layer-file budget is spent") instead of reading them without bound.
On LIVE no ship of `real_blender_packages` reaches it.

## Consequences

The package no longer reads a layer file as a library of its own, and a layer file is parsed once
per provider whether a library load or the package asks first.
