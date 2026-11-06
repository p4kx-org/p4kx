// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source-pinned log classifiers reject producer errors even with exit code zero.

use p4kx_testkit::TestEntry;

use super::TestResult;
use super::manifest_fixtures::{Fixture, freeze, selected, selection};
use super::reference_error::Code;
use super::reference_log_report::{Capability, Profile, Report, Stream};
use super::reference_log_scan::{self, Observed};
use super::reference_manifest::{PatchScope, SelectionMode, Tool};
use super::unp4k;

#[test]
fn every_pinned_tool_error_format_is_detected_in_its_captured_stream() -> TestResult {
    for (profile, stream, text) in [
        (
            Profile::Unp4kB492,
            Stream::Stdout,
            "Exception while extracting synthetic.bin: failed",
        ),
        (
            Profile::UnforgeB492,
            Stream::Stdout,
            "Error converting synthetic.cdf: failed",
        ),
        (
            Profile::Starbreaker450,
            Stream::Stderr,
            "Error exporting synthetic.xml: failed",
        ),
        (
            Profile::Starbreaker7d3,
            Stream::Stderr,
            "Error writing synthetic.json: failed",
        ),
    ] {
        let mut observed = Observed::default();
        reference_log_scan::scan(profile, stream, text.as_bytes(), &mut observed)?;
        assert_eq!(
            observed.errors.len(),
            1,
            "caught producer errors are not exit-code success"
        );
    }
    Ok(())
}

#[test]
fn a_forged_empty_error_report_does_not_hide_actual_stdout_failures() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    let mut manifest = fixture.raw(b"data")?;
    fixture.write(
        "evidence/stdout.log",
        b"Exception while extracting Data/a.bin: synthetic failure",
    )?;
    fixture.rehash(&mut manifest, "evidence/stdout.log")?;
    fixture.save(&manifest)?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("hidden producer error passed")?;
    assert_eq!(
        error.code,
        Code::Incomplete,
        "recompute error reports from captured bytes"
    );
    Ok(())
}

#[test]
fn unknown_revision_local_patch_and_unknown_log_lines_require_review() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    let original = fixture.raw(b"data")?;
    for mutation in 0..3 {
        let mut manifest = original.clone();
        match mutation {
            0 => manifest.producer.revision = "f".repeat(40),
            1 => {
                manifest.producer.patch_scope = PatchScope::Local {
                    description: "unreviewed synthetic patch".to_owned(),
                    digest: manifest.producer.binary.digest.clone(),
                }
            }
            _ => {
                fixture.write("evidence/stdout.log", b"unclassified diagnostic format")?;
                fixture.rehash(&mut manifest, "evidence/stdout.log")?;
            }
        }
        freeze(&mut manifest)?;
        fixture.save(&manifest)?;
        let error = unp4k::verify(&fixture.archive, &fixture.root)
            .err()
            .ok_or("unclassified capability passed")?;
        assert_eq!(
            error.code,
            Code::Incomplete,
            "unknown reporting capability is review_required"
        );
    }
    Ok(())
}

#[test]
fn duplicate_logical_source_lines_cannot_compensate_for_another_expected_source() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::stored("Data/a.bin", b"same".to_vec()),
        TestEntry::stored("Data/b.bin", b"same".to_vec()),
    ])?;
    let plan = Fixture::plan(
        Tool::Unp4k,
        vec![
            selected("Data/a.bin", "a.bin"),
            selected("Data/b.bin", "b.bin"),
        ],
        selection(SelectionMode::All),
        &["a.bin", "b.bin"],
    );
    let mut manifest = fixture.publish(
        plan,
        &[("a.bin", b"same".to_vec()), ("b.bin", b"same".to_vec())],
    )?;
    fixture.write(
        "evidence/stdout.log",
        b"Store | Plain | Data/a.bin\nStore | Plain | Data/a.bin\n",
    )?;
    fixture.rehash(&mut manifest, "evidence/stdout.log")?;
    fixture.save(&manifest)?;
    assert!(
        unp4k::verify(&fixture.archive, &fixture.root).is_err(),
        "equal file/log counts do not replace the missing source identity"
    );
    Ok(())
}

#[test]
fn an_unknown_capability_never_claims_a_complete_error_review() -> TestResult {
    let digest = super::reference_digest::reader(b"".as_slice(), 0)?;
    let report = Report {
        schema_version: 1,
        capability: Capability::ReviewRequired,
        stdout: digest.clone(),
        stderr: digest,
        errors: Vec::new(),
        review_required: Vec::new(),
    };
    assert!(
        !report.complete(),
        "empty logs do not prove an unclassified producer's error behavior"
    );
    Ok(())
}

#[test]
fn the_completion_marker_cannot_exceed_the_combined_finding_bound() -> TestResult {
    let mut observed = Observed::default();
    reference_log_scan::scan(
        Profile::Starbreaker450,
        Stream::Stdout,
        "unclassified\n".repeat(1000).as_bytes(),
        &mut observed,
    )?;
    let error = reference_log_scan::finish(Profile::Starbreaker450, &[], 0, &mut observed)
        .err()
        .ok_or("completion finding exceeded its declared bound")?;
    assert_eq!(
        error.code,
        Code::Bounds,
        "completion uses the same combined bound"
    );
    assert_eq!(
        observed.review.len(),
        1000,
        "no excess finding was appended"
    );
    Ok(())
}

#[test]
fn raw_progress_paths_are_bounded_before_the_owned_path_is_recorded() -> TestResult {
    let mut observed = Observed::default();
    let text = format!("Store | Plain | {}", "a".repeat(4097));
    let error = reference_log_scan::scan(
        Profile::Unp4kB492,
        Stream::Stdout,
        text.as_bytes(),
        &mut observed,
    )
    .err()
    .ok_or("oversized raw progress path was accepted")?;
    assert_eq!(
        error.code,
        Code::Bounds,
        "raw progress paths share metadata path bounds"
    );
    Ok(())
}
