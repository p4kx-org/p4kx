# Installed test-channel compatibility audit

Research date: 2026-11-06. Source baseline:
[1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).

The installed PTU Archive was compared with the installed LIVE Archive using a fresh build
of the current checkout. This report contains technical structures and implementation findings
only. Game values, text dumps, concrete test-build Record identities, build payloads and
extracted assets are not included. Public technical issue updates were explicitly authorized.

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
FactionIdentity families. Preserve separate checks for Contract-level scope attribution, a
runtime or authored PTU reputation multiplier, and actual source mapping of lock UI states.

The current DataForge declaration exposes the Bool `ContractResult_CompletionTags.splitPointsForParty`
and the Int32 `LocationResourceSlot.numberOfResourceSlotsPerEntity`. The LIVE declaration of
the latter holds a `resourceSlotTag` Reference, and the exact type count in the compared DataForges
changes from 3 to 66. Source controls, target identities and older authored members need coverage
under [#1145](https://github.com/p4kx-org/p4kx/issues/1145).
`AmmoParams.impactAIStimulusDistance` already has its own catalog cell.
The current PTU declaration trace resolves the added `allowFPSPingWave` Bool through the
owned scalar `SStandardParams.radarOptionParams` Class. Resource slots, Mode radar options
and completion-tag outcomes have follow-ups in
[#1146](https://github.com/p4kx-org/p4kx/issues/1146),
[#1147](https://github.com/p4kx-org/p4kx/issues/1147) and
[#1148](https://github.com/p4kx-org/p4kx/issues/1148), respectively.

The changed GFx and SWF Entries each parse successfully in both installations. Both files
change the container version from 17 to 15 while keeping the same frame count and the same
unread tag inventory. The exported symbols and frame labels do not identify the missile-lock
interface. A file-type match by itself cannot attribute this change to the announced UI.

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

The remaining per-domain source probes and any exporter follow-ups are tracked in
[#1145](https://github.com/p4kx-org/p4kx/issues/1145).
The current tuning test and other patch-note items come from the
[CIG notes](https://robertsspaceindustries.com/spectrum/community/SC/forum/190048/thread/star-citizen-alpha-5-0-ptu-patch-notes/9230254)
and separate
[TTK update](https://robertsspaceindustries.com/spectrum/community/SC/forum/190048/thread/star-citizen-alpha-5-0-ptu-patch-notes/9230300).

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
cargo test --release --locked --offline -p p4kx-export --test real_archive -- the_catalog_of_a_second_real_archive_is_complete_plausible_and_within_the_limit --ignored --nocapture
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
