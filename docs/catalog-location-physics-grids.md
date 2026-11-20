# Location physics grids

Catalog schema v3 exposes `placements.locationPhysicsGrids` for authored
location physics found in the canonical system descriptor walk. Each row
contains the original container and inner descriptor Entry identities. A
placement joins to that row through `placements.containerInstances.rows[].container.value`.
First select the exact `instanceId`, including `rootEntryIndex`,
`childNodeIndices` and `externalChildStepIndices`, then join the physical
container reference. Repeated placements share the authored descriptor row.

The radius and typed declarations carry their original source states and
per-value encryption markers. Gravity contains the three authored components
in source order. Declarations retain the original parent and
sibling position. A `childParts` row remains present when its group is
empty. The `sourceElements` arena retains the original declarations and
unknown members inside the grid and part roots in source order. Join a typed
declaration to a raw element by its original position.

Voxel targets use the descriptor's own physical container first, followed
by the root Archive directory. Only directory evidence is read. A repeated
path in the first matching tier reports every physical candidate. Missing
and invalid selectors have separate states. The result describes the
physical descriptor's lookup scope. A caller resolving a custom hierarchy
of nested archives can use the Assembly reader with its own ordered
container indices.

Availability describes the selected descriptor scope. `available` means
at least one original grid declaration was observed. `absent` means the
complete scope had no matching declaration. `unavailable` means an empty
result comes from an incompletely observed scope. Original descriptor
read and projection failures contribute to Starmap export health and SDK
coverage. Read that coverage alongside a positive table to establish its
completeness. Target availability is tracked per voxel. CargoGrid
relationships, vehicle physics proxies and room volumes have separate
source models.

`catalog_queries::starmap::read_container_placements` returns the same
authored descriptor rows for the selected occurrence routes and their
coverage, without generating a Catalog. For a descriptor outside that
walk, use `read_location_physics_descriptor` with its original outer
container and inner descriptor Entry indices in the same Build. A mismatch
with the admitted main descriptor is `selectorMismatch`. The
`read` status and a sourced `present: false` prove absence within that
exact readable descriptor.
