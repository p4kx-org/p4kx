// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::support::{Failure, Fixture, SOURCE, Scene, TestResult};
use crate::origin_test_support as origins;

const CONFIGURATION: &[&str] = &[
    "/data/record_type",
    "/data/payload/failure",
    "/data/payload/failure/reason",
    "/data/payload/failure/budget",
    "/data/payload/failure/limit",
    "/data/payload/counts",
    "/data/payload/span",
    "/data/payload/span/basis",
    "/data/payload/span/start",
    "/data/payload/span/end_exclusive",
];

#[test]
fn depth_configuration_guard_is_independent_of_an_encrypted_leaf_payload() -> TestResult {
    let fixture = Fixture::new(Scene::EncryptedLeaf, Failure::ConfigurationDepth)?;
    fixture.preserved()?;
    fixture.domain("/data/payload/file_name", "archiveDirectoryMetadata", false)?;
    for pointer in CONFIGURATION {
        fixture.independent(pointer)?;
    }
    fixture.metadata_only()?;
    for pointer in ["/source", ""] {
        assert_eq!(
            origins::marker(fixture.field(pointer)?),
            Some(true),
            "retained bytes remain actual contributors independently of rejected parser configuration"
        );
    }
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/span/start")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/span/end_exclusive")
            .and_then(Value::as_u64),
        Some(0)
    );
    Ok(())
}

#[test]
fn configuration_failure_keeps_only_the_encrypted_outer_directory_name() -> TestResult {
    let fixture = Fixture::new(Scene::EncryptedOuter, Failure::ConfigurationDepth)?;
    fixture.preserved()?;
    fixture.domain("/data/payload/file_name", "archiveDirectoryMetadata", true)?;
    for pointer in CONFIGURATION {
        fixture.independent(pointer)?;
    }
    fixture.metadata_only()?;
    for pointer in ["/data", "/data/payload"] {
        assert_eq!(
            origins::marker(fixture.field(pointer)?),
            Some(true),
            "the known outer filename domain survives the independent configuration failure"
        );
    }
    Ok(())
}

#[test]
fn actual_nesting_failure_under_a_valid_configuration_keeps_its_payload_uses() -> TestResult {
    let fixture = Fixture::new(Scene::EncryptedLeaf, Failure::ParsedDepth)?;
    fixture.preserved()?;
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/failure/budget")
            .and_then(Value::as_str),
        Some("depth")
    );
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/failure/limit")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        fixture
            .record
            .pointer("/data/payload/span/end_exclusive")
            .and_then(Value::as_u64)
            .is_some_and(|end| end > 0),
        "the actual parser inspected a nonempty source prefix"
    );
    for pointer in [
        "/data/payload/failure/reason",
        "/data/payload/span/start",
        "/data/payload/span/end_exclusive",
    ] {
        fixture.domain(pointer, "payload", true)?;
    }
    fixture.independent("/data/payload/failure/budget")?;
    fixture.independent("/data/payload/failure/limit")?;
    assert_eq!(origins::marker(fixture.field("/data")?), Some(true));
    Ok(())
}

#[test]
fn the_original_source_size_refusal_precedes_invalid_depth_configuration() -> TestResult {
    let fixture = Fixture::new(Scene::EncryptedLeaf, Failure::SourceAndConfigurationDepth)?;
    fixture.preserved()?;
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/failure/budget")
            .and_then(Value::as_str),
        Some("source-bytes")
    );
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/failure/limit")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/span/end_exclusive")
            .and_then(Value::as_u64),
        Some(u64::try_from(SOURCE.len())?),
        "source size retains the earlier original error span"
    );
    fixture.domain(
        "/data/payload/span/end_exclusive",
        "archiveDirectoryMetadata",
        false,
    )?;
    fixture.independent("/data/payload/failure")?;
    fixture.metadata_only()?;
    Ok(())
}
