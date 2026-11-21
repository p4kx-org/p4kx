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
  parse. Discovery continues until no new physical source remains. The placement walk uses
  an explicit stack to compose positions in authored depth-first order. A physical container
  can contribute under multiple positions. Only a link to an active ancestor is a cycle.
- **Complete by default, with optional caller allowances.** `StarmapTraversalBudget` defaults
  to no Child-count quota. `source_children` optionally bounds declarations across uniquely parsed
  physical descriptors; `placed_children_per_system` separately bounds instantiated Child visits
  for each root, including reused containers. A zero allowance accepts an empty source or walk.
  Exhaustion retains readable rows, marks the affected source inventory or placement routes
  incomplete and refuses normal publication before output creation. Diagnostic partial publication
  remains explicit. Parallel parse order can determine which descriptor reaches an exhausted
  shared source allowance; no budgeted partial result claims complete selection.
  The source tree is read, visited and freed iteratively, while the placement walk detects cycles
  on its active physical ancestry. Cooperative cancellation, format validation and coordinate
  overflow checks remain in force. Raw XML and its DOM live only while one descriptor is parsed.
- **A container whose XML is not named like it is reported.** When no `<stem>.xml` exists and the
  first eligible XML entry is taken, the report names it. Any directory segment
  named `hcd` marks a companion and excludes it from both main XML selection and fallback.
  A container with
  no eligible main XML reports an unreadable placement body.
- **Resolve authored external Child paths with the shared grammar.** The external flag
  accepts `1` and `true`. Existing exact root Archive matches take priority. For
  other declarations the shared lookup tries the written path, optional `.socpak`
  extension, and root Archive prefixes used by interior assembly.
- **Require a unique physical Entry at the selected path.** Ordinary Starmap and Loot routes
  retain the authored selector and every candidate when the winning path is ambiguous.
  Starmap applies the same check to system roots and to the chosen descriptor inside a container.
  Candidate identities are retained in the health report without opening their payloads.
- **Read every direct `ChildObjectContainers` list in document order.** Each `Child` keeps
  its actual owning list. The legacy wrapped-list fallback applies when the element has
  no direct list.
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
including each reused file's separate Child route. The instance key holds the system root Archive
Entry ordinal, the ordered XML Child node ordinals and the positions where the walk entered an
external descriptor. The parent key identifies the enclosing physical container occurrence.
[ADR-0414](0414-container-occurrence-keys-record-external-child-steps.md) defines the exact key. An inline XML Child can
lie between the enclosing physical container and the declaring Child.

Local positions are metres in the immediate XML parent's frame. Local rotations
are the authored quaternion components in w, x, y, z order. Missing authored
components are absent and the world transform uses zero translation or identity
rotation for those slots. World positions are in the system frame at millimetre
precision. World rotations use the composed unit quaternions in w, x, y, z order.
The root frame is the identity. Unread, malformed and unreachable placements
remain visible through the traversal diagnostics. Table availability describes
the discovered source family independently of traversal completeness.

Each external occurrence preserves the declaring Child's `label` and `entityName`
as `declaredLabel` and `declaredEntityName`. Empty attributes remain present.
The label retains its literal text or localization key and carries `x-gameText`.
Each name cites its own original attribute or the observed absence at that Child.

## Unresolved declared container occurrences

The separate additive `placements.unresolvedContainerLinks` table retains each reached
external Child whose normal path lookup has zero or multiple winning physical candidates, or
one physical target whose body is unreadable or unparsed.
A row uses the system root and original Child route for its occurrence identity and the
enclosing physical container occurrence for its parent. Its immutable declaration retains the
exact descriptor input, Child node position, raw authored path, independent local pose fields,
original Starmap Record GUID and every original attribute occurrence with per-value evidence.

Typed target states distinguish an exhausted supported lookup, ambiguous candidates,
unsupported authored syntax and uncertain root directory membership. A unique target keeps
Directory status `resolved`; `targetReadStatus` separately records the actual cached body outcome
as `unreadable` or `unparsed`. A nonunique lookup is `notAttempted`. A successfully parsed empty
target remains an ordinary resolved ContainerInstance. Candidate-set cells cite
the original consulted root directory path range and the authored selector. One directory
survey is shared by the source walk. The survey establishes the admitted path domain without
reading target payloads or using a basename as a source join. Recovered membership keeps
candidate-set completeness false.

The source walk captures sparse missing/ambiguous declarations while their XML is borrowed.
For unique targets the same original parse seals private raw attribute packets without
materializing SourceUses. After the next breadth-first frontier classifies each target, failed
bodies retain those original packets; successful bodies release them without orphan uses.
The packet is bound to the actual Entry receipt and cannot be restored from equal DTOs or paths.
Raw XML and its DOM are still dropped after that one parse; no second Archive read or parse is
introduced. Temporary raw-attribute retention lasts until target classification and requires
separate resource measurement. The Harvest descriptor scope captures no packets and runs no
additional release traversal.

A shared declaration is reused for every reached route. The production row writer uses its own
staged stream with the same cancellation and cleanup behavior as the existing placement tables.
Resolved occurrence membership and intrinsic read failures retain their current meanings.

A retained unresolved occurrence provides source evidence. Configured dependency failures,
unknown descendant inventory and incomplete derived projections still affect export health.
Normal publication continues to require complete export health. Consumer removal decisions
continue to require the intended Build, complete selection and complete overall health.

## Authored location context

The typed location tree is an owned placement table. Its occurrence key is the
system Entry and original Child route, and its parent is the nearest typed Child
on that route. Its component declarations are separate from location templates.

A typed Child can select a Hierarchical Context Data companion through
`hasHierarchicalContextData` and its own `guid`. Search the opened container's
directory for the exact GUID basename below an `hcd` segment. The same physical
container can list zero, one or multiple candidates. Read each unique selected
companion once. Keep per-occurrence selectors and the companion's own XML proof
separate so reused Children still have the correct source references. The
companion's `preset` is an authored Context Preset GUID. Its recorded name and
the location's component names do not establish a Harvestable Provider Preset
binding.

## Authored entity XML and harvestable setup

A typed location can own an EntXML entity in the same physical container. Match
the original Child GUID to the verified root Entity GUID in the eligible member
family. The central directory supplies member positions, while the decoded XML
supplies entity identity. Preserve each physical candidate and original element
occurrence. Require a complete scan to select a unique candidate.

The direct HarvestableComponent children of the EntXML root carry an authored
SubHarvestable Multi Configuration GUID. Join that GUID to its independently
owned DataForge definition. AreaSphere Entity references remain independent.
The in-game placement or selection of a particular resource requires actual
source evidence beyond this stored configuration.

The installed PTU survey verified all 16,948 members in one selected planet
container. It established 16,947 exact Child/Entity joins, including all 1,791
typed locations. The 1,211 locations with an authored Harvestable metadata marker
each retained two root-owned HarvestableComponent occurrences. The independent
DataForge query admitted the referenced SubHarvestableMultiConfigRecord. A separate
named entity document supplied the planet's authored entity and procedural controls.

The selected HCD preset, the mission location template family and the changed
ecosystem descriptors were also inspected for an explicit Harvestable Provider
Preset association. No direct provider placement or component-to-AreaSphere
mapping was found in those scoped sources. See the catalog's existing SOC-based
provider placement rules for separately authored relationships.
