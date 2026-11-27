// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::location_vocabulary::{
    ContractScopeSnapshot, with_contract_scopes,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as origin;
use crate::sdk_full_proof as full_proof;

#[path = "catalog_contract_scope_sdk_fixture.rs"]
mod fixture;

const TABLE: &str = "/definitions/contractScopes";

#[test]
fn contract_scope_sdk_matches_all_canonical_slots_and_public_text() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::Complete),
        DcbOrigin::Encrypted,
    )?;
    let catalog: Value = serde_json::from_slice(
        build_catalog(&archive, common::options().with_full_provenance())?.json(),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?
    .with_contract_scopes(CatalogRecordSelection::All, None, |snapshot| {
        check_parity(&snapshot, &catalog)
    })??;
    Ok(())
}

fn check_parity(snapshot: &ContractScopeSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one physical contract scope definition".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&0),
        "the original owner Record"
    );
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "the malformed Record of a different family is excluded",
    );
    let choice = row.sector().value().ok_or("authored sector slot")?;
    assert_eq!(
        choice.icon().value(),
        Some(&fixture::ICON),
        "original icon locator"
    );
    assert_eq!(
        choice.icon().origin().contains_encrypted_source(),
        Some(true),
        "per-field source encryption"
    );
    let table = origin::at(catalog, TABLE)?;
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "source declaration availability"
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    let rows = table.get("rows").ok_or("canonical scope definitions")?;
    assert_eq!(
        origin::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        origin::with_origins(rows.clone(), catalog)?,
        "all seven canonical slots and their precise source uses",
    );
    check_label(
        choice
            .display_name()
            .value()
            .ok_or("public label reference")?,
        snapshot,
        catalog,
        &provenance,
    )
}

fn check_label(
    reference: &p4kx_export::catalog_queries::PublicNameRef,
    snapshot: &ContractScopeSnapshot<'_>,
    catalog: &Value,
    provenance: &Value,
) -> TestResult {
    let text = snapshot
        .public_name(reference)
        .ok_or("selected public name row")?;
    assert_eq!(
        text.key().value(),
        Some(&fixture::LABEL),
        "the original locale key"
    );
    assert!(
        snapshot.game_texts().is_empty(),
        "scope labels use the public domain"
    );
    let row = serde_json::to_value(text)?;
    assert_eq!(
        origin::at(&row, "/translations/value/de/displayText/value")?,
        "Gemeinsamer Text",
        "available localization of the original label",
    );
    let expected = common::text_row(catalog, "publicNames", &json!(reference))?;
    assert_eq!(
        origin::with_origins(row, provenance)?,
        origin::with_origins(expected.clone(), catalog)?,
        "public label and translations retain original source evidence",
    );
    Ok(())
}

#[test]
fn contract_scope_sdk_keeps_readable_slots_when_one_icon_is_unreadable() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::Unreadable),
        DcbOrigin::Encrypted,
    )?;
    with_contract_scopes(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "the selected owner contains an unreadable value"
            );
            let [row] = snapshot.rows() else {
                return Err("partial physical scope Record".into());
            };
            let sector = row.sector().value().ok_or("original sector declaration")?;
            assert_eq!(
                sector.icon().state(),
                SourceValueState::Unreadable,
                "failed icon remains distinct from absence"
            );
            let location = row
                .location()
                .value()
                .ok_or("original location declaration")?;
            assert_eq!(
                location.icon().value(),
                Some(&fixture::ICON),
                "independent scope remains available"
            );
            Ok(())
        },
    )??;
    Ok(())
}
