# The starmap containers are parsed once and composed in memory

Decided for issue #455. Follows up ADR-0002 and the starmap kind of #450.

The first reading of the system containers walked each star system depth first, opening,
decompressing and parsing every external container where a Child named it. A container instanced
several times (the Nyx ring) was parsed again for each reference, and the raw XML of every
container on the chain (up to 128 MiB each) was held at once. Only the systems ran in parallel.

## Decision

- **Parse once, then compose.** `starmap_parse` reads the reachable containers level by level
  from the system containers: the containers of one level are read and parsed in parallel
  (rayon), each exactly once for the whole Archive, into a tree of Children (pose, record,
  entity name of a placed Child, external path, nested Children). The raw XML is dropped after the
  parse. `starmap_walk` then composes the positions per system, in parallel, over those trees,
  in the same document order, with the same limits and counts as before. The levels are bounded
  by the external chain limit (32); a cycle ends because a container is read once.
- **Memory bound.** The parse keeps at most `Limits::children` (4,194,304) Children in all
  containers together, counted by one shared counter, and nests lists at most `Limits::depth`
  (64) deep. A list cut by the counter is marked and reported as a skipped Child; which
  container is cut when the bound is hit depends on the thread order, which only matters for an
  Archive that is already reported as over the limit. The raw XML and its DOM are alive only
  while one container is parsed, so the peak is the kept Children plus, per rayon thread, one
  XML of up to 128 MiB and its DOM.
- **A container whose XML is not named like it is reported.** When no `<stem>.xml` exists and the
  first other XML entry is taken, the report gets an issue line naming it. It is read, not counted
  as unreadable, because the position data is intact.
- **Resolve authored external Child paths with the shared grammar.** The external flag
  accepts `1` and `true`. Existing exact root Archive matches take priority. For
  other declarations the shared lookup tries the written path, optional `.socpak`
  extension, and root Archive prefixes used by interior assembly.
- **A second `ChildObjectContainers` is reported.** Only the first list of an element is read;
  when its direct siblings hold another one, an issue line says so. The positions are unchanged.
- **Type names are read once.** `TypeNames` caches the name of each type Record by GUID for the
  parallel scan.
- **`StarmapReport` stays public, read-only.** Its lines are diagnostics for the real-Archive
  verification and for a person checking the placements; the fields are crate-private and
  `CatalogSummary::starmap_report()` hands out accessor methods. The wording of the lines is
  documented as not being a contract. A feature flag was not worth the second build
  configuration.

## Consequences

- The catalog JSON and its schema do not change.
- An out-of-range position is reported with the composed number and the entity name of a placed
  Child, no longer with the raw attribute text, which is not kept.

## Catalog placement evidence

The same walk retains the selected root directory and the parsed XML node and attribute
coordinates used by each placement. A reused container shares its parsed source sites.
Each placement retains its own ancestor route so the same Child can supply different
world positions in different instances.

The route selects the occurrence for all placement fields. Position uses the leaf
translation and ancestor translations and rotations. Entity name and parent GUID
retain their respective original attribute uses. The two positions that establish an
ambiguous placement supply the absence evidence. Referenced or attempted targets
stay in the operation ledger unless their contents contribute to a placement.

## Container instance table

`placements.containerInstances` contains one row per resolved SOCpak occurrence,
including each reused file's separate Child route. The instance key is the system
root Archive entry ordinal and the ordered XML Child node ordinals. The parent key
identifies the enclosing physical container occurrence. An inline XML Child can
lie between the enclosing physical container and the declaring Child.

Local positions are metres in the immediate XML parent's frame. Local rotations
are the authored quaternion components in w, x, y, z order. Missing authored
components are absent and the world transform uses zero translation or identity
rotation for those slots. World positions are in the system frame at millimetre
precision. World rotations use the composed unit quaternions in w, x, y, z order.
The root frame is the identity. Unread, malformed and unreachable placements
remain visible through the traversal diagnostics. Table availability describes
the discovered source family independently of traversal completeness.
