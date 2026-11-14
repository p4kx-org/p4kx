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
metadata and the editor document inside the same SOCPAK. `Entity/@CryGUID` selects an
`Object/@guid` in the editor document. The candidates retain exact authored type, name
and other stored attributes. A duplicate keeps its candidates and has no selected object.
Unreadable entries carry their own state and report the failed read separately. A readable
logical placement can have no original gate marker.

`declaredPartner` describes the inspected endpoint declaration scope. A successful read of
that grammar without a partner records absence. Missing or unreadable evidence leaves this
field incomplete. The governing evidence is the original declaration, with no pairing by
endpoint names. All source-backed fields retain their immediate origin.
