// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::vehicle_armor::{self, VehicleArmorDamage};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
    ResourceFloat32,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForgeEntry, Guid};
use serde_json::{Value, json};

use super::{TABLE, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn vehicle_armor_query_keeps_producer_rows_and_physical_origins() -> TestResult {
    let database = fixture::database(fixture::Layout::Complete);
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted] {
        let (_directory, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let known = forge
            .record(0)
            .ok_or("known physical fixture owner")?
            .guid();
        let session = CatalogQuerySession::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let parsed = session.parsed(None)?;
        check_all(&parsed, &catalog)?;
        check_guid(&parsed, known)?;
    }
    Ok(())
}

fn check_all(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    parsed.with_vehicle_armor(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "retained derived damage shapes are readable"
            );
            assert!(
                snapshot.requested_languages().is_empty()
                    && snapshot.available_languages().is_empty(),
                "armor consumes no locale payloads"
            );
            let inventory = json!({"provenance": snapshot.source_inventory()});
            let actual = proof::with_origins(serde_json::to_value(snapshot.rows())?, &inventory)?;
            let expected = proof::with_origins(proof::at(catalog, TABLE)?.clone(), catalog)?;
            assert_eq!(
                actual, expected,
                "selected armor rows retain producer values and exact source coordinates"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn check_guid(parsed: &ParsedCatalogQueries<'_>, known: Guid) -> TestResult {
    parsed.with_vehicle_armor(
        CatalogRecordSelection::Guid(known),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one selected physical armor owner".into());
            };
            let Some(VehicleArmorDamage::Info { value }) = row.damage_multiplier().value() else {
                return Err("the concrete DamageInfo alternative".into());
            };
            assert_eq!(
                row.owner().record_index().value(),
                Some(&0),
                "the GUID selects the exact physical owner"
            );
            assert_eq!(
                row.signal_infrared().value().map(ResourceFloat32::raw_bits),
                Some(fixture::SIGNAL_BITS),
                "the original IEEE word survives"
            );
            assert_eq!(
                value
                    .channels()
                    .physical()
                    .value()
                    .and_then(ResourceFloat32::finite),
                Some(1.0),
                "the concrete typed original damage amount remains queryable"
            );
            Ok(())
        },
    )??;
    Ok(())
}

#[test]
fn a_malformed_armor_curve_reports_incomplete_scope_without_losing_the_original() -> TestResult {
    let database = fixture::database(fixture::Layout::BrokenCurve);
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    vehicle_armor::with_vehicle_armor_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "only the reached malformed owner is named"
            );
            let [row] = snapshot.rows() else {
                return Err("the diagnostic armor row".into());
            };
            let inventory = json!({"provenance": snapshot.source_inventory()});
            let actual = proof::with_origins(serde_json::to_value(row)?, &inventory)?;
            let expected = proof::with_origins(
                proof::at(&catalog, "/configurations/vehicleArmor/rows/0")?.clone(),
                &catalog,
            )?;
            assert_eq!(
                actual, expected,
                "the scope preserves the authored error and its physical proof"
            );
            Ok(())
        },
    )??;
    Ok(())
}
