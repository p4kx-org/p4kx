### Fixed

- Catalog v3 keeps original Starmap and Loot container references with `notFound`, provenance, and
  a warning when the archive directory proves that the selected target is absent. Harvestable
  locations retain the warning when they depend on the same Starmap lookup. Incomplete lookups
  and read failures block publication.
