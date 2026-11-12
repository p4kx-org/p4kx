# Authored starmap definitions

The catalog retains each physical `StarMapObjectType`, `StarMapPreset` and
`StarMapAmenityTypeEntry` in `definitions.starmapTypes`, `definitions.starmapPresets` and
`definitions.starmapAmenities`, respectively. Selection uses the exact Record type at any
path and the ordinary filter. Rows are ordered by GUID and physical Record index. Table
availability follows the original declaration, independently of the filtered row count.

| Definition | Authored content |
| --- | --- |
| `StarMapObjectType` | String name, Locale classification, `WorldDisplayObjectFacingMode` option, display size and rotation settings, selectable and other routing/display flags, marker and sub-nav-point References, owned geometry and material paths. |
| `StarMapPreset` | One type Reference, owned visibility, display, location and radar classes, and the separately stateful `quantumTravelData` StrongPointer. |
| `StarMapAmenityTypeEntry` | String name and icon path, short Locale display name, and one `LocationEntityDeclaration` Reference. |

A preset's `visibilitySettings` retains `hideInStarmap`, `hideInWorld`,
`hideWhenInAdoptionRadius`, `blockTravel` and `onlyShowWhenParentSelected`. The type's own
`validQuantumTravelDestination`, `spawnNavPoints` and `showInMapSelectList` are independent
Bool facts. Consumers can interpret them together with an object's authored settings.

`displaySettings` carries the original localized description, `NavPointIconEnum` label,
Double `size`, Single `minimumDisplaySize`, holo-material flag, geometry, material, shape and
medical image paths, and the original asset-manager preview paths. `locationParams` retains
`setEntityLocationOnEnter`, `exposeForPlayerCreatedMissions` and `excludeFromLevelLoad`.
`quantumTravelData` distinguishes the original obstruction, arrival and adoption radii,
the arrival-point detection offset and the sub-point radius multiplier. Their IEEE bits and
original widths are retained. Values keep the source's scale and units.

`radarSettings` preserves `isScannable` and the separate `radarProperties` pointer. The
original `SSCRadarContactProperites` body carries a `RadarContactTypeEntry` Reference.
Its base signature, cross-section, emission modifier, death and scan-bounds pointers keep
their own source states and original owned bodies. `roomParams` is an inline class with
`ignoreInteriorVsExteriorCheck`. Additional authored members retain original declarations,
references, order and values in their owner-local source arenas.

Reference payloads retain the original target identity and availability. The export health
section records whether the selected catalog includes a resolved target. Each original
field and retained owned occurrence carries its own source marker. Descriptions reference
`texts.gameTexts` and carry `x-gameText`. Short type classification and amenity labels
reference `texts.publicNames`.

A preset is a reusable authored configuration. The export preserves links as stored.
Placement, travel policy and release status are separate concepts.


## Starmap objects

`definitions.starmapObjects` contains selected physical `StarMapObject` Records with the
canonical Record header. The original `type`, `parent`, `jurisdiction` and
`locationHierarchyTag` are References to `StarMapObjectType`, `StarMapObject`,
`Jurisdiction` and `Tag`, respectively. `affiliation` declares `Faction_LEGACY` and retains
that original Record identity. `amenities` is the ordered Reference array to
`StarMapAmenityTypeEntry`. Repeated and dangling slots retain their original positions.

Object flags retain the same five visibility fields as a preset and the original
`isScannable`, `skipCollisionCheckIfInArrivalRadius`, `overrideRotationSpeed`,
`showOrbitLine` and `noAutoBodyRecovery`. `overrideSpawnNavPoints`,
`overrideShowInAllZones`, `overrideShowInRouteOnly` and `overridePermanent` are
`StarmapBoolOverride` option labels. `respawnLocationType` retains the
`ERespawnLocationType` label, and the rotation speed value is an independent Single.

The object carries the same original display fields as a preset and adds
`locationImagePath`. `name`, `description` and `callout1` through `callout3` use the
protected game-text domain. The object stores `locationParams`, `quantumTravelData` and
`radarProperties` as StrongPointers. `asteroidRings` retains ordered `StarMapAsteroidRing`
classes with the original Single scales and depth and Double inner and outer radii. The
audio play and stop trigger classes retain their original `GlobalResourceAudio` bodies.

`placements.starmapPlacements` joins these definitions by `objectGuid.value` and
`objectRecordIndex.value`. The existing traversal supplies its world position and hierarchy
values separately. Until the full XML chain is captured, those computed cells mark the
source set incomplete and the encryption origin unknown.
