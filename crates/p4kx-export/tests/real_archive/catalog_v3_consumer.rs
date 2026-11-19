// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::HashSet;
use std::error::Error;

use p4kx_export::Kind;
use p4kx_extraction::Guid;
use serde_json::Value;

#[path = "catalog_v3_category_lists.rs"]
mod category_lists;

#[path = "catalog_v3_gameplay_properties.rs"]
pub(crate) mod gameplay_properties;

#[path = "catalog_v3_named_tables.rs"]
mod named_tables;

#[path = "catalog_v3_source_values.rs"]
mod source_values;

use named_tables::uses_named_definition_table;
use source_values::record_header;
pub(crate) use source_values::{nullable_guid, present, source_value};

type TestResult<T> = Result<T, Box<dyn Error>>;

/// Reads the one actual dataset for a Kind.
pub(crate) fn rows(catalog: &Value, kind: Kind) -> TestResult<&Vec<Value>> {
    named_rows(catalog, kind.name())
}

/// Resolves the existing Kind vocabulary before reading its actual canonical rows.
pub(crate) fn named_rows<'catalog>(
    catalog: &'catalog Value,
    name: &str,
) -> TestResult<&'catalog Vec<Value>> {
    let kind = Kind::ALL
        .into_iter()
        .find(|kind| kind.name() == name)
        .ok_or("unknown catalog Kind")?;
    rows_of_kind(catalog, kind)
}

fn rows_of_kind(catalog: &Value, kind: Kind) -> TestResult<&Vec<Value>> {
    if kind == Kind::Commodities {
        presentations(catalog)?;
        return table(catalog, &row_location(kind));
    }
    let path = row_location(kind);
    if kind == Kind::ClientProfileSettings {
        return catalog
            .pointer(&path)
            .and_then(Value::as_array)
            .ok_or_else(|| "client profile input dispositions are missing".into());
    }
    if path.starts_with('/') {
        return checked_table(catalog, kind, &path);
    }
    let rows = catalog
        .get(path.as_ref())
        .and_then(Value::as_array)
        .ok_or_else(|| format!("required {} rows are missing", kind.name()))?;
    if matches!(kind, Kind::Items | Kind::Ships) {
        for row in rows {
            identity(row, kind)?;
            record_header(row)?;
        }
    }
    Ok(rows)
}

/// Returns a plain top-level field or the JSON pointer of the canonical table that owns a Kind.
pub(crate) fn row_location(kind: Kind) -> Cow<'static, str> {
    if uses_named_definition_table(kind) {
        return Cow::Owned(format!("/definitions/{}", kind.name()));
    }
    let path = match kind {
        Kind::GameplayProperties => "/definitions/gameplayProperties",
        Kind::RefiningProcesses => "/definitions/refiningProcesses",
        Kind::MiningGlobalParams => "/configurations/miningGlobal",
        Kind::BlueprintCategoryLists => "/declarations/blueprintCategoryLists",
        Kind::Blueprints => "/definitions/blueprints",
        Kind::ShopInventories => "/definitions/shopInventories",
        Kind::ParticleLibraries => "/definitions/particleLibraries",
        Kind::ClientProfileSettings => "/configurations/clientProfileSettings/documents",
        Kind::ExplosionDefinitions => "/definitions/explosionDefinitions",
        Kind::AmmoDefinitions => "/definitions/ammoDefinitions",
        Kind::MissionScenarios => "/definitions/missionScenarios",
        Kind::MissionModuleDeclarations => "/definitions/missionModuleDeclarations",
        Kind::ReputationContexts => "/definitions/reputationContexts",
        Kind::ReputationScopes => "/definitions/reputationScopes",
        Kind::ReputationStandings => "/definitions/reputationStandings",
        Kind::Contracts => "/projections/contracts",
        Kind::ContractTemplates => "/definitions/contractTemplates",
        Kind::MissionBrokerEntries => "/definitions/missionBrokerEntries",
        Kind::BlueprintCategories => "/definitions/blueprintCategories",
        Kind::Manufacturers => "/definitions/manufacturers",
        Kind::Factions => "/definitions/factions",
        Kind::FactionIdentities => "/definitions/factionIdentities",
        Kind::FactionReputations => "/definitions/factionReputations",
        Kind::ReputationPerks => "/definitions/reputationPerks",
        Kind::ReputationTriggerRecords => "/definitions/reputationTriggerRecords",
        Kind::ReputationRewardAmounts => "/definitions/reputationRewardAmounts",
        Kind::Infractions => "/definitions/infractions",
        Kind::InfractionSets => "/definitions/infractionSets",
        Kind::Jurisdictions => "/definitions/jurisdictions",
        Kind::SecurityNetworkManifests => "/definitions/securityNetworkManifests",
        Kind::LocationClasses => "/definitions/locationClasses",
        Kind::LocationFunctions => "/definitions/locationFunctions",
        Kind::LocationSizes => "/definitions/locationSizes",
        Kind::LocationResourceSlots => "/definitions/locationResourceSlots",
        Kind::LocationEntityDeclarations => "/definitions/locationEntityDeclarations",
        Kind::LocationClassRegistries => "/definitions/locationRegistries/classes",
        Kind::LocationFunctionRegistries => "/definitions/locationRegistries/functions",
        Kind::LocationSizeRegistries => "/definitions/locationRegistries/sizes",
        Kind::QualityDistributions => "/definitions/qualityDistributions",
        Kind::QualityQuantizations => "/definitions/qualityQuantizations",
        Kind::Tags => "/definitions/tags",
        Kind::ResourceTypes => "/definitions/resourceTypes",
        Kind::ResourceGroups => "/definitions/resourceGroups",
        Kind::StarmapTypes => "/definitions/starmapTypes",
        Kind::StarmapPresets => "/definitions/starmapPresets",
        Kind::StarmapAmenities => "/definitions/starmapAmenities",
        Kind::StarmapObjects => "/definitions/starmapObjects",
        Kind::Commodities => "/definitions/commodities",
        Kind::CargoLoadingGlobals => "/configurations/cargoLoadingGlobals",
        Kind::ShopCommodityGlobals => "/configurations/shopCommodityGlobals",
        _ => kind.name(),
    };
    Cow::Borrowed(path)
}

fn checked_table<'catalog>(
    catalog: &'catalog Value,
    kind: Kind,
    path: &str,
) -> TestResult<&'catalog Vec<Value>> {
    let rows = table(catalog, path)?;
    for row in rows {
        identity(row, kind)?;
        if has_record_header(kind) {
            record_header(row)?;
        }
        if kind == Kind::BlueprintCategoryLists {
            category_lists::check(row)?;
        }
        if kind == Kind::GameplayProperties {
            gameplay_properties::check(row)?;
        }
    }
    Ok(rows)
}

pub(crate) fn table<'catalog>(
    catalog: &'catalog Value,
    path: &str,
) -> TestResult<&'catalog Vec<Value>> {
    let table = catalog
        .pointer(path)
        .ok_or_else(|| format!("required canonical table {path}"))?;
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("required canonical rows")?;
    match table.get("availability").and_then(Value::as_str) {
        Some("available") => Ok(rows),
        Some("absent") if rows.is_empty() => Ok(rows),
        Some("unavailable") => Err(format!("canonical table {path} is unavailable").into()),
        _ => Err(format!("invalid canonical availability/rows at {path}").into()),
    }
}

/// Reads a source-backed field on canonical definitions and an owned field elsewhere.
pub(crate) fn field<'row>(row: &'row Value, kind: Kind, name: &str) -> TestResult<&'row Value> {
    let value = row
        .get(name)
        .ok_or_else(|| format!("required {kind} field {name}"))?;
    let sourced = match kind {
        Kind::Contracts => name == "guid",
        Kind::Items => item_source_field(name),
        Kind::Loadouts => matches!(name, "entityGuid" | "entityKind" | "sourceRecordIndex"),
        Kind::Ships => matches!(
            name,
            "guid"
                | "recordName"
                | "className"
                | "recordPath"
                | "recordIndex"
                | "sourceType"
                | "fileName"
                | "metadataTag"
                | "crewSize"
                | "size"
                | "vehicleDefinition"
                | "insurance"
        ),
        _ => all_fields_sourced(kind),
    };
    if sourced { present(value) } else { Ok(value) }
}

fn item_source_field(name: &str) -> bool {
    matches!(
        name,
        "guid"
            | "recordName"
            | "className"
            | "recordPath"
            | "recordIndex"
            | "sourceType"
            | "fileName"
            | "metadataTag"
            | "itemClass"
            | "playability"
    )
}

