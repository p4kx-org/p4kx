// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::mining_locations::{HarvestableOccurrenceRoute, MiningLocationSnapshot};
use p4kx_extraction::SourcedValue;
use serde::Serialize;
use serde_json::Value;

use super::{TestResult, require_equal, rows, rows_at};

pub(super) fn compare_routes(catalog: &Value, snapshot: &MiningLocationSnapshot) -> TestResult {
    if catalog
        .pointer("/placements/containerInstances/availability")
        .and_then(Value::as_str)
        != Some("available")
    {
        return Err("Catalog placement inventory is unavailable".into());
    }
    let mut exported = BTreeMap::new();
    for row in rows_at(catalog, rows::CONTAINERS)? {
        let route = row
            .pointer("/instanceId/value")
            .ok_or("catalog route key")?;
        if exported
            .insert(serde_json::to_string(route)?, row)
            .is_some()
        {
            return Err("duplicate Catalog harvest occurrence route".into());
        }
    }
    let count = exported.len();
    for occurrence in &snapshot.harvestable_occurrences {
        let HarvestableOccurrenceRoute::Reached(reached) = &occurrence.route else {
            continue;
        };
        let key = reached.id.value().ok_or("SDK reached route key")?;
        let row = exported
            .remove(&serde_json::to_string(&serde_json::to_value(key)?)?)
            .ok_or("SDK harvest occurrence route missing from the Catalog")?;
        compare_route_value(row, "parentInstanceId", &reached.parent)?;
        compare_route_value(row, "container", &occurrence.container)?;
        compare_route_value(row, "sourcePath", &occurrence.container_path)?;
        require_equal(
            row.pointer("/system/value"),
            Some(&Value::from(occurrence.system.as_str())),
            "routed container system",
        )?;
    }
    if !exported.is_empty() || count == 0 {
        return Err("unmatched or empty Catalog harvest occurrence routes".into());
    }
    Ok(())
}

fn compare_route_value<T: Serialize>(
    row: &Value,
    field: &str,
    sdk: &SourcedValue<T>,
) -> TestResult {
    let exported = row.get(field).ok_or("required Catalog route cell")?;
    let projected = serde_json::to_value(sdk)?;
    for part in ["value", "state"] {
        require_equal(exported.get(part), projected.get(part), field)?;
    }
    Ok(())
}

pub(super) fn compare_texts(catalog: &Value, snapshot: &MiningLocationSnapshot) -> TestResult {
    let mut names = index_texts(rows_at(catalog, rows::PUBLIC_NAMES)?)?;
    let mut texts = index_texts(rows_at(catalog, rows::GAME_TEXTS)?)?;
    for row in &snapshot.public_names {
        let id = row.id.value().ok_or("SDK public name ID")?.as_str();
        let exported = names.remove(id).ok_or("Catalog public name")?;
        compare_text_identity(exported, &row.id, &row.key)?;
        let forms = row
            .translations
            .value()
            .and_then(|all| all.get(snapshot.language.code()))
            .ok_or("missing SDK public name translation forms")?;
        compare_locale(exported, snapshot, &forms.source_text, &forms.display_text)?;
    }
    for row in &snapshot.game_texts {
        let id = row.id.value().ok_or("SDK protected text ID")?.as_str();
        let exported = texts.remove(id).ok_or("Catalog game text")?;
        compare_text_identity(exported, &row.id, &row.key)?;
        let forms = row
            .translations
            .value()
            .and_then(|all| all.get(snapshot.language.code()))
            .ok_or("missing SDK game text translation forms")?;
        compare_locale(exported, snapshot, &forms.source_text, &forms.display_text)?;
    }
    if !names.is_empty() || !texts.is_empty() {
        return Err("unmatched Catalog text IDs".into());
    }
    Ok(())
}

fn index_texts(rows: &[Value]) -> TestResult<BTreeMap<&str, &Value>> {
    let mut index = BTreeMap::new();
    for row in rows {
        let id = row
            .pointer("/id/value")
            .and_then(Value::as_str)
            .ok_or("required Catalog text ID")?;
        if index.insert(id, row).is_some() {
            return Err("duplicate Catalog text ID".into());
        }
    }
    Ok(index)
}

fn compare_text_identity<T: Serialize>(
    exported: &Value,
    id: &SourcedValue<T>,
    key: &SourcedValue<String>,
) -> TestResult {
    compare_text_cell(exported.get("id"), id)?;
    compare_text_cell(exported.get("key"), key)
}

fn compare_locale<T: Serialize>(
    exported: &Value,
    snapshot: &MiningLocationSnapshot,
    source: &SourcedValue<T>,
    display: &SourcedValue<T>,
) -> TestResult {
    let path = format!("/translations/value/{}", snapshot.language.code());
    let forms = exported
        .pointer(&path)
        .ok_or("Catalog locale forms are missing")?;
    compare_text_cell(forms.get("sourceText"), source)?;
    compare_text_cell(forms.get("displayText"), display)
}

fn compare_text_cell<T: Serialize>(exported: Option<&Value>, sdk: &SourcedValue<T>) -> TestResult {
    let exported = exported.ok_or("required Catalog text cell")?;
    let projected = serde_json::to_value(sdk)?;
    for field in ["value", "state"] {
        require_equal(
            exported.get(field),
            projected.get(field),
            "localized text cell",
        )?;
    }
    let actual = exported
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("required Catalog text source uses")?;
    for expected in sdk.origin().source_uses_refs() {
        if !actual
            .iter()
            .any(|value| value.as_str() == Some(expected.as_str()))
        {
            return Err("Catalog text lacks a scoped SDK source contribution".into());
        }
    }
    Ok(())
}
