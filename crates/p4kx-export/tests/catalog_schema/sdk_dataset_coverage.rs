// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::{JsonSchema, Schema};
use serde_json::{Value, json};

use super::super::schema_navigation::{resolve, schema_at};
use super::{Entry, Owner, Shape, TestResult, dataset_owners};

#[path = "sdk_dataset_routes.rs"]
mod routes;
#[path = "sdk_row_schema.rs"]
mod row_schema;

const LANGUAGE_SOURCES: &str = "/texts/languageSources";
const ITEM_DEFINITIONS: &str = "/definitions/items";
const LOADOUTS: &str = "/loadouts";
const ITEM_PORTS: &str = "/configurations/itemPorts";
const ATTACHMENT_CONSTRAINTS: &str = "/configurations/itemAttachmentConstraints";
const ITEM_BOUNDS: &str = "/configurations/itemBounds";
const ENTITY_TAGS: &str = "/declarations/entityTags";
const TAG_DEFINITIONS: &str = "/definitions/tags";
const TAG_ROOTS: &str = "/declarations/tagRoots";
const TAG_PATHS: &str = "/projections/tagPaths";
const INVENTORY_CONTAINERS: &str = "/definitions/inventoryContainers";
const INVENTORY_CONFIGURATIONS: &str = "/configurations/inventoryConfigurations";
const FACILITIES: &str = "/configurations/facilities";
const MEDICAL_ITEM_TIER_CONFIGS: &str = "/definitions/medicalItemTierConfigs";
const MANUFACTURERS: &str = "/definitions/manufacturers";
const FACTION_IDENTITIES: &str = "/definitions/factionIdentities";
const FACTION_REPUTATIONS: &str = "/definitions/factionReputations";
const REPUTATION_SCOPES: &str = "/definitions/reputationScopes";
const REPUTATION_STANDINGS: &str = "/definitions/reputationStandings";
const REPUTATION_REWARD_AMOUNTS: &str = "/definitions/reputationRewardAmounts";
const COMMODITY_DEFINITIONS: &str = "/definitions/commodities";
const COMMODITY_PRESENTATIONS: &str = "/projections/commodityPresentation";
const SHOP_INVENTORIES: &str = "/definitions/shopInventories";
const FLARE_LIBRARIES: &str = "/definitions/flareLibraries";
const SUBSUMPTION_EVENTS: &str = "/definitions/subsumptionEvents";
const CLIENT_PROFILE_DOCUMENTS: &str = "/configurations/clientProfileSettings/documents";
const RADAR_AUDIO: &str = "/configurations/radarAudio";
const AMMUNITION: &str = "/definitions/ammoDefinitions";
const AMMO_BINDINGS: &str = "/declarations/ammoBindings";
const AMMO_CONTAINERS: &str = "/configurations/ammoContainers";
const COUNTERMEASURE_EMITTERS: &str = "/configurations/countermeasureEmitters";
const REPUTATION_TRIGGER_CONFIGURATIONS: &str = "/configurations/reputationTriggers";
const WEAPON_ACTIONS: &str = "/configurations/weaponActions";
const LOCATION_ENTITIES: &str = "/definitions/locationEntityDeclarations";
const MISSION_LOCATION_TEMPLATES: &str = "/missionLocationTemplates";
const MISSION_TYPES: &str = "/definitions/missionTypes";
const BLUEPRINT_POOLS: &str = "/definitions/blueprintPools";
const BLUEPRINT_POOL_WEIGHTS: &str = "/projections/blueprintPoolWeights";
const BLUEPRINT_CATEGORY_LISTS: &str = "/declarations/blueprintCategoryLists";
const BLUEPRINT_CATEGORIES: &str = "/definitions/blueprintCategories";
const GAMEPLAY_PROPERTIES: &str = "/definitions/gameplayProperties";
const BLUEPRINT_REWARDS: &str = "/declarations/blueprintRewardSources";
const LOCAL_CONTRACT_GENERATORS: &str = "/configurations/localContractGenerators";
const CONTRACT_SCENARIO_REQUIREMENTS: &str = "/declarations/contractScenarioRequirements";
const BLUEPRINT_DEFINITIONS: &str = "/definitions/blueprints";
const BLUEPRINT_TIERS: &str = "/configurations/blueprintTiers";
const BLUEPRINT_PRESENTATION: &str = "/projections/blueprintPresentation";
const CRAFTING_GLOBAL: &str = "/configurations/craftingGlobal";
const QUALITY_DISTRIBUTIONS: &str = "/definitions/qualityDistributions";
const QUALITY_QUANTIZATIONS: &str = "/definitions/qualityQuantizations";
const QUALITY_QUANTIZATION_RESOURCES: &str = "/projections/qualityQuantizationResources";
const SUIT_ARMOR: &str = "/configurations/suitArmor";
const VEHICLE_ARMOR: &str = "/configurations/vehicleArmor";
const LOOT_GENERATION: &str = "/configurations/lootGeneration";
const RESOURCE_GROUPS: &str = "/definitions/resourceGroups";
const RESOURCE_GROUP_MEMBERS: &str = "/declarations/resourceGroupMembers";
const RESOURCE_GROUP_PARENTS: &str = "/projections/resourceGroupParents";
const RESOURCE_TYPES: &str = "/definitions/resourceTypes";
const RESOURCE_COMPONENTS: &str = "/configurations/resourceComponents";
const RESOURCE_CONNECTIONS: &str = "/configurations/resourceConnections";
const FUEL_TANKS: &str = "/configurations/fuelTanks";
const FUEL_INTAKES: &str = "/configurations/fuelIntakes";
const RESOURCE_CONTAINERS: &str = "/configurations/resourceContainers";
const CARGO_MANIFESTS: &str = "/definitions/cargoManifests";
const CARGO_LOADING_GLOBALS: &str = "/configurations/cargoLoadingGlobals";
const SHOP_COMMODITY_GLOBALS: &str = "/configurations/shopCommodityGlobals";
const SHIELD_GENERATORS: &str = "/configurations/shieldGenerators";
const QUANTUM_DRIVES: &str = "/configurations/quantumDrives";
const CLOTHING: &str = "/configurations/clothing";
const ITEM_RADARS: &str = "/configurations/itemRadars";
const REPUTATION_CONTEXTS: &str = "/definitions/reputationContexts";
const REPUTATION_PERKS: &str = "/definitions/reputationPerks";
const REPUTATION_TRIGGER_RECORDS: &str = "/definitions/reputationTriggerRecords";
const MINEABLE_ENTITIES: &str = "/definitions/mineableEntities";
const MINEABLE_ELEMENTS: &str = "/mineableElements";
const MINEABLE_COMPOSITIONS: &str = "/mineableCompositions";
const REFINING_PROCESSES: &str = "/definitions/refiningProcesses";
const MINING_GLOBALS: &str = "/configurations/miningGlobal";
const HARVESTABLE_PROVIDER_PRESETS: &str = "/harvestableProviderPresets";
const MINING_LASERS: &str = "/configurations/miningLasers";
const ATTACHABLE_MODIFIERS: &str = "/configurations/attachableModifiers";
const CONSUMABLE_CONFIGURATIONS: &str = "/configurations/consumables";
const PROJECTILE_AMMUNITION: &str = "/projections/weaponProjectileAmmoSelections";
const MAGAZINE_RESOLUTION: &str = "/projections/weaponMagazineResolutions";
const WEAPON_COMBAT: &str = "/projections/weaponCombatBases";
const WEAPON_RECOIL: &str = "/definitions/weaponActionRecoilDefinitions";
const WEAPON_ANIMATION: &str = "/projections/weaponPlayerAnimationCandidates";
const CONTAINERS: &str = "/placements/containerInstances";
const LOCATION_PHYSICS_GRIDS: &str = "/placements/locationPhysicsGrids";
const LOOT_DOCUMENTS: &str = "/declarations/lootEntityContainerDocuments";
const UNRESOLVED: &str = "/placements/unresolvedContainerLinks";
const NO_WITNESS: &str = "no query witness recorded by the audit";
const EXPORT_ATTEMPT_FIELDS: &[&str] = &["/schemaVersion", "/completeness", "/exportHealth"];

