// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_archive::DataError;
use p4kx_extraction::{ExtractOptions, ExtractionError};
use p4kx_testkit::TestEntry;

use super::{TestResult, converting, database, files, fixture, run};

#[test]
fn an_overlong_dataforge_fails_before_record_publication() -> TestResult {
    let expected = database(1);
    let size = u64::try_from(expected.len())?;
    let mut overlong = expected.clone();
    overlong.push(b'!');
    let fixture = fixture(vec![
        TestEntry::stored("Data\\Game2.dcb", expected.as_slice()).with_payload(overlong),
    ])?;

    for verify in [false, true] {
        let report = run(
            &fixture,
            &ExtractOptions {
                verify,
                ..converting()
            },
        );
        assert!(
            matches!(
                report.failures(),
                [failure] if failure.path() == "Data\\Game2.dcb"
                    && matches!(failure.error(), ExtractionError::Read(source)
                        if matches!(source.get_ref().and_then(|cause| cause.downcast_ref::<DataError>()),
                            Some(DataError::TooLong { expected }) if *expected == size))
            ),
            "conversion reports the exact invalid extent with verify={verify}: {report:?}"
        );
        assert!(
            files(&fixture.target)?.is_empty(),
            "no Records are published with verify={verify}"
        );
    }

    let report = run(
        &fixture,
        &ExtractOptions {
            verify: true,
            ..ExtractOptions::default()
        },
    );
    assert!(report.is_complete(), "verified raw extraction: {report:?}");
    assert_eq!(
        fs::read(fixture.target.join("Data/Game2.dcb"))?,
        expected,
        "raw extraction still copies the declared bytes with the original checksum"
    );
    Ok(())
}
