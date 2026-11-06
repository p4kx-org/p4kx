// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Coverage follows pre-conversion logical identities, not surviving file counts.

use p4kx_testkit::TestEntry;

use super::TestResult;
use super::manifest_fixtures::{Fixture, selected, selection};
use super::reference_error::Code;
use super::reference_manifest::{SelectionMode, Tool};
use super::reference_prepare;
use super::{unforge, unp4k};

#[test]
fn same_stem_cdf_and_chrparams_keep_their_distinct_sources_and_trees() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::stored("Data/shared.cdf", b"<model value=\"cdf\"/>".to_vec()),
        TestEntry::stored(
            "Data/shared.chrparams",
            b"<model value=\"chrparams\"/>".to_vec(),
        ),
    ])?;
    let expected = vec![
        selected("Data/shared.cdf", "cdf/shared.xml"),
        selected("Data/shared.chrparams", "chrparams/shared.xml"),
    ];
    let plan = Fixture::plan(
        Tool::Unforge,
        expected,
        selection(SelectionMode::All),
        &["cdf/shared.xml", "chrparams/shared.xml"],
    );
    fixture.publish(
        plan,
        &[
            ("cdf/shared.xml", b"<model value=\"cdf\"/>".to_vec()),
            (
                "chrparams/shared.xml",
                b"<model value=\"chrparams\"/>".to_vec(),
            ),
        ],
    )?;
    let tally = unforge::verify(&fixture.archive, &fixture.root)?;
    assert_eq!(tally.compared, 2, "both logical XML sources are compared");
    assert_eq!(
        tally.equal, 2,
        "same-stem outputs map to their actual source extensions"
    );
    Ok(())
}

#[test]
fn a_same_stem_overwrite_collision_is_detected_before_conversion() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::stored("Data/shared.cdf", b"<cdf/>".to_vec()),
        TestEntry::stored("Data/shared.chrparams", b"<params/>".to_vec()),
    ])?;
    let plan = Fixture::plan(
        Tool::Unforge,
        vec![
            selected("Data/shared.cdf", "shared.xml"),
            selected("Data/shared.chrparams", "shared.xml"),
        ],
        selection(SelectionMode::All),
        &["shared.xml"],
    );
    let error = reference_prepare::begin(&fixture.archive, &fixture.root, plan, &fixture.binary)
        .err()
        .ok_or("overwrite plan passed")?;
    assert_eq!(
        error.code,
        Code::Identity,
        "two logical sources cannot own one XML output location"
    );
    Ok(())
}

#[test]
fn source_copy_artifacts_are_verified_against_the_selected_archive_entry() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored(
        "Data/source.cdf",
        b"<source/>".to_vec(),
    )])?;
    let expected = selected("Data/source.cdf", "source.xml");
    let mut plan = Fixture::plan(
        Tool::Unforge,
        vec![expected.clone()],
        selection(SelectionMode::Filtered),
        &["source.xml"],
    );
    plan.artifacts.push(super::reference_manifest::Artifact {
        path: "input/source.cdf".to_owned(),
        role: super::reference_manifest::ArtifactRole::SourceCopy {
            identity: expected.identity,
        },
        fingerprint: None,
    });
    fixture.write("input/source.cdf", b"<wrong/>")?;
    let error = reference_prepare::begin(&fixture.archive, &fixture.root, plan, &fixture.binary)
        .err()
        .ok_or("wrong source copy passed")?;
    assert_eq!(
        error.code,
        Code::Provenance,
        "source copies cannot silently substitute another input"
    );
    Ok(())
}

#[test]
fn deleting_or_renaming_a_selected_output_fails_the_complete_inventory() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    fixture.raw(b"data")?;
    std::fs::rename(
        fixture.root.join("raw/a.bin"),
        fixture.root.join("raw/renamed.bin"),
    )?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("renamed output passed")?;
    assert_eq!(
        error.code,
        Code::Coverage,
        "renamed output is both missing and extra, not coverage"
    );
    Ok(())
}

#[test]
fn an_extra_json_or_nested_manifest_is_never_excluded_from_payload_inventory() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    fixture.raw(b"data")?;
    for path in [
        "extra.json",
        "nested/.p4kx-reference.json",
        "unexpected.log",
    ] {
        fixture.write(path, b"extra")?;
        let error = unp4k::verify(&fixture.archive, &fixture.root)
            .err()
            .ok_or("extra artifact passed")?;
        assert_eq!(
            error.code,
            Code::Coverage,
            "only the exact root manifest is excluded"
        );
        std::fs::remove_file(fixture.root.join(path))?;
    }
    Ok(())
}

#[test]
fn a_duplicate_identity_cannot_compensate_for_another_selected_source() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::stored("Data/a.bin", b"same".to_vec()),
        TestEntry::stored("Data/b.bin", b"same".to_vec()),
    ])?;
    let plan = Fixture::plan(
        Tool::Unp4k,
        vec![
            selected("Data/a.bin", "a.bin"),
            selected("Data/a.bin", "b.bin"),
        ],
        selection(SelectionMode::All),
        &["a.bin", "b.bin"],
    );
    let error = reference_prepare::begin(&fixture.archive, &fixture.root, plan, &fixture.binary)
        .err()
        .ok_or("duplicate identity passed")?;
    assert_eq!(
        error.code,
        Code::Identity,
        "coverage counts unique sources, not two files"
    );
    Ok(())
}

#[test]
fn sampled_scope_is_valid_but_all_scope_cannot_omit_an_available_source() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::stored("Data/a.bin", b"data".to_vec()),
        TestEntry::stored("Data/b.bin", b"unselected".to_vec()),
    ])?;
    let manifest = fixture.raw(b"data")?;
    let tally = unp4k::verify(&fixture.archive, &fixture.root)?;
    assert_eq!(
        tally.compared, 1,
        "the explicitly bounded subset is compared"
    );
    assert_eq!(
        tally.missing_in_reference, 0,
        "unselected sources are not missing selected outputs"
    );
    let mut all = manifest;
    all.selection.mode = SelectionMode::All;
    super::manifest_fixtures::freeze(&mut all)?;
    fixture.save(&all)?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("partial all-source scope passed")?;
    assert_eq!(
        error.code,
        Code::Coverage,
        "all-source scope must match the complete source universe"
    );
    Ok(())
}

#[test]
fn a_preexisting_payload_cannot_be_post_hoc_certified_as_preconversion_selection() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    let plan = Fixture::plan(
        Tool::Unp4k,
        vec![selected("Data/a.bin", "a.bin")],
        selection(SelectionMode::Filtered),
        &["a.bin"],
    );
    fixture.write("a.bin", b"data")?;
    let error = reference_prepare::begin(&fixture.archive, &fixture.root, plan, &fixture.binary)
        .err()
        .ok_or("post-hoc survivor plan passed")?;
    assert_eq!(
        error.code,
        Code::Incomplete,
        "freeze expectations before producing reference payloads"
    );
    Ok(())
}
