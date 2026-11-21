// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::cargo_globals::{
    with_cargo_loading_globals, with_cargo_loading_globals_from, with_shop_commodity_globals,
    with_shop_commodity_globals_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState, SourcedValue};
use serde::Serialize;
use serde_json::{Value, json};

use super::{DcbOrigin, LOADING_ROWS, SHOP_ROWS, TestResult, common, fixture, proof};

#[test]
fn typed_cargo_global_queries_match_canonical_rows_and_physical_evidence() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) =
            common::open(&fixture::database(fixture::Layout::Authored), origin)?;
        let catalog = canonical(&archive)?;
        verify_loading(&archive, &catalog, marker)?;
        verify_shop(&archive, &catalog, marker)?;
    }
    Ok(())
}

fn verify_loading(archive: &Archive, catalog: &Value, marker: Option<bool>) -> TestResult {
    with_cargo_loading_globals(archive, CatalogRecordSelection::All, None, |snapshot| {
        let [row, _] = snapshot.rows() else {
            return Err("two loading configuration rows".into());
        };
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [] as [u32; 0],
            "complete projected fields"
        );
        assert_eq!(
            row.source().record_index().value(),
            Some(&0),
            "first physical loading record"
        );
        let time = row
            .auto_loading_base_loading_time()
            .value()
            .ok_or("authored base time")?;
        assert_eq!(time.finite(), Some(1.0), "authored base loading time");
        assert_eq!(
            row.auto_loading_base_loading_time()
                .origin()
                .contains_encrypted_source(),
            marker,
            "input encryption status reaches the original cell",
        );
        let sizes = row
            .auto_loading_box_size_loading_time()
            .value()
            .ok_or("original box timing Class")?;
        assert_eq!(
            sizes.source_type().value(),
            Some(&fixture::TIME_TYPE),
            "declared box timing struct"
        );
        let zero = sizes
            .by_scu()
            .get("0.125")
            .and_then(SourcedValue::value)
            .ok_or("eighth-SCU timing")?;
        assert_eq!(
            zero.raw_bits(),
            (-0.0_f32).to_bits(),
            "original sign of zero"
        );
        assert!(
            !row.additional_fields().roots().is_empty(),
            "additional loading fields remain available"
        );
        assert!(
            !sizes.additional_fields().roots().is_empty(),
            "additional box timing fields remain available"
        );
        parity(&snapshot, catalog, LOADING_ROWS)
    })??;
    Ok(())
}

fn verify_shop(archive: &Archive, catalog: &Value, marker: Option<bool>) -> TestResult {
    with_shop_commodity_globals(archive, CatalogRecordSelection::All, None, |snapshot| {
        let [row] = snapshot.rows() else {
            return Err("one commodity configuration row".into());
        };
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [] as [u32; 0],
            "complete projected fields"
        );
        assert_eq!(
            row.auto_loading_base_price().value(),
            Some(&-300),
            "negative base fee remains signed"
        );
        assert_eq!(
            row.auto_loading_base_price()
                .origin()
                .contains_encrypted_source(),
            marker,
            "input encryption status reaches the original cell",
        );
        let sizes = row
            .auto_loading_box_size_prices()
            .value()
            .ok_or("original fee Class")?;
        assert_eq!(
            sizes.by_scu().get("0.125").and_then(SourcedValue::value),
            Some(&i32::MIN),
            "fees retain signed storage",
        );
        parity(&snapshot, catalog, SHOP_ROWS)
    })??;
    Ok(())
}

#[test]
fn cargo_global_selection_keeps_physical_aliases() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::DuplicateIdentity),
        DcbOrigin::Plain,
    )?;
    let catalog = canonical(&archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let guid = CatalogRecordSelection::Guid(common::guid(fixture::SHARED_LOADING_GUID).parse()?);
    with_cargo_loading_globals_from(&archive, &entry, guid, None, |snapshot| {
        let [first, second] = snapshot.rows() else {
            return Err("both physical aliases within the selected GUID".into());
        };
        assert_eq!(first.source().record_index().value(), Some(&0));
        assert_eq!(second.source().record_index().value(), Some(&1));
        parity(&snapshot, &catalog, LOADING_ROWS)
    })??;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    session.with_cargo_loading_globals(
        CatalogRecordSelection::RecordIndex(2),
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.rows().is_empty(),
                "shop owner is outside the loading family"
            );
            assert!(matches!(
                snapshot.availability(),
                DatasetAvailability::Available
            ));
            Ok(())
        },
    )??;
    let parsed = session.parsed(None)?;
    parsed.with_shop_commodity_globals(CatalogRecordSelection::All, None, |snapshot| {
        parity(&snapshot, &catalog, SHOP_ROWS)
    })??;
    with_shop_commodity_globals_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(2),
        None,
        |snapshot| parity(&snapshot, &catalog, SHOP_ROWS),
    )??;
    Ok(())
}

#[test]
fn cargo_global_selection_keeps_partial_cells() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::InvalidTimeMember),
        DcbOrigin::Encrypted,
    )?;
    let catalog = canonical(&archive)?;
    with_cargo_loading_globals(
        &archive,
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| {
            let [row] = snapshot.rows() else {
                return Err("one selected partial row".into());
            };
            assert_eq!(snapshot.coverage().incomplete_fields, [1]);
            let sizes = row
                .auto_loading_box_size_loading_time()
                .value()
                .ok_or("retained box timing Class")?;
            assert_eq!(
                sizes.by_scu().get("0.5").map(SourcedValue::state),
                Some(SourceValueState::Invalid),
            );
            parity(&snapshot, &catalog, LOADING_ROWS)
        },
    )??;
    Ok(())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn parity<Row: Serialize>(
    snapshot: &EquipmentSnapshot<'_, Row>,
    catalog: &Value,
    pointer: &str,
) -> TestResult {
    assert_eq!(
        snapshot.requested_languages(),
        [],
        "cargo globals use no localization"
    );
    let sdk = json!({"provenance": snapshot.source_inventory()});
    let canonical = proof::rows(catalog, pointer)?;
    for row in snapshot.rows() {
        let row = serde_json::to_value(row)?;
        let index = proof::at(&row, "/recordIndex/value")?;
        let expected = canonical
            .iter()
            .find(|candidate| candidate.pointer("/recordIndex/value") == Some(index))
            .ok_or("matching canonical physical row")?;
        assert_eq!(
            proof::with_origins(row, &sdk)?,
            proof::with_origins(expected.clone(), catalog)?,
            "the complete original row and physical sources match",
        );
    }
    Ok(())
}
