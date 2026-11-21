// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::worn_gear::{SuitArmorSnapshot, with_suit_armor_from};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ResourceFloat32,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::DataForgeEntry;
use serde_json::Value;

use super::super::{CLOTHING, ITEM_RADARS, SUIT_ARMOR};
use super::component_parity::parity;
use super::{Route, TestResult};

#[path = "sdk_worn_gear_fixture.rs"]
mod fixture;
#[path = "sdk_shared_radar_definition_route.rs"]
mod shared_radar;

const ALL: CatalogRecordSelection = CatalogRecordSelection::All;
const REMAINING: &str = "typed physical owner query and synthetic producer parity witnessed; complete installed producer parity open";

pub(super) fn read() -> TestResult<[Route; 4]> {
    let (_directory, archive) = fixture::archive()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    Ok([
        parsed.with_suit_armor(ALL, None, |snapshot| {
            Route::queried_rows(SUIT_ARMOR, snapshot.rows(), REMAINING)
        })??,
        parsed.with_clothing(ALL, None, |snapshot| {
            Route::queried_rows(CLOTHING, snapshot.rows(), REMAINING)
        })??,
        parsed.with_item_radars(ALL, None, |snapshot| {
            Route::queried_rows(ITEM_RADARS, snapshot.rows(), REMAINING)
        })??,
        shared_radar::read(&archive, &parsed)?,
    ])
}

#[test]
fn worn_gear_queries_keep_original_positions_and_resolve_a_macro_separately() -> TestResult<()> {
    let (_directory, archive) = fixture::archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge
        .record(fixture::FIRST)
        .ok_or("first original owner")?
        .guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let target =
        parsed.with_suit_armor(CatalogRecordSelection::Guid(guid), None, |snapshot| {
            selected_resistance_macro(&snapshot)
        })??;
    parsed.with_damage_resistance_macros(
        CatalogRecordSelection::RecordIndex(target),
        None,
        |snapshot| -> TestResult<()> {
            let [row] = snapshot.rows() else {
                return Err("the exact selected macro definition".into());
            };
            assert_eq!(
                row.source().record_index().value(),
                Some(&target),
                "the link resolves through the independent macro reader",
            );
            Ok(())
        },
    )??;
    with_suit_armor_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(fixture::FIRST),
        None,
        |snapshot| {
            assert_eq!(snapshot.rows().len(), 1, "one physical suit owner");
        },
    )?;
    Ok(())
}

fn selected_resistance_macro(snapshot: &SuitArmorSnapshot<'_>) -> TestResult<u32> {
    let [first, alias] = snapshot.rows() else {
        return Err("both physical suit records for the shared GUID".into());
    };
    assert_eq!(
        first.owner().record_index().value(),
        Some(&fixture::FIRST),
        "the first physical owner remains distinct",
    );
    assert_eq!(
        alias.owner().record_index().value(),
        Some(&fixture::ALIAS),
        "the physical GUID alias remains distinct",
    );
    assert_eq!(
        alias.owner().component_ordinal().value(),
        Some(&1),
        "the aliased suit retains its original Components slot",
    );
    assert_eq!(
        alias
            .integrity_milestone_to_break()
            .value()
            .map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "the original negative zero bits survive",
    );
    assert_eq!(
        alias
            .damage_resistance()
            .value()
            .and_then(|link| link.resolved_index()),
        None,
        "a dangling macro link never picks an unrelated target",
    );
    assert!(
        snapshot.requested_languages().is_empty() && snapshot.available_languages().is_empty(),
        "a worn gear query reads no locale payloads",
    );
    let cell = first.damage_resistance();
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        Some(true),
        "the linked source value is marked as encrypted",
    );
    cell.value()
        .and_then(|link| link.resolved_index())
        .ok_or_else(|| "the linked macro retains its original physical index".into())
}

#[test]
fn worn_gear_queries_match_synthetic_producer_rows() -> TestResult<()> {
    let (_directory, archive) = fixture::archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_suit_armor(ALL, None, |snapshot| {
        parity(SUIT_ARMOR, &snapshot, &catalog)
    })??;
    parsed.with_clothing(ALL, None, |snapshot| -> TestResult<()> {
        parity(CLOTHING, &snapshot, &catalog)?;
        let [row] = snapshot.rows() else {
            return Err("one original clothing component".into());
        };
        let temperature = row
            .temperature_resistance()
            .value()
            .ok_or("inline temperature")?;
        assert_eq!(
            temperature
                .min_resistance()
                .value()
                .and_then(ResourceFloat32::finite),
            Some(-5.0),
            "nested authored clothing value",
        );
        Ok(())
    })??;
    parsed.with_item_radars(ALL, None, |snapshot| -> TestResult<()> {
        parity(ITEM_RADARS, &snapshot, &catalog)?;
        let [row] = snapshot.rows() else {
            return Err("one original radar component".into());
        };
        let ping = row
            .ping_properties()
            .value()
            .ok_or("inline ping properties")?;
        assert_eq!(
            ping.cooldown_time()
                .value()
                .and_then(ResourceFloat32::finite),
            Some(8.0),
            "nested authored radar value",
        );
        Ok(())
    })??;
    Ok(())
}
