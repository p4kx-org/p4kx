# Canonical Commodity definitions and presentation

The [shared Record-header contract](catalog-record-headers.md) defines the eight sourced header
fields, exact original filename/metadata-tag states and separate Header/Body provenance coordinates.

Catalog schema 3 publishes `definitions.commodities` and
`projections.commodityPresentation`. Each is a typed dataset with `availability` and `rows`.
The root `commodities` array is removed. Both tables use the existing Commodity selection and
canonical GUID order; their rows form a one-to-one pair through the Commodity GUID. Kind
`commodities`, its Filter/completeness policy and its measured logical-row summary keep their
meaning. The paired presentation does not count as another selected source Record.

Commodity `guid` remains the unique identity. A shared, nullable ResourceType relationship is
not an alternative primary key. An ingredient's `resourceGuid` joins
`definitions.resourceTypes.rows[].guid.value` for resource facts, and can match zero, one or
several presentation `subtypeGuid.value` values for Commodity alternatives.

## Authored definitions

Every source-backed field and retained authored list member has the common five-key
`SourcedValue` carrier. Read `state` before `value`; missing, null, unreadable, unsupported and
invalid remain distinct from present zero, false, empty text or empty lists. Encryption origin
is independent of game-text protection, selection completeness and publication health.

| Definition field | Actual source and scope |
| --- | --- |
| `guid` | The actual Entity Record GUID, or the actual ResourceType GUID for resource fallback. |
| `className` | The existing type-prefix removal from the original Record name; original header recordName conversion and sourceType selector inputs are retained; a no-string name keeps only its actual default selector. |
| `recordName`, `fileName`, `metadataTag` | Exact stored name, filename and separate Record-table metadata tag, with the shared sentinel/layout states. |
| `recordPath`, `recordIndex`, `sourceType` | The original normalized Record path, physical DataForge Record position and concrete source struct name. Physical position and nominal GUID remain different identities. |
| `selection` | Closed `entity` or `resourceFallback` choice, with the actual selection inputs. Resource fallback retains its eligibility and representation-check inputs here. |
| `entity` | Commodity-owned authored bindings for an Entity. It is absent for a ResourceType-backed row; no Entity source is invented. |

`entity.value` contains ordered `purchasableParameters`, `displayParameters` and
`resourceContainers` lists. Every proved matching component retains `sourcePosition` and
`sourceType`. Purchasable bindings retain their authored `displayName` and `displayType`;
display bindings retain `displayDescription`. Resource-container bindings retain their
physical owner index and source position. Join those fields to the canonical
`configurations.resourceContainers` rows for the ordered composition, original entries
and other container settings. The presentation's source selection still reads the
first authored entry under its existing rules. [Fuel and resource capacity](catalog-fuel-capacity.md)

Matching bindings keep the original `Components[i]` or `StaticEntityClassData[i]` position.
Nonmatching components are selection inputs, not fabricated Commodity bindings. Unreadable
classification cannot become a successful empty list. Known bindings remain available with an
incomplete contributor set and the actual source diagnosis. Each nested field, reference GUID,
physical target header and aggregate retains its own actual contribution roles.

Declared keys become canonical text references without losing empty or placeholder keys.
`displayName` uses `texts.publicNames`; `displayDescription` and `displayType` use
`texts.gameTexts`. Protected fields and their typed references carry `x-gameText`. A public
short name does not become protected merely because its key also appears in the protected domain.
The original key and each supported available language value remain in the canonical text row.

## Established presentation choices

Every presentation field has its own direct source marker and contributor references. Nullable
recipe results use `SourcedValue<Option<T>>`: a completed choice with no value is
`state: present, value: null`. This does not erase the separate raw absent/null/error states in
the definition. Translation absence is represented by the canonical language value's own state.

