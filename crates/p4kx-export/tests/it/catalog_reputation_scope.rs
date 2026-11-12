// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Filter;
use serde_json::{Value, json};

use super::{TestResult, build, guid};
use crate::health_origin_assertions as proof;
use crate::reputation_fixture::{SCOPE, STANDING_HIGH, STANDING_LOW};

#[test]
fn a_scope_exports_its_authored_description_and_standing_map() -> TestResult {
    let (_, catalog) = build(None)?;
    let row = proof::at(&catalog, "/definitions/reputationScopes/rows/0")?;
    assert!(catalog.get("reputationScopes").is_none());
    proof::present(row, "guid", &json!(guid(SCOPE)), Some(true))?;
    proof::present(
        row,
        "icon",
        &json!("ui/fixture/reputation_scope.dds"),
        Some(true),
    )?;
    let description = proof::cell(row, "description", "present")?;
    let reference = proof::at(description, "/value")?;
    let text = proof::rows(&catalog, "/texts/gameTexts")?
        .iter()
        .find(|row| row.pointer("/id/value") == Some(reference))
        .ok_or("protected scope description")?;
    assert_eq!(
        proof::at(text, "/key/value")?,
        &json!("@scope_fixture_description")
    );
    let map = proof::at(row, "/standingMap/value")?;
    proof::present(
        map,
        "sourceType",
        &json!("SReputationStandingMapParams"),
        Some(true),
    )?;
    proof::present(
        map,
        "reputationCeiling",
        &json!({"type":"int64","value":"9007199254740993"}),
        Some(true),
    )?;
    proof::present(
        map,
        "initialReputation",
        &json!({"type":"int64","value":"0"}),
        Some(true),
    )?;
    proof::closure(&catalog)
}

#[test]
fn a_scope_keeps_standing_reference_order_when_targets_are_filtered() -> TestResult {
    let filter = Filter::new("*/reputation/scopes/*");
    let (_, catalog) = build(Some(&filter))?;
    let row = proof::at(&catalog, "/definitions/reputationScopes/rows/0")?;
    let references = proof::rows(row, "/standingMap/value/standings/value")?;
    let guids = references
        .iter()
        .map(|reference| proof::at(reference, "/value/guid/value"))
        .collect::<Result<Vec<&Value>, _>>()?;
    assert_eq!(
        guids,
        vec![&json!(guid(STANDING_LOW)), &json!(guid(STANDING_HIGH))]
    );
    assert_eq!(
        proof::rows(&catalog, "/definitions/reputationStandings/rows")?.len(),
        0
    );
    proof::closure(&catalog)
}

#[test]
fn a_legacy_scope_string_key_still_resolves_as_protected_text() -> TestResult {
    use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

    use crate::commodity_v3_fixture::{self as archive, DcbOrigin, text_row};

    let source_type = "SReputationScopeParams";
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(source_type).field("displayName", TestType::String))
        .record(
            TestRecord::new(
                SCOPE,
                source_type,
                "SReputationScopeParams.Legacy",
                "libs/foundry/records/reputation/scopes/legacy.xml",
            )
            .value("displayName", TestValue::text("@shared")),
        );
    let (_folder, input) = archive::open(&database, DcbOrigin::Plain)?;
    let built = p4kx_export::build_catalog(&input, &archive::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::at(&catalog, "/definitions/reputationScopes/rows/0")?;
    let display = proof::cell(row, "displayName", "present")?;
    let text = text_row(&catalog, "gameTexts", proof::at(display, "/value")?)?;
    assert_eq!(proof::at(text, "/key/value")?, &json!("@shared"));
    assert_eq!(
        proof::at(text, "/translations/value/en/displayText/value")?,
        &json!("Shared text")
    );
    proof::closure(&catalog)
}
