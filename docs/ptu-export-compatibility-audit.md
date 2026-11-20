# Installed test-channel compatibility audit

Research date: 2026-11-06. Source baseline:
[1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).

The installed PTU Archive was compared with the installed LIVE Archive using a fresh build
of the current checkout. This report contains technical structures and implementation findings
only. Game values, text dumps, concrete test-build Record identities, build payloads and
extracted assets are not included. Public technical issue updates were explicitly authorized.

## PTU 13215521 comparison

The installed PTU manifest identifies change `13215521`. The checks use installed LIVE
`13165946`, the retained PTU `13189690` Archive and saved scoped evidence from PTU
`13201184`. The retained Archive supplies a common baseline for the two ContractGenerator
comparisons. The saved evidence supports a lower bound on changes since `13201184`.

| Area | Observed source and export status |
| --- | --- |
| DataForge schema | The schema comparison with `13189690` adds one structure, `TransportVehicleAccessInterface`, with zero own properties and zero standalone Records. Existing properties and enums agree. The LIVE comparison has the same declaration delta as the retained comparison with LIVE plus this structure. No new typed Catalog mapping is indicated. |
| Contracts and reputation | Against `13189690`, the scoped comparison reports one added and 20 changed ContractGenerator Records, with zero removals or unreadable Records. Comparison with the saved delta identifies at least ten changed generators since `13201184`. They use existing result, reputation, prerequisite, scope, template and location fields. The selected resource-gathering generator changes authored outcome flags, reputation reward references and Blueprint pool fields. [ContractGenerator catalog contract](catalog-contract-generators.md) |
| Nyx I Starmap | The selected Lagrange Child changes position and rotation against `13189690`. Its `pos`, `rot` and container relationship remain in the existing source format. The inspected LIVE descriptor has no corresponding authored Child. [Starmap definitions and placements](catalog-starmap-definitions.md) |
| Starchitect interiors | A selected Watchtower SOCPAK passes all 12 root-member CRC checks in each PTU. Of its four scene-container occurrences, three nested occurrences expose additional RoomMapping companions in `13215521`. All three are published via `roomCompanions.roomMapping` as complete ordered XML. The sampled light and mission-link declarations agree between the two PTUs. The minimap filename selection shows no directory delta. [Authored SOC scene contract](soc-scene.md) |
| Flight suit compatibility | The scoped Xanthule Record comparison with `13189690` reports zero changes. The LIVE comparison has two changed Records in the same selection. Their changed fields cover attachment alignment, materials and a ContractTemplate search-reporting flag. The 17 English search excerpts that contain the suit name agree across all three Archives. Actual backpack compatibility and the full description source remain unverified. [Inventory storage](catalog-inventory-storage.md) |

The archive evidence does not establish live board availability, courier generation, actual
reward payouts or an effective reputation multiplier. Nine Tactical Strike Group filename
matches have no directory delta against `13189690`. An English Locale search for the
patch-note contract wording found no match in either PTU. The interaction reset and temporary
contract disable remain unverified. The sampled Watchtower documents do not establish the
power-to-light-to-objective link. The selected loose XML and JSON territory filename probes
found no matching Entry. A client-side territory box/grid reader or replication limit field
would require an identified original configuration source.

