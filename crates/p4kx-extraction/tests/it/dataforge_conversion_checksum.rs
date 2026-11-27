// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ExtractOptions, ExtractionError};
use p4kx_testkit::TestEntry;

use super::{TestResult, converting, database, files, fixture, run};

#[test]
fn a_dataforge_checksum_failure_precedes_record_publication() -> TestResult {
    let fixture = fixture(vec![
        TestEntry::stored("Data\\Game2.dcb", database(1)).with_crc32c(0),
    ])?;
    let report = run(
        &fixture,
        &ExtractOptions {
            verify: true,
            ..converting()
        },
    );

    assert!(
        matches!(
            report.failures(),
            [failure] if failure.path() == "Data\\Game2.dcb"
                && matches!(failure.error(), ExtractionError::ChecksumMismatch { .. })
        ),
        "verified conversion retains the original checksum error: {report:?}"
    );
    assert!(
        files(&fixture.target)?.is_empty(),
        "failed verification publishes no Records"
    );
    Ok(())
}
