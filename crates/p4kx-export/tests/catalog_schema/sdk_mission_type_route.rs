// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, MissionTypeSnapshot,
};
use p4kx_export::{CatalogOptions, Language, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope};
use p4kx_testkit::TestType;
use serde_json::Value;

use super::super::MISSION_TYPES;
use super::{Route, TestResult};

#[path = "../it/mission_type_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<Route> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic-mission-types.p4k");
    fixture::write(&path, TestType::Locale)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_mission_types(CatalogRecordSelection::RecordIndex(1), None, |snapshot| {
        let [row] = snapshot.rows.as_slice() else {
            return Err("one physical mission type".into());
        };
        assert_eq!(
            row.header().record_index().value(),
            Some(&1),
            "physical selection excludes other mission types",
        );
        assert_eq!(
            row.type_name().value(),
            None,
            "the placeholder is not promoted to a protected name reference",
        );
        let [] = snapshot.game_texts.as_slice() else {
            return Err("unselected names do not enter the query result".into());
        };
        Ok::<(), Box<dyn std::error::Error>>(())
    })??;
    parsed.with_mission_types(CatalogRecordSelection::All, None, |snapshot| {
        let expected = catalog
            .pointer("/definitions/missionTypes/rows")
            .ok_or("canonical mission type rows")?;
        assert_eq!(
            &serde_json::to_value(&snapshot.rows)?,
            expected,
            "typed SDK rows match the exact Catalog rows",
        );
        assert_eq!(
            catalog.pointer("/texts/gameTexts"),
            Some(&serde_json::to_value(&snapshot.game_texts)?),
            "the protected-text link resolves to the same localized source values",
        );
        verify_selected_name(&snapshot)?;
        Route::queried_rows(
            MISSION_TYPES,
            &snapshot.rows,
            "encrypted synthetic producer parity available; installed producer parity open",
        )
    })?
}

fn verify_selected_name(snapshot: &MissionTypeSnapshot<'_>) -> TestResult {
    let [first, ..] = snapshot.rows.as_slice() else {
        return Err("at least one authored mission type".into());
    };
    assert_eq!(
        snapshot.available_languages.as_slice(),
        &[Language::English, Language::French],
        "only the installed fixture languages are available",
    );
    let guid = first.header().guid();
    let name = first.type_name();
    for origin in [guid.origin(), name.origin()] {
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "both the GUID and name cite the encrypted DataForge payload",
        );
        assert!(
            origin.source_set_complete()
                && snapshot.source_inventory.iter_uses().any(|used| {
                    used.scope() == InputScope::Payload
                        && origin.source_uses_refs().contains(used.id())
                }),
            "the source inventory closes each original field use",
        );
    }
    let name = name.value().ok_or("protected name reference")?;
    let text = snapshot.game_text(name).ok_or("protected name text")?;
    assert_eq!(
        text.key().value().copied(),
        Some("@fixture_type"),
        "the selected name resolves to the authored key",
    );
    Ok(())
}
