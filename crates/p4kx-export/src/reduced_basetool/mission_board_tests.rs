// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::mission_boards;
use super::record_targets::RecordTargets;
use super::reputation_catalog;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

#[path = "../../tests/it/mission_board_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reduced_boards_keep_original_variants_and_close_faction_links() -> TestResult {
    let scratch = tempfile::tempdir()?;
    for (index, database) in [fixture::database(), fixture::shared_item_database()]
        .into_iter()
        .enumerate()
    {
        let path = scratch.path().join(format!("fixture-{index}.p4k"));
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
            .write_to(&path)?;
        let archive = Archive::open_with_key(path, Some(test_key()))?;
        let session =
            CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let tables = reputation_catalog::collect(
            &parsed,
            scratch.path(),
            None,
            &mut SelectedTexts::default(),
        )?;
        let table = tables.mission_boards;
        verify(&serde_json::to_value(table)?)?;
        assert!(matches!(
            mission_boards::collect(&parsed, scratch.path(), None, &RecordTargets::default()),
            Err(ReducedError::UnresolvedRecordLink {
                path: mission_boards::PATH,
                index: 1
            })
        ));
    }
    Ok(())
}

fn verify(table: &Value) -> TestResult {
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("board rows")?;
    let [career, notice, authored, empty, base] = rows.as_slice() else {
        return Err("all original board occurrences".into());
    };
    for (row, slot, variant) in [
        (career, 1, "career"),
        (notice, 2, "notice"),
        (authored, 4, "authored"),
        (base, 6, "base"),
    ] {
        assert_eq!(row.pointer("/componentOrdinal/value"), Some(&json!(slot)));
        assert_eq!(
            row.pointer("/boardType/value/details/type"),
            Some(&json!(variant))
        );
        assert_eq!(
            row.pointer("/boardType/containsEncryptedSource"),
            Some(&json!(true))
        );
        assert!(row.pointer("/boardType/value/authoredFields").is_none());
    }
    assert_eq!(
        authored.pointer("/boardType/value/sourceType/value"),
        Some(&json!("FixtureFutureMissionBoardType"))
    );
    assert_eq!(empty.pointer("/boardType/state"), Some(&json!("null")));
    let details = career
        .pointer("/boardType/value/details")
        .ok_or("career details")?;
    assert_eq!(
        details.pointer("/guildFactionReputation/value/targetRecordIndex/value"),
        Some(&json!(1))
    );
    let affiliations = details
        .pointer("/factions/value")
        .and_then(Value::as_array)
        .ok_or("ordered affiliations")?;
    let [primary, null, dangling, repeated] = affiliations.as_slice() else {
        return Err("all authored faction slots".into());
    };
    assert_eq!(
        primary.pointer("/value/targetRecordIndex/value"),
        Some(&json!(0))
    );
    assert_eq!(repeated, primary);
    assert_eq!(null.get("state"), Some(&json!("null")));
    assert_eq!(dangling.pointer("/value/status"), Some(&json!("dangling")));
    Ok(())
}
