# Declared resource-network topology

Catalog schema v3 exports the resource-network declarations owned by each
selected physical Entity Record. Each row includes the original owner GUID,
Record index and Components ordinal. The arrays preserve their source states
and authored slot order, including repeated, self-referential and cyclic links.

| Catalog table | Original declaration |
|---|---|
| `declarations.resourceNetworkLinks` | `SItemPortContainerComponentParams.InternalHardpointLinks` holds pairs of stored port names. `InternalResourceLinks` holds pairs of weak pointers to `SItemPortDef`. |
| `declarations.roomAssociations` | `VehicleComponentParams.objectContainers` holds the stored `resourceNetworkItemportName`, `boneName`, container path and identifier, and NavMesh flag. Additional members include the original offset when present. |

The link row also carries the original `LinkAllItemsToResourceNetwork` flag and
the stored resource-network power-pool setup. Weak links retain their admitted
target type, Record and original owned-instance location when available.
Endpoint names and pointer identities describe the authored declaration.
The two endpoint positions retain their ordering in the source pair.

Each stored value retains its source state, exact contributors and encryption
marker. A room association describes the authored attachment context.

The Equipment SDK can read the three placement tables for one original physical
Ship Record. It returns the scoped Loadout, typed dependency coverage and shared
source inventory. See [resource topology placements](catalog-sdk.md#resource-topology-placements).

## Installed loadout contexts

A `placements` position uses `loadoutRecordIndex` to match
`loadouts[].sourceRecordIndex.value` in the same catalog. Apply the ordered
`portOrdinals` to the root's `ports` array and each selected port's child
`ports` array. An empty path denotes the ship root. A binding records the
declaring context path. An endpoint candidate's `loadoutPortOrdinal` selects a
direct port in that context.

- `resourceNetworkNodes` contains authored endpoint placements and installed
  resource components, including disconnected and invisible placements. A
  resolved attachment carries its physical target identity. The
  `resourceComponents` array joins the canonical resource configuration
  rows and carries each component's stored `isRelay` flag.
- `resourceNetworkLinkBindings` joins each contextual edge to the original
  link family, declaring component and slot. Named endpoints are compared in
  their declaring Entity's direct ports with the Loadout resolver's ASCII
  case matching. Every physical port definition remains a candidate when
  the loadout contains a single selected occurrence.
- `roomAssociationBindings` joins each contextual room association to its
  original container slot and the same direct port inventory.

Each endpoint is a sourced value. Its `value.state` is `resolved`, `ambiguous`,
`unplaced`, `missing`, `unavailable`, `unspecified` or `outsideDeclaringContext`.
Named bindings carry the original selector and actual same-context name
contributors. Those names come from the selected DataForge field or XML
attribute, including applied patches and Elem overrides. Incomplete inspections
retain the known candidates and an incomplete contributor set. Weak links use
their admitted Record, owned-instance steps and exact matching placement.

Docked vehicles have a separate root loadout. An installed node's
`vehicleBoundary` retains the selected attachment and original type evidence.
Link rows preserve source endpoint order. The placement graph visits the
loadout tree once.

The canonical `configurations.itemPorts` rows carry stored port type and
subtype restrictions, tags, flags, size limits, port rules and
`resourceLinkToParent`. The `configurations.itemAttachmentConstraints`
rows carry the installed item's original type, subtype, size and tags. Unexpected
readable restriction storage is retained in the original source arena.
`fuseEvidence` is `unestablished` until an original fuse discriminator
can be verified from source evidence.
## Installed SDK verification

The installed witness compares the direct placements of two independently
selected ship Records with the same Build's scoped catalog. It also verifies
the reached physical declarations, loadout issue codes, attachment and relay
identities, Schema v3 row shapes, and the cited source inventory. Both Builds
produce 60 nodes, 52 link bindings and four room bindings for one sample, and
75 nodes, 70 link bindings and two room bindings for the other. The original
loadout for the latter reports an unresolved class selection. The SDK retains
that issue alongside the verified placement records.

Each cold process opens the Archive and DataForge and runs one physical-root
SDK query on a Windows AMD64 host with one worker.

| Build | Sample | Private commit peak | Working set peak | Elapsed |
| --- | --- | ---: | ---: | ---: |
| LIVE 13220821 | 1 | 1,091,858,432 B | 1,024,737,280 B | 14.024 s |
| LIVE 13220821 | 2 | 1,091,833,856 B | 1,024,708,608 B | 12.668 s |
| PTU 13248297 | 1 | 1,116,155,904 B | 1,034,899,456 B | 14.044 s |
| PTU 13248297 | 2 | 1,116,135,424 B | 1,034,899,456 B | 13.529 s |

All four samples meet the 2,000,000,000 B process criterion. The reported
process I/O was 299 B written per run, including the test output stream.
