// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::durability::TurretHealthSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DefinitionStatus,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use serde_json::{Value, json};

use super::{COMPONENT_ROWS, database};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn turret_component_sdk_preserves_physical_rows_references_and_provenance() -> TestResult {
    let (_directory, archive) = common::open(&database(true), DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    session.with_turret_health(CatalogRecordSelection::RecordIndex(0), None, |snapshot| {
        check_turrets(&snapshot, &catalog)
    })??;
    session.with_turret_health(CatalogRecordSelection::RecordIndex(1), None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "a standalone modifier Record is outside the component scope"
        );
    })?;
    Ok(())
}

fn check_turrets(snapshot: &TurretHealthSnapshot<'_>, catalog: &Value) -> TestResult {
    let [present, null, dangling] = snapshot.rows() else {
        return Err("three original turret component occurrences".into());
    };
    for (ordinal, row) in [present, null, dangling].into_iter().enumerate() {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&0),
            "the physical Entity owns all original components"
        );
        assert_eq!(
            row.owner().component_ordinal().value(),
            Some(&u32::try_from(ordinal)?),
            "the original component ordinals remain distinct"
        );
    }
    let target = present
        .health_modifier_record()
        .value()
        .ok_or("linked modifier")?;
    assert_eq!(
        target.resolved_index(),
        Some(1),
        "the exact target definition is selectable"
    );
    assert_eq!(
        null.health_modifier_record().state(),
        SourceValueState::Null,
        "the original null Reference remains null"
    );
    let missing = dangling
        .health_modifier_record()
        .value()
        .ok_or("dangling modifier")?;
    assert!(
        matches!(missing.status(), DefinitionStatus::Dangling),
        "the authored missing target remains observable"
    );
    assert_eq!(
        missing.guid().origin().contains_encrypted_source(),
        Some(true),
        "the dangling GUID retains its encrypted datum provenance"
    );
    check_parity(snapshot, catalog)
}

fn check_parity(snapshot: &TurretHealthSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.requested_languages().is_empty(),
        "component lookups require no locale input"
    );
    assert_eq!(
        catalog.pointer("/configurations/turretHealth/availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "the declared availability matches the producer"
    );
    let rows = Value::Array(proof::rows(catalog, COMPONENT_ROWS)?.to_vec());
    let origin_inventory = json!({"provenance": snapshot.source_inventory()});
    assert_eq!(
        proof::with_origins(serde_json::to_value(snapshot.rows())?, &origin_inventory)?,
        proof::with_origins(rows, catalog)?,
        "the exact component rows and original per-datum uses match the producer"
    );
    Ok(())
}
