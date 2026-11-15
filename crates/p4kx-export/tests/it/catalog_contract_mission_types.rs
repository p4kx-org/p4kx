// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Filter;
use serde_json::Value;

use super::{A, C, SUB_ONE, TestResult, build, entry, v3};
use crate::commodity_v3_fixture;

pub(super) fn name<'a>(catalog: &'a Value, row: &Value) -> TestResult<v3::Text<'a>> {
    let cell = row.get("missionType").ok_or("contract mission type cell")?;
    let id = v3::sourced(cell)?.ok_or("resolved mission type text ID")?;
    let text = commodity_v3_fixture::text_row(catalog, "gameTexts", id)?;
    Ok(v3::Text {
        key: v3::sourced(text.get("key").ok_or("mission type original key")?)?
            .and_then(Value::as_str),
        en: display(text, "en")?,
        de: display(text, "de")?,
    })
}

fn display<'a>(text: &'a Value, language: &str) -> TestResult<Option<&'a str>> {
    let Some(cell) = text.pointer(&format!("/translations/value/{language}/displayText")) else {
        return Ok(None);
    };
    Ok(v3::sourced(cell)?.and_then(Value::as_str))
}

#[test]
fn the_mission_type_is_the_nearest_override_else_the_template_type() -> TestResult {
    let (_, catalog) = build(None)?;
    let parent = entry(&catalog, A, None)?;
    assert_eq!(
        name(&catalog, parent)?,
        v3::Text {
            key: Some("@type_delivery"),
            en: Some("Delivery"),
            de: Some("Lieferung"),
        },
        "the template's original type"
    );
    assert_eq!(
        name(&catalog, entry(&catalog, A, Some(SUB_ONE))?)?.en,
        Some("Escort"),
        "the sub contract's override"
    );
    let handler = entry(&catalog, C, None)?;
    assert_eq!(
        name(&catalog, handler)?.en,
        Some("Bounty"),
        "handler override"
    );
    let guid = handler.get("missionTypeGuid").ok_or("selected type GUID")?;
    assert_eq!(
        guid.as_str(),
        Some("50000000-0000-4000-8000-000000000002"),
        "the original override's GUID"
    );
    let definition = catalog
        .get("missionTypes")
        .and_then(Value::as_array)
        .ok_or("mission types")?
        .iter()
        .find(|definition| definition.get("guid") == Some(guid))
        .ok_or("selected mission type definition")?;
    assert_eq!(
        handler.get("missionType"),
        definition.get("typeName"),
        "the name and exact per-field origin agree with the selected type definition"
    );
    Ok(())
}

#[test]
fn filtered_type_definitions_keep_the_original_contract_name_dependency() -> TestResult {
    let filter = Filter::new("contractgenerator");
    let (_, catalog) = build(Some(&filter))?;
    assert!(
        catalog
            .get("missionTypes")
            .and_then(Value::as_array)
            .ok_or("mission types")?
            .is_empty(),
        "the filter leaves out the canonical definition rows"
    );
    let contract = entry(&catalog, A, None)?;
    assert_eq!(
        name(&catalog, contract)?.en,
        Some("Delivery"),
        "the filtered dependency still supplies the original name"
    );
    let cell = contract
        .get("missionType")
        .ok_or("contract mission type cell")?;
    crate::text_forms_assertions::direct(cell)?;
    crate::health_origin_assertions::closure(&catalog)?;
    Ok(())
}
