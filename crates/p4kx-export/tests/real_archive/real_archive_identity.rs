// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::error::Error;

use p4kx_export::Kind;
use serde_json::Value;

use super::{TestResult, definition_identity, list};
use crate::catalog_v3_consumer;

/// Checks declared family identities and the physical `ResourceType` fallback.
pub(crate) fn check_unique_guids(catalog: &Value) -> TestResult {
    check_family_aliases(catalog, Kind::BlueprintCategoryLists)?;
    check_family_aliases(catalog, Kind::GameplayProperties)?;
    let mut seen: HashMap<&str, (Kind, &Value)> = HashMap::new();
    check_contract_order(catalog)?;
    let kinds = Kind::ALL
        .into_iter()
        .filter(|kind| !kind.is_derived() && !matches!(kind, Kind::Contracts | Kind::Crafters));
    for kind in kinds {
        if kind == Kind::BlueprintCategoryLists {
            definition_identity::check(catalog, kind)?;
        }
        if matches!(
            kind,
            Kind::BlueprintCategories
                | Kind::GameplayProperties
                | Kind::QualityDistributions
                | Kind::QualityQuantizations
        ) {
            definition_identity::check(catalog, kind)?;
            continue;
        }
        let mut family = HashSet::new();
        for entity in list(catalog, kind)? {
            let guid = catalog_v3_consumer::identity(entity, kind)?;
            if !family.insert(guid) && kind != Kind::BlueprintCategoryLists {
                return Err(format!("{guid} occurs twice inside {kind}").into());
            }
            if let Some((previous_kind, previous)) = seen.get(guid) {
                let same_list = kind == Kind::BlueprintCategoryLists && *previous_kind == kind;
                if !same_list && !same_resource_fallback(*previous_kind, previous, kind, entity)? {
                    return Err(format!("{guid} occurs twice, also in {kind}").into());
                }
            } else {
                seen.insert(guid, (kind, entity));
            }
        }
    }
    Ok(())
}

fn check_contract_order(catalog: &Value) -> TestResult {
    let identities: Vec<(&str, &str)> = list(catalog, Kind::Contracts)?
        .iter()
        .map(|entry| {
            let guid = catalog_v3_consumer::identity(entry, Kind::Contracts)?;
            let cell = entry
                .get("subContractGuid")
                .ok_or("contract sub-identity carrier")?;
            let value = catalog_v3_consumer::source_value(cell)?;
            let sub = match (cell.get("state").and_then(Value::as_str), value) {
                (Some("present"), Some(Value::String(sub))) => sub.as_str(),
                (Some("absent"), None) => "",
                _ => return Err("contract sub-identity must be present or absent".into()),
            };
            Ok((guid, sub))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    assert!(
        identities.is_sorted_by(|a, b| a < b),
        "contracts strictly ordered by guid and subContractGuid, none twice"
    );
    Ok(())
}

fn same_resource_fallback(
    left_kind: Kind,
    left: &Value,
    right_kind: Kind,
    right: &Value,
) -> Result<bool, Box<dyn Error>> {
    let (commodity, resource) = match (left_kind, right_kind) {
        (Kind::Commodities, Kind::ResourceTypes) => (left, right),
        (Kind::ResourceTypes, Kind::Commodities) => (right, left),
        _ => return Ok(false),
    };
    if catalog_v3_consumer::field(commodity, Kind::Commodities, "selection")?.as_str()
        != Some("resourceFallback")
        || catalog_v3_consumer::field(commodity, Kind::Commodities, "sourceType")?.as_str()
            != Some("ResourceType")
    {
        return Ok(false);
    }
    let commodity_guid = commodity.get("guid").ok_or("Commodity GUID carrier")?;
    let resource_guid = resource.get("guid").ok_or("ResourceType GUID carrier")?;
    let references = commodity_guid
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("original GUID uses")?;
    Ok(!references.is_empty()
        && commodity_guid.get("sourceUsesRefs") == resource_guid.get("sourceUsesRefs")
        && commodity_guid
            .get("sourceSetComplete")
            .and_then(Value::as_bool)
            == Some(true)
        && resource_guid
            .get("sourceSetComplete")
            .and_then(Value::as_bool)
            == Some(true)
        && catalog_v3_consumer::field(commodity, Kind::Commodities, "recordPath")?
            == catalog_v3_consumer::field(resource, Kind::ResourceTypes, "recordPath")?)
}

fn check_family_aliases(catalog: &Value, owner: Kind) -> TestResult {
    let family_guids: HashSet<_> = list(catalog, owner)?
        .iter()
        .map(|row| catalog_v3_consumer::identity(row, owner))
        .collect::<Result<_, _>>()?;
    if family_guids.is_empty() {
        return Ok(());
    }
    for kind in Kind::ALL.into_iter().filter(|kind| {
        !kind.is_derived() && !matches!(kind, Kind::Contracts | Kind::Crafters) && *kind != owner
    }) {
        for row in list(catalog, kind)? {
            if family_guids.contains(catalog_v3_consumer::identity(row, kind)?) {
                return Err(format!("a {owner} GUID also occurs in {kind}").into());
            }
        }
    }
    Ok(())
}
