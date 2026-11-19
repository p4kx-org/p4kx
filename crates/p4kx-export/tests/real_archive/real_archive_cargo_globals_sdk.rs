// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::Kind;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde::Serialize;
use serde_json::Value;

use super::TestResult;
use crate::catalog_v3_consumer;

#[path = "real_archive_cargo_globals_complete_catalog.rs"]
mod complete_catalog;

pub(super) fn verify(archive: &Archive, entry: &DataForgeEntry, catalog: &Value) -> TestResult {
    let loading = catalog_v3_consumer::rows(catalog, Kind::CargoLoadingGlobals)?;
    let prices = catalog_v3_consumer::rows(catalog, Kind::ShopCommodityGlobals)?;
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_cargo_loading_globals(CatalogRecordSelection::All, None, |sdk| {
        verify_sdk_rows(&sdk, loading)
    })??;
    parsed.with_shop_commodity_globals(CatalogRecordSelection::All, None, |sdk| {
        verify_sdk_rows(&sdk, prices)
    })??;
    Ok(())
}

fn verify_sdk_rows<Row: Serialize>(
    sdk: &EquipmentSnapshot<'_, Row>,
    expected: &[Value],
) -> TestResult {
    if !sdk.coverage().incomplete_fields.is_empty() || sdk.rows().len() != expected.len() {
        return Err("the selective cargo query did not read every complete physical row".into());
    }
    let mut by_index = HashMap::new();
    for row in expected {
        if by_index.insert(record_index(row)?, row).is_some() {
            return Err("a canonical cargo Record occurs more than once".into());
        }
    }
    for row in sdk.rows() {
        let exported: Value = serde_json::from_slice(&serde_json::to_vec(row)?)?;
        let expected = by_index
            .remove(&record_index(&exported)?)
            .ok_or("selective cargo row lacks a unique canonical physical owner")?;
        if let Some(path) = first_cargo_difference(expected, &exported) {
            return Err(format!("the selective cargo row differs at {path}").into());
        }
    }
    Ok(())
}

fn record_index(row: &Value) -> TestResult<u64> {
    row.pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or_else(|| "a cargo row needs a physical Record position".into())
}

fn first_cargo_difference(expected: &Value, actual: &Value) -> Option<String> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, child) in expected {
                match actual.get(key) {
                    Some(actual) => {
                        if let Some(path) = first_cargo_difference(child, actual) {
                            return Some(format!("/{key}{path}"));
                        }
                    }
                    None => return Some(format!("/{key}")),
                }
            }
            Some("/additionalKey".into())
        }
        (Value::Array(expected), Value::Array(actual)) if expected.len() == actual.len() => {
            expected
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(at, (left, right))| {
                    first_cargo_difference(left, right).map(|path| format!("/{at}{path}"))
                })
        }
        _ => Some(String::new()),
    }
}
