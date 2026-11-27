// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, Language, LocalizationEntry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::{Coverage, Oracle};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn raw_oracle_matches_an_original_unterminated_eof_cr() -> TestResult {
    original("name=Value\r", "Value", "Value")
}

#[test]
fn raw_oracle_removes_exactly_one_original_eof_cr() -> TestResult {
    original("name=Value\r\r", "Value\r", "Value")
}

#[test]
fn raw_oracle_preserves_an_original_internal_cr() -> TestResult {
    original("name=Va\rlue\r\n", "Va\rlue", "Va\rlue")
}

#[test]
fn raw_oracle_matches_an_original_crlf_delimiter() -> TestResult {
    original("name=Value\r\n", "Value", "Value")
}

fn original(input: &str, source: &str, display: &str) -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            input.as_bytes().to_vec(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = LocalizationEntry::read(&archive, Language::English)?;
    let index = entry.parse()?;
    assert_eq!(
        index.resolve("@name"),
        Some(source),
        "the actual original reader establishes the literal winning RHS before the oracle"
    );
    let oracle = Oracle::parse(entry.text().text());
    assert_eq!(
        oracle.source_text("@name"),
        Some(source),
        "the independent oracle agrees with the exact original RHS, including retained CR"
    );
    assert_eq!(
        oracle.canonical_text("@name"),
        Some(display),
        "the existing independent display result stays unchanged"
    );
    let mut coverage = Coverage::default();
    coverage.count_forms(&oracle, "@name", &json!(source), &json!(display));
    assert_eq!(
        (
            coverage.resolved,
            coverage.fallback,
            coverage.contradictions,
            coverage.total()
        ),
        (1, 0, 0, 1),
        "two correct forms count one original occurrence without a false raw contradiction"
    );
    Ok(())
}
