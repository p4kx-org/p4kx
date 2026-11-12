# Authored starmap types, presets and amenities

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