| Presentation field | Preserved recipe |
| --- | --- |
| `commodityGuid` | References the definition's original Commodity identity. |
| `subtypeGuid` | The first non-null `entry` in the first ResourceContainer's stored composition. An unresolved first link stops the search; no later readable target replaces it. A resource fallback uses its own ResourceType GUID. |
| `typeGuid` | The first selected, readable, non-leftover direct ResourceTypeGroup membership in original source Record order. The sorted full membership list in the ResourceType definition is a different fact. A later group failure does not make an already proved earlier winner incomplete; unknown earlier groups remain relevant. |
| `name`, `description` | The first matching own component's nonempty key wins when it is not a placeholder. Otherwise use the nonempty ResourceType key, or retain the own placeholder when no resource key is available. This choice precedes translation lookup; a valid own key with no translation does not trigger a language fallback. |
| `displayType` | The first matching own purchasable component's key, with no ResourceType fallback. Resource-only rows have no display-type key. |
| `densityGramsPerCubicCentimetre` | The existing finite Single in `densityType.densityUnit.gramsPerCubicCentimeter`. Other source units are not converted. No value is synthesized for nonfinite, missing or unreadable inputs. Original IEEE bits and units remain owned by the ResourceType definition. |
| `refinedSubtypeGuid` | The existing resolved `refinedVersion` reference GUID, without following its target body. The original reference and source-resolution status remain owned by the ResourceType definition. |

Resource-only rows remain limited to otherwise unrepresented resources used by any retained
Blueprint tier cost or kept quality quantization. Filter and leftover rules are unchanged.
Capture eligibility at the existing reader decision points, with actual physical Record indices
and original cost/property paths; do not infer it later from the new JSON. The existing treatment
of Commodity/Crafter wrappers is preserved. Reading a resource dependency never includes its
filtered-out definition automatically.

An own name uses only its actually consulted own selectors and key. Its source marker does not
include an available but unused ResourceType or Locale. A resource fallback includes the actually
consulted own-key selector, first-resource link and selected resource key. Translation rows carry
only the language Entries actually used. Failed reads stay attempts, not retained payload sources.
Known encrypted contributors keep a true marker even when the set is incomplete; without such
proof, an incomplete set is null, never false.

Resource fallback uses the original Blueprint selectors and the actual matching Quality inverse
members. A Quality-backed choice retains the effective quantizer decision, the matching authored
Resource relationship slots, and the existing packaging and representation checks. Unrelated
members of the same inverse do not contribute to that Commodity. Every original applicable
selector is retained. Unreadable inputs remain attempted evidence with explicit incompleteness.
Source-use buffer allocation failures abort catalog preparation before publication.

## Consumer migration and evidence

For each old Commodity row, find its definition by `guid.value` and its presentation by
`commodityGuid.value`. Definition header payloads reconstruct old `guid`, `className` and
`recordPath`; presentation payloads reconstruct old nullable relationship and finite density
values. Dereference `name` through `texts.publicNames`, and `description`/`displayType` through
`texts.gameTexts`. The text row's exact key and `en`/`de` language payloads reconstruct those old
localized fields; missing language values remain null. Retain the full canonical language map
rather than truncate it to that reconstruction.

Consumers must negotiate schema 3 and its actual named datasets before import or deletion. A
released schema-2 artifact cannot supply discarded raw states, physical positions or source
history; re-export original sources. The released schema-2 fixture is frozen. Existing identity
algorithms, GUID joins and stored ordering do not depend on serializing these new tables.
Other Media, Blender and package schemas keep their independent version contracts.

The [source verification guide](resource-source-verification.md) records bounded installed-Archive
checks and the historical evidence boundary. The original Commodity observation's complete Build
identity and export are no longer available. Treat its reported cardinalities as an unprovable
historical observation, not an activated exact Build baseline. Never guess a full Build from a
version prefix, unrelated statistics or the currently installed Archive. Current evidence from a
fully identified Build replaces exact pins for that unavailable historical export. The installed
current LIVE/PTU evidence remains required and deferred; synthetic checks do not replace it.
