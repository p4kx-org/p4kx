// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::material_origin_support::{self as support, Fixture, MTL, Storage, TestResult};

const RESULT: &str = "/data/payload/shader_references/1/reference/resolution";

#[test]
fn a_known_property_match_excludes_the_unused_common_global_flag_value() -> TestResult {
    let fixture = fixture(
        b"<Material Shader='demo' StringGenMask='%A'/>",
        b"UsesCommonGlobalFlags\nProperty { Name = %A\n }\n",
    )?;
    assert_status(&fixture, "resolved")?;
    assert!(
        !has_field(&fixture, "UsesCommonGlobalFlags")?,
        "the common/global flag cannot affect an already nonempty matching candidate inventory"
    );
    assert!(
        has_field(&fixture, "Name")?,
        "the actual matching Property.Name remains a selector"
    );
    Ok(())
}

#[test]
fn an_empty_local_property_query_keeps_the_actual_common_global_flag_selector() -> TestResult {
    for (definition, status) in [
        (
            b"UsesCommonGlobalFlags\nProperty { Name = %A\n }\n".as_slice(),
            "unavailable",
        ),
        (b"Property { Name = %A\n }\n".as_slice(), "missing"),
    ] {
        let fixture = fixture(b"<Material Shader='demo' StringGenMask='%B'/>", definition)?;
        assert_status(&fixture, status)?;
        assert!(
            has_field(&fixture, "UsesCommonGlobalFlags")?,
            "empty local matches retain the actually used presence/absence predicate"
        );
    }
    Ok(())
}

#[test]
fn an_unknown_property_name_short_circuits_the_unused_common_global_flag_selector() -> TestResult {
    let fixture = fixture(
        b"<Material Shader='demo' StringGenMask='%B'/>",
        b"UsesCommonGlobalFlags\nProperty { Name(); }\n",
    )?;
    assert_status(&fixture, "unavailable")?;
    assert!(
        !has_field(&fixture, "UsesCommonGlobalFlags")?,
        "the already unknown Name inventory discards the common/global flag value"
    );
    assert!(
        has_field(&fixture, "Property.Name")?,
        "the actual unknown-name predicate remains an input"
    );
    Ok(())
}

fn fixture(material: &[u8], definition: &[u8]) -> TestResult<Fixture> {
    Fixture::new([
        support::material(MTL, material, Storage::Plain),
        support::material(support::SHADER, definition, Storage::Encrypted),
    ])
}

fn assert_status(fixture: &Fixture, status: &str) -> TestResult {
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer(&format!("{RESULT}/status"))
            .and_then(Value::as_str),
        Some(status),
        "the local property query retains its existing inventory classification"
    );
    fixture.marker(MTL, &format!("{RESULT}/status"), Some(true))
}

fn has_field(fixture: &Fixture, field: &str) -> TestResult<bool> {
    Ok(fixture
        .contributions(MTL, &format!("{RESULT}/status"))?
        .iter()
        .any(|used| {
            fixture.input_path(used) == Some(support::SHADER)
                && used.get("role").and_then(Value::as_str) == Some("selector")
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
        }))
}
