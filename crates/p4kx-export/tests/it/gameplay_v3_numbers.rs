// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::commodity_v3_fixture::DcbOrigin;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::Value as Raw;
use p4kx_testkit::{TestType, TestValue};
use serde_json::{Value, json};

#[test]
fn gameplay_v3_keeps_exact_single_bits_and_does_not_waive_other_widths_paths_or_owners()
-> TestResult {
    for bits in [
        0,
        0x8000_0000,
        0x7f7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_1234,
        0x7f80_0042,
    ] {
        let database = schema::schema().record(values::record(
            values::FIRST,
            "Primary",
            values::scale(TestValue::Single(f32::from_bits(bits))),
        ));
        original_single(&database, 0, bits)?;
        let (_, catalog) = support::normal(&database)?;
        number(
            support::at(support::row(&catalog, 0)?, support::SCALE)?,
            "f32",
            &format!("{bits:08x}"),
        )?;
        original::check(&catalog, &database, 0)?;
        assert_eq!(
            support::at(&catalog, "/exportHealth/status")?,
            &json!("complete"),
            "a preserved legal Single token is complete independently of finiteness"
        );
    }
    wrong_width()?;
    uncaptured_path()?;
    unexported_alias()
}

fn original_single(
    database: &p4kx_testkit::DataForgeBuilder,
    record: u32,
    bits: u32,
) -> TestResult {
    source::with_tree(database, record, |tree| {
        let display = source::structure(tree.root().field("displayTransformation"))?;
        let Some(Raw::Single(value)) = display.field("scale") else {
            return Err("original Single".into());
        };
        assert_eq!(
            value.to_bits(),
            bits,
            "the exact original token exists before projection"
        );
        Ok(())
    })
}

fn number(value: &Value, kind: &str, bits: &str) -> TestResult {
    support::state(value, "present")?;
    assert_eq!(
        support::at(value, "/value/type")?,
        &json!(kind),
        "actual original numeric storage width"
    );
    assert_eq!(
        support::at(value, "/value/rawBits")?,
        &json!(bits),
        "actual original IEEE sign/payload bits"
    );
    Ok(())
}

fn wrong_width() -> TestResult {
    let database = schema::custom(schema::property(), TestType::Double).record(values::record(
        values::FIRST,
        "Primary",
        values::scale(TestValue::Double(1.0e300)),
    ));
    source::with_tree(&database, 0, |tree| {
        let display = source::structure(tree.root().field("displayTransformation"))?;
        let Some(Raw::Double(value)) = display.field("scale") else {
            return Err("actual wrong-width Double".into());
        };
        assert_eq!(
            value.to_bits(),
            1.0e300_f64.to_bits(),
            "the readable wrong-width source is not a fixture cast"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    number(
        support::at(support::row(&catalog, 0)?, support::SCALE)?,
        "f64",
        "7e37e43c8800759c",
    )?;
    original::check(&catalog, &database, 0)?;
    support::diagnosis(
        &catalog,
        "malformedSource",
        0,
        "displayTransformation.scale",
        schema::SCALE,
    )?;
    support::refused(&database, None)
}

fn uncaptured_path() -> TestResult {
    let bits = 0x7fc0_0042;
    let root = schema::property().field("spare", TestType::StrongPointer(schema::SCALE.to_owned()));
    let database = schema::custom(root, TestType::Single).record(
        values::record(
            values::FIRST,
            "Primary",
            values::scale(TestValue::Single(f32::from_bits(bits))),
        )
        .value(
            "spare",
            values::scale(TestValue::Single(f32::from_bits(bits))),
        ),
    );
    original_single(&database, 0, bits)?;
    source::with_tree(&database, 0, |tree| {
        let spare = source::structure(tree.root().field("spare"))?;
        let Some(Raw::Single(value)) = spare.field("scale") else {
            return Err("original unexported token".into());
        };
        assert_eq!(
            value.to_bits(),
            bits,
            "same owner and bits exist at a distinct original path"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    let row = support::row(&catalog, 0)?;
    assert!(
        row.get("spare").is_none(),
        "unknown fields are diagnosed, not invented as arbitrary payload"
    );
    number(support::at(row, support::SCALE)?, "f32", "7fc00042")?;
    original::check(&catalog, &database, 0)?;
    support::diagnosis(
        &catalog,
        "unsupportedValue",
        0,
        "spare.scale",
        schema::SCALE,
    )?;
    support::refused(&database, None)
}

fn unexported_alias() -> TestResult {
    let bits = 0xffc0_0042;
    let database = schema::schema()
        .record(values::record(
            values::FIRST,
            "Hidden",
            values::scale(TestValue::Single(f32::from_bits(bits))),
        ))
        .record(values::record(
            values::FIRST,
            "Alias",
            values::scale(TestValue::Single(f32::from_bits(bits))),
        ));
    for index in [0, 1] {
        original_single(&database, index, bits)?;
    }
    let filter = Filter::new("*/Alias");
    let (_, catalog) = support::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        Some(&filter),
    )?;
    assert_eq!(
        support::rows(&catalog)?.len(),
        1,
        "an actual dependency does not materialize another row"
    );
    number(
        support::at(support::row(&catalog, 1)?, support::SCALE)?,
        "f32",
        "ffc00042",
    )?;
    original::check(&catalog, &database, 1)?;
    support::diagnosis(
        &catalog,
        "unsupportedValue",
        0,
        "displayTransformation.scale",
        schema::SCALE,
    )?;
    emitted_alias_is_covered(&catalog)?;
    support::refused(&database, Some(&filter))
}

fn emitted_alias_is_covered(catalog: &Value) -> TestResult {
    let diagnostic = support::at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    assert!(
        !diagnostic.iter().any(|entry| {
            entry.get("code").and_then(Value::as_str) == Some("unsupportedValue")
                && entry
                    .get("examples")
                    .and_then(Value::as_array)
                    .is_some_and(|examples| {
                        examples.iter().any(|example| {
                            example.get("recordIndex").and_then(Value::as_u64) == Some(1)
                                && example.get("field").and_then(Value::as_str)
                                    == Some("displayTransformation.scale")
                        })
                    })
        }),
        "the emitted Record 1 receipt covers its own original token, independently of the blocked same-GUID dependency"
    );
    Ok(())
}
