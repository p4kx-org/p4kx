// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::ResourceCapacity;
use p4kx_export::catalog_queries::equipment::consumables::{
    ConsumableSnapshot, with_consumables_from,
};
use p4kx_export::catalog_queries::equipment::mining_tools::{
    AttachableEffect, AttachableModifierSnapshot, MiningLaserSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ResourceFloat32,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use serde_json::Value;

use super::super::{ATTACHABLE_MODIFIERS, CONSUMABLE_CONFIGURATIONS, MINING_LASERS};
use super::component_parity::parity;
use super::{Route, TestResult};

#[path = "sdk_component_tool_fixture.rs"]
mod fixture;

const ALL: CatalogRecordSelection = CatalogRecordSelection::All;

pub(super) fn read() -> TestResult<[Route; 3]> {
    let (_directory, archive) = fixture::archive()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    Ok([
        parsed.with_mining_lasers(ALL, None, |snapshot| {
            Route::queried_rows(MINING_LASERS, snapshot.rows(), None)
        })??,
        parsed.with_attachable_modifiers(ALL, None, |snapshot| {
            Route::queried_rows(ATTACHABLE_MODIFIERS, snapshot.rows(), None)
        })??,
        parsed.with_consumables(ALL, None, |snapshot| {
            Route::queried_rows(CONSUMABLE_CONFIGURATIONS, snapshot.rows(), None)
        })??,
    ])
}

#[test]
fn mining_tools_and_consumables_match_synthetic_producer_rows() -> TestResult<()> {
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
    parsed.with_mining_lasers(ALL, None, |snapshot| {
        parity(MINING_LASERS, &snapshot, &catalog)
    })??;
    parsed.with_attachable_modifiers(ALL, None, |snapshot| {
        parity(ATTACHABLE_MODIFIERS, &snapshot, &catalog)
    })??;
    parsed.with_consumables(ALL, None, |snapshot| {
        parity(CONSUMABLE_CONFIGURATIONS, &snapshot, &catalog)
    })??;
    Ok(())
}

#[test]
fn component_tools_keep_original_slots_and_independent_dependencies() -> TestResult<()> {
    let (_directory, archive) = fixture::archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge
        .record(fixture::ITEM)
        .ok_or("physical equipment owner")?
        .guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let global =
        parsed.with_mining_lasers(CatalogRecordSelection::Guid(guid), None, |snapshot| {
            check_lasers(&snapshot)
        })??;
    let item = CatalogRecordSelection::RecordIndex(fixture::ITEM);
    let subtype = with_consumables_from(&archive, &entry, item, None, |snapshot| {
        check_consumable(&snapshot)
    })??;
    parsed.with_attachable_modifiers(item, None, |snapshot| check_effects(&snapshot))??;
    parsed.with_mining_laser_globals(
        CatalogRecordSelection::RecordIndex(global),
        None,
        |snapshot| -> TestResult<()> {
            let [row] = snapshot.rows() else {
                return Err("exact original global definition".into());
            };
            assert_eq!(
                row.source().record_index().value(),
                Some(&fixture::GLOBAL),
                "mining dependency keeps its physical index"
            );
            Ok(())
        },
    )??;
    parsed.with_consumable_subtypes(
        CatalogRecordSelection::RecordIndex(subtype),
        None,
        |snapshot| -> TestResult<()> {
            let [row] = snapshot.rows() else {
                return Err("exact original consumable subtype".into());
            };
            assert_eq!(
                row.source().record_index().value(),
                Some(&fixture::SUBTYPE),
                "subtype dependency keeps its physical index"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn check_lasers(snapshot: &MiningLaserSnapshot<'_>) -> TestResult<u32> {
    let [first, alias] = snapshot.rows() else {
        return Err("both physical owners must retain their laser rows".into());
    };
    for (row, index, ordinal) in [(first, fixture::ITEM, 1), (alias, fixture::ALIAS, 0)] {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&index),
            "physical alias index"
        );
        assert_eq!(
            row.owner().component_ordinal().value(),
            Some(&ordinal),
            "original Components slot"
        );
    }
    let minimum = alias.throttle_minimum();
    assert_eq!(
        minimum.value().map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "negative zero remains an original Single"
    );
    assert_eq!(
        minimum.origin().contains_encrypted_source(),
        Some(true),
        "authored minimum carries immediate encryption"
    );
    assert!(
        !minimum.origin().source_uses_refs().is_empty(),
        "minimum refers to its original source use"
    );
    assert_eq!(
        alias
            .global_params()
            .value()
            .and_then(|link| link.resolved_index()),
        None,
        "dangling Reference stays unresolved"
    );
    first
        .global_params()
        .value()
        .and_then(|link| link.resolved_index())
        .ok_or_else(|| "resolved physical global index".into())
}

fn check_consumable(snapshot: &ConsumableSnapshot<'_>) -> TestResult<u32> {
    let [row] = snapshot.rows() else {
        return Err("the selected consumable component".into());
    };
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&2),
        "original consumable slot"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "a component never loads subtype locales"
    );
    let Some(ResourceCapacity::MicroCargoUnit { micro_scu, .. }) = row.consumable_volume().value()
    else {
        return Err("the original consumable capacity unit".into());
    };
    assert_eq!(
        micro_scu.value(),
        Some(&19),
        "unit and payload remain original"
    );
    let Some([first, second]) = row.default_contents().value().map(Vec::as_slice) else {
        return Err("both original consumable contents slots".into());
    };
    let first = first.value().ok_or("first authored contents slot")?;
    let second = second.value().ok_or("second authored contents slot")?;
    assert_eq!(
        second.ratio().value().map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "ratio retains negative zero bits"
    );
    assert_eq!(
        second
            .consumable_subtype()
            .value()
            .and_then(|link| link.resolved_index()),
        None,
        "unresolved subtype stays in the second slot"
    );
    first
        .consumable_subtype()
        .value()
        .and_then(|link| link.resolved_index())
        .ok_or_else(|| "resolved physical subtype index".into())
}

fn check_effects(snapshot: &AttachableModifierSnapshot<'_>) -> TestResult<()> {
    let [row] = snapshot.rows() else {
        return Err("the selected attachable component".into());
    };
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&3),
        "original attachable slot"
    );
    let Some([weapon, empty, other]) = row.modifiers().value().map(Vec::as_slice) else {
        return Err("all original effect slots".into());
    };
    let Some(AttachableEffect::Weapon(weapon)) = weapon.value() else {
        return Err("first slot keeps the weapon effect".into());
    };
    let Some(AttachableEffect::Authored(source)) = other.value() else {
        return Err("third slot keeps the newly authored effect".into());
    };
    assert_eq!(
        weapon.fire_action_index().value(),
        Some(&2),
        "authored fire action"
    );
    assert_eq!(
        empty.state(),
        SourceValueState::Null,
        "null slot is retained"
    );
    assert_eq!(
        source.source_type().value(),
        Some(&"FixtureOtherModifier"),
        "concrete unknown effect type is retained"
    );
    Ok(())
}
