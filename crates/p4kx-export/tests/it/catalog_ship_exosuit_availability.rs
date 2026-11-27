// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::with_ship_sources_from;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForge, DataForgeEntry, Guid, Record, SourceValueState};
use serde_json::{Value, json};

use super::fixture::{self, GEO_OWNER_GUID, GeoOwner};
use super::{compare_row, uses_record};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn missing_and_unreadable_atls_owners_have_distinct_source_coverage() -> TestResult {
    let guid_text = super::super::guid(GEO_OWNER_GUID);
    let guid: Guid = guid_text.parse()?;
    for state in [GeoOwner::Absent, GeoOwner::Unreadable] {
        let (_directory, archive) = common::open(
            &fixture::database_with_geo_owner(state),
            DcbOrigin::Encrypted,
        )?;
        let entry = DataForgeEntry::read(&archive)?;
        let prospects: Vec<_> = entry
            .open()?
            .records()
            .filter(|record| record.guid() == guid)
            .map(|record| (record.index(), record.tree().is_err()))
            .collect();
        let unreadable = match (state, prospects.as_slice()) {
            (GeoOwner::Absent, []) => None,
            (GeoOwner::Unreadable, [(index, true)]) => Some(*index),
            _ => return Err("the fixture must distinguish a missing and unreadable Actor".into()),
        };
        let normal = build_catalog_from(&archive, &entry, common::options().with_full_provenance());
        let built = if unreadable.is_some() {
            assert!(
                matches!(normal, Err(CatalogError::IncompleteExport(_))),
                "an unreadable physical ATLS owner blocks Normal publication"
            );
            let partial = CatalogOptions {
                publication_mode: CatalogPublicationMode::DiagnosticPartial,
                ..common::options()
            };
            build_catalog_from(&archive, &entry, partial.with_full_provenance())?
        } else {
            normal?
        };
        let catalog: Value = serde_json::from_slice(built.json())?;
        verify_catalog_absence(&catalog, &guid_text, unreadable)?;
        with_ship_sources_from(
            &archive,
            &entry,
            CatalogRecordSelection::Guid(guid),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| -> TestResult {
                assert!(
                    matches!(snapshot.availability(), DatasetAvailability::Available),
                    "the Ship dataset is declared in both Builds"
                );
                assert!(
                    snapshot.rows().is_empty(),
                    "neither case publishes a readable row for the selected owner"
                );
                assert_eq!(
                    snapshot.coverage().unreadable_bodies.as_slice(),
                    unreadable.as_slice(),
                    "the SDK reports the physical read failure only when the Actor is present"
                );
                Ok(())
            },
        )??;
    }
    Ok(())
}

fn verify_catalog_absence(catalog: &Value, guid: &str, unreadable: Option<u32>) -> TestResult {
    let has_guid = |row: &Value| row.pointer("/guid/value").and_then(Value::as_str) == Some(guid);
    assert!(
        !proof::rows(catalog, "/ships")?.iter().any(has_guid),
        "the selected owner supplies no readable row"
    );
    let failed = proof::rows(catalog, "/exportHealth/failedRecords")?
        .iter()
        .find(|row| has_guid(row));
    match (failed, unreadable) {
        (None, None) => {}
        (Some(record), Some(index)) => {
            proof::present(record, "guid", &json!(guid), Some(true))?;
            proof::present(record, "recordIndex", &json!(index), Some(true))?;
        }
        _ => return Err("only the unreadable Actor must retain the physical failure".into()),
    }
    proof::closure(catalog)
}

#[test]
fn an_unreadable_driver_mode_keeps_the_actor_and_its_unreadable_classification() -> TestResult {
    let (_directory, archive) = common::open(
        &fixture::database_with_unreadable_geo_mode(),
        DcbOrigin::Encrypted,
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let geo = fixture_record(&forge, "EntityClassDefinition.ARGO_ATLS_GEO")?;
    let built = build_catalog_from(&archive, &entry, common::options().with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let owner = proof::rows(&catalog, "/ships")?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64)
                == Some(u64::from(geo.index()))
        })
        .ok_or("the unresolved physical owner remains observable")?;
    let driver = fixture_record(
        &forge,
        "EntityClassDefinition.argo_atls_geo_scitem_seat_driver",
    )?;
    assert!(
        uses_record(
            &proof::uses(&catalog, proof::at(owner, "/vehicleKind")?)?,
            driver.index()
        ),
        "the unresolved operator mode retains the original driver Record"
    );
    full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_ship_sources(
        CatalogRecordSelection::RecordIndex(geo.index()),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one physical owner with incomplete control evidence".into());
            };
            assert_eq!(
                row.playability().state(),
                SourceValueState::Unreadable,
                "an unreadable mode leaves player control unproved"
            );
            assert_eq!(
                row.vehicle_kind().state(),
                SourceValueState::Unreadable,
                "an unreadable mode leaves the sourced classification unproved"
            );
            let context = json!({"provenance": snapshot.source_inventory()});
            compare_row(row, owner, &context, &catalog)
        },
    )??;
    Ok(())
}

fn fixture_record<'source>(
    forge: &'source DataForge<'_>,
    name: &str,
) -> TestResult<Record<'source>> {
    forge
        .records()
        .find(|record| record.name().is_ok_and(|actual| actual == name))
        .ok_or_else(|| format!("missing synthetic Record {name}").into())
}
