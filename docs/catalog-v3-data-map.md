# Catalog v3 data-family map

This maps the selected target contract in [ADR-0356](adr/0356-catalog-v3-uses-canonical-source-typed-tables.md).
It is a coverage inventory, not a claim that the current serializer implements every target field.
D=definitions, C=configurations, A=declarations, P=placements, V=projections, T=texts. Tables are
specific typed datasets with explicit source scope, availability, identity, selection and measured
health. Seed field lists never exclude further useful declared source facts found by a ticket's survey.
[ADR-0357](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md) requires the origin
marker directly on every source-backed data value, including nested values and language variants.

The inventory contains all 50 existing Kinds and all 51 catalog-lane tickets at the decision baseline,
including the completed #816/#846/#1011. Independent media, package and platform tasks keep their
own formats and acceptance gates. Declined #269/#284 stay closed and do not create output dialects.
## Every open catalog issue

| Issue | Owning contract location | Facts/cardinality/scope kept |
|---|---|---|
| #844 | P.containerInstances, P.starmapPlacements | XML/Child/entxml/HCD declared traversal; reusable tile instances; local/world transforms, source chain and limits; existing LIVE placement convention |
| #845 | P.locationInstances; A.harvestableBindings | sector/cluster/location tree, type, parent/children, source labels/keys/tags, local/world placement, InstanceMetadata host links; binding only if proved, otherwise explicit bounded finding |
| #847 | D.lootThemes; C.lootSecondaryChoices; C.lootContexts | theme links in subconfigs/constraints; complete secondary layers/entries/order/weights/conditions; HCD hierarchy/override and category order; no effective drop chance |
| #848 | D.planetResources | Entry GUID/name/bMinable/tintPalette and source Entry, additional declared fields, exact missing/null/zero/invalid states; owns shared bounded JSON/GUID/path reader |
| #849 | D.climateAssets, D.geologyAssets, D.creatures; A.planetAssetBindings | promotion/entity/resource/shading links; climate envelope and subasset order; Mineable_cfg_path; actual fauna definitions; proved per-body assignment or explicit notEstablished finding |
| #1041 | T.publicNames, T.gameTexts + language source diagnostics | key-by-key oracle and independent resolver regression; missing language/key/placeholder separate; no lowered global threshold |
| #1044 | D.commodities, D.resourceTypes; V.commodityPresentation | Commodity GUID identity; shared nullable ResourceType link; resource-only fallback; first selected-group/refined/name facts; physical target provenance; Build-bound observed counts |
| #1047 | C.missionProperties[].value.scopedLocation | faction/class/size/function/other/relative filter families, positive/negative singular/plural typed variants, raw enum and owner-local weak links; no chosen location |
| #1048 | All matrix locations + integrated inventory | domain tracker; full source inventory/serialization and final lane coverage; no separate runtime dump |
| #1049 | exportHealth, completeness, sources, SourceValue, Link | measured source-vs-row counters, exact failed identities, source/Entry operations, strict accounting, normal gate before create, diagnostic mode/cancellation/write safety |
| #1050 | Development-only source map/reference tests | declared/present/populated/exported/unhandled; exact Build offline comparison, primitive arrays, silent omission/unknown union/empty target; no runtime export mode |
| #1051 | D.ammunition; C.ammoContainers; A.ammoBindings | primary/secondary/magazine roles, count/restock/flags, Bullet/Tachyon and complete projectile/physics/electron/falloff/penetration/pierceability/explosion/conversion fields; one definition, many bindings |
| #1052 | C.weaponConfigurations.actionTree | sequence/parallel/charged/conditional/default/leaf structure and entry metadata; full modifiers/spin/burst/charge, launcher selector, beam/medical/salvage/gathering, effects/state/audio/tags; separate heat inputs |
| #1053 | D.weaponConfigurations, D.curves; A.actionConfigurationLinks | source recoil/aim/reload configs by identity, stance/state/modifiers/ordered curve knots; genuine event/fragment/timing references; action-node links; no reload score |
| #1054 | C.shieldConfigurations | original five fields, four reserve ratios, electrical resistance, stun, all ordered indexed resistance/absorption slots, state/effect metadata; no guessed slot semantics |
| #1055 | C.quantumDriveConfigurations | distinct standard/spline parameter sets and full calibration/state/effect/VFX, heat/thermal-energy and boost fields; legacy flat values map only to standard |
| #1056 | C.missileConfigurations; D.launchZoneConfigurations | launch/lifetime/safety, targeting/signal/emissions/proximity, boost/intercept/terminal GCS phases/PID/vectors, launch-zone links; shared damage/explosion with #1051 |
| #1057 | C.inventoryConfigurations, C.itemBounds, C.interactions | occupancy/local bounds/UI conditional dimension overrides; InventoryContainer capacity/restrictions; declared interactions/conditions; definition-space, no hull volume |
| #1058 | C.durability, C.degradation, C.distortion | health-only available independently, full health/caps/resistance/repair/salvage/destruction/propagation/degradation/distortion; owning SHealth reader shared |
| #1059 | C.resourceComponents, C.resourceConnections, C.resourceNetworkParameters | flags/priority/repair, states/signatures, paired consumption-generation deltas, named unit members, power ranges/constraints/self-repair and actual power/heat connection fields; separately owned global shipPassDestroyedPartDamageToComponent, shipPassDestroyedPartDamageToChild, shipDestructionBufferMultiplier and shipDestructionHealthThreshold |
| #1060 | C.flightControllers, C.ifcs, C.thrusters; A.installedConfigurations | both authored/new and legacy motion/afterburner; limits/profiles/jerk/curves/fuel/response/efficiency; invisible resolved loadout links; child vehicle scope separate |
| #1061 | C.fuelTanks, C.fuelIntakes, C.resourceContainers | all flow multipliers/rates, typed capacity, resource/composition/quality/fill/mutability; repeated tank instances via placement; no cargo-unit conflation |
| #1062 | C.vehicleArmor | all damage/signature/penetration/deflection + curve links; suit Armor and clothing separate; Health/resource readers reused |
| #1063 | C.facilities; P.facilityInstances | seat/eject/bed/med/rack/storage definition facts and per-ship context placement; reuse #1057 inventory; count repeated definitions separately, no inferred minimum crew |
| #1064 | A.resourceNetworkLinks; P.resourceNetworkNodes; A.roomAssociations | all declared InternalHardpointLinks including repeats/cycles/non-relay links; exact context endpoints, ambiguity; isRelay evidence, declared fuse restrictions, room/bone associations; no name heuristics |
| #1065 | D.tags with authored child arrays; A.tagRoots, A.entityTags; V.tagPaths | original root/child edges/multiparents/repeats/unlisted/legacy IDs; physical duplicate GUIDs; actual selected-occurrence path provenance; Entity GUID tags distinct from AttachDef words/Record tags; complete taxonomy scope ([contract](catalog-tags.md)) |
| #1066 | D.factions | actual behavior/reaction/legal/arrest flags and direct description; links to reputation; never merges FactionIdentity/Reputation |
| #1067 | D.starmapObjects, D.starmapTypes, D.starmapPresets, D.starmapAmenities | all visibility/travel/loading/scanning/body-recovery/type/nav/radar/asset facts; jurisdiction/Faction/amenity links; full preset owned blocks, existing placement separate |
| #1068 | C.jumpPointConfigurations; P.jumpEndpoints; A.jumpConnections | every stored operating/global/override parameter, placed endpoint identity, declared partner link/evidence/direction; unpaired/ambiguous explicit, no name-based pairing |
| #1069 | D.resourceTypes; C.resourceContainers | named cargo packaging slots/media/validation/volatility/properties/quality and actual links; no second ResourceType model; MineableElement multiplicity unchanged |
| #1070 | D.shopInventories; A.shopInventoryResolution | both source JSON shapes, unlinked/placeholder files, ShopID evidence, ordered duplicate lines/all GUID alternatives, price/stock/rental raw units; per-shop resolved/placeholder/ambiguous/missing/unreadable/unscanned/noDeclaredFile |
| #1071 | C.consumables; D.consumableTypes, D.consumableSubtypes; typed buff links | ordered composition/ratios, typed volume/container flags, typed status/health/buff/resource/gas effects and own links; no totals/dosage |
| #1072 | D.jurisdictions, D.infractions; C.penaltyRules | one canonical jurisdiction reader, fines/severity/thresholds/timers/prison/exemptions/conditions/scopes/ordered overrides, legal source links; no evaluated outcome |
| #1073 | D.factionReputations, D.reputationStandings/scopes/contexts; D.reputationPerks, C.reputationTriggers | complete perks/unlocks/rewards/conditions/display/headquarters/founding/leadership/focus/assets; exact String vs Locale; no fabricated reward |
| #1074 | C.locationData, C.missionModules, C.missionLimits; D.securityManifests | exact modules/limits/autoSpawnSettings, location-qualified declarations, moduleDeclaration.spawnedEntities, security graph and faction inheritance; independent of current enabled/matched state |
| #1075 | D.missionItems; A.missionTokenInputs; V.missionTitles | all static definition/candidate/tag/link/config data; debug selections separate; only unambiguous non-debug static expansion, original key/token/unresolved reason retained |
| #1076 | C.blueprintTiers[].research | absent block/present-empty/present-known/unsupported/unreadable; known CraftingRecipeCosts_Base/CraftingResearchUnlock_Base bases, no invented concrete production payloads |
| #1077 | C.emp, C.quantumInterdiction, C.jumpDrives | all device modes/timing/state/trigger/resource fields, jammer/pulse distinct, jump-drive tuning/tunnel forces; separate quantum-drive/world endpoint owners |
| #1078 | C.countermeasureEmitters, C.explosives, C.hackingChips | source signature profiles, capacity/restock, arming/fuse/sensors/triggers; damage/physics shared with Ammo; ordered duplicate chip values/tags/modifiers |
| #1079 | T.publicNames, T.gameTexts, T.languageSources | all available supported languages once, exact keys, resolution policy/per-kind/per-key/per-language states and counters, separate public/protected reference domains with x-gameText closure; canonical maps replace duplicate old wire fields |
| #1080 | C.itemRecoveryEconomy | distinct legacy direct-source and nested cooldown/delivery/repair fields, absent removed override stays absent; no aliases/baseTime formula |
| #1081 | C.missionFlows[].actions | typed recursive MultipleActions and PickRandomAction, ordered entries/raw probability/ownerlocal weak links, strict unknown failure/depth/node cancellation |
| #1082 | C.missionProperties[].value.autoSpawns | AutoSpawns target + Tags or schema-backed EntityDeclaration closet options, positive/negative lists, no legacy NPCSpawnDescriptions flattening |
| #1083 | D.geologySets, D.geologyTextureSets, D.soils, D.soilTextureSets, D.shadingClasses, D.ecosystems | all listed layer/scalar/texture/audio blocks and declared graph, Entry identity/path; empty RTPC/switch arrays only establish presence/count, populated unknown diagnosed |
| #1084 | D.autoSpawns, D.autoSpawnSets, D.encounterIndices, D.npcArchetypes, D.npcClassPools | direct/archetype/pool classes, manual/reference sets, ordered identity branches/weights/delays/assignments/inventory/tags, all declared targets; no chosen NPC |
| #1085 | D.factionIdentities | name/description/factionTag/Faction/security links, ordered encounter weights and theme tags, multiple identities per Faction; separate identity namespace |
| #1086 | D.contractScopes, D.locationClasses/functions/sizes, D.locationRegistries; C.missionBoards, C.localContractGenerators | all seven scope members/text/icon, zero-field dictionaries, registry order, source scope/positive-negative DNF/identity list; board variants, inherited required scenarios |
| #1087 | C.itemPorts, C.itemAttachmentConstraints; P.loadoutItems evidence | port/item attachAxesToAlign independent, ignoredAttachAxis/COM check, original enum/missing/false and owner location; unchanged placement/noRotation behavior |

