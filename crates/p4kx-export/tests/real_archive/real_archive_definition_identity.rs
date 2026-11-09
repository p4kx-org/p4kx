// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, Record, RecordText};
use serde_json::Value;

use super::{TestResult, list};
use crate::catalog_v3_consumer;

pub(super) fn check(catalog: &Value, kind: Kind) -> TestResult {
    let mut physical = HashSet::new();
    let mut order = Vec::new();
    for row in list(catalog, kind)? {
        let index = catalog_v3_consumer::field(row, kind, "recordIndex")?
            .as_u64()
            .ok_or("original physical Record index")?;
        if !physical.insert(index) {
            return Err("one physical definition cannot occur twice".into());
        }
        let source_type = catalog_v3_consumer::field(row, kind, "sourceType")?
            .as_str()
            .ok_or("definition source type")?;
        let matching = match kind {
            Kind::QualityDistributions => matches!(
                source_type,
                "CraftingQualityDistributionRecord" | "CraftingQualityLocationOverrideRecord"
            ),
            Kind::BlueprintCategories => source_type == "BlueprintCategoryRecord",
            Kind::BlueprintCategoryLists => source_type == "BlueprintCategoryDatabaseRecord",
            Kind::GameplayProperties => source_type == "CraftingGameplayPropertyDef",
            Kind::QualityQuantizations => source_type == "CraftingQualityQuantizationRecord",
            _ => false,
        };
        if !matching {
            return Err("a physical definition row belongs to its declared source family".into());
        }
        order.push((catalog_v3_consumer::identity(row, kind)?, index));
    }
    if !order.is_sorted() {
        return Err("equal nominal GUIDs keep increasing original physical positions".into());
    }
    Ok(())
}

/// Checks each retained physical list against its original Record metadata.
pub(super) fn check_original_lists(catalog: &Value, forge: &DataForge<'_>) -> TestResult {
    check_original(catalog, forge, Kind::BlueprintCategoryLists)
}

/// Checks each retained physical property against its original Record metadata.
pub(super) fn check_original_properties(catalog: &Value, forge: &DataForge<'_>) -> TestResult {
    check_original(catalog, forge, Kind::GameplayProperties)
}

fn check_original(catalog: &Value, forge: &DataForge<'_>, kind: Kind) -> TestResult {
    let expected_type = match kind {
        Kind::BlueprintCategoryLists => "BlueprintCategoryDatabaseRecord",
        Kind::GameplayProperties => "CraftingGameplayPropertyDef",
        _ => {
            return Err(
                "original metadata check requires a declared physical source family".into(),
            );
        }
    };
    check(catalog, kind)?;
    for row in list(catalog, kind)? {
        let index = catalog_v3_consumer::field(row, kind, "recordIndex")?
            .as_u64()
            .ok_or("original physical Record index")?;
        let record = forge
            .record(u32::try_from(index)?)
            .ok_or("actual original Record owner")?;
        if record.type_name()? != expected_type
            || catalog_v3_consumer::identity(row, kind)? != record.guid().to_string()
        {
            return Err("the retained identity belongs to the actual original Record owner".into());
        }
        original_header(row, kind, &record)?;
    }
    Ok(())
}

fn original_header(row: &Value, kind: Kind, record: &Record<'_>) -> TestResult {
    for (field, expected) in [
        ("recordPath", record.record_path()?),
        ("sourceType", record.type_name()?),
        ("fileName", record.stored_file_name()?),
    ] {
        if catalog_v3_consumer::field(row, kind, field)?.as_str() != Some(expected) {
            return Err(format!("original {kind} {field} differs").into());
        }
    }
    original_text(
        row.get("recordName").ok_or("original name carrier")?,
        record.stored_name()?,
    )?;
    original_text(
        row.get("metadataTag").ok_or("original tag carrier")?,
        record.stored_tag()?,
    )?;
    let name = record.name()?;
    let class = name
        .strip_prefix(record.type_name()?)
        .and_then(|name| name.strip_prefix('.'))
        .filter(|name| !name.is_empty())
        .unwrap_or(name);
    if catalog_v3_consumer::field(row, kind, "className")?.as_str() != Some(class) {
        return Err("original name/type convenience class differs".into());
    }
    Ok(())
}

fn original_text(carrier: &Value, original: RecordText<'_>) -> TestResult {
    catalog_v3_consumer::source_value(carrier)?;
    let (state, expected) = original.fold(
        || ("absent", None),
        || ("null", None),
        |text| ("present", Some(text)),
    );
    let value = carrier.get("value").ok_or("original stored text value")?;
    let matching = expected.map_or_else(|| value.is_null(), |text| value.as_str() == Some(text));
    if carrier.get("state").and_then(Value::as_str) != Some(state) || !matching {
        return Err("original stored header text/state differs".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "catalog_v3_category_list_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "catalog_v3_gameplay_identity_tests.rs"]
mod gameplay_tests;
