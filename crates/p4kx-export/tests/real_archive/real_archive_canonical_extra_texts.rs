// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

use super::{CoverageCounters, TestResult, TextDomain, TextIndex, TextReferenceShape};
use crate::catalog_v3_consumer;

pub(super) fn compare(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    kind: Kind,
    row: &Value,
) -> TestResult<()> {
    let (field, domain) = match kind {
        Kind::ResourceGroups | Kind::ReputationScopes => ("description", TextDomain::GameText),
        Kind::ReputationStandings => ("perkDescription", TextDomain::GameText),
        Kind::GameplayProperties => ("unitFormat", TextDomain::PublicName),
        Kind::FactionReputations => return compare_faction_description(counters, index, row),
        _ => return Ok(()),
    };
    let target = index.reference(
        row.get(field).ok_or("authored text carrier")?,
        domain,
        TextReferenceShape::Required,
    )?;
    counters.count_supplemental(target)?;
    if kind == Kind::GameplayProperties {
        compare_overrides(counters, index, row)?;
    }
    Ok(())
}

fn compare_faction_description(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    row: &Value,
) -> TestResult<()> {
    let carrier = row
        .get("description")
        .ok_or("faction description carrier")?;
    let Some(description) = catalog_v3_consumer::source_value(carrier)? else {
        return Ok(());
    };
    match description.get("type").and_then(Value::as_str) {
        Some("localized") => {
            let id = description
                .get("reference")
                .and_then(Value::as_str)
                .ok_or("faction description reference")?;
            let target = index.target(id, TextDomain::GameText)?;
            counters.count_supplemental(Some(target))
        }
        Some("literal") if description.get("text").is_some_and(Value::is_string) => Ok(()),
        _ => Err("invalid faction description form".into()),
    }
}

fn compare_overrides(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    row: &Value,
) -> TestResult<()> {
    let list = row
        .get("nameOverrides")
        .ok_or("authored name override array")?;
    let Some(list) = catalog_v3_consumer::source_value(list)? else {
        return Ok(());
    };
    let items = list.as_array().ok_or("ordered name override array")?;
    for item in items {
        let Some(item) = catalog_v3_consumer::source_value(item)? else {
            continue;
        };
        let body = item.get("payload").ok_or("authored name override body")?;
        let Some(body) = catalog_v3_consumer::source_value(body)? else {
            continue;
        };
        let target = index.reference(
            body.get("propertyName")
                .ok_or("authored property name override")?,
            TextDomain::PublicName,
            TextReferenceShape::Required,
        )?;
        counters.count_supplemental(target)?;
    }
    Ok(())
}

pub(super) fn items(catalog: &Value, counters: &mut CoverageCounters<'_>) -> TestResult<()> {
    let index = TextIndex::new(catalog)?;
    for item in catalog_v3_consumer::table(catalog, "/definitions/items")? {
        let components = item.get("components").ok_or("Item component carrier")?;
        let Some(components) = catalog_v3_consumer::source_value(components)? else {
            continue;
        };
        for component in components.as_array().ok_or("ordered Item components")? {
            let body = component.get("value").ok_or("Item component body")?;
            match component.get("kind").and_then(Value::as_str) {
                Some("attachment") => compare_item_attachment(counters, &index, body)?,
                Some("purchasable") => {
                    let target = index.reference(
                        body.get("displayType").ok_or("Item display type carrier")?,
                        TextDomain::GameText,
                        TextReferenceShape::Required,
                    )?;
                    counters.count(target)?;
                }
                Some("physics") => {}
                _ => return Err("unknown Item component kind".into()),
            }
        }
    }
    Ok(())
}

fn compare_item_attachment(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    attachment: &Value,
) -> TestResult<()> {
    let Some(attachment) = catalog_v3_consumer::source_value(attachment)? else {
        return Ok(());
    };
    let localization = attachment
        .get("localization")
        .ok_or("Item localization carrier")?;
    let Some(localization) = catalog_v3_consumer::source_value(localization)? else {
        return Ok(());
    };
    for (field, domain) in [
        ("name", TextDomain::PublicName),
        ("description", TextDomain::GameText),
    ] {
        let target = index.reference(
            localization
                .get(field)
                .ok_or("localized Item text carrier")?,
            domain,
            TextReferenceShape::Required,
        )?;
        counters.count(target)?;
    }
    Ok(())
}