struct Route {
    dataset: &'static str,
    schema: Value,
    remaining: Option<&'static str>,
}

impl Route {
    fn queried_value<Item: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        value: &Item,
        remaining: &'static str,
    ) -> TestResult<Self> {
        Self::queried_rows(dataset, std::slice::from_ref(value), remaining)
    }

    fn queried_rows<Row: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        rows: &[Row],
        remaining: impl Into<Option<&'static str>>,
    ) -> TestResult<Self> {
        if rows.is_empty() {
            return Err(format!("SDK witness returned no row for {dataset}").into());
        }
        Ok(Self {
            dataset,
            schema: schemars::generate::SchemaSettings::draft2020_12()
                .for_serialize()
                .with_transform(row_schema::remove_descriptions)
                .into_generator()
                .into_root_schema_for::<Vec<Row>>()
                .to_value(),
            remaining: remaining.into(),
        })
    }
}

const REQUIRED_WITNESSES: &[&str] = &[
    LANGUAGE_SOURCES,
    ITEM_DEFINITIONS,
    LOADOUTS,
    ITEM_PORTS,
    ATTACHMENT_CONSTRAINTS,
    ITEM_BOUNDS,
    ENTITY_TAGS,
    TAG_DEFINITIONS,
    TAG_ROOTS,
    TAG_PATHS,
    INVENTORY_CONTAINERS,
    INVENTORY_CONFIGURATIONS,
    FACILITIES,
    MEDICAL_ITEM_TIER_CONFIGS,
    MANUFACTURERS,
    FACTION_IDENTITIES,
    FACTION_REPUTATIONS,
    REPUTATION_SCOPES,
    REPUTATION_STANDINGS,
    REPUTATION_REWARD_AMOUNTS,
    COMMODITY_DEFINITIONS,
    COMMODITY_PRESENTATIONS,
    LOCATION_ENTITIES,
    BLUEPRINT_POOLS,
    BLUEPRINT_POOL_WEIGHTS,
    BLUEPRINT_CATEGORY_LISTS,
    BLUEPRINT_CATEGORIES,
    GAMEPLAY_PROPERTIES,
    BLUEPRINT_REWARDS,
    LOCAL_CONTRACT_GENERATORS,
    CONTRACT_SCENARIO_REQUIREMENTS,
    BLUEPRINT_DEFINITIONS,
    BLUEPRINT_TIERS,
    BLUEPRINT_PRESENTATION,
    CRAFTING_GLOBAL,
    QUALITY_DISTRIBUTIONS,
    QUALITY_QUANTIZATIONS,
    QUALITY_QUANTIZATION_RESOURCES,
    SUIT_ARMOR,
    VEHICLE_ARMOR,
    LOOT_GENERATION,
    RESOURCE_GROUPS,
    RESOURCE_GROUP_MEMBERS,
    RESOURCE_GROUP_PARENTS,
    RESOURCE_TYPES,
    RESOURCE_COMPONENTS,
    RESOURCE_CONNECTIONS,
    FUEL_TANKS,
    FUEL_INTAKES,
    RESOURCE_CONTAINERS,
    CARGO_MANIFESTS,
    routes::dismantle::DATASET,
    CARGO_LOADING_GLOBALS,
    SHOP_COMMODITY_GLOBALS,
    SHIELD_GENERATORS,
    QUANTUM_DRIVES,
    CLOTHING,
    ITEM_RADARS,
    MINEABLE_ENTITIES,
    MINEABLE_ELEMENTS,
    MINEABLE_COMPOSITIONS,
    REFINING_PROCESSES,
    MINING_GLOBALS,
    HARVESTABLE_PROVIDER_PRESETS,
    REPUTATION_CONTEXTS,
    REPUTATION_PERKS,
    REPUTATION_TRIGGER_RECORDS,
    MINING_LASERS,
    ATTACHABLE_MODIFIERS,
    CONSUMABLE_CONFIGURATIONS,
    SHOP_INVENTORIES,
    FLARE_LIBRARIES,
    SUBSUMPTION_EVENTS,
    CLIENT_PROFILE_DOCUMENTS,
    RADAR_AUDIO,
    AMMUNITION,
    AMMO_BINDINGS,
    AMMO_CONTAINERS,
    COUNTERMEASURE_EMITTERS,
    REPUTATION_TRIGGER_CONFIGURATIONS,
    WEAPON_ACTIONS,
    PROJECTILE_AMMUNITION,
    MAGAZINE_RESOLUTION,
    WEAPON_COMBAT,
    WEAPON_RECOIL,
    WEAPON_ANIMATION,
    CONTAINERS,
    LOCATION_PHYSICS_GRIDS,
    LOOT_DOCUMENTS,
    UNRESOLVED,
];

