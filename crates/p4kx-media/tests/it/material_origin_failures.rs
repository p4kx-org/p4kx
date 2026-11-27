// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::MaterialSourceLimits;
use serde_json::Value;

use super::material_origin_support::{self as support, Fixture, MTL, SHADER, Storage, TestResult};

#[test]
fn pre_read_material_size_failure_uses_metadata_and_keeps_its_bound_independent() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.source_bytes = 1;
    limits.depth = 65;
    let fixture = Fixture::with(
        [support::material(MTL, b"<Material/>", Storage::Encrypted)],
        support::options(),
        limits,
    )?;
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer("/data/payload/reason/type")
            .and_then(Value::as_str),
        Some("source-bytes"),
        "the original source-size guard still precedes invalid configuration"
    );
    fixture.marker(MTL, "/data/payload/reason/type", Some(false))?;
    assert!(
        fixture.has_use(
            MTL,
            "/data/payload/reason/type",
            MTL,
            "archiveDirectoryMetadata",
            "conversion"
        )?,
        "the actual original directory size triggered the refusal"
    );
    fixture.no_payload(MTL, "/data", MTL)?;
    fixture.independent(MTL, "/data/payload/reason/limit")
}

#[test]
fn invalid_material_configuration_does_not_use_discarded_encrypted_payload_bytes() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.depth = 65;
    let fixture = Fixture::with(
        [support::material(MTL, b"<Material/>", Storage::Encrypted)],
        support::options(),
        limits,
    )?;
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer("/data/payload/reason/type")
            .and_then(Value::as_str),
        Some("invalid-limits")
    );
    fixture.independent(MTL, "/data/payload/reason/type")?;
    fixture.independent(MTL, "/data/payload/reason")?;
    fixture.no_payload(MTL, "/data", MTL)
}

#[test]
fn an_actual_material_parse_failure_retains_the_used_encrypted_content() -> TestResult {
    let fixture = Fixture::new([support::material(MTL, b"<Material>", Storage::Encrypted)])?;
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer("/data/payload/reason/type")
            .and_then(Value::as_str),
        Some("xml")
    );
    fixture.marker(MTL, "/data/payload/reason/type", Some(true))?;
    assert!(
        fixture.has_use(
            MTL,
            "/data/payload/reason/type",
            MTL,
            "payload",
            "conversion"
        )?,
        "the emitted parser failure depends on actual consumed material bytes"
    );
    fixture.independent(MTL, "/data/record_type")
}

#[test]
fn an_authored_tree_cut_does_not_erase_actual_typed_value_and_default_selectors() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.text_bytes = 1;
    let source = b"<Material Name='still-read' Opacity='bad'/>";
    let fixture = Fixture::with(
        [support::material(MTL, source, Storage::Encrypted)],
        support::options(),
        limits,
    )?;
    assert!(
        fixture
            .record(MTL)?
            .1
            .pointer("/data/payload/authored/root")
            .is_some_and(Value::is_null),
        "the authored root was actually cut"
    );
    for pointer in [
        "/data/payload/materials/0/name",
        "/data/payload/materials/0/interpreted_surface/opacity/value",
    ] {
        fixture.marker(MTL, pointer, Some(true))?;
        assert!(
            fixture.has_use(MTL, pointer, MTL, "payload", "selector")?,
            "the actual typed parser branch survives independent authored retention cuts"
        );
    }
    fixture.marker(MTL, "/data/payload/projection_complete", Some(true))
}

#[test]
fn used_shader_syntax_failure_keeps_digest_origin_without_tainting_original_material_tokens()
-> TestResult {
    let source = b"<Material Shader='demo' StringGenMask='%A'/>";
    let fixture = Fixture::new([
        support::material(MTL, source, Storage::Plain),
        support::material(SHADER, b"Property {", Storage::Encrypted),
    ])?;
    let prefix = "/data/payload/shader_references/0";
    assert!(
        fixture
            .record(MTL)?
            .1
            .pointer(&format!("{prefix}/input"))
            .is_some_and(Value::is_object),
        "actual shader bytes were read despite a syntax failure"
    );
    fixture.marker(MTL, &format!("{prefix}/input/digest/hex"), Some(true))?;
    fixture.marker(
        MTL,
        &format!("{prefix}/reference/original/value"),
        Some(false),
    )?;
    fixture.marker(
        MTL,
        &format!("{prefix}/reference/resolution/reason/type"),
        Some(true),
    )?;
    fixture.no_payload(MTL, &format!("{prefix}/reference/original/value"), SHADER)
}
