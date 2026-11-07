// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::compare::{Class, Rule};
use super::fixtures::{self, GUID, OTHER, PUBLICATION, PublishedFixture, RESOURCE};
use super::path::Path;
use super::published;
use super::published_model::{BuildGate, Exclusion, ResourceJoin};
use super::units::Unit;

fn verify(fixture: &PublishedFixture, catalog: &Value) -> super::Result<published::Report> {
    published::verify(
        &fixture.load()?,
        catalog,
        &fixtures::build(),
        &[PUBLICATION],
        &|| false,
    )
}

#[test]
fn published_bytes_and_exact_channel_version_and_change_are_independent_gates() -> super::Result<()>
{
    let fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(0.1))])?;
    let dataset = fixture.load()?;
    let catalog = fixtures::catalog(vec![fixtures::item(GUID, json!(f64::from(0.1_f32)))]);
    let good = published::verify(
        &dataset,
        &catalog,
        &fixtures::build(),
        &[PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        good.gate,
        BuildGate::SamePublishedBuild,
        "the reviewed publication and actual Build identities match"
    );
    assert_eq!(
        good.fields
            .values()
            .map(super::compare::Summary::compared)
            .sum::<u64>(),
        1,
        "one owning raw field was compared"
    );
    for mismatch in 0..3 {
        let mut build = fixtures::build();
        match mismatch {
            0 => build.channel = "PTU".to_owned(),
            1 => build.change_number = "1235".to_owned(),
            _ => build.game_version = "1.2.4".to_owned(),
        }
        let report = published::verify(&dataset, &catalog, &build, &[PUBLICATION], &|| false)?;
        assert_eq!(
            report.gate,
            BuildGate::Mismatched,
            "any Build discriminator mismatch skips numerical assertions"
        );
        assert!(
            report.fields.is_empty(),
            "wrong-Build values never count as agreement"
        );
    }
    Ok(())
}

#[test]
fn an_unreviewed_revision_cannot_establish_numerical_build_evidence() -> super::Result<()> {
    let fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(0.1))])?;
    let catalog = fixtures::catalog(vec![fixtures::item(GUID, json!(0.1))]);
    let mut snapshot = fixture.snapshot.clone();
    snapshot.provenance.data_revision = "2".repeat(40);
    std::fs::write(
        fixture.root.join(super::published_model::METADATA),
        serde_json::to_vec(&snapshot)?,
    )?;
    let report = published::verify(
        &fixture.load()?,
        &catalog,
        &fixtures::build(),
        &[PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        report.gate,
        BuildGate::UnverifiedPublication,
        "unreviewed publication provenance is explicit"
    );
    assert!(
        report.fields.is_empty(),
        "invented revision/Build claims cannot enable numeric assertions"
    );
    Ok(())
}

#[test]
fn every_missing_side_and_explicit_selection_exclusion_is_accounted_for() -> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![
        fixtures::row(GUID, json!(1)),
        fixtures::row(OTHER, json!(2)),
    ])?;
    fixture.snapshot.selection = vec![GUID.to_owned(), OTHER.to_owned(), RESOURCE.to_owned()];
    fixture.save()?;
    let catalog = fixtures::catalog(vec![
        fixtures::item(GUID, json!(1)),
        fixtures::item(RESOURCE, json!(3)),
    ]);
    let report = verify(&fixture, &catalog)?;
    let field = report
        .fields
        .values()
        .next()
        .ok_or("field summary absent")?;
    assert_eq!(
        field.count(Class::Equal),
        1,
        "the shared GUID and raw value agree"
    );
    assert_eq!(
        field.count(Class::ReferenceOnly),
        1,
        "reference-only GUID/value is visible"
    );
    assert_eq!(
        field.count(Class::P4kxOnly),
        1,
        "catalog-only GUID/value is visible"
    );
    assert_eq!(
        report.reference_only_guids, 1,
        "source/export GUID joins have separate diagnostics"
    );
    assert_eq!(
        report.catalog_only_guids, 1,
        "the opposite missing-GUID direction is distinct"
    );
    fixture.snapshot.selection = vec![GUID.to_owned()];
    fixture.snapshot.exclusions = vec![Exclusion {
        guid: OTHER.to_owned(),
        reason: "reviewed development leftover".to_owned(),
    }];
    fixture.save()?;
    let report = verify(&fixture, &catalog)?;
    assert_eq!(
        report
            .fields
            .values()
            .next()
            .ok_or("field summary absent")?
            .count(Class::Excluded),
        2,
        "out-of-scope and explicit leftover GUIDs never masquerade as missing selected outputs"
    );
    Ok(())
}

