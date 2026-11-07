// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_channels::PublicPatches;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn archive_info_uses_the_shared_cached_header_dto_without_changing_existing_counts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let info = serde_json::to_value(super::info(&archive, &PublicPatches::default()))?;
    let expected: Value = serde_json::from_str(include_str!(
        "../../p4kx-testkit/fixtures/archive-layout-v2-empty.json"
    ))?;
    assert_eq!(
        info.get("metadata"),
        Some(&expected),
        "shared header projection"
    );
    assert_eq!(
        info.get("entry_count"),
        Some(&json!(1)),
        "existing Entry count"
    );
    assert_eq!(
        info.get("encrypted_count"),
        Some(&json!(0)),
        "existing encrypted count"
    );
    assert_eq!(
        info.get("compressed_size"),
        Some(&json!(0)),
        "existing stored size"
    );
    assert_eq!(info.get("size"), Some(&json!(0)), "existing decoded size");
    Ok(())
}

#[test]
fn archive_info_output_schema_requires_all_shared_header_fields() -> TestResult {
    let schema = serde_json::to_value(
        rmcp::handler::server::tool::schema_for_output::<crate::shapes::ArchiveInfo>().as_ref(),
    )?;
    assert_eq!(
        schema.pointer("/properties/metadata/$ref"),
        Some(&json!("#/$defs/ArchiveLayoutInfo")),
        "MCP output schema shares the header DTO"
    );
    let required = schema
        .pointer("/$defs/ArchiveLayoutInfo/required")
        .and_then(Value::as_array)
        .ok_or("required header properties")?;
    for field in [
        "version",
        "sourceLength",
        "layout",
        "declarationState",
        "zip",
        "v2",
    ] {
        assert!(
            required.contains(&json!(field)),
            "required output field {field}"
        );
    }
    Ok(())
}
