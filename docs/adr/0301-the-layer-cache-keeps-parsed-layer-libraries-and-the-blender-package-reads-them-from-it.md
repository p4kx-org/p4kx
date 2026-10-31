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
the surface loads and the Blender package fetch and parse a file once between them.
`LayerCache::library` returns the status to any caller. The byte limit of ADR-0291 still counts the
bytes of the files; the parsed libraries held are of the same order, which bounds the memory as
before. The Submtl pick, the per-load budget and the cache limits are unchanged.

**One reader for the package.** `ModelProvider::layer_library(path, budget)` returns that status;
`ArchiveModels` answers it from its cache, the wrappers forward it, and the default reads the file
as a library, so a provider without a shared cache keeps working. The package charges its own
`LayerBudget` (the default per-load limits), so a ship that names more layer files than a load may
read reports the rest as `layer_material` issues instead of reading them without bound.

## Consequences

The package no longer reads a layer file as a library of its own, and a layer file is parsed once
per provider whether a library load or the package asks first. The `library` field of a layer
description is spelled from the normalised request path, in the case of the first reader of the
file.
