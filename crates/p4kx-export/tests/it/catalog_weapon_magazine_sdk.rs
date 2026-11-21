// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    WeaponMagazineDependencySnapshot, WeaponMagazineOutcome, WeaponMagazineSnapshot,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog, build_catalog_from};
use p4kx_extraction::Filter;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::{DcbOrigin, TestResult, fixture, proof, resolution, source};
use crate::commodity_v3_fixture as common;
use crate::health_origin_fixture::{self as archive_fixture, Fixture, Input};

const ROWS: &str = "/projections/weaponMagazineResolutions/rows";

#[test]
fn magazine_sdk_matches_canonical_own_and_linked_routes() -> TestResult {
    let builder = fixture::magazine_database(fixture::MagazineCase::Resolved);
    for (origin, marker) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&builder, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let session =
            CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        for (index, expected) in [
            (source::DIRECT_INDEX, Expected::Own),
            (source::PERSONAL_INDEX, Expected::Linked),
        ] {
            parsed.with_weapon_magazine_resolutions(index, None, |snapshot| {
                compare(&snapshot, &catalog, index, expected, marker)
            })??;
        }
    }
    Ok(())
}

#[test]
fn magazine_dependency_admission_uses_the_same_catalog_item_filter() -> TestResult {
    let builder = fixture::magazine_database(fixture::MagazineCase::Resolved);
    let (_folder, archive) = common::open(&builder, DcbOrigin::Encrypted)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let filter = Filter::new("fixture_personal");

    for (item_filter, expected) in [(None, 0), (Some(&filter), 1)] {
        parsed.with_weapon_magazine_dependencies(
            source::PERSONAL_INDEX,
            item_filter,
            None,
            |snapshot| -> TestResult {
                let [owner] = snapshot.rows() else {
                    return Err("one primary attempt for the selected weapon".into());
                };
                assert_eq!(
                    owner.dependencies.len(),
                    expected,
                    "dependency admission tracks the requested Catalog filter"
                );
                Ok(())
            },
        )??;
    }
    parsed.with_weapon_magazine_dependencies(
        source::DIRECT_INDEX,
        Some(&filter),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "a filter may exclude the explicitly named caller"
            );
        },
    )?;
    Ok(())
}

#[test]
fn magazine_sdk_keeps_an_unreadable_reached_target_scoped_to_its_caller() -> TestResult {
    let mut bytes = fixture::magazine_database(fixture::MagazineCase::Resolved).build();
    resolution::break_instance(&mut bytes, source::PRIMARY_INDEX)?;
    let input = Fixture::open(
        &bytes,
        Input::Encrypted,
        vec![TestEntry::stored(
            archive_fixture::REGISTRY,
            b"<LayerBackups/>",
        )],
    )?;
    let session = CatalogQuerySession::from_entry(
        &input.archive,
        &input.entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let filter = Filter::parse("*/fixture_personal")?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&input.archive, &input.entry, &options)?.json())?;
    parsed.with_weapon_magazine_dependencies(
        source::PERSONAL_INDEX,
        Some(&filter),
        None,
        |snapshot| verify_unreadable_dependency(&snapshot),
    )??;
    parsed.with_weapon_magazine_resolutions(source::PERSONAL_INDEX, None, |snapshot| {
        compare(
            &snapshot,
            &catalog,
            source::PERSONAL_INDEX,
            Expected::Unreadable,
            true,
        )
    })??;
    parsed.with_weapon_magazine_resolutions(source::DIRECT_INDEX, None, |snapshot| {
        verify_unrelated_own_route(&snapshot)
    })?
}

fn verify_unreadable_dependency(snapshot: &WeaponMagazineDependencySnapshot<'_>) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one attempted magazine dependency".into());
    };
    assert!(
        owner.dependencies.is_empty(),
        "an unreadable magazine never fabricates dependency rows"
    );
    assert_eq!(
        owner.dependency_coverage.unreadable_bodies,
        vec![source::PRIMARY_INDEX],
        "the reached physical body remains visible"
    );
    assert_eq!(
        snapshot.coverage().incomplete_fields,
        vec![source::PERSONAL_INDEX],
        "the selected weapon reports incomplete dependency coverage"
    );
    Ok(())
}

fn verify_unrelated_own_route(snapshot: &WeaponMagazineSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one own route despite an unreadable external target".into());
    };
    assert!(
        matches!(
            row.outcome().value(),
            Some(WeaponMagazineOutcome::OwnContainer { .. })
        ),
        "the own route does not activate the external target"
    );
    assert!(
        row.outcome().origin().source_set_complete(),
        "the own selection is complete"
    );
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "the unreadable target stays out of scope"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum Expected {
    Own,
    Linked,
    Unreadable,
}

fn compare(
    snapshot: &WeaponMagazineSnapshot<'_>,
    catalog: &Value,
    index: u32,
    expected: Expected,
    marker: bool,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one observed primary route for the physical Item".into());
    };
    let outcome = row.outcome();
    assert_eq!(
        row.owner().record_index().value(),
        Some(&index),
        "the actual weapon owner"
    );
    assert_eq!(
        outcome.origin().contains_encrypted_source(),
        Some(marker),
        "the actual source Entry"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "primary routes require no localization"
    );
    compare_outcome(expected, outcome.value())?;
    let incomplete = matches!(expected, Expected::Unreadable);
    assert_eq!(
        outcome.origin().source_set_complete(),
        !incomplete,
        "the actual attempt's coverage"
    );
    assert_eq!(
        snapshot.coverage().incomplete_fields,
        if incomplete { vec![index] } else { Vec::new() },
        "the caller reports the observed incomplete selection"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "the queried owner body was read"
    );
    let scoped = json!({
        "projections": { "weaponMagazineResolutions": { "rows": snapshot.rows() } },
        "provenance": snapshot.source_inventory(),
    });
    let [observed] = proof::rows(&scoped, ROWS)? else {
        return Err("one serialized SDK route".into());
    };
    let expected = proof::rows(catalog, ROWS)?
        .iter()
        .find(|row| row.pointer("/ownerRecordIndex/value") == Some(&json!(index)))
        .ok_or("the same physical caller in the Catalog")?;
    assert_eq!(
        observed, expected,
        "the exact canonical row, values and physical source uses"
    );
    proof::closure(&scoped)
}

fn compare_outcome(expected: Expected, outcome: Option<&WeaponMagazineOutcome>) -> TestResult {
    match (expected, outcome) {
        (Expected::Own, Some(WeaponMagazineOutcome::OwnContainer { container_ordinal })) => {
            assert_eq!(
                container_ordinal.value(),
                Some(&2),
                "the selected owner container slot"
            );
        }
        (
            Expected::Linked,
            Some(WeaponMagazineOutcome::LinkedContainer {
                magazine_record_index,
                container_ordinal,
            }),
        ) => {
            assert_eq!(
                magazine_record_index.value(),
                Some(&source::PRIMARY_INDEX),
                "the reached physical magazine"
            );
            assert_eq!(
                container_ordinal.value(),
                Some(&1),
                "the reached original container slot"
            );
        }
        (
            Expected::Unreadable,
            Some(WeaponMagazineOutcome::LinkedTargetUnreadable {
                magazine_record_index,
                ..
            }),
        ) => {
            assert_eq!(
                magazine_record_index.value(),
                Some(&source::PRIMARY_INDEX),
                "the attempted target retains its header"
            );
        }
        _ => return Err("the typed SDK outcome must match the original route".into()),
    }
    Ok(())
}
