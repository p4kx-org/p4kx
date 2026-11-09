// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::origin_test_support as origins;

#[path = "origin_shader_failure_support.rs"]
mod support;
use support::{Failure, Fixture, Scene, TestResult};

const PARSED: &[&str] = &[
    "/data/payload/failure/reason",
    "/data/payload/failure/construct",
    "/data/payload/span/start",
    "/data/payload/span/end_exclusive",
];

#[test]
fn actual_syntax_failures_retain_encrypted_payload_witnesses_per_datum_and_parent() -> TestResult {
    for scene in [Scene::EncryptedLeaf, Scene::EncryptedOuter] {
        let fixture = Fixture::new(scene, Failure::Syntax)?;
        fixture.preserved()?;
        for pointer in PARSED {
            fixture.domain(pointer, "payload", true)?;
        }
        for pointer in [
            "/data",
            "/data/payload",
            "/data/payload/failure",
            "/data/payload/span",
            "/source",
            "",
        ] {
            assert_eq!(
                origins::marker(fixture.field(pointer)?),
                Some(true),
                "{scene:?}: known used ciphertext survives compound datum {pointer}"
            );
        }
        let metadata = matches!(scene, Scene::EncryptedOuter);
        fixture.domain(
            "/data/payload/file_name",
            "archiveDirectoryMetadata",
            metadata,
        )?;
    }
    Ok(())
}

#[test]
fn plain_parser_failures_keep_their_actual_plain_domains_and_independent_controls() -> TestResult {
    let fixture = Fixture::new(Scene::Plain, Failure::Syntax)?;
    fixture.preserved()?;
    for pointer in PARSED {
        fixture.domain(pointer, "payload", false)?;
    }
    fixture.domain("/data/payload/file_name", "archiveDirectoryMetadata", false)?;
    for pointer in [
        "/data/record_type",
        "/data/payload/counts",
        "/data/payload/span/basis",
    ] {
        fixture.independent(pointer)?;
    }
    Ok(())
}

#[test]
fn early_source_size_limit_uses_directory_size_without_unused_leaf_payload_taint() -> TestResult {
    let fixture = Fixture::new(Scene::EncryptedLeaf, Failure::SourceLimit)?;
    fixture.preserved()?;
    fixture.domain("/data/payload/file_name", "archiveDirectoryMetadata", false)?;
    fixture.domain(
        "/data/payload/span/end_exclusive",
        "archiveDirectoryMetadata",
        false,
    )?;
    for pointer in [
        "/data/record_type",
        "/data/payload/counts",
        "/data/payload/span/basis",
        "/data/payload/span/start",
        "/data/payload/failure",
        "/data/payload/failure/reason",
        "/data/payload/failure/budget",
        "/data/payload/failure/limit",
    ] {
        fixture.independent(pointer)?;
    }
    fixture.metadata_only()?;
    for pointer in ["/source", ""] {
        assert_eq!(
            origins::marker(fixture.field(pointer)?),
            Some(true),
            "retained raw source bytes remain a real encrypted contributor independently of provider pre-read failure"
        );
    }
    Ok(())
}

#[test]
fn early_source_limit_retains_encrypted_outer_directory_evidence_without_a_payload_use()
-> TestResult {
    let fixture = Fixture::new(Scene::EncryptedOuter, Failure::SourceLimit)?;
    fixture.preserved()?;
    for pointer in [
        "/data/payload/file_name",
        "/data/payload/span/end_exclusive",
    ] {
        fixture.domain(pointer, "archiveDirectoryMetadata", true)?;
    }
    fixture.metadata_only()?;
    for pointer in ["/data", "/data/payload", "/data/payload/span"] {
        assert_eq!(
            origins::marker(fixture.field(pointer)?),
            Some(true),
            "containing encrypted bytes remain an actual directory contributor"
        );
    }
    Ok(())
}

#[test]
fn parser_token_limits_keep_configured_literals_separate_from_actual_payload_diagnostics()
-> TestResult {
    let fixture = Fixture::new(Scene::EncryptedLeaf, Failure::TokenLimit)?;
    fixture.preserved()?;
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/failure/reason")
            .and_then(Value::as_str),
        Some("limit")
    );
    assert_eq!(
        fixture
            .record
            .pointer("/data/payload/failure/budget")
            .and_then(Value::as_str),
        Some("token-bytes")
    );
    fixture.independent("/data/payload/failure/budget")?;
    fixture.independent("/data/payload/failure/limit")?;
    for pointer in [
        "/data/payload/failure/reason",
        "/data/payload/span/start",
        "/data/payload/span/end_exclusive",
    ] {
        fixture.domain(pointer, "payload", true)?;
    }
    assert_eq!(origins::marker(fixture.field("/data")?), Some(true));
    Ok(())
}

#[test]
fn failed_diagnostic_payload_bytes_status_counts_and_current_receipts_are_unchanged() -> TestResult
{
    for failure in [Failure::Syntax, Failure::SourceLimit, Failure::TokenLimit] {
        Fixture::new(Scene::EncryptedLeaf, failure)?.preserved()?;
    }
    Ok(())
}

#[path = "origin_shader_configuration_tests.rs"]
mod configuration;
