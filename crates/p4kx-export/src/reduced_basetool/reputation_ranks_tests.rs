// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use serde_json::{Value, json};

use super::ReducedError;
use super::reputation_catalog;
use super::reputation_ranks::{self, SCOPES};
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::ReputationSelection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};

#[path = "../../tests/it/reputation_fixture/mod.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reduced_rank_tables_close_standing_slots_and_keep_authored_precision() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = fixture::write_archive(scratch.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let ranks =
        reputation_catalog::collect(&session.parsed(None)?, scratch.path(), None, &mut texts)?;
    let scopes = serde_json::to_value(ranks.scopes)?;
    let standings = serde_json::to_value(ranks.standings)?;
    let contexts = serde_json::to_value(ranks.contexts)?;
    assert_eq!(
        contexts.pointer("/rows/0/primaryScopeContext/value/scope/value/targetRecordIndex/value"),
        scopes.pointer("/rows/0/recordIndex/value")
    );
    assert_eq!(
        contexts
            .pointer("/rows/0/scopeContextList/value/0/value/scope/value/targetRecordIndex/value"),
        scopes.pointer("/rows/0/recordIndex/value")
    );
    assert_eq!(
        contexts.pointer("/rows/0/scopeContextList/value/1/value/scope/value/targetType/value"),
        Some(&json!("Placeholder"))
    );
    let scope = scopes.pointer("/rows/0").ok_or("original scope row")?;
    let map = scope.pointer("/standingMap/value").ok_or("selected map")?;
    let slots = map
        .pointer("/standings/value")
        .and_then(Value::as_array)
        .ok_or("ordered standing slots")?;
    for (slot, ordinal) in [(0, 4), (1, 5)] {
        assert_eq!(
            slots[slot].pointer("/value/targetRecordIndex/value"),
            Some(&json!(ordinal))
        );
        assert_eq!(
            slots[slot].pointer("/value/guid/value"),
            standings.pointer(&format!("/rows/{slot}/guid/value"))
        );
    }
    assert_eq!(
        map.pointer("/reputationCeiling/value"),
        Some(&json!({"type":"int64", "value":"9007199254740993"}))
    );
    assert_eq!(
        serde_json::to_string(
            standings
                .pointer("/rows/0/driftTimeHours/value/finite")
                .ok_or("negative zero in hours")?
        )?,
        "-0.0"
    );
    assert_eq!(
        standings.pointer("/rows/1/driftTimeHours/value/finite"),
        Some(&json!(24.5))
    );
    assert_eq!(standings.pointer("/rows/1/gated/value"), Some(&json!(true)));
    assert_eq!(
        standings.pointer("/rows/1/minReputation/containsEncryptedSource"),
        Some(&json!(true))
    );
    let texts = serde_json::to_value(texts)?;
    let selected_key = scope
        .pointer("/displayName/value")
        .ok_or("protected scope name")?;
    let named = texts
        .get("gameTexts")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.pointer("/id/value") == Some(selected_key))
        })
        .ok_or("selected text closure")?;
    assert_eq!(
        named.pointer("/translations/value/en/displayText/value"),
        Some(&json!("Standing"))
    );
    assert!(named.pointer("/translations/value/de").is_none());
    assert!(scope.get("authoredFields").is_none());
    Ok(())
}

#[test]
fn ranks_refuse_a_resolved_standing_outside_the_staged_row_set() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        fixture::write_archive(scratch.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let selection = ReputationSelection {
        standings: Some(CatalogRecordSelection::RecordIndex(5)),
        ..ReputationSelection::ranks()
    };
    let result = session.with_reputation(selection, None, |snapshot| {
        reputation_ranks::stage(&snapshot, scratch.path(), None, &mut TextRefs::default())
    })?;
    assert!(matches!(
        result,
        Err(ReducedError::UnresolvedRecordLink {
            path: SCOPES,
            index: 4
        })
    ));
    Ok(())
}
