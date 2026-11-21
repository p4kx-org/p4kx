// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_testkit::{TestInstance, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{DURABILITY, HEALTH_ONLY, database, health, item};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn durability_covers_only_the_emitted_original_number_with_equal_bits() -> TestResult {
    let number = f32::from_bits(0x7fc0_1234);
    let other = TestValue::Struct(
        TestInstance::new("FixtureUnprojected").value("Noise", TestValue::Single(number)),
    );
    let source = database(Some(TestType::Single))
        .structure(
            TestStruct::new("FixtureUnprojected")
                .parent("DataForgeComponentParams")
                .field("Noise", TestType::Single),
        )
        .record(item(
            HEALTH_ONLY,
            "durability_uncovered",
            "Misc",
            vec![health(TestValue::Single(number)), other],
        ));
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    let [row] = proof::rows(&catalog, &format!("{DURABILITY}/rows"))? else {
        return Err("only the emitted Health component supplies a row".into());
    };
    proof::present(
        row,
        "health",
        &json!({"rawBits": "7fc01234", "finite": null}),
        Some(false),
    )?;
    let diagnostics = proof::rows(&catalog, "/exportHealth/diagnostics")?;
    let unsupported = diagnostics
        .iter()
        .find(|row| {
            row.get("code").and_then(Value::as_str) == Some("unsupportedValue")
                && row.get("kind").and_then(Value::as_str) == Some("items")
        })
        .ok_or("the unrelated original number remains uncovered")?;
    assert_eq!(
        unsupported.get("count"),
        Some(&json!(1)),
        "only one original number is uncovered"
    );
    let [example] = proof::rows(unsupported, "/examples")? else {
        return Err("one uncovered original occurrence".into());
    };
    assert_eq!(
        proof::example_text(&catalog, example, "field")?,
        Some(&json!("Components[2].Noise")),
        "equal bits must not borrow the emitted sibling Health receipt"
    );
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication still rejects the unrelated unsupported number"
    );
    Ok(())
}
