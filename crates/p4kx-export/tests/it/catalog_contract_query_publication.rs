// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ContractKey, ContractSnapshot,
};
use p4kx_extraction::{Archive, DataForgeEntry, Language};
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry};

#[path = "catalog_contract_publication_checks.rs"]
mod checks;
#[path = "catalog_contract_publication_fixture.rs"]
mod fixture;

use crate::catalog_contracts::TestResult;
use crate::sdk_full_proof as full_proof;

#[test]
fn retained_outside_aliases_publish_all_property_and_npc_texts() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let english = fixture::TEXTS
        .into_iter()
        .fold(GlobalIniBuilder::new(), |builder, (key, value)| {
            builder.text(key, value)
        })
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database().build(),
        ))
        .entry(TestEntry::stored(Language::English.global_ini_path(), english).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let positions: Vec<_> = entry
        .open()?
        .records()
        .filter(|record| matches!(record.type_name(), Ok("ContractGenerator")))
        .map(|record| record.index())
        .collect();
    let [outside, selected] = positions.as_slice() else {
        return Err("two physical Generator aliases".into());
    };
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_contracts(
        CatalogRecordSelection::RecordIndex(*selected),
        None,
        |snapshot| verify(&snapshot, *outside, *selected),
    )??;
    let key = ContractKey::new(
        crate::commodity_v3_fixture::guid(fixture::SELECTED_CONTRACT).parse()?,
        None,
    );
    parsed.with_contract(key, None, |snapshot| verify(&snapshot, *outside, *selected))??;
    parsed.with_contract_summary(key, None, |summary| {
        assert_eq!(
            summary.rows.len(),
            1,
            "omitting property publication preserves the selected contract"
        );
        assert_eq!(
            summary.coverage.dependencies.related_generators,
            [*outside],
            "the outside alias remains part of the canonical compact query"
        );
        assert!(
            summary.game_texts.is_empty(),
            "the exclusive property and NPC localizations were not published in the summary"
        );
    })?;
    Ok(())
}

fn verify(snapshot: &ContractSnapshot<'_>, outside: u32, selected: u32) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("the selected alias's distinct logical contract".into());
    };
    assert_eq!(
        row.base.guid.value().map(|guid| guid.get().to_string()),
        Some(crate::commodity_v3_fixture::guid(
            fixture::SELECTED_CONTRACT
        )),
        "the earlier alias retains a separate logical contract"
    );
    assert_eq!(
        row.base.source.record_index(),
        selected,
        "the logical row belongs to the selected source"
    );
    assert!(
        snapshot
            .dependency_generators
            .iter()
            .any(|source| source.header().record_index().value() == Some(&outside)),
        "the canonical alias is retained as an outside-root source dependency"
    );
    assert!(
        snapshot.coverage.projection_failures.is_empty(),
        "all compatible source projections succeeded"
    );
    assert!(
        snapshot.coverage.shadowed_candidates.is_empty(),
        "the selected logical contract has no duplicate"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::English],
        "the encrypted requested locale was decoded"
    );
    for guid in &row.base.property_set_guids {
        assert!(
            snapshot
                .mission_property_sets
                .iter()
                .any(|set| &set.guid == guid),
            "every selected property-set link resolves in this snapshot"
        );
    }
    checks::verify(snapshot, outside, selected)
}