#[test]
fn sdk_query_witnesses_keep_their_canonical_dataset_contract() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let registered = dataset_owners::entries();
    let witnesses = routes::read()?;
    let gaps = audit(schema, &registered, &witnesses)?;
    for path in REQUIRED_WITNESSES
        .iter()
        .copied()
        .chain(routes::loot_datasets())
        .chain(routes::contract_datasets())
        .chain(routes::law_datasets())
        .chain(routes::mission_datasets())
    {
        assert_ne!(
            gaps.get(path),
            Some(&NO_WITNESS),
            "missing typed SDK row for {path}"
        );
    }
    Ok(())
}

#[test]
fn acknowledged_new_dataset_still_needs_a_query() -> TestResult {
    let mut schema = p4kx_export::catalog_schema();
    let mut registered = dataset_owners::entries();
    let properties = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .ok_or("catalog root properties")?;
    properties.insert("fixtureRows".to_owned(), json!({"type":"array","items":{}}));
    let gaps = audit(schema.clone(), &registered, &[]);
    assert!(
        gaps.is_err(),
        "a dataset without ownership fails the strategy audit"
    );
    registered.insert(
        "/fixtureRows".to_owned(),
        Entry {
            owner: Owner::RootDataset,
            shape: Shape::Array,
        },
    );
    let gaps = audit(schema, &registered, &[])?;
    assert_eq!(
        gaps.get("/fixtureRows"),
        Some(&NO_WITNESS),
        "assigning ownership does not establish an SDK route"
    );
    Ok(())
}

