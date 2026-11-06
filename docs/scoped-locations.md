# Scoped mission-location values

Catalog schema version 3 exports `MissionPropertyValue_ScopedLocation` as a
`PropertyValue` with `"type": "scopedLocation"`. It is available in template
`contractProperties`, objective property values and broker `missionProperties`.
Derived contract texts use the established LOCATION placeholder; the exporter does not choose
a final location or evaluate the game rules.

The source is configuration owned by the original Record. Scope/location vocabulary definitions
are separate datasets; preserving a link does not imply that the linked kind was selected or
that a location is currently available.

## Filter families

All six keys are always present. A missing or null source StrongPointer produces null. A present
filter preserves its concrete variant and payload. Unknown concrete variants fail the owning
Record rather than silently removing the filter.

| Catalog key | Source concrete type after LocationCondition_ | Catalog type tag | Payload |
| --- | --- | --- | --- |
| factionFilter | RequiredFactionHostility | requiredFactionHostility | hostility |
| factionFilter | ExcludedFactionHostility | excludedFactionHostility | hostility |
| locationClassFilter | RequiredLocationClass | requiredLocationClass | locationClass |
| locationClassFilter | ExcludedLocationClass | excludedLocationClass | locationClass |
| locationClassFilter | RequiredLocationClasses | requiredLocationClasses | locationClasses |
| locationClassFilter | ExcludedLocationClasses | excludedLocationClasses | locationClasses |
| locationSizeFilter | RequiredLocationSize | requiredLocationSize | locationSize |
| locationSizeFilter | ExcludedLocationSize | excludedLocationSize | locationSize |
| locationSizeFilter | RequiredLocationSizes | requiredLocationSizes | locationSizes |
| locationSizeFilter | ExcludedLocationSizes | excludedLocationSizes | locationSizes |
| locationFunctionFilter | RequiredLocationFunction | requiredLocationFunction | locationFunction |
| locationFunctionFilter | ExcludedLocationFunction | excludedLocationFunction | locationFunction |
| locationFunctionFilter | RequiredLocationFunctions | requiredLocationFunctions | locationFunctions |
| locationFunctionFilter | ExcludedLocationFunctions | excludedLocationFunctions | locationFunctions |
| otherLocationFilter | NotLocation | notLocation | location |
| otherLocationFilter | SameLocation | sameLocation | location |
| otherLocationFilter | NotLocations | notLocations | locations |
| relativeLocationFilter | SameScope | sameScope | scope, location |
| relativeLocationFilter | DifferentScope | differentScope | scope, location |

`hostility` and `scope` retain the raw ReactionType and ContractScope option names. A stored name
not known to the exporter is retained. A value whose enum bytes cannot be read is a source error,
not a null or a guessed known option. Required/excluded and same/different remain distinct tags.

Plural payloads preserve source order, repeated targets and null elements. An empty array is [];
an absent array is null. No sorting, deduplication or inferred inheritance is applied.

## Definition References

Each `locationClass`, `locationSize` and `locationFunction` target is null or an object:

| Field | Meaning |
| --- | --- |
| guid | The declared target Record GUID, including a dangling GUID |
| recordPath | The known source Record path without .xml; null for a missing Record |
| targetType | The known source target struct name; null for a missing Record |
| status | present when the DataForge identifies the Record; dangling when only its GUID is known |

The plural fields contain the same objects. `present` is source availability, not a guarantee that
another catalog kind includes the target. Unsupported/malformed source values remain explicit
Record failures.

## Weak property links

The `location` and `locations` payloads retain weak-pointer identities instead of exporting only
a resolved variable name:

| Field | Meaning |
| --- | --- |
| recordGuid | The target Record GUID when available |
| recordPath | The target Record path when available |
| targetType | The source target struct name when available |
| location | The canonical position inside the target Record when available |
| variable | The original owner's linked missionVariableName; empty source strings stay empty |
| status | The resolution result listed below |
| sourcePointer | An unresolved source pointer's structIndex and instanceIndex, or null |
| sourceError | An available source read error, or null |

Resolution is restricted to the owning Record GUID before its canonical location is followed.
An external Record is never searched, and its location is never interpreted against a different
Record with a similarly named property.

- `resolved`: a supported MissionProperty inside the original owner supplied its variable.
- `missingTarget`: the target cannot be located, including an invalid stored instance pointer.
- `outsideOwner`: the target is outside the owning Record; available source identity or raw
  coordinates remain intact.
- `unsupportedTargetType`: the local linked instance has an unsupported source struct; its
  identity is retained without guessing a mission variable.
- `unreadableTarget`: the target variable exists but cannot be read as a string; sourceError
  distinguishes that failure from missing or null data.

A null weak pointer stays null. No replacement GUID, synthetic variable or effective runtime
location is invented for unresolved targets.

## Verification and compatibility

The synthetic Archive tests cover all 19 concrete variants, the six independently nullable
families, declared schema alternatives, raw future enum names, singular/plural links, empty and
ordered/repeated/null list members, dangling definition GUIDs, owner-scoped weak links and
unreadable source payloads. Template, broker, objective and derived-contract entrypoints are
exercised with the same source mapper.

`cargo test -p p4kx-export --test it --locked scoped_location` runs these cases. They compare
the emitted JSON and runtime-generated catalog schema, rather than merely proving that generic
DataForge traversal succeeds.

This tagged property branch is a schema version 3 change. Version 2's released closed PropertyValue
union cannot represent it; consumers must explicitly support version 3. The version/health
contract is recorded separately from this source mapping. Build-specific PTU verification uses
the installed Archive privately; source identities, payloads and exact game values are not test
fixtures or public verification attachments.
