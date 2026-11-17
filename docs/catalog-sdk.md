# Selective Catalog v3 data

Use `p4kx_export::catalog_queries::read_blueprint_category_lists` to read canonical Blueprint
category lists directly from an opened Archive. Supply `BlueprintCategoryListSelection::All` to
read the domain, `Guid` to read all matching physical declarations, or `RecordIndex` to read a
known physical Record in the current DataForge. The physical index is local to the selected
input. The `_from` form reuses an already decoded `DataForgeEntry` from the same Archive.

The query opens the complete DataForge and projects the selected category-list bodies.
Original category
links carry their target header, resolution state and source uses. Their order, duplicates, null
slots and dangling links match `declarations.blueprintCategoryLists` in the complete Catalog.
The snapshot carries family availability, unreadable Record positions and a source inventory for
the returned cells. The query opens no localization or placement payloads.
