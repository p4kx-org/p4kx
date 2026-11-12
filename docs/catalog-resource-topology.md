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

## Installed loadout contexts

The `placements` tables refer to the same `loadouts` array emitted by the
catalog. A position uses a zero-based `loadoutOrdinal` and ordered
`portOrdinals` into its nested `ports` arrays. An empty port route denotes
the ship root. A binding records the declaring context route, and an endpoint
candidate's `loadoutPortOrdinal` selects a direct port in that context.

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