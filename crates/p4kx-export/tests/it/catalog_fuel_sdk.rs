// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::fuel::{
    self, ResourceCapacity, ResourceContainerSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
    ResourceFloat32,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::{CONTAINERS, INTAKES, TANKS, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn fuel_source_queries_keep_original_rows_links_and_origin() -> TestResult {
    let database = fixture::database(fixture::Layout::Full);
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted] {
        let (_directory, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = CatalogQuerySession::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let parsed = session.parsed(None)?;
        check_components(&parsed, &catalog)?;
        fuel::with_resource_containers_from(
            &archive,
            &entry,
            CatalogRecordSelection::All,
            None,
            |snapshot| check_containers(&snapshot, &catalog),
        )??;
    }
    Ok(())
}

fn check_components(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    parsed.with_fuel_tanks(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "both fuel tanks are readable"
            );
            assert!(
                snapshot.requested_languages().is_empty(),
                "the fuel query reads no locale payloads"
            );
            compare_rows(
                TANKS,
                snapshot.rows(),
                &json!({"provenance": snapshot.source_inventory()}),
                catalog,
            )
        },
    )??;
    parsed.with_fuel_intakes(
        CatalogRecordSelection::RecordIndex(3),
        None,
        |snapshot| -> TestResult {
            let [intake] = snapshot.rows() else {
                return Err("the selected physical intake".into());
            };
            assert_eq!(
                intake.owner().record_index().value(),
                Some(&3),
                "the exact owner index was selected"
            );
            assert_eq!(
                intake.minimum_rate.value().map(ResourceFloat32::raw_bits),
                Some((-0.0_f32).to_bits()),
                "negative zero retains its original representation"
            );
            compare_rows(
                INTAKES,
                snapshot.rows(),
                &json!({"provenance": snapshot.source_inventory()}),
                catalog,
            )
        },
    )??;
    Ok(())
}

fn check_containers(snapshot: &ResourceContainerSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "fully inspected dangling source links stay visible"
    );
    let has_unknown_capacity = snapshot.rows().iter().any(|row| {
        matches!(
            row.capacity.value(),
            Some(ResourceCapacity::UnknownUnit { .. })
        )
    });
    assert!(
        has_unknown_capacity,
        "a future capacity subtype retains its original representation"
    );
    compare_rows(
        CONTAINERS,
        snapshot.rows(),
        &json!({"provenance": snapshot.source_inventory()}),
        catalog,
    )
}

#[test]
fn malformed_fuel_intake_isolated_from_other_owner_components() -> TestResult {
    let database = fixture::invalid_intake_database();
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let selection = CatalogRecordSelection::RecordIndex(0);
    let parsed = session.parsed(None)?;
    parsed.with_fuel_intakes(selection, None, |snapshot| -> TestResult {
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [0],
            "the malformed intake is incomplete"
        );
        let [intake] = snapshot.rows() else {
            return Err("the original malformed intake".into());
        };
        proof::cell(&serde_json::to_value(intake)?, "fuelPushRate", "invalid")?;
        compare_rows(
            INTAKES,
            snapshot.rows(),
            &json!({"provenance": snapshot.source_inventory()}),
            &catalog,
        )
    })??;
    parsed.with_fuel_tanks(selection, None, |snapshot| -> TestResult {
        assert!(
            snapshot.coverage().incomplete_fields.is_empty(),
            "the malformed intake does not enter the tank scope"
        );
        compare_rows(
            TANKS,
            snapshot.rows(),
            &json!({"provenance": snapshot.source_inventory()}),
            &catalog,
        )
    })??;
    Ok(())
}

fn compare_rows(
    path: &str,
    actual: &[impl serde::Serialize],
    inventory: &Value,
    catalog: &Value,
) -> TestResult {
    let actual = proof::with_origins(serde_json::to_value(actual)?, inventory)?;
    let expected = proof::with_origins(proof::at(catalog, path)?.clone(), catalog)?;
    assert_eq!(
        actual, expected,
        "{path} original rows and physical source uses"
    );
    Ok(())
}
