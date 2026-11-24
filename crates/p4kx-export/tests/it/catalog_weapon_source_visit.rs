// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::equipment::{
    WeaponSourceSnapshot, visit_weapon_sources, visit_weapon_sources_from, with_weapon_sources,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability,
};
use p4kx_extraction::{CancelToken, DataForgeEntry};
use p4kx_testkit::TestRecord;
use serde_json::{Value, json};

use super::{Case, DcbOrigin, TestResult, WEAPON, WEAPON_ALIAS, common, fixture};

fn database() -> p4kx_testkit::DataForgeBuilder {
    fixture::build(Case::Unique).record(
        TestRecord::new(
            fixture::WEAPON,
            "EntityClassDefinition",
            "EntityClassDefinition.ProjectileVisitAlias",
            "libs/foundry/records/entities/scitem/fixture/projectile_visit_alias.xml",
        )
        .with_instance_index(0),
    )
}

#[test]
fn a_weapon_visit_matches_exact_physical_rows_and_their_original_inventories() -> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let guid = CatalogRecordSelection::Guid(common::guid(fixture::WEAPON).parse()?);
    let mut expected = Vec::new();
    for index in [WEAPON, WEAPON_ALIAS] {
        let point = with_weapon_sources(&archive, index, None, |snapshot| payload(&snapshot))?;
        expected.push((index, point));
    }

    let mut all = Vec::new();
    let summary = visit_weapon_sources(
        &archive,
        CatalogRecordSelection::All,
        None,
        |index, snapshot| -> TestResult {
            assert_eq!(snapshot.build(), archive.build());
            assert_eq!(snapshot.coverage(), &CatalogRecordCoverage::default());
            all.push((index, payload(&snapshot)));
            Ok(())
        },
    )?;
    assert_eq!(all, expected);
    assert_eq!(summary.owner_count(), 2);
    assert_eq!(summary.row_count(), 2);
    assert_eq!(summary.coverage(), &CatalogRecordCoverage::default());
    assert!(matches!(
        summary.availability(),
        DatasetAvailability::Available
    ));

    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let mut by_guid = Vec::new();
    let guid_summary =
        session.visit_weapon_sources(guid, None, |index, snapshot| -> TestResult {
            by_guid.push((index, payload(&snapshot)));
            Ok(())
        })?;
    assert_eq!(by_guid, expected);
    assert_eq!(guid_summary.owner_count(), 2);

    let mut by_index = Vec::new();
    let index_summary = visit_weapon_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(WEAPON_ALIAS),
        None,
        |index, snapshot| -> TestResult {
            by_index.push((index, payload(&snapshot)));
            Ok(())
        },
    )?;
    assert_eq!(by_index, expected[1..]);
    assert_eq!(index_summary.owner_count(), 1);
    Ok(())
}

#[test]
fn a_weapon_visit_stops_on_consumer_failure_or_cancellation() -> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let mut seen = Vec::new();
    let error = parsed.visit_weapon_sources(
        CatalogRecordSelection::All,
        None,
        |index, _| -> TestResult {
            seen.push(index);
            Err("the consumer stopped".into())
        },
    );
    assert_eq!(seen, [WEAPON]);
    assert_eq!(
        error.err().map(|error| error.to_string()).as_deref(),
        Some("the consumer stopped")
    );

    let cancel = CancelToken::new();
    let mut seen = Vec::new();
    let outcome = parsed.visit_weapon_sources(
        CatalogRecordSelection::All,
        Some(&cancel),
        |index, _| -> Result<(), CatalogError> {
            seen.push(index);
            cancel.cancel();
            Ok(())
        },
    );
    assert_eq!(seen, [WEAPON]);
    assert!(matches!(outcome, Err(CatalogError::Cancelled)));
    Ok(())
}

fn payload(snapshot: &WeaponSourceSnapshot<'_>) -> Value {
    let owners = snapshot.rows();
    json!({
        "actions": owners.iter().flat_map(|owner| &owner.weapon_actions).collect::<Vec<_>>(),
        "containers": owners.iter().flat_map(|owner| &owner.ammo_containers).collect::<Vec<_>>(),
        "bindings": owners.iter().flat_map(|owner| &owner.ammo_bindings).collect::<Vec<_>>(),
        "inventory": snapshot.source_inventory(),
    })
}
