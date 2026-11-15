// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::Kind;
use serde_json::Value;

use super::{CoverageCounters, TestResult, TextDomain, TextIndex, TextReferenceShape};
use crate::catalog_v3_consumer;

#[path = "real_archive_contract_type_texts.rs"]
mod mission_type;

pub(super) fn count(
    catalog: &Value,
    index: &TextIndex<'_>,
    contracts: &[Value],
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    mission_type::count(catalog, contracts, index, counters)?;
    let mut objects = HashMap::<&str, Vec<&Value>>::new();
    for row in catalog_v3_consumer::rows(catalog, Kind::StarmapObjects)? {
        let guid = row
            .pointer("/guid/value")
            .and_then(Value::as_str)
            .ok_or("canonical starmap object GUID")?;
        objects
            .entry(guid)
            .or_default()
            .push(row.get("name").ok_or("canonical starmap name cell")?);
    }
    for contract in contracts {
        count_faction_names(contract, index, counters)?;
        let contractor = contract.get("contractor").ok_or("contractor text cell")?;
        counters.count(index.reference(
            contractor,
            TextDomain::GameText,
            TextReferenceShape::Required,
        )?)?;
        for place in places(contract)? {
            let guid = place
                .get("starmapObjectGuid")
                .and_then(Value::as_str)
                .ok_or("contract starmap place GUID")?;
            let name = place.get("place").ok_or("contract place name cell")?;
            match objects.get(guid) {
                None if name.is_null() => {}
                Some(names) if names.contains(&name) => {
                    let target = index.reference(
                        name,
                        TextDomain::GameText,
                        TextReferenceShape::Required,
                    )?;
                    counters.count(target.filter(|target| !target.key.is_empty()))?;
                }
                _ => return Err("contract place name differs from the corresponding object".into()),
            }
        }
    }
    Ok(())
}

fn count_faction_names(
    contract: &Value,
    index: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    for faction in [
        contract.get("faction"),
        contract.pointer("/giverFaction/faction"),
    ]
    .into_iter()
    .flatten()
    .filter(|faction| !faction.is_null())
    {
        let name = faction
            .get("displayName")
            .ok_or("faction display-name cell")?;
        if !name.is_null() {
            counters.count(index.reference(
                name,
                TextDomain::GameText,
                TextReferenceShape::Required,
            )?)?;
        }
    }
    Ok(())
}

fn places(contract: &Value) -> TestResult<impl Iterator<Item = &Value>> {
    let pickup = contract
        .get("pickupPlaces")
        .and_then(Value::as_array)
        .ok_or("contract pickup places")?;
    let offered = contract
        .pointer("/offeredAt/locations")
        .and_then(Value::as_array)
        .ok_or("contract offered locations")?;
    Ok(pickup.iter().chain(offered))
}