#[test]
fn duplicate_guids_malformed_json_wrong_hash_and_missing_files_never_pass() -> super::Result<()> {
    let duplicate = PublishedFixture::new(vec![
        fixtures::row(GUID, json!(1)),
        fixtures::row(GUID, json!(1)),
    ])?;
    assert!(
        verify(
            &duplicate,
            &fixtures::catalog(vec![fixtures::item(GUID, json!(1))])
        )
        .is_err(),
        "duplicate GUID rows cannot replace a missing identity"
    );
    let fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    std::fs::write(fixture.root.join("synthetic.json"), b"not json")?;
    assert!(
        fixture.load().is_err(),
        "payload mutation cannot bypass its fingerprint"
    );
    std::fs::remove_file(fixture.root.join("synthetic.json"))?;
    assert!(
        fixture.load().is_err(),
        "a missing selected file fails before comparison"
    );
    std::fs::write(fixture.root.join(super::published_model::METADATA), b"{")?;
    assert!(fixture.load().is_err(), "malformed metadata is a failure");
    Ok(())
}

#[test]
fn a_shared_resource_key_retains_every_distinct_commodity_or_mineable_member() -> super::Result<()>
{
    let mut reference = fixtures::row(RESOURCE, json!(4));
    reference["resourceGuid"] = json!(RESOURCE);
    let mut fixture = PublishedFixture::new(vec![reference])?;
    fixture
        .snapshot
        .fields
        .first_mut()
        .ok_or("field mapping absent")?
        .resource_join = Some(ResourceJoin {
        reference: Path::fields(&["resourceGuid"]),
        catalog: Path::fields(&["resourceTypeGuid"]),
    });
    fixture.save()?;
    let mut first = fixtures::item(GUID, json!(4));
    first["resourceTypeGuid"] = json!(RESOURCE);
    let mut second = fixtures::item(OTHER, json!(4));
    second["resourceTypeGuid"] = json!(RESOURCE);
    let report = verify(&fixture, &fixtures::catalog(vec![first, second]))?;
    let summary = report
        .fields
        .values()
        .next()
        .ok_or("field summary absent")?;
    assert_eq!(
        summary.count(Class::Equal),
        2,
        "a ResourceType shared by two distinct GUIDs does not overwrite either member"
    );
    assert_eq!(
        summary.compared(),
        2,
        "all one-to-many raw comparisons are preserved"
    );
    Ok(())
}

#[test]
fn normalized_derived_and_curated_reference_values_remain_explicit() -> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    fixture
        .snapshot
        .fields
        .first_mut()
        .ok_or("field mapping absent")?
        .reference_unit = Unit::Seconds;
    fixture.save()?;
    let report = verify(
        &fixture,
        &fixtures::catalog(vec![fixtures::item(GUID, json!(1))]),
    )?;
    assert_eq!(
        report
            .fields
            .values()
            .next()
            .ok_or("field summary absent")?
            .count(Class::Uncomparable),
        1,
        "resource-unit mismatch remains a visible incompatible result"
    );
    let field = fixture
        .snapshot
        .fields
        .first_mut()
        .ok_or("field mapping absent")?;
    field.rule = Rule::Derived {
        reason: "producer's rounded combat formula".to_owned(),
    };
    fixture.save()?;
    let report = verify(
        &fixture,
        &fixtures::catalog(vec![fixtures::item(GUID, json!(99))]),
    )?;
    assert_eq!(
        report
            .fields
            .values()
            .next()
            .ok_or("field summary absent")?
            .count(Class::Derived),
        1,
        "derived reference values are not raw-field equality"
    );
    Ok(())
}

#[test]
fn a_later_reference_change_or_cancelled_walk_is_not_silently_accepted() -> super::Result<()> {
    let fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    let dataset = fixture.load()?;
    std::fs::write(fixture.root.join("synthetic.json"), b"changed")?;
    assert!(
        dataset.unchanged(&|| false).is_err(),
        "post-load mutation is detected"
    );
    assert!(
        super::published_io::Dataset::load(&fixture.root, &|| true).is_err(),
        "local reference reading is cancellable"
    );
    let root = p4kx_testkit::offline_seam::direct_root(&fixture.root)?;
    assert!(
        p4kx_testkit::offline_seam::inventory(
            &root,
            p4kx_testkit::offline_seam::Limits {
                path_bytes: 4096,
                entries: 10
            },
            None,
            || true
        )
        .is_err(),
        "local reference walks honor cancellation before enumerating"
    );
    Ok(())
}
