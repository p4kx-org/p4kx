# The catalog ships carry the ship base values as flat nullable fields

Decided for issue #395. Builds on ADR-0054 and ADR-0061.

Schema v3 replaces the flat cargo fields with the [sourced projection](0416-ship-cargo-projection-retains-physical-mount-evidence.md).

Catalog consumers need the mass, hull points, cargo capacity, size and kind of a ship from the Archive.
ADR-0061 reads them for `p4kx ships` with a source per value; this decision puts them into the
`ships` kind of the catalog.

## Decision

- **Flat fields on the ship, additive in version 1 (ADR-0054):** `mass` (kg), `massBasis`,
  `hullHealth`, `cargoScu`, `cargoGridsWithIssue`, `size` and `vehicleKind`. They are facts, so
  none carries `x-gameText`. The per-value sources, the destruction and detach thresholds, the
  cells and the Loadout stay in `p4kx ships`: the catalog is the import's table, the ship base
  export is the audit trail.
- **Unknown is `null`, never 0.** `mass`, `hullHealth` and `size` are `null` when the Archive
  states none. `cargoScu` is 0 for a ship without a cargo grid and `null` when it has grids and
  none is readable (the Javelin and the Hornet F7C on LIVE: their containers are dangling
  References). When some grids are readable and others are not, `cargoScu` is the sum of the
  readable ones and `cargoGridsWithIssue` is above 0, which makes the number a lower bound.
- **Closed sets are strings.** `vehicleKind` (`spaceship`, `groundVehicle`) and `massBasis`
  (`parts`, `physics`) are strings in the schema, so a new value is additive and a reader maps an
  unknown one to `null`.
- **Vehicle kind from evidence, not only from the path.** A Record under `entities/groundvehicles`
  is a `groundVehicle`. A Record under `entities/spaceships` is a `spaceship` only when its
  vehicle implementation has no wheel (`SubPartWheel`) and no tread (`Tread`) part; with one the
  folder and the implementation disagree and the kind is `null` rather than guessed.
- **One walk.** The catalog scan reads a ship's base data from the Record tree it has already built
  (`ShipReader::read_tree`). The ship base walk is not run a second time, and Entities and
  containers that ships share are read once per reader.
- **A pool of readers, not one per thread.** The scan keeps the `ShipReader`s in a mutex-guarded
  pool and a job takes one for the duration of a ship. `LoadoutResolver::prefetch` runs its own
  rayon `par_iter`, so while a worker waits there, rayon can steal another ship's job onto that
  thread; a per-thread `RefCell` reader would be entered twice. The pool hands the stolen job a
  separate reader.
- **Docked vehicles** are not entered (ADR-0061), so they are in neither `mass`, `hullHealth`
  nor `cargoScu`.

## Considered Options

- Nesting the values as `{value, source}` objects as in `p4kx ships`: rejected, since it would
  multiply the ships' size by sources the import does not read; the flat form keeps the table
  shape of the other kinds.
- Taking the kind from the record path only: rejected by the issue; the implementation is
  evidence, and a disagreement is shown as `null`.
- Running `build_ship_base` after the catalog scan: rejected, it would read every vehicle
  implementation and Loadout again.