Two scoped installed checks passed on `13215521` using integration commit
`3343f50c7c34847fde327afecc1e391cc2456ce0`. One covers six SDK queries for mining,
refining, quality and resource data. The other covers Starchitect placement and source
associations. Complete Catalog publication remains an acceptance step. The inspected changes
provide no evidence of a new parser defect or schema change.
[Build comparison](https://github.com/p4kx-org/p4kx/issues/1151#issuecomment-6971711263)
· [SOCPAK and Territory follow-up](https://github.com/p4kx-org/p4kx/issues/1151#issuecomment-6977222564)

### Selected material shader functions

The installed LIVE `13165946` and PTU `13215521` shader caches agree for all 19 selected
complete functions, including the scalar gloss helper. The comparison derives a keyword
mapping independently from each Build's executable and compares canonical token
sequences. It binds nine selected inner members per Archive using the current physical
directory, checks all 18 inner CRC32 values and verifies stable source identities.

The comparison reads 41,631,414 bytes across the two Archives after outer directory
discovery, including both inner ZIP directories. The read budget is derived from their
measured lengths, the local header bounds and the selected compressed payload allowance.
Runtime permutation selection, CPU/GPU parameter packing, texture-channel binding and
game appearance require their own evidence.

## PTU 13201184 comparison

The installed PTU manifest identifies change `13201184` and internal version
`5.0.201.28448`. The older local Archive matches the retained 13189690 size, modification time
and original DataForge directory size and CRC. Its copied location has no standalone Build
manifest. The installed LIVE comparison uses change `13165946`. The read-only checks verified
that all three Archive stamps stayed stable.

| To PTU 13201184 | Added | Removed | Changed |
| --- | ---: | ---: | ---: |
| Archive Entries from copied PTU 13189690 | 269 | 75 | 2,857 |
| Archive Entries from installed LIVE | 38,941 | 13,481 | 24,395 |
| DataForge structures, properties, enums and options from PTU 13189690 | 0 | 0 | 0 |
| Selected ContractGenerator Records from PTU 13189690 | 1 | 0 | 16 |

Both DataForge schema comparisons reported zero unreadable declarations.
Against LIVE the new PTU has the same declaration delta counts as 13189690.
The absence of a DataForge declaration delta leaves independent Archive formats and new
uses of existing declarations to check.

The scoped ContractGenerator comparison found changed reputation award links, rank bounds,
completion-tag requirements, logical variants and authored text tokens. The Catalog has
distinct owners for those facts. All 70 authored reward amount Records and all 380 queried
standing Records match the prior PTU in their selected value fields. The 58 rewards shared
with LIVE also agree. This establishes nominal source amounts. The effective PTU 25×
multiplier requires an independent configuration or runtime witness. The directory filename scan found
zero changed `.cfg` Entries and zero changed XML, JSON, INI or text Entries with `reputation`
or `multiplier` in the filename. The effective scaling remains unverified.

| Area | Observed source and export status |
| --- | --- |
| Inventory occupancy | The physical Record comparison reports 1,956 changed volume selections in the shared armour path cohort, 1,968 in clothing and 54 in the magazine path cohort. Selected magazine ammunition counts agree. The shared backpack path cohort has no changed occupancy or dimensions against the prior PTU. The recorded volume units and vectors already have separate Catalog v3 cells. Concrete equipment fit also depends on the selected container's limits and filters. [Inventory contract](catalog-inventory-storage.md) |
| Client settings | A new profile attribute document contains 39 ordered declarations. 35 carry an optional CVar association. The existing ActionMap covers the single added action with its Hold mode and empty default bindings. `configurations.clientProfileSettings` carries the ordered settings with individual origins. The ActionMap keeps the authored description. [#1157](https://github.com/p4kx-org/p4kx/issues/1157) |
| Contract item results | Six added result occurrences use an authored entity reference, quantity, ordered outcome flags and a weak instance reference to a MissionProperty. The source arena keeps these values and the separate Blueprint result. `itemRewardResults` carries the authored result rows and original selectors. The contract projection exposes the exact location targets it can bind. [#1158](https://github.com/p4kx-org/p4kx/issues/1158) |
| Interior companions | A verified changed SOCPAK contains both a room mapping tree and a distinct room connector metadata file. The connector owner joins to the original SOC entity in the same container. The authored SOC scene exports both companions, retaining the original documents and positioned entity join. The sampled extraction tile descriptor changed only its editor Build marker. [#1159](https://github.com/p4kx-org/p4kx/issues/1159) |
| Purchasable names | The selected module cohort has five changed Purchasable name selectors, three accompanied by separate changed attachable name and description selectors. The magazine Record comparison finds two independently changed Purchasable names. The canonical item definition carries the Purchasable name and its origin separately from AttachDef text. The tracked shop inventory path family had no changed Entry. [#1160](https://github.com/p4kx-org/p4kx/issues/1160) |
| Climate and audio | The sampled changed climate asset exercises existing size, light and growth fields, including removed optional arrays. The sampled ATL environment library remains in the existing environment, auxiliary bus and RTPC control families. The engine's runtime terrain and audio behavior remains outside these source checks. |

The `actionmap settings` CLI verified 316 general declarations in each PTU. The VR
document is absent in `13189690` and contains 39 declarations in `13201184`.
The ActionMap JSON read the added action description and the empty bindings from
their own source. A scoped Catalog witness compared 57 typed item reward
occurrences from one selected `13201184` ContractGenerator to their original
outcomes, amounts and physical references. The normal `interior source` CLI
verified the selected RoomMapping and connector documents in both PTUs directly
against the retained CryXmlB node, attribute and string tables. The semantic
owner link resolves to the same-container SOC entity. Its sampled inline loadout selector agrees
between the two PTUs. The inspected Starchitect members establish no typed link for the new
AI closet or loot changes. A separate changed maintenance SOCPAK yielded 18 complete XML
documents on each PTU. Both contain the same empty `EntityComponentSpawnCloset` declaration
under `ObjectContainer.ComponentMetadata`. This occurrence identifies container metadata. A
positioned spawn or loot relationship requires its own entity and configuration evidence.

An exact extraction-container path matched one physical Entry in each PTU. Its directory
grew from 17 to 20 members, with one added `.rmxml` and two added `.xml` members.
Seven shared paths changed size metadata. The count remained one SOC member and seven
EntXML members. Every listed member passed CRC verification and both Archive stamps
stayed stable. The new `.rmxml` decodes as `RoomMapping`. The two new `.xml` documents use
`EntityComponentEntityReferenceOverride`. Each has an authored `Entity@CryGUID` and one
`AutoSpawnOverrideElement` with `AutoSpawnIdx`, `Target` and `AA`. In both documents,
`AutoSpawnIdx` is unsigned decimal text, `AA` is a UUID, and `Target` is an ordered chain of
four comma-separated UUIDs. The first token equals the document's `CryGUID`. None of these
selectors alone equals one of the eight `SuperGUID` attributes in the new room mapping.
A placement check on PTU `13201184` found one exact EntXML root identity per selector
among the seven EntXML members. The selected descriptor declares one matching SOC stream.
The stream's independently decoded Entities chunk contains 230 placed Entity rows, all
with valid IDs. Neither override selector occurs among those IDs. The Entity document
matches establish identity within the selected container. A placed owner and the remaining
target semantics require separate evidence.

The standalone authored SOC scene reports zero override companions on the older
PTU and two on the current PTU. Independent CryXmlB table decoding agrees with
all eight nodes, eight attributes, six typed declarations and eight typed values.
The verifier matched all 16 typed field source uses and both full payload sources.
The two raw rows replay the 1,092 original decoded bytes exactly.
[Override companion contract](soc-scene.md#entity-reference-overrides)

An exact Item Filter verified the old and new purchase-name Locale, its
DataForge selector and the separate unchanged attachable name. The normal
Catalog writer refused the older filtered export with one `shopInventories`
`MalformedSource` projection failure. The purchase-name witnesses use explicit
diagnostic Catalogs with independent source checks for the selected Item.

The scoped prerequisite delta contains 50 current `ContractPrerequisite_CompletedContractTags`
occurrences with both count fields and one required tag each. Of the 52 old occurrences in the
same delta, 12 had an empty required-tag list. `requiredIntros` presents the ordered tags and
matching intro grants. The original Generator arena retains prerequisite grouping, empty lists,
`requiredCountValue`, `excludedCountValue` and `includePrerequisiteWhenSharing`.
[ContractGenerator catalog contract](catalog-contract-generators.md#intro-prerequisites)

The raw source receipts are private. This section records source structure, exact scope counts
and implementation ownership. System-memory acceptance uses 64 GiB total RAM for the full
packaged catalog and 16 GiB total RAM for individual SDK queries and the reduced Basetool catalog.

## PTU 13189690 follow-through

The 2026-11-14 comparison verifies the installed manifests for PTU 13189690
(`sc-alpha-5.0.0`, internal `5.0.201.16954`) and LIVE 13165946
(`sc-alpha-4.10.0`, internal `4.10.200.58746`). The earlier 5.0 PTU changelist
13119881 (`5.0.200.12681`) is identified by its retained client build header.
The older comparison below covers that earlier set of declaration families. No full prior PTU
Archive has been found in the installed channels.

The read-only comparison used `p4kx compare LIVE PTU --entries`, `--schema`, and a filtered
`--records` pass over the FactionReputation path family. Both source Archive identities stayed
stable through the runs. Only counts and technical declarations are reported here.

| LIVE to current PTU comparison | Added | Removed | Changed |
| --- | ---: | ---: | ---: |
| Archive central-directory Entries | 38,744 | 13,478 | 24,145 |
| DataForge structures | 161 | 7 | 62 |
| DataForge properties in shared structures | 60 | 33 | 2 |
| DataForge enums | 6 | 0 | 5 |
| DataForge options in shared enums | 15 | 1 | 0 |
| Filtered FactionReputation Records | 11 | 1 | 38 |

All 38 shared changed FactionReputation Records show the four added presentation fields
`description`, `background`, `missionBoardLogo` and `guildBanner`. The catalog already
exports these fields under [#1073](https://github.com/p4kx-org/p4kx/issues/1073).
The archived 5.0 audit below already identified the broader scope, Mission Board and
FactionIdentity families. In the current comparison, all 58 authored mission-reputation reward
Records in the shared path family are unchanged, with zero unreadable Records. The type-wide
inventory contains 58 LIVE and 70 PTU Records. The 12 additional PTU Records occur outside that
path family and are already admitted by `definitions.reputationRewardAmounts`.
The announced PTU reputation multiplier has no evidence in this comparison of stored awards.
The existing Handler export exposes authored scope and default board availability.
A distinct Contract- or SubContract-level override and the runtime multiplier remain unverified.

The current DataForge declarations expose the Bool
`ContractResult_CompletionTags.splitPointsForParty`, the Int32
`LocationResourceSlot.numberOfResourceSlotsPerEntity` and the Bool `allowFPSPingWave` in the
owned scalar `SStandardParams.radarOptionParams` Class. The LIVE resource-slot layout holds a
`resourceSlotTag` Reference. The exact type count changes from 3 in LIVE to 66 in PTU.
The catalog preserves both resource-slot layouts with physical source and target identities,
GameMode radar options and ordered completion-tag outcomes. Each change passed focused checks
on both installed Builds.
[#1146](https://github.com/p4kx-org/p4kx/issues/1146),
[#1147](https://github.com/p4kx-org/p4kx/issues/1147),
[#1148](https://github.com/p4kx-org/p4kx/issues/1148)
`AmmoParams.impactAIStimulusDistance` already has its own catalog cell.

The changed GFx and SWF Entries each parse successfully in both installations. Both files
change the container version from 17 to 15 while keeping the same frame count and the same
unread tag inventory. The exported symbols and frame labels do not identify the missile-lock
interface. A file-type match by itself cannot attribute this change to the announced UI.

A separate archive path census finds 13 missile-lock geometry Entries, their 13
same-stem companions, and one material added relative to LIVE. The installed
PTU CRC check passed on all 116 Entries in the broader missile-and-lock path
selection with zero unreadable Entries or mismatches. Three sampled geometry
pairs open from both the Archive and loose files through `model info`. Each has
one node, one LOD, no declared material-library binding and no model issue.
The independent CryXml material contains `Material`, `PublicParams` and
`Textures` with render parameters. The Media model and material providers
admit these physical families separately. Neither checked asset source exposes an
authored mapping for the four lock states or the effective range indicator.

The directory and changed declaration inventories have no matches for territory,
server meshing or claim reservation names. This name-based check leaves the
territory transform, entity-count bias and runtime candidate behavior unverified.

The PTU declaration adds an ordered `EntityComponentSpawnClosetParams.autoSpawnsRecords`
Reference array with target `AutoSpawns`. The field census across 31,060 readable
`EntityClassDefinition` Records reports zero occurrences of that path and zero
unreadable Records or values. Three representative changed Spawn Closet
containers supplied 12 readable XML members and no literal occurrence of the new
field in those members. The existing AutoSpawns definition and mission-property
tables cover their independently evidenced owners. This sample leaves the new
component occurrence and its runtime spawn placement unverified.

Current PTU has 24 `.resource`, 35 `.climasset`, 68 `.geoasset` and 156 `.dst` Entry additions
against LIVE. The prior 5.0 audit already observed the loose planet descriptor families.
The present Entry comparison shows 980 additions, 80 removals and 7,677 changes for SOCPAKs.
Audio includes 19/10/304 BNK and 1,962/1,469/113 WEM additions/removals/changes. These
path, size and checksum observations guide the selection of concrete source witnesses.
They do not attribute a change to the current PTU update relative to the prior PTU.

The stored physical-damage channel changed in one ammunition witness alongside
`impactAIStimulusDistance`. The narrow weapon and related-record filter found four changes
and zero unreadable Records. Existing typed ammunition inputs cover both changed fields.
The comparison establishes a content change against LIVE for that sample. A nominal value
from that source supplies no evidence of actual kill times or a delta against the previous PTU.

The schema also adds two scalar `AudioRtpc` Class members to `ProceduralPlanetAudioData`,
with two exact Records in each Build. They qualify the original room controls
`firstRoomIsPlanetRoomNotIncludingShipsRtpc` and `firstRoomIsPlanetRoomOrVehicleRoomRtpc`.
Their DataForge owner is tracked separately in
[#1149](https://github.com/p4kx-org/p4kx/issues/1149).

The system-root probe admitted three ObjectContainer v14 system roots and counted
847 Child positions and rotations. The independent directory comparison found 722 Starchitect
Tile SOCPAKs added relative to current LIVE. Directly opening the smallest selected Tile
verified its descriptor and 15-element layer list.

A second CRC-verified read establishes an authored chain from the Nyx system descriptor to
the Nyx I descriptor and from there to a physical Starchitect Tile. The Nyx I descriptor has
16,947 named external children with local position and rotation, including 15,076 Starchitect
Tile instances and 125 distinct Tile targets. One target selected by its original Child
reference opened successfully and its ObjectContainer v14 descriptor passed the CRC check.
The Nyx I container holds 16,948 EntXML members. A direct inventory of a separate large Tile
found 640 members and no EntXML or HCD members under the tested naming rules. The placement
walk and native transform test are complete in [#844](https://github.com/p4kx-org/p4kx/issues/844).
The EntXML survey in [#845](https://github.com/p4kx-org/p4kx/issues/845) matched 16,947 root
`EntityCryGUID` values with exact `Child@guid` declarations, including all 1,791 typed
locations. Their original HarvestableComponent occurrences provide a
`SubHarvestableMultiConfigRecord` link. The HCD companion independently provides a Context
Preset GUID. AI and generated resource placement require separate source evidence.

The filtered diagnostic Catalog preserves all 2,248 occurrences of one independently
selected Tile, including distinct occurrence identities, system and planet parent links,
authored labels and local and composed world transforms. The sampled transform's 32 distinct
source-use references resolve in the inventory. The source uses state unknown effective
encryption for the XML payload on this input, so the transforms carry null markers.
This scoped Catalog has incomplete overall export health.

A filtered LIVE control produces 2,094 physical Starmap rows in each of the two tested
integration builds. Their placement field states and values are identical. Each export has
1,189 positioned rows, three missing containers and zero unreadable containers or malformed
children. This compares a retained source snapshot whose ten placement files match the
pre-#844 integration tip with the updated catalog on the same Archive. Both wrote diagnostic catalogs.

CRC-verified weather and lightning ParticleLibrary Entries are readable in both installed
builds. The weather library has 62 elements on each Build and adds eight lexical Boolean
`RainOcclusion` attributes in PTU. The lightning sample has 30 Lightning elements in LIVE and
31 in PTU. The catalog preserves existing particle reference paths.
[#1150](https://github.com/p4kx-org/p4kx/issues/1150) exports the original library definitions and
field origins in [the particle-library table](catalog-particle-libraries.md). The installed
source-to-projection test checked both original library shapes in each Build and verified the
Boolean slots, node counts and authored light-attribute keys.
The added climate-asset sample has 87 distinct field-path/type pairs, including climate limits,
shading GUIDs and ordered harvestable-related sub-assets. The producing family is tracked in
[#849](https://github.com/p4kx-org/p4kx/issues/849).

The per-domain source probes, exporter changes and evidence gaps are tracked in
[#1145](https://github.com/p4kx-org/p4kx/issues/1145).
The current tuning test and other patch-note items come from the
[CIG notes](https://robertsspaceindustries.com/spectrum/community/SC/forum/190048/thread/star-citizen-alpha-5-0-ptu-patch-notes/9230254)
and separate
[TTK update](https://robertsspaceindustries.com/spectrum/community/SC/forum/190048/thread/star-citizen-alpha-5-0-ptu-patch-notes/9230300).

### Patch-domain results

The physical DataForge probe opened the exact installed LIVE 13165946 and
PTU 13189690 Archives and checked that each stayed stable. It selected
Entity owners by the original `Components` types and GameMode rules and
global definitions by their original Record types. For each directly
comparable value, it matched the original Record GUID and path and the
physical component and field positions. Additions and removals in this
comparison refer to unmatched positions. The probe compared the original
numeric words and reported only counts and directions of change.

| Area | Observed source and Catalog disposition | Remaining evidence |
| --- | --- | --- |
| Reputation and Contracts | Shared nominal awards are unchanged. The faction presentation, Handler scope and board defaults have Catalog paths. Four changed declaration families were verified and delivered in #1146–#1149. | The tested awards and Handler declarations do not establish a separate Contract or SubContract override, board ordering or the effective PTU reputation multiplier. |
| Missile-lock UI and assets | The GFx/SWF samples open and the missile-lock geometry and material samples pass the focused readers. #1056 preserves the authored missile configurations in Catalog. | The original UI state and range bindings remain unverified. |
| Starchitect placements, AI, loot and traversal | #844 and #845 verify source-linked transforms and EntXML/HCD occurrences. #847 exports separately owned loot source definitions and original component links. The new Spawn Closet Reference array was declared but absent from the selected Entity census. | Source evidence for this build's AI relocation, traversal and ledge changes, and exact loot or delivery placements remains unverified. |
| Terrain, vegetation, mining and harvestables | Current directory changes and the sampled climate Class/JSON shapes are recorded. #849 and #1083 cover admitted descriptors and links, and #845 covers the original harvestable setup bindings. | Runtime density, generation, terrain LOD, memory or water changes, and a previous-PTU content delta were outside the available source comparisons. |
| Weather and VFX | The independent LIVE/PTU particle-library comparison found authored Boolean and lightning-node changes. #1150 publishes the original definitions and references. | Runtime weather frequency and the VFX-to-sound behavior remain unverified. |
| Audio | The retained comparison has 19 added, 10 removed and 821 changed `Libs/GameAudio` XML Entries. Path-name searches surfaced four changed weather-audio candidates and 26 changed Nyx-audio candidates. The current Archive comparison also accounts for BNK/WEM path populations. #1149 exports the changed procedural planet room RTPC declarations. | The path inventory cannot attribute an ATL trigger to an exact changed bank or prove audible thunder, footstep mixing, music, propagation, or building transitions. The retained earlier PTU bank receipt has a different source identity. |
| Server meshing | The changed Entry and declaration names provided no territory candidate under the stated name filters. Both position and rotation occur on the separate authored placement descriptors. | No original data binding establishes server territory transforms, claim bias, candidate gating or physics-authority behavior. |
| TTK armor and turrets | Of 223 matched armor-owner `SHealthComponentParams` slots, `Health` changed in 220, with 105 increases and 115 decreases. Of 460 matched turret-owner slots, `Health` increased in 272 and stayed unchanged in 188. Every matched `DamageCap` was unchanged. Selected Item components retain these two words in `configurations.durability`. | This measures the currently installed LIVE-to-PTU change. The earlier PTU values and any across-the-board balance claim remain unverified. |
| TTK destruction rules | All 24 matching physical GameMode Records have an original `ResourceNetworkParams` Class. Its `shipDestructionHealthThreshold` increased in 23 and stayed unchanged in one. Its `shipDestructionBufferMultiplier` remained unchanged in all 24. Both fields are already exported in `configurations.resourceNetworkParameters`. | The exact runtime relationship between these two settings and a single-hit destruction event is unverified. |
| TTK beam and projectile inputs | The scoped ammunition comparison found an authored damage change and existing Catalog coverage for that value. #1052 checks the complete authored weapon-action input paths, including independently stored beam and launcher heat in synthetic fixtures. | The narrow sample does not measure the other weapon-series, beam and projectile-speed claims. |
| Component power and distribution | All 14 matched wheeled-controller `MinimumPowerAmount` Singles were unchanged. The five matched legacy power connection rows retained identical base, draw and overclock threshold Singles. The separate authored `ItemResourceNetworkGlobal` Record changed five Singles in `defaultPowerDistributionParams`, including `powerDistCoolerMin`. | `definitions.resourceNetworkGlobals` preserves the original distribution block. The field evidence leaves the general minimum assignment rule unverified. |
| Other fixes and platform behavior | The source audit records archive readers, source declarations and scoped Catalog checks. | Gameplay fixes, VR, runtime rendering and crashes require runtime evidence. The installed release export and native package acceptance are tracked in #1116. |

The standalone `ItemResourceNetworkGlobal` definition owns the power distribution block
and `powerParams`. The Catalog keeps its physical Record distinct from the per-GameMode
resource-network rules. Direct comparisons of the selected LIVE and PTU Records checked the
modeled values, ordered modifiers and physical source coordinates.
[Global resource-network definitions](catalog-resource-components.md#original-global-resource-network-definitions)

The standalone `STurretHealthModifierDef` contains a `damageMovementModifier` curve.
Its nine stored point pairs and LUT flag are unchanged in this comparison. All 474 LIVE and
475 PTU `SCItemTurretParams.healthModifierRecord` slots are authored null.
`configurations.turretHealth` preserves the individual Reference states and
`definitions.turretHealthModifiers` preserves the independently owned curve.
The scoped original-to-catalog test passed for LIVE 13165946 and PTU 13189690. It checked
one physical turret component and one standalone definition per Build, including all nine
ordered point pairs, their exact Single bits, the LUT flag and per-value source coordinates.
A synthetic control proves filtered dependency closure for a resolved Reference.
[Turret health](catalog-turret-health.md)

The retained older PTU declaration evidence can establish what already
existed by that earlier 5.0 snapshot. The two current Archive receipts
cannot reconstruct the overwritten earlier payload. The full installed
release verification is tracked by [#1116](https://github.com/p4kx-org/p4kx/issues/1116).

## Schema v3 follow-through

Schema v3 exports the original item-recovery economy, override and policy tables. The
normal source-to-catalog check passed on the installed LIVE direct layout and on the
installed TECH-PREVIEW layout with all three nested groups. The installed PTU checked
during follow-through used the direct layout. A separate malformed offline inventory
prevents normal publication on that PTU build. [#1080](https://github.com/p4kx-org/p4kx/issues/1080) · [#1070](https://github.com/p4kx-org/p4kx/issues/1070)

Both mission action wrappers are typed in schema v3. The current installed PTU lacked
both original declarations. The installed TECH-PREVIEW contained both, and the positive
test matched their ordered actions and stored weights against their original Template
owners. [#1081](https://github.com/p4kx-org/p4kx/issues/1081)

The following findings describe the source baseline above.

## Scope and method

- Open both Archives and compare directory metadata (path, size and checksum) across all Entries.
- Compare declared DataForge structs, fields, targets, parents and enums.
- Build every Record tree in each Archive and aggregate value errors by kind.
- Independently write and parse every PTU Record as Record XML in memory.
- Run format canaries on both DataForges: header words, pointer layout, property conversion flags,
  struct layouts and Record trailing-size conventions.
- Survey floating-point values in every Record, including primitive array elements.
- Produce complete typed catalogs from both Archives, then inspect existing compressed output for
  field-presence regressions, selected joins and kind/stat-block counts.
- Run a targeted source-versus-export recovery regression and the existing PTU catalog/crafting tests.
- Extract, verify and parse the loose planet/environment descriptor families privately, then
  check their declared reference relationships.

The executable was built with `cargo build --release --locked --offline -p p4kx`.
All scratch builds, probes, logs and extracted files stayed in the task-specific Codex folder
outside the repository. No game Installation was written to.

## Verification results

| Check | Result |
| --- | --- |
| Sampled Archive reads across available compression/encryption states | Passed |
| Format canaries on LIVE and PTU | Passed |
| Exhaustive Record-tree construction on both Archives | Passed |
| Exhaustive PTU Record XML writing/parsing | Passed |
| Floating-point survey including arrays | No NaN or infinity found |
| Pure crafting-data reader | Passed |
| Complete PTU catalog verification | Failed on the known unsupported `MissionPropertyValue_ScopedLocation` |
| Recovery source-versus-typed-export regression | Reproduced silent omission; filtered catalog itself exits successfully |
| Selected loose planet/environment descriptor verification | CRC verification and JSON parsing passed |

Reproduction uses the existing ignored tests:

~~~powershell
cargo test --release --locked --offline -p p4kx-archive --test real_archive -- reads_entries_of_every_compression_and_encryption_state --ignored --nocapture
cargo test --release --locked --offline -p p4kx-extraction --test real_archive -- dataforge_canary --ignored --nocapture
cargo test --release --locked --offline -p p4kx-extraction --test real_archive -- builds_and_writes_every_record_of_a_real_archive --ignored --nocapture
cargo test --release --locked --offline -p p4kx-export --test real_archive -- reads_the_crafting_data_of_a_real_archive --ignored --nocapture
~~~

Point `P4KX_DATA_P4K` at the PTU Archive for those checks. The two-Archive canary also takes
`P4KX_DATA_P4K_B`. For the catalog's second-Archive verification, set primary LIVE and secondary PTU:

~~~powershell
cargo test --release --locked --offline -p p4kx-export --test real_archive -- the_catalog_of_a_second_real_archive_is_complete_and_plausible --ignored --nocapture
~~~

Apply the configured user-provided Archive key through the existing profile/testkit mechanism;
never copy it into a command, log, fixture or document.

## Confirmed export failure: nested recovery data

The generic reader exposes populated `ItemRecoveryEconomyParams.cooldownParams`,
`deliveryParams` and `repairParams`. The current
[recovery reader](../crates/p4kx-export/src/recovery.rs#L82) still reads the corresponding
legacy scalars directly from `economyParams`. The typed catalog omits all three new structures
and emits null for the absent legacy source fields, without diagnosing the populated omission.

- `ItemRecoveryCooldownParams` owns `baseTime`, `scalingPriceFloor`, `aUECperSecond` and
  `defaultLoadoutMultiplier`.
- `ItemRecoveryDeliveryTimeParams` owns the same field names plus `expeditedTimeMultiplier`
  and `expeditedDefaultTimeMultiplier`.
- `ItemRecoveryRepairParams` owns the chassis/component/missing-item repair cost multipliers.
- The source also removes the override member `flatMinimumDeliveryTimerSecondsMultiplier`.

Preserve the independent owners of overlapping names. A base time or rate in one block is not
automatically an alias for a removed flat field; no inverse-rate or timer formula was established.
Support legacy and nested layouts with synthetic fixtures and an explicit compatibility decision.

Tracking: [#1080](https://github.com/p4kx-org/p4kx/issues/1080).

A local red-capable check ran `dcb fields` for the source and a filtered `catalog` for
`libs/foundry/records/entitlementpolicies/*`, then asserted that populated source groups must
appear in the typed output. The assertion failed although the catalog command returned success.

## Known and masked mission-mapping failures

[#1047](https://github.com/p4kx-org/p4kx/issues/1047) is reproduced by the fresh executable.
Generic Record construction succeeds; the typed property mapping rejects `ScopedLocation`.
The missing target templates also cause otherwise retained contract references to point outside
the delivered catalog. This reinforces the artifact-health work of
[#1049](https://github.com/p4kx-org/p4kx/issues/1049).

The fix must preserve all six optional, polymorphic filter families: faction, location class,
size, function, other location and relative location. They include required/excluded alternatives,
singular/plural References, owner-relative weak links and scope enums. A marker-only variant
or a template-only sample showing nulls is insufficient evidence for omitting those fields.

Additional populated structures are unsupported by the current strict unions:

| Structure | Stored shape | Status and tracking |
| --- | --- | --- |
| `MissionFlowAction_MultipleActions` | Ordered action pointers | Code-confirmed missing branch; masked end-to-end by the earlier property failure; [#1081](https://github.com/p4kx-org/p4kx/issues/1081) |
| `MissionFlowAction_PickRandomAction` | Entries containing an action pointer and raw probability | Same evidence level; [#1081](https://github.com/p4kx-org/p4kx/issues/1081) |
| `MissionPropertyValue_AutoSpawns` | AutoSpawns Reference and typed closet-search options | Code-confirmed missing branch with populated tag options; [#1082](https://github.com/p4kx-org/p4kx/issues/1082) |

These are source-confirmed support gaps, not claimed post-fix end-to-end reproductions.
The issues require independent synthetic fixtures containing no `ScopedLocation`, strict
recursive handling and previous-schema validator checks. A new tagged-union branch is not
necessarily accepted by an old validator merely because ordinary added object fields are.

## Additional data to export

| Source family | Required export work |
| --- | --- |
| `AutoSpawn`/`AutoSpawns`, `EncounterIndex`, `NPCArchetype`/`NPCClassPool` | Preserve direct/archetype/pool alternatives, ordered weights, assignments, limits, identity and encounter links; [#1084](https://github.com/p4kx-org/p4kx/issues/1084) |
| `FactionIdentity` | Independent identity with Faction, Tag, security, encounter and NPC relationships; [#1085](https://github.com/p4kx-org/p4kx/issues/1085) |
| Scope and location vocabulary | ContractScopeGlobalParams, class/function/size dictionaries and registries, local generator/board configuration and scenario prerequisites; [#1086](https://github.com/p4kx-org/p4kx/issues/1086) |
| Extra loose environment formats | Geology sets/texture sets, soils/soil texture sets, shading classes and ecosystems; [#1083](https://github.com/p4kx-org/p4kx/issues/1083), reusing [#848](https://github.com/p4kx-org/p4kx/issues/848)/[#849](https://github.com/p4kx-org/p4kx/issues/849) |
| Port/item alignment declarations | Preserve `attachAxesToAlign`, item `ignoredAttachAxis` and `attachCheckCenterOfMass` independently; no rendering defect inferred; [#1087](https://github.com/p4kx-org/p4kx/issues/1087) |
| `StarMapPreset` | Full type, visibility, display, QT, location and radar definitions; added to [#1067](https://github.com/p4kx-org/p4kx/issues/1067) |
| Thermal action inputs | `SProjectileLauncher.heatCost` and `SWeaponActionFireBeamParams.heatCostPerSecond` remain distinct from legacy heat fields; added to [#1052](https://github.com/p4kx-org/p4kx/issues/1052) |
| Resource-network destruction parameters | Preserve declared damage-propagation flags, destruction buffer and threshold with source ownership; added to [#1059](https://github.com/p4kx-org/p4kx/issues/1059) |
| Mission declarations and presentation | Populated module/spawn declarations, board assets, reputation descriptions/banners/icons; added to [#1074](https://github.com/p4kx-org/p4kx/issues/1074) and [#1073](https://github.com/p4kx-org/p4kx/issues/1073) |

The loose descriptor checks established declared geology-to-asset, geology-to-texture-set and
soil-to-texture-set links. Missing targets within one loose family are retained, not treated as
proof of a parser defect. Ecosystem texture/audio fields expose source paths and configuration;
they do not by themselves establish a body-to-biome distribution or texture-channel meaning.

## Changes that are not established new bugs

- Missing-target DataForge References occur in both Archives. They are distinct from the format
  canary's invalid physical pointers, and do not alone demonstrate a parsing regression.
- The shared empty shop inventory warning also occurs on LIVE; it is not a PTU-specific failure.
- Loadout links to entities outside the selected catalog must be distinguished from missing
  source Records and selection exclusions; not every unresolved catalog join is a new bug.
- `CraftingResearch` envelopes exist, but `researchCosts` and `unlockRequirements` remain empty.
  The declared bases are `CraftingRecipeCosts_Base` and `CraftingResearchUnlock_Base`.
  [#1076](https://github.com/p4kx-org/p4kx/issues/1076) now states these known bases and empty-state
  requirements; this audit did not discover populated research payloads.
- Quality-location overrides now declare a polymorphic base target, but the observed concrete
  location entries remain supported. The resource-override family is declared without an
  observed populated Record; account for it as future schema coverage, not reproduced loss.
- New crafter base-time fields are already read by the current implementation.
- `SFlashlightParams.lightCookie` changes its declared resource target from material to texture.
  Generic Record reading handles it; no specialized rendering failure was demonstrated.
- Direct schema changes and propagated layout-size/order changes are different census measures.
  Their differing totals are not evidence that the schema comparator missed a format error.

## Boundaries and follow-through

The audit covers all directory metadata and all DataForge Records, with focused typed-export
and small descriptor-format checks. It does not checksum/decode every asset in the whole Archive
or validate every model, texture, animation, audio event or rendered screen against the running game.
Binary planet data, every reverse reference, texture semantics and the full Starchitect placement
graph were not exhausted; existing [#844](https://github.com/p4kx-org/p4kx/issues/844) and
[#845](https://github.com/p4kx-org/p4kx/issues/845) remain relevant.

No implementation fix was applied during the investigation. The eight new follow-ups and the
refined existing issues are collected under
[#1048](https://github.com/p4kx-org/p4kx/issues/1048), milestone
[v0.5.0](https://github.com/p4kx-org/p4kx/milestone/17). Every new issue contains source seams,
structural requirements, synthetic regression cases and export/schema completion criteria.