D.weaponConfigurations above is the reusable referenced recoil/aim/reload definition family; C.weaponConfigurations is the owned weapon component/action tree. They are distinct source owners despite related names and should get clearer concrete Rust names (WeaponConfigDefinition vs WeaponComponentConfiguration). The final field survey decides exact concrete structs, not this naming coincidence.

## Every existing catalog data family

Source inventory is current contract.rs:69-181 and generated schema's top required list. No existing family disappears merely because its issue is closed. Mappings below describe its canonical owner, not blanket copying of old aggregates.

| Existing root field(s) | Proposed owning contract | Preserved distinction |
|---|---|---|
| manufacturers | D.manufacturers | Sourced raw header/code and owned Localization states; canonical public Name/protected Description; physical target-row joins |
| items | D.items | Entity identity, AttachDef classification/word tags, source text; component links |
| ships | D.ships; V.shipBaseFacts | Entity identity/playability/role/career; existing mass/hull/cargo/size/crew projections remain defined, raw configs separate |
| commodities | D.commodities; V.commodityPresentation | Entity vs resource fallback, GUID identity, existing first Resource/group/name/density meaning |
| resourceTypes | D.resourceTypes | complete resource source fact owner |
| resourceGroups | D.resourceGroups; A.resourceGroupMembers; V.resourceGroupParents | all direct ordered memberships, repeats/nulls/cycles; first-selected parent retains actual physical/slot provenance ([contract](catalog-resource-groups.md)) |
| blueprints | D.blueprints; C.blueprintTiers | tiers, first-tier ingredients, costs/quality/outputs, research separate |
| blueprintCategories | D.blueprintCategories | category rules and identity |
| gameplayProperties | D.gameplayProperties | quality-property targets, no guessed stat link |
| qualityDistributions | D.qualityDistributions | quality curves/location overrides, inline vs referenced wrappers |
| qualityQuantizations | D.qualityQuantizations | ordered bands/steps and ResourceType references |
| blueprintPools | D.blueprintPools; A.blueprintRewardSources | reward definitions and actual targeted-source relations |
| dismantleProcesses | D.dismantleProcesses | costs/results, original units/links |
| craftingGlobal | C.craftingGlobal | true global parameters, explicit presence/owner |
| paints | D.paints; A.paintShipCompatibility | item identity plus fit relationship |
| loadouts | C.itemPorts; P.loadoutItems | source definitions/restrictions vs resolved default attachments/full path/parent and merge evidence |
| starmapObjects | D.starmapObjects; P.starmapPlacements | identity/navigation definition vs physical placement/world projection |
| contracts | V.contracts; A.contractPropertySetLinks; V.missionTitles | fanout identity/subcontract order, resolved title/standing/reward/fact precedence; original declarations not duplicated per row |
| mineableElements | D.mineableElements | element-local instability/resistance; many per ResourceType |
| refiningProcesses | D.refiningProcesses | stored refining relationships/parameters |
| mineableCompositions | D.mineableCompositions | ordered elements/ranges and source links |
| miningGlobalParams | C.miningGlobal | all current/v5 values, including completed #846 compositionMassMultiplier |
| harvestableProviderPresets | D.harvestableProviderPresets | provider groups/areas/elements/loot references |
| subHarvestableMultiConfigs | D.subHarvestableMultiConfigs | ordered slots/configs/themes; not current server contents |
| harvestableLocations | P.harvestableLocations | original preset/container/system/object relation, contextual identity |
| lootContainers | P.lootContainers | original config/container/system/object relation, contextual identity |
| shops | D.shops; A.shopInventoryResolution | registry identity/type/brand/place, conservative all-sources-agree resolution |
| factionReputations | D.factionReputations | distinct from Faction/FactionIdentity |
| reputationScopes | D.reputationScopes | standing membership/order/configuration |
| reputationStandings | D.reputationStandings | rank/source thresholds, not player state |
| reputationContexts | D.reputationContexts | display scopes/relations |
| missionTypes | D.missionTypes | mission-type definitions and source text |
| missionLocalities | D.missionLocalities | stored starmap-object membership |
| contractScenarios | D.contractScenarios | faction reward tiers/progression, source flags, no active state |
| lootTables | D.lootTables | weighted archetype links/order |
| lootArchetypes | D.lootArchetypes | weighted typed selectors/constraints/order |
| lootPoolFilters | D.lootPoolFilters | complete typed filter trees |
| legacyLootTables | D.legacyLootTables | actual distinct source generation, not compatibility duplicate of LootV3 |
| legacyLootArchetypes | D.legacyLootArchetypes | source generation/secondary/spawn-with relationships |
| mineableEntities | D.mineableEntities | scan signature and entity identity |
| crafters | D.crafters | station limits/modifiers/queues, identity |
| cargoManifests | D.cargoManifests | stored cargo fill/spawn configuration, not runtime cargo |
| missionLocationTemplates | D.missionLocationTemplates; C.locationData | completed #816 remains; actual configuration distinct from placed location |
| contractShipSpawns | V.shipSpawnPools; A.spawnOccurrences | existing content identity/projection and authored occurrences separated |
| contractLocationQueries | V.locationQueries; A.locationQueryOccurrences | strict typed source condition tree (completed #1011), content identity, no query execution |
| contractTemplates | D.contractTemplates; C.missionProperties/missionFlows | parts/objectives/property values, typed structure and source order |
| missionBrokerEntries | D.missionBrokerEntries | source strict union/parts/identity/text/provenance |
| contractNpcSpawns | V.npcSpawnPools; A.spawnOccurrences | legacy content description remains distinct from new independent NPC graph |
| contractEntitySpawns | V.entitySpawnPools; A.spawnOccurrences | source/content identities, no active spawned Entity claim |
| missionPropertySets | A.missionPropertySets; A.contractPropertySetLinks | exact owner/location hash, all source declarations and selected Generator scope |
| shipStoreSlugs | V.shipStoreSlugs | explicitly curated source, slugs/retired mapping, not Archive-authored |
| itemRecovery | C.itemRecoveryEconomy; C.itemRecoveryOverrides | true global/override source generations, no nested/legacy alias |
| LocalisedText nested everywhere | source Locale/String at owner + T.publicNames/T.gameTexts | all languages and existing normalization rules without permanent duplicates |
| ItemStats specialist blocks | family C tables | Weapon/Shield/Quantum/PowerPlant/Cooler/Missile/SuitArmor/Clothing/Radar/Tractor/WeaponModifier/MiningLaser/MiningModifier; exact old values move, no loss |
| ItemStats shared Health/ResourceStates | C.durability/C.resourceComponents | independently available, old gate not transferred |

## Noncatalog inventory boundaries

Catalog does not absorb the remaining media/audio/archive/platform/render/screen packaging tasks merely because it shares sources. #1088-1114, #827/#1019/#1022-1024/#1030/#1037/#1038/#1040, #627/#668/#775/#979/#972/#993-995/#1006/#1090/#1091 and #1103-1108 remain their actual lane. Archive metadata may feed sources/health only through the owning source Interface; native/runtime work belongs to Root. #1115/#1116 coordinate integration/release, not new catalog payloads. Completed #1118/#1121/#1123 Archive-key disclosure is retained where applicable; no key material enters catalog. Declined #269/#284 create no compatibility format requirement.
