// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Preparation freezes source identity before conversion and never promotes failed runs.

use p4kx_testkit::TestEntry;

use super::TestResult;
use super::manifest_fixtures::{Fixture, selected, selection, success};
use super::reference_error::Code;
use super::reference_manifest::{SelectionMode, Tool};
use super::{reference_load, reference_prepare};

fn plan() -> reference_prepare::Plan {
    Fixture::plan(
        Tool::Unp4k,
        vec![selected("Data/a.bin", "a.bin")],
        selection(SelectionMode::Filtered),
        &["a.bin"],
    )
}

fn outputs(fixture: &Fixture) -> TestResult {
    fixture.write("a.bin", b"data")?;
    fixture.write("evidence/stdout.log", b"Store | Plain | Data/a.bin\n")?;
    fixture.write("evidence/stderr.log", b"")?;
    Ok(())
}

#[test]
fn changing_the_producer_binary_after_preparation_blocks_finalization() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    reference_prepare::begin(&fixture.archive, &fixture.root, plan(), &fixture.binary)?;
    outputs(&fixture)?;
    std::fs::write(&fixture.binary, b"changed producer")?;
    let error =
        reference_prepare::finalize(&fixture.archive, &fixture.root, &fixture.binary, success())
            .err()
            .ok_or("changed binary was finalized")?;
    assert_eq!(
        error.code,
        Code::Provenance,
        "binary identity must agree before and after conversion"
    );
    let pending = reference_load::read(&fixture.root)?.manifest;
    assert!(
        !pending.preparation.finalized,
        "failure preserves an explicitly incomplete preparation"
    );
    Ok(())
}

#[test]
fn changing_the_archive_after_preparation_blocks_finalization() -> TestResult {
    use std::io::Write as _;
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    reference_prepare::begin(&fixture.archive, &fixture.root, plan(), &fixture.binary)?;
    outputs(&fixture)?;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(fixture.archive.path())?;
    file.write_all(b"changed")?;
    drop(file);
    let error =
        reference_prepare::finalize(&fixture.archive, &fixture.root, &fixture.binary, success())
            .err()
            .ok_or("changed Archive was finalized")?;
    assert_eq!(
        error.code,
        Code::Changed,
        "the Archive watch rejects mutation before source verification"
    );
    Ok(())
}

#[test]
fn missing_payloads_cannot_be_promoted_by_a_success_receipt() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    reference_prepare::begin(&fixture.archive, &fixture.root, plan(), &fixture.binary)?;
    fixture.write("evidence/stdout.log", b"Store | Plain | Data/a.bin\n")?;
    fixture.write("evidence/stderr.log", b"")?;
    let error =
        reference_prepare::finalize(&fixture.archive, &fixture.root, &fixture.binary, success())
            .err()
            .ok_or("missing payload was finalized")?;
    assert_eq!(
        error.code,
        Code::Coverage,
        "success claims cannot replace a missing selected output"
    );
    let pending = reference_load::read(&fixture.root)?.manifest;
    assert!(
        !pending.preparation.finalized,
        "missing output leaves the preparation incomplete"
    );
    Ok(())
}

#[test]
fn hard_linked_outputs_cannot_inflate_independent_artifact_coverage() -> TestResult {
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
    fixture.publish(
        plan,
        &[("a.bin", b"same".to_vec()), ("b.bin", b"same".to_vec())],
    )?;
    super::unp4k::verify(&fixture.archive, &fixture.root)?;
    std::fs::remove_file(fixture.root.join("b.bin"))?;
    std::fs::hard_link(fixture.root.join("a.bin"), fixture.root.join("b.bin"))?;
    let error = super::unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("hard-link alias passed")?;
    assert_eq!(
        error.code,
        Code::Path,
        "physical aliases are detected even when hashes and file counts agree"
    );
    Ok(())
}
