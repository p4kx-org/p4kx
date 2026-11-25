// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::ShipSourceSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::DataForgeEntry;
use serde_json::Value;

use super::fixture::{self, Case};
use crate::catalog_ship_base::source_sdk_walk;
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn direct_ship_sdk_cells_match_canonical_available_and_unavailable_aggregates() -> TestResult {
    for (name, case) in cases() {
        let (_folder, archive) = fixture::open(case)?;
        let entry = DataForgeEntry::read(&archive)?;
        let catalog = fixture::catalog(&archive)?;
        full_proof::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::Selected(&[]),
            None,
        )?
        .with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
            verify(&snapshot, &catalog, case, name)
        })??;
        proof::closure(&catalog)?;
    }
    Ok(())
}

const fn cases() -> [(&'static str, Case); 14] {
    [
        ("parts", Case::Parts),
        ("physics", Case::Physics),
        ("empty", Case::Empty),
        ("missing", Case::Missing),
        ("malformed", Case::Malformed),
        ("invalid authored values", Case::InvalidValues),
        (
            "invalid authored values with physics",
            Case::InvalidValuesWithPhysics,
        ),
        ("unreadable", Case::Unreadable),
        ("unreadable with physics", Case::UnreadableWithPhysics),
        ("broken patch", Case::BrokenPatch),
        ("broken patch with values", Case::BrokenPatchWithValues),
        ("repaired with mass", Case::RepairedWithMass),
        ("repaired without mass", Case::RepairedWithoutMass),
        ("rounded positive zero", Case::RoundedZero),
    ]
}

fn verify(
    snapshot: &ShipSourceSnapshot<'_>,
    catalog: &Value,
    case: Case,
    name: &str,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err(format!("one physical Ship row for {name}").into());
    };
    let [canonical] = proof::rows(catalog, "/ships")? else {
        return Err("one canonical Ship row".into());
    };
    source_sdk_walk::row(row, snapshot.source_inventory(), canonical)?;
    assert_eq!(
        row.source().record_index().value(),
        Some(&0),
        "{name} physical owner"
    );
    assert_eq!(
        row.source().guid().origin().contains_encrypted_source(),
        Some(false),
        "{name} original DCB stays plain"
    );
    metadata(snapshot, name);
    dependencies(snapshot, case, name);
    Ok(())
}

fn metadata(snapshot: &ShipSourceSnapshot<'_>, name: &str) {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "{name} declared Ship family exists"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "{name} requests no locale"
    );
    assert!(
        snapshot.available_languages().is_empty(),
        "{name} reads no locale"
    );
    assert!(
        snapshot.localization_notes().is_empty(),
        "{name} adds no locale notes"
    );
    assert!(
        snapshot.public_names().is_empty(),
        "{name} has no authored public keys"
    );
    assert!(
        snapshot.game_texts().is_empty(),
        "{name} has no authored protected keys"
    );
    assert!(snapshot.build().is_none(), "{name} has no fabricated Build");
    assert!(
        snapshot.coverage().unreadable_headers.is_empty(),
        "{name} readable original header"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "{name} readable original owner body"
    );
}

fn dependencies(snapshot: &ShipSourceSnapshot<'_>, case: Case, name: &str) {
    let healthy = matches!(
        case,
        Case::Parts | Case::Physics | Case::Empty | Case::RoundedZero
    );
    assert_eq!(
        snapshot.dependency_coverage().is_complete(),
        healthy,
        "{name} dependency completeness is independent of retained values"
    );
    if healthy {
        assert!(
            snapshot.dependency_coverage().failed_records().is_empty(),
            "{name} has no failed dependency Records"
        );
        assert!(
            snapshot.dependency_coverage().failed_entries().is_empty(),
            "{name} has no failed dependency Entries"
        );
        assert_eq!(
            snapshot.dependency_coverage().incomplete_fields(),
            0,
            "{name} has no uninspected dependency fields"
        );
    }
}

#[test]
fn direct_ship_sdk_keeps_each_actual_dcb_origin_independent_of_encrypted_xml() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = fixture::open_with_origin(Case::Parts, origin)?;
        let catalog = fixture::catalog(&archive)?;
        full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
            .with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
                let [row] = snapshot.rows() else {
                    return Err("one original Ship with selected DCB origin".into());
                };
                let [canonical] = proof::rows(&catalog, "/ships")? else {
                    return Err("one canonical Ship with selected DCB origin".into());
                };
                source_sdk_walk::row(row, snapshot.source_inventory(), canonical)?;
                assert_eq!(
                    row.source().guid().origin().contains_encrypted_source(),
                    marker,
                    "the DCB's unknown origin never becomes plain"
                );
                assert_eq!(
                    row.mass().origin().contains_encrypted_source(),
                    Some(true),
                    "a known encrypted XML contribution stays true even with unknown DCB origin"
                );
                assert!(
                    row.mass().origin().source_set_complete(),
                    "encryption knowledge does not change complete original inputs"
                );
                Ok::<_, Box<dyn std::error::Error>>(())
            })??;
    }
    Ok(())
}
