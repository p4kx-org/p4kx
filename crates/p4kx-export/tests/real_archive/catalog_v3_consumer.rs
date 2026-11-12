// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;

use p4kx_export::Kind;
use p4kx_extraction::Guid;
use serde_json::Value;

#[path = "catalog_v3_category_lists.rs"]
mod category_lists;

#[path = "catalog_v3_gameplay_properties.rs"]
pub(crate) mod gameplay_properties;

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
    match kind {
        Kind::GameplayProperties => checked_table(catalog, kind, "/definitions/gameplayProperties"),
        Kind::LootPoolFilters => checked_table(catalog, kind, "/definitions/lootPoolFilters"),
        Kind::BlueprintCategoryLists => {
            checked_table(catalog, kind, "/declarations/blueprintCategoryLists")
        }
        Kind::Blueprints => checked_table(catalog, kind, "/definitions/blueprints"),
        Kind::ShopInventories => checked_table(catalog, kind, "/definitions/shopInventories"),
        Kind::ExplosionDefinitions => {
            checked_table(catalog, kind, "/definitions/explosionDefinitions")
        }
        Kind::AmmoDefinitions => checked_table(catalog, kind, "/definitions/ammoDefinitions"),
        Kind::ReputationContexts => checked_table(catalog, kind, "/definitions/reputationContexts"),
        Kind::ReputationScopes => checked_table(catalog, kind, "/definitions/reputationScopes"),
        Kind::ReputationStandings => {
            checked_table(catalog, kind, "/definitions/reputationStandings")
        }
        Kind::Contracts => checked_table(catalog, kind, "/projections/contracts"),
        Kind::ContractTemplates => checked_table(catalog, kind, "/definitions/contractTemplates"),
        Kind::MissionBrokerEntries => {
            checked_table(catalog, kind, "/definitions/missionBrokerEntries")
        }
        Kind::BlueprintCategories => {
            checked_table(catalog, kind, "/definitions/blueprintCategories")
        }
        Kind::Manufacturers => checked_table(catalog, kind, "/definitions/manufacturers"),
        Kind::Factions => checked_table(catalog, kind, "/definitions/factions"),
        Kind::FactionReputations => checked_table(catalog, kind, "/definitions/factionReputations"),
        Kind::ReputationPerks => checked_table(catalog, kind, "/definitions/reputationPerks"),
        Kind::ReputationTriggerRecords => {
            checked_table(catalog, kind, "/definitions/reputationTriggerRecords")
        }
        Kind::ReputationRewardAmounts => {
            checked_table(catalog, kind, "/definitions/reputationRewardAmounts")
        }
        Kind::QualityDistributions => {
            checked_table(catalog, kind, "/definitions/qualityDistributions")
        }
        Kind::QualityQuantizations => {
            checked_table(catalog, kind, "/definitions/qualityQuantizations")
        }
        Kind::Tags => checked_table(catalog, kind, "/definitions/tags"),
        Kind::ResourceTypes => checked_table(catalog, kind, "/definitions/resourceTypes"),
        Kind::ResourceGroups => checked_table(catalog, kind, "/definitions/resourceGroups"),
        Kind::Commodities => {
            presentations(catalog)?;
            table(catalog, "/definitions/commodities")
        }
        _ => catalog
            .get(kind.name())
            .and_then(Value::as_array)
            .ok_or_else(|| format!("required {} rows are missing", kind.name()).into()),
    }
}

