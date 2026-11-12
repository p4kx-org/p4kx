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