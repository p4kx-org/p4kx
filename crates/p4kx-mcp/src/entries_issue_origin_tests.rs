// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_channels::PublicPatches;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn issue_fields_bind_original_cached_root_metadata() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(
            TestEntry::stored("lossy", b"x")
                .with_path_bytes(b"bad\xff.bin")
                .with_encryption_block([5, 0, 0])
                .with_empty_encryption_blocks(1),
        )
        .entry(
            TestEntry::stored("repeated.bin", b"x")
                .with_encryption_field(0)
                .with_empty_encryption_blocks(2),
        )
        .entry(
            TestEntry::stored("hash.bin", b"x")
                .with_hash_block([1, 2])
                .with_extra_hash_block(&[3; 32]),
        )
        .write_to(&archive_path)?;

    let archive = Archive::open(&archive_path)?;
    let info = serde_json::to_value(super::info(&archive, &PublicPatches::default())?)?;
    assert_eq!(
        info.get("issue_count"),
        Some(&json!(5)),
        "only the first issue of each kind is reported per Entry"
    );
    assert_eq!(
        info.pointer("/issues/0/path").and_then(Value::as_str),
        Some("bad\u{fffd}.bin"),
        "the lossily decoded path retains its original location"
    );
    verify_site(&info, 0, "path", "path", "conversion")?;
    verify_site(&info, 0, "issue", "path", "conversion")?;
    verify_site(
        &info,
        1,
        "issue",
        "encryptionDeclarations[0].declaredBlockLength",
        "conversion",
    )?;
    verify_site(
        &info,
        2,
        "issue",
        "encryptionDeclarations[0].rawWord",
        "conversion",
    )?;
    verify_site(
        &info,
        3,
        "issue",
        "encryptionDeclarations[1].declaredBlockLength",
        "conversion",
    )?;
    verify_site(&info, 3, "issue", "encryptionDeclarations[0]", "selector")?;
    verify_site(&info, 4, "path", "path", "payload")?;

    let unknown = marker(&info, 4, "issue")?;
    assert_eq!(
        unknown.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "a later valid hash cannot provide the earlier defective block's location"
    );
    assert_eq!(
        unknown.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the missing original hash-block observation remains unknown"
    );
    assert!(
        unknown
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "the unsupported defect has no fabricated metadata contributor"
    );
    assert_eq!(
        info.pointer("/fieldOrigins/~1structuredContent~1issue_count"),
        Some(&json!({
            "containsEncryptedSource": null,
            "sourceUsesRefs": [],
            "sourceSetComplete": false,
        })),
        "MCP explicitly marks the full count unknown independently of the sourced Issue sample"
    );
    Ok(())
}

#[test]
fn issue_field_uses_the_original_fixed_v2_encryption_word() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("bad.bin", b"x").with_encryption_field(u16::MAX))
        .write_to(&archive_path)?;

    let archive = Archive::open(archive_path)?;
    let info = serde_json::to_value(super::info(&archive, &PublicPatches::default())?)?;
    assert!(
        info.pointer("/issues/0/issue")
            .and_then(Value::as_str)
            .is_some_and(|issue| issue.contains("65535")),
        "the issue displays the actual invalid 16-bit word"
    );
    verify_site(
        &info,
        0,
        "issue",
        "encryptionDeclarations[0].rawWord",
        "conversion",
    )
}

fn marker<'a>(info: &'a Value, row: usize, key: &str) -> TestResult<&'a Value> {
    info.get("fieldOrigins")
        .and_then(|origins| origins.get(format!("/structuredContent/issues/{row}/{key}")))
        .ok_or_else(|| "issue field origin".into())
}

fn verify_site(info: &Value, row: usize, key: &str, original: &str, role: &str) -> TestResult {
    let evidence = marker(info, row, key)?;
    assert_eq!(
        evidence.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the original operands are available for {row}/{key}"
    );
    assert_eq!(
        evidence
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "the root directory supplies {row}/{key} independently of encrypted payload"
    );
    let references = evidence
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("issue field references")?;
    let inventory = info
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("issues source inventory")?;
    assert!(
        inventory.iter().any(|site| {
            site.get("id")
                .is_some_and(|identifier| references.contains(identifier))
                && site.get("scope").and_then(Value::as_str) == Some("archiveDirectoryMetadata")
                && site.get("role").and_then(Value::as_str) == Some(role)
                && site.pointer("/location/field").and_then(Value::as_str) == Some(original)
        }),
        "issue {row}/{key} references the exact original {original} metadata as {role}"
    );
    Ok(())
}
