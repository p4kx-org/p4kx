// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    ConflictPolicy, ExtractionError, ExtractionReport, NameRule, TextureConversion,
};
use p4kx_testkit::TestEntry;

use super::fixture::{Fixture, rock, run_with, texture_entries};

type TestResult = Result<(), Box<dyn Error>>;

const LOCK: &str = "Data/Textures/.p4kx-origin.lock";
const SIMILAR: &str = "Data/Textures/.p4kx-origin.lock.asset";
const MERGED: &str = "Data/Textures/rock_diff.dds";
const MERGE_ONLY: TextureConversion = TextureConversion {
    merge: true,
    png: None,
    keep_raw: false,
};

#[test]
fn raw_lock_source_is_a_failure_even_when_existing_outputs_are_kept() -> TestResult {
    let mut entries = vec![TestEntry::stored(LOCK, b"raw file")];
    entries.extend(texture_entries(&rock()));
    let fixture = Fixture::new(entries)?;
    fixture.write(LOCK, b"")?;

    let report = run_with(&fixture, ConflictPolicy::Skip, MERGE_ONLY);
    assert_only_reserved_failure(&report, LOCK)?;
    assert!(
        fixture.target.join(LOCK).is_file(),
        "the existing lock remains a file"
    );
    assert!(
        fixture.read(LOCK)?.is_empty(),
        "the existing lock remains empty"
    );
    assert_merge(&report, &fixture)
}

#[test]
fn a_raw_file_or_folder_cannot_replace_an_ephemeral_dds_lock() -> TestResult {
    for path in [LOCK, "Data/Textures/.P4KX-ORIGIN.LOCK/asset.bin"] {
        let mut entries = texture_entries(&rock());
        entries.push(TestEntry::stored(path, b"raw file"));
        entries.push(TestEntry::stored(SIMILAR, b"ordinary sibling"));
        let fixture = Fixture::new(entries)?;

        let report = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
        assert_only_reserved_failure(&report, path)?;
        assert_merge(&report, &fixture)?;
        for lock in [LOCK, "Data/Textures/.P4KX-ORIGIN.LOCK"] {
            assert!(
                !fixture.target.join(lock).exists(),
                "no temporary lock or folder remains"
            );
        }
        assert_eq!(
            fixture.read(SIMILAR)?,
            b"ordinary sibling",
            "a similar name keeps normal extraction"
        );
    }
    Ok(())
}

fn assert_only_reserved_failure(report: &ExtractionReport, path: &str) -> TestResult {
    let [failure] = report.failures() else {
        return Err(format!("expected exactly one failure: {report:?}").into());
    };
    assert_eq!(
        failure.path().replace('\\', "/"),
        path,
        "exact requested raw entry"
    );
    assert!(
        matches!(
            failure.error(),
            ExtractionError::InvalidName {
                rule: NameRule::ReservedOriginLock,
                ..
            }
        ),
        "the reserved name is an explicit output-name failure"
    );
    assert!(
        report.skipped().is_empty(),
        "the lock cannot count as an existing raw asset"
    );
    Ok(())
}

fn assert_merge(report: &ExtractionReport, fixture: &Fixture) -> TestResult {
    let [artifact] = report.merged_texture_artifacts() else {
        return Err(format!("expected exactly one merged DDS receipt: {report:?}").into());
    };
    assert_eq!(artifact.path, MERGED, "the requested DDS was merged");
    assert!(
        fixture.target.join(&artifact.origin_receipt).is_file(),
        "the proof remains beside the DDS"
    );
    assert_eq!(
        fixture.read(MERGED)?,
        rock().merged_bytes(),
        "exact merged bytes"
    );
    Ok(())
}
