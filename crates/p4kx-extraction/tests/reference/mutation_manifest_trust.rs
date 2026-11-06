// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic trust failures are rejected before any comparison or allowance.

use p4kx_testkit::TestEntry;

use super::TestResult;
use super::manifest_fixtures::{Fixture, freeze};
use super::reference_error::Code;
use super::reference_manifest::{SelectionMode, Tool};
use super::reference_run::{self, Dataset};
use super::unp4k;

fn fixture() -> Result<(Fixture, super::reference_manifest::Manifest), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(vec![
        TestEntry::stored("Data/a.bin", b"prefix\0\0".to_vec()).encrypted(),
    ])?;
    let manifest = fixture.raw(b"prefix")?;
    Ok((fixture, manifest))
}

#[test]
fn a_configured_empty_root_fails_and_an_unset_root_never_runs() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    let Err(error) = Dataset::load(&fixture.root, &fixture.archive, Tool::Unp4k) else {
        return Err("empty configured root passed".into());
    };
    assert_eq!(
        error.code,
        Code::MissingManifest,
        "configured roots require manifests"
    );
    let invoked = std::cell::Cell::new(false);
    reference_run::optional(None, "synthetic optional input", |_| {
        invoked.set(true);
        Err("an unset reference ran".into())
    })?;
    assert!(
        !invoked.get(),
        "an unset optional input skips without touching sources"
    );
    Ok(())
}

#[test]
fn validated_direct_zero_trimming_is_allowed_but_incomplete_producers_are_not() -> TestResult {
    let (fixture, mut manifest) = fixture()?;
    let tally = unp4k::verify(&fixture.archive, &fixture.root)?;
    assert_eq!(
        tally.allowed_items, 1,
        "valid producer-specific trim is separately counted"
    );
    assert_eq!(tally.equal, 0, "allowed loss is not exact equality");
    manifest
        .completion
        .errors
        .push("synthetic decode failure".to_owned());
    fixture.save(&manifest)?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("partial producer passed")?;
    assert_eq!(
        error.code,
        Code::Incomplete,
        "producer errors stop before allowance evaluation"
    );
    Ok(())
}

#[test]
fn source_build_archive_and_inventory_claims_are_recomputed() -> TestResult {
    let (fixture, original) = fixture()?;
    for field in 0..3 {
        let mut manifest = original.clone();
        match field {
            0 => manifest.source.build.change_number = "stale".to_owned(),
            1 => manifest.source.archive.digest.value = "0".repeat(64),
            _ => manifest.source.inventory.value = "0".repeat(64),
        }
        freeze(&mut manifest)?;
        fixture.save(&manifest)?;
        let error = unp4k::verify(&fixture.archive, &fixture.root)
            .err()
            .ok_or("stale source passed")?;
        assert_eq!(
            error.code,
            Code::Provenance,
            "source claims are matched to local measurements"
        );
    }
    Ok(())
}

#[test]
fn required_producer_evidence_and_success_receipt_cannot_be_omitted() -> TestResult {
    let (fixture, original) = fixture()?;
    for mutation in 0..5 {
        let mut manifest = original.clone();
        match mutation {
            0 => manifest.producer.revision.clear(),
            1 => manifest.producer.binary.digest.value.clear(),
            2 => manifest.completion.exit_code = Some(1),
            3 => manifest.completion.owner_asserts_success = false,
            _ => manifest.preparation.finalized = false,
        }
        freeze(&mut manifest)?;
        fixture.save(&manifest)?;
        assert!(
            unp4k::verify(&fixture.archive, &fixture.root).is_err(),
            "missing provenance or unsuccessful producer cannot pass"
        );
    }
    Ok(())
}

#[test]
fn a_producer_error_line_fails_even_when_the_owner_receipt_claims_success() -> TestResult {
    let (fixture, mut manifest) = fixture()?;
    fixture.write(
        "evidence/stderr.log",
        b"Unhandled exception. synthetic producer error",
    )?;
    fixture.rehash(&mut manifest, "evidence/stderr.log")?;
    fixture.save(&manifest)?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("error log passed")?;
    assert_eq!(
        error.code,
        Code::Incomplete,
        "captured producer errors prevent complete verification"
    );
    Ok(())
}

#[test]
fn declared_empty_selection_is_explicit_and_is_not_independent_verification() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    let mut selection = super::manifest_fixtures::selection(SelectionMode::Sampled);
    selection.empty_reason = Some("synthetic explicitly empty sample".to_owned());
    let plan = Fixture::plan(Tool::Unp4k, Vec::new(), selection, &[]);
    fixture.publish(plan, &[])?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("empty comparison passed")?;
    assert_eq!(
        error.code,
        Code::Coverage,
        "declared empty scope reports no independent comparisons"
    );
    Ok(())
}

#[test]
fn changed_preconversion_mapping_is_rejected_without_relabelling_survivors() -> TestResult {
    let (fixture, mut manifest) = fixture()?;
    let target = manifest
        .expected
        .first_mut()
        .and_then(|source| source.targets.first_mut())
        .ok_or("target missing")?;
    target.path = "renamed.bin".to_owned();
    fixture.save(&manifest)?;
    assert!(
        unp4k::verify(&fixture.archive, &fixture.root).is_err(),
        "post-conversion mapping changes cannot turn survivors into complete coverage"
    );
    Ok(())
}