fn checked_table<'catalog>(
    catalog: &'catalog Value,
    kind: Kind,
    path: &str,
) -> TestResult<&'catalog Vec<Value>> {
    let rows = table(catalog, path)?;
    for row in rows {
        identity(row, kind)?;
        if matches!(
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
                | Kind::LootPoolFilters
                | Kind::Factions
                | Kind::FactionReputations
                | Kind::ReputationTriggerRecords
                | Kind::ReputationRewardAmounts
                | Kind::ReputationPerks
        ) {
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

fn table<'catalog>(catalog: &'catalog Value, path: &str) -> TestResult<&'catalog Vec<Value>> {
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

/// Validates immediate state/value/origin metadata before reading a typed caller's payload.
pub(crate) fn source_value(carrier: &Value) -> TestResult<Option<&Value>> {
    origin(carrier)?;
    let value = carrier.get("value").ok_or("required source value")?;
    match carrier.get("state").and_then(Value::as_str) {
        Some("present") => Ok(Some(value)),
        Some("absent" | "null" | "unreadable" | "unsupported" | "invalid") if value.is_null() => {
            Ok(None)
        }
        _ => Err("invalid source state/value correlation".into()),
    }
}

fn origin(carrier: &Value) -> TestResult<()> {
    let complete = carrier
        .get("sourceSetComplete")
        .and_then(Value::as_bool)
        .ok_or("source-set completeness")?;
    let marker = carrier
        .get("containsEncryptedSource")
        .ok_or("direct source marker")?;
    if !marker.is_null() && !marker.is_boolean() {
        return Err("invalid direct source marker".into());
    }
    if marker.as_bool() == Some(false) && !complete {
        return Err("false source marker requires complete contributors".into());
    }
    let references = carrier
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("required source-use references")?;
    let empty_marker = if complete { Some(false) } else { None };
    if references.is_empty() && marker.as_bool() != empty_marker {
        return Err("empty contributor origin has an inconsistent marker".into());
    }
    let mut seen = HashSet::new();
    for reference in references {
        let reference = reference
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or("source-use reference string")?;
        if !seen.insert(reference) {
            return Err("duplicate immediate source-use reference".into());
        }
    }
    Ok(())
}

/// Requires an actual present source result, including nullable derived choices.
pub(crate) fn present(carrier: &Value) -> TestResult<&Value> {
    source_value(carrier)?.ok_or_else(|| "required present source result".into())
}

/// Reads a source-backed field on canonical definitions and an owned field elsewhere.
pub(crate) fn field<'row>(row: &'row Value, kind: Kind, name: &str) -> TestResult<&'row Value> {
    let value = row
        .get(name)
        .ok_or_else(|| format!("required {kind} field {name}"))?;
    if kind == Kind::Contracts && name == "guid" {
        return present(value);
    }
    if matches!(
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
            | Kind::FactionReputations
            | Kind::ReputationTriggerRecords
            | Kind::ReputationRewardAmounts
            | Kind::ReputationPerks
            | Kind::Commodities
            | Kind::ResourceTypes
            | Kind::ResourceGroups
            | Kind::Tags
            | Kind::QualityDistributions
            | Kind::QualityQuantizations
            | Kind::LootPoolFilters
    ) {
        present(value)
    } else {
        Ok(value)
    }
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

/// Retains a present nullable GUID choice without turning missing/wrong states into null.
pub(crate) fn nullable_guid(carrier: &Value) -> TestResult<Option<&str>> {
    match present(carrier)? {
        Value::Null => Ok(None),
        Value::String(text) => {
            let parsed: Guid = text.parse()?;
            if parsed.to_string() != *text {
                return Err("nullable GUID must be canonical lowercase text".into());
            }
            Ok(Some(text))
        }
        _ => Err("nullable GUID choice must be a string or null".into()),
    }
}

fn record_header(row: &Value) -> TestResult<()> {
    for name in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "fileName",
        "metadataTag",
    ] {
        let value = source_value(row.get(name).ok_or("required category header carrier")?)?;
        if let Some(value) = value {
            let valid = if name == "recordIndex" {
                value
                    .as_u64()
                    .is_some_and(|index| u32::try_from(index).is_ok())
            } else {
                value.is_string()
            };
            if !valid {
                return Err("Record header source payload type".into());
            }
        }
    }
    Ok(())
}