#[test]
fn a_public_query_cannot_discharge_a_different_dataset_or_value_shape() -> TestResult {
    let mut schema = p4kx_export::catalog_schema();
    let registered = dataset_owners::entries();
    let route = routes::ammunition()?;
    let incorrect = Route {
        dataset: "/definitions/inventoryContainers",
        ..route
    };
    assert!(
        audit(schema.clone(), &registered, &[incorrect]).is_err(),
        "a different physical row type cannot satisfy a table-shaped route"
    );
    let float = schema
        .pointer_mut("/$defs/ResourceFloat32/properties/finite")
        .ok_or("referenced numeric field schema")?;
    *float = json!({"type":"string"});
    assert!(
        audit(schema, &registered, &[routes::ammunition()?]).is_err(),
        "a changed reachable field shape cannot use the old query witness"
    );
    Ok(())
}

#[test]
#[ignore = "strict SDK dataset gate: run explicitly to list outstanding routes"]
fn every_catalog_v3_dataset_has_a_verified_sdk_query() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let gaps = audit(schema, &dataset_owners::entries(), &routes::read()?)?;
    if gaps.is_empty() {
        return Ok(());
    }
    Err(CoverageGaps(gaps).into())
}

struct CoverageGaps(BTreeMap<String, &'static str>);

impl fmt::Display for CoverageGaps {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(output, "{} unfinished SDK dataset routes", self.0.len())?;
        for (path, reason) in &self.0 {
            writeln!(output, "{path} {reason}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for CoverageGaps {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, output)
    }
}

impl std::error::Error for CoverageGaps {}

fn audit(
    schema: Value,
    registered: &BTreeMap<String, Entry>,
    witnesses: &[Route],
) -> TestResult<BTreeMap<String, &'static str>> {
    let drift = super::compare(&schema, registered)?;
    if !drift.is_empty() {
        return Err(format!("catalog ownership drift: {drift:?}").into());
    }
    let mut schema = Schema::try_from(schema)?;
    row_schema::remove_descriptions(&mut schema);
    let schema = schema.to_value();
    let mut gaps: BTreeMap<_, _> = registered
        .keys()
        .filter(|path| !EXPORT_ATTEMPT_FIELDS.contains(&path.as_str()))
        .map(|path| (path.clone(), NO_WITNESS))
        .collect();
    let mut seen = BTreeSet::new();
    for route in witnesses {
        let entry = registered
            .get(route.dataset)
            .ok_or_else(|| format!("SDK query claims an unknown dataset {}", route.dataset))?;
        if !seen.insert(route.dataset) || EXPORT_ATTEMPT_FIELDS.contains(&route.dataset) {
            return Err(format!("invalid SDK route claim for {}", route.dataset).into());
        }
        let catalog_row = canonical_value(&schema, route.dataset, entry.shape)?;
        let query_row = resolve(
            &route.schema,
            route.schema.get("items").ok_or("SDK row list")?,
        )?;
        row_schema::compare(&schema, catalog_row, &route.schema, query_row)
            .map_err(|error| format!("SDK route {}: {error}", route.dataset))?;
        match route.remaining {
            Some(reason) => {
                gaps.insert(route.dataset.to_owned(), reason);
            }
            None => {
                gaps.remove(route.dataset);
            }
        }
    }
    Ok(gaps)
}

fn canonical_value<'s>(schema: &'s Value, dataset: &str, shape: Shape) -> TestResult<&'s Value> {
    let path = match shape {
        Shape::Table => format!("{dataset}/rows/[]"),
        Shape::Array => format!("{dataset}/[]"),
        Shape::Object | Shape::Scalar => return schema_at(schema, dataset),
    };
    schema_at(schema, &path)
}
