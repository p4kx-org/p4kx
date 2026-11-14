# Procedural planet audio

`definitions.proceduralPlanetAudio` in Catalog schema v3 contains the physical DataForge
Records of exact type `ProceduralPlanetAudioData`. Records keep the eight-field
[canonical source header](catalog-record-headers.md). The common table reports availability.
`completeness.proceduralPlanetAudio` describes filter scope. The matching source kind is
`proceduralPlanetAudio`.

Each RTPC below has its own source carrier. A present control contains an `rtpc` String with
its own source carrier and an `additionalFields` original-value arena for other members.

| Stored fields | Meaning in the catalog |
| --- | --- |
| `breathabilityRtpc`, `humidityRtpc`, `pressureRtpc`, `temperatureRtpc` | Authored environment RTPC identifiers |
| `disturbanceVelocityRtpc` | Authored disturbance velocity RTPC identifier |
| `firstRoomIsPlanetRoomRtpc`, `firstRoomIsPlanetRoomNotIncludingShipsRtpc`, `firstRoomIsPlanetRoomOrVehicleRoomRtpc` | Three separately authored room-control RTPC identifiers |

The labeled `entries`, `disturbanceLists` and `effectTagMovementRtpcs` retain complete original
subtrees with declaration types, order, nulls, separate links and individual provenance. These
include algorithm instances, audio triggers, disturbance thresholds and references, and effect-tag
references. Other original Record fields are under `additionalFields`. An unsupported typed
control also retains its readable original storage there.

An empty identifier is a present String. Older declarations can produce an absent control.
The shared five-field [source carrier](basetool-catalog.md#per-value-source-origin-for-canonical-definitions) reports each observation's
state, encrypted origin and source-use references. These values are the authored DataForge
identifiers. Audio bank objects and loose ecosystem sound declarations have their own source
families. The catalog makes no inference about runtime ranges or binding targets from RTPC names.
