# Jump points

`configurations.jumpPointConfigurations` retains each original `SJumpPointParams` component
by Entity Record and Components slot. The operating definitions keep `requiredFuel` and
`linkingRange` as original `Single` values with exact bits. The source establishes storage
but leaves units unconfirmed. `largestShipSize` preserves an original `LandingPadSize`
Record Reference. `jumpGateSuperGuid` is stored as a string and retains its source value,
including an authored empty string. Push-area and tunnel-effect settings preserve original
numbers, ordered members and source coordinates. Additional declarations use the authored
field arena.

`configurations.globalJumpPointConfigurations` preserves `GlobalJumpPointParams` and
`GlobalJumpTunnelHostParams` Records independently. The canonical Record header identifies
the physical source. `fields` retains each declared setting, nested value and original
Reference. The point Record owns alignment, closing and pull-in timing, debris, effect and
UI cone settings. The tunnel-host Record owns acceleration, material, generation, distortion,
failure, exit, visual and effect settings. Generation probabilities link to their original
tunnel sections. These links describe section generation. Global values have their own
source scope and are kept separately from every placement.

`placements.jumpEndpoints` keeps each qualifying `Child/@starMapRecord` occurrence with
its physical parent, target container and local and world transform. A selected Starmap
type `JumpPoint` qualifies through the original Record Reference. An `Anomaly` qualifies
through a typed gate marker in its linked SOCPAK. The route identity distinguishes repeated
placements of the same Record. The editor geometry can remain unknown independently of
the endpoint's authored placement.

The `gate` row checks the original `EntityComponentJumpPoint` marker, its companion
metadata and the editor document inside the same SOCPAK. The metadata member is either at
the container root or under its exact basename directory. If both entries exist, the
metadata is ambiguous. `Entity/@CryGUID` selects an
`Object/@guid` in the editor document. The candidates retain exact authored type, name
and other stored attributes. A duplicate keeps its candidates and has no selected object.
Unreadable entries carry their own state and report the failed read separately. A readable
logical placement can have no original gate marker.

`declaredPartner` describes the inspected endpoint declaration scope. A successful read of
that grammar without a partner records absence. Missing or unreadable evidence leaves this
field incomplete. The governing evidence is the original declaration, with no pairing by
endpoint names. All source-backed fields retain their immediate origin.

The endpoint’s `targetCandidates` retains the physical directory Entry identities for its
authored target path. Multiple matching Entries make the gate invalid without electing
one. Incomplete Anomaly gate evidence keeps the occurrence visible with its state.

For each selected editor Object, its exact authored `type` can select one original
`EntityClassDefinition`. `operatingEntity` then points to its physical Record and
`operatingConfigurations` links the original `SJumpPointParams` values by owning
Record index and component ordinal. These selected components are retained as
dependencies of the placement. Competing source candidates record invalid resolution.

`projections.jumpConnections` reports absence for the admitted and readable endpoint
grammar. Unresolved source coverage makes it unavailable. Both states currently carry
empty rows. The inspected grammar establishes endpoint facts, while route pairing needs
an authored target relation and direction.

## Installed LIVE survey

The survey of LIVE build 13165946 read 5,060 original container descriptors reachable from
the three system roots. A second traversal covered 216 additional physical descriptors,
starting from the 180 remaining system-container paths and following their authored
references. It also inspected all 118,499 DataForge Record bodies and the original main,
metadata and editor XML for the six marked gates.

The reachable graph had eight endpoint declarations. All eight local container targets
resolved to a unique physical Entry. The original XML attributes and element texts
supplied no cross-system partner or travel direction. The 14 DataForge references to
recognized endpoint Records occurred in the Starmap parent hierarchy and the mission
location exclusion list. There was no additional endpoint declaration in the
supplemental descriptors.

The survey records its coverage gaps separately. Three reachable and 26 supplemental
references had no matching external container Entry. The DataForge pass recorded 1,042
unreadable values, with none in the relation-named fields of the selected Record
families. The finding applies to the inspected LIVE sources. No connections are
projected from them. If the placement traversal skips a route, both jump tables report
unavailable coverage and retain any endpoint rows already read.