const fn all_fields_sourced(kind: Kind) -> bool {
    uses_named_definition_table(kind)
        || matches!(
            kind,
            Kind::Blueprints
                | Kind::ContractTemplates
                | Kind::MissionBrokerEntries
                | Kind::BlueprintCategories
                | Kind::BlueprintCategoryLists
                | Kind::ExplosionDefinitions
                | Kind::AmmoDefinitions
                | Kind::ReputationContexts
                | Kind::ReputationScopes
                | Kind::ReputationStandings
                | Kind::GameplayProperties
                | Kind::Manufacturers
                | Kind::Factions
                | Kind::FactionIdentities
                | Kind::FactionReputations
                | Kind::ReputationTriggerRecords
                | Kind::ReputationRewardAmounts
                | Kind::ReputationPerks
                | Kind::Infractions
                | Kind::InfractionSets
                | Kind::Jurisdictions
                | Kind::SecurityNetworkManifests
                | Kind::MissionModuleDeclarations
                | Kind::MissionLocationTemplates
                | Kind::LocationClasses
                | Kind::LocationFunctions
                | Kind::LocationSizes
                | Kind::LocationResourceSlots
                | Kind::LocationEntityDeclarations
                | Kind::LocationClassRegistries
                | Kind::LocationFunctionRegistries
                | Kind::LocationSizeRegistries
                | Kind::StarmapObjects
                | Kind::StarmapTypes
                | Kind::StarmapPresets
                | Kind::StarmapAmenities
                | Kind::Commodities
                | Kind::ResourceTypes
                | Kind::ResourceGroups
                | Kind::Tags
                | Kind::RefiningProcesses
                | Kind::MiningGlobalParams
                | Kind::QualityDistributions
                | Kind::QualityQuantizations
                | Kind::CargoLoadingGlobals
                | Kind::ShopCommodityGlobals
        )
}

/// Returns a row's original identity, checking canonical UUID spelling where it is a GUID.
pub(crate) fn identity(row: &Value, kind: Kind) -> TestResult<&str> {
    let name = match kind {
        Kind::Paints => "itemGuid",
        Kind::Loadouts => "entityGuid",
        Kind::Shops => "key",
        Kind::ShopInventories => "inputRef",
        _ => "guid",
    };
    let text = field(row, kind, name)?
        .as_str()
        .ok_or("required row identity string")?;
    if !matches!(kind, Kind::Shops | Kind::ShopInventories) {
        let parsed: Guid = text.parse()?;
        if parsed.to_string() != text {
            return Err("row GUID must use canonical lowercase spelling".into());
        }
    }
    Ok(text)
}

/// Requires exactly paired original Commodity identities without deduplicating alternatives.
pub(crate) fn presentations(catalog: &Value) -> TestResult<&Vec<Value>> {
    let definitions = table(catalog, "/definitions/commodities")?;
    let rows = table(catalog, "/projections/commodityPresentation")?;
    let mut expected = HashSet::new();
    for row in definitions {
        let id = identity(row, Kind::Commodities)?;
        if !expected.insert(id) {
            return Err("duplicate Commodity definition identity".into());
        }
    }
    let mut actual = HashSet::new();
    for row in rows {
        let id = nullable_guid(
            row.get("commodityGuid")
                .ok_or("presentation identity carrier")?,
        )?
        .ok_or("presentation Commodity GUID cannot be null")?;
        if !actual.insert(id) {
            return Err("duplicate Commodity presentation identity".into());
        }
        nullable_guid(row.get("subtypeGuid").ok_or("nullable subtype carrier")?)?;
    }
    if expected != actual {
        return Err("Commodity definition/presentation identity closure differs".into());
    }
    Ok(rows)
}

const fn has_record_header(kind: Kind) -> bool {
    uses_named_definition_table(kind)
        || matches!(
            kind,
            Kind::Blueprints
                | Kind::ContractTemplates
                | Kind::MissionBrokerEntries
                | Kind::BlueprintCategories
                | Kind::BlueprintCategoryLists
                | Kind::ExplosionDefinitions
                | Kind::AmmoDefinitions
                | Kind::InventoryContainers
                | Kind::MedicalItemTierConfigs
                | Kind::ReputationContexts
                | Kind::ReputationScopes
                | Kind::ReputationStandings
                | Kind::GameplayProperties
                | Kind::RefiningProcesses
                | Kind::MiningGlobalParams
                | Kind::Factions
                | Kind::FactionIdentities
                | Kind::FactionReputations
                | Kind::ReputationTriggerRecords
                | Kind::ReputationRewardAmounts
                | Kind::ReputationPerks
                | Kind::Infractions
                | Kind::InfractionSets
                | Kind::Jurisdictions
                | Kind::SecurityNetworkManifests
                | Kind::MissionModuleDeclarations
                | Kind::MissionLocationTemplates
                | Kind::LocationClasses
                | Kind::LocationFunctions
                | Kind::LocationSizes
                | Kind::LocationResourceSlots
                | Kind::LocationEntityDeclarations
                | Kind::LocationClassRegistries
                | Kind::LocationFunctionRegistries
                | Kind::LocationSizeRegistries
                | Kind::StarmapObjects
                | Kind::StarmapTypes
                | Kind::StarmapPresets
                | Kind::StarmapAmenities
                | Kind::CargoLoadingGlobals
                | Kind::ShopCommodityGlobals
        )
}
