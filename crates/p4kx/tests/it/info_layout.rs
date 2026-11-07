// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn info_json_uses_the_shared_cached_layout_without_changing_existing_counts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let bytes = p4kx_testkit::p4kx()?
        .args(["--json", "info"])
        .arg(&path)
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let info: Value = serde_json::from_slice(&bytes)?;
    let expected: Value = serde_json::from_str(include_str!(
        "../../../p4kx-testkit/fixtures/archive-layout-v2-empty.json"
    ))?;
    assert_eq!(info.get("metadata"), Some(&expected));
    assert_eq!(info.get("entry_count"), Some(&json!(1)));
    assert_eq!(info.get("encrypted_count"), Some(&json!(0)));
    assert_eq!(info.get("compressed_size"), Some(&json!(0)));
    assert_eq!(info.get("size"), Some(&json!(0)));
    Ok(())
}
