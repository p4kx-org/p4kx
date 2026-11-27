// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Record coverage keeps standalone and embedded identities separate.

use p4kx_extraction::DataForgeEntry;

use super::TestResult;
use super::manifest_fixtures::{Fixture, freeze, record, selection};
use super::reference_error::Code;
use super::reference_manifest::{Location, SelectionMode, Tool};
use super::{samples, starbreaker};

fn payload(guid: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let doc = samples::resolve(guid).ok_or("synthetic Record missing")?;
    Ok(serde_json::to_vec(&samples::reference_json(&doc))?)
}

fn compare(fixture: &Fixture) -> Result<super::tally::Tally, Box<dyn std::error::Error>> {
    let dcb = DataForgeEntry::read(&fixture.archive)?;
    Ok(starbreaker::verify(
        &fixture.archive,
        &dcb.open()?,
        &fixture.root,
    )?)
}

#[test]
fn a_claimed_main_only_dataset_compares_its_declared_subset_without_demanding_all_records()
-> TestResult {
    let fixture = Fixture::records()?;
    let plan = Fixture::plan(
        Tool::Starbreaker,
        vec![record(samples::SAMPLE_GUID, "one.json", None)],
        selection(SelectionMode::MainOnly),
        &["one.json"],
    );
    fixture.publish(plan, &[("one.json", payload(samples::SAMPLE_GUID)?)])?;
    let tally = compare(&fixture)?;
    assert_eq!(
        tally.compared, 1,
        "the declared subset covers one GUID without attesting main disposition"
    );
    assert_eq!(
        tally.missing_in_reference, 0,
        "nonselected Records are outside this scope"
    );
    Ok(())
}

#[test]
fn duplicate_guid_content_does_not_replace_a_different_expected_record() -> TestResult {
    let fixture = Fixture::records()?;
    let plan = Fixture::plan(
        Tool::Starbreaker,
        vec![
            record(samples::SAMPLE_GUID, "one.json", None),
            record(samples::OTHER_GUID, "other.json", None),
        ],
        selection(SelectionMode::All),
        &["one.json", "other.json"],
    );
    fixture.publish(
        plan,
        &[
            ("one.json", payload(samples::SAMPLE_GUID)?),
            ("other.json", payload(samples::SAMPLE_GUID)?),
        ],
    )?;
    assert!(
        compare(&fixture).is_err(),
        "repeated GUID content cannot compensate for the missing expected GUID"
    );
    Ok(())
}

#[test]
fn one_record_can_have_several_distinct_declared_artifacts_without_double_counting() -> TestResult {
    let fixture = Fixture::records()?;
    let mut expected = record(samples::SAMPLE_GUID, "one.json", None);
    expected.targets.push(Location {
        path: "copy.json".to_owned(),
        subresource: None,
    });
    let plan = Fixture::plan(
        Tool::Starbreaker,
        vec![expected],
        selection(SelectionMode::Sampled),
        &["one.json", "copy.json"],
    );
    fixture.publish(
        plan,
        &[
            ("one.json", payload(samples::SAMPLE_GUID)?),
            ("copy.json", payload(samples::SAMPLE_GUID)?),
        ],
    )?;
    let tally = compare(&fixture)?;
    assert_eq!(
        tally.compared, 1,
        "two declared artifacts cover one logical Record"
    );
    assert_eq!(
        tally.equal, 1,
        "identical copies do not inflate equal-source counts"
    );
    Ok(())
}

#[test]
fn a_shared_file_requires_distinct_proven_record_locations() -> TestResult {
    let fixture = Fixture::records()?;
    let other = samples::resolve(samples::OTHER_GUID).ok_or("other Record missing")?;
    let mut json = samples::reference_json(&samples::ours());
    json[super::starbreaker_json::RECORD_VALUE_KEY]["link"] = samples::embedded_json(&other);
    let plan = Fixture::plan(
        Tool::Starbreaker,
        vec![
            record(samples::SAMPLE_GUID, "shared.json", None),
            record(
                samples::OTHER_GUID,
                "shared.json",
                Some("/_RecordValue_/link"),
            ),
        ],
        selection(SelectionMode::Sampled),
        &["shared.json"],
    );
    let manifest = fixture.publish(plan, &[("shared.json", serde_json::to_vec(&json)?)])?;
    let tally = compare(&fixture)?;
    assert_eq!(
        tally.compared, 2,
        "standalone and declared embedded GUIDs are distinct coverage"
    );
    let mut wrong = manifest;
    let embedded = wrong
        .expected
        .get_mut(1)
        .and_then(|record| record.targets.first_mut())
        .ok_or("embedded location missing")?;
    embedded.subresource = Some("/missing".to_owned());
    freeze(&mut wrong)?;
    fixture.save(&wrong)?;
    assert!(
        compare(&fixture).is_err(),
        "an absent declared subresource is a coverage failure"
    );
    Ok(())
}

#[test]
fn a_stale_dcb_fingerprint_is_rejected_before_record_value_comparison() -> TestResult {
    let fixture = Fixture::records()?;
    let plan = Fixture::plan(
        Tool::Starbreaker,
        vec![record(samples::SAMPLE_GUID, "one.json", None)],
        selection(SelectionMode::MainOnly),
        &["one.json"],
    );
    let mut manifest = fixture.publish(plan, &[("one.json", payload(samples::SAMPLE_GUID)?)])?;
    manifest
        .source
        .dcb
        .as_mut()
        .ok_or("DCB fingerprint absent")?
        .fingerprint
        .digest
        .value = "0".repeat(64);
    freeze(&mut manifest)?;
    fixture.save(&manifest)?;
    let dcb = DataForgeEntry::read(&fixture.archive)?;
    let error = starbreaker::verify(&fixture.archive, &dcb.open()?, &fixture.root)
        .err()
        .ok_or("stale DCB passed")?;
    assert_eq!(
        error.code,
        Code::Provenance,
        "DCB source identity is measured, not trusted as a claim"
    );
    Ok(())
}
