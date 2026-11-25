// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractKey,
    ContractSnapshot, ContractSummaries,
};
use p4kx_extraction::{Archive, CancelToken};
use serde_json::{Value, json};

use super::TestResult;
use crate::contracts_fixture;
use crate::sdk_full_proof as full_proof;
use crate::spawns_fixture;

#[test]
fn contract_summaries_match_selected_canonical_rows_and_location_queries() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        contracts_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let (expected, key) =
        parsed.with_contracts(CatalogRecordSelection::All, None, |snapshot| {
            let first = snapshot.rows.first().ok_or("a canonical contract")?;
            let guid = first.base.guid.value().ok_or("parent GUID")?.get();
            let sub = first.base.sub_contract_guid.value().map(|guid| guid.get());
            Ok::<_, Box<dyn std::error::Error>>((
                full_rows(&snapshot)?,
                ContractKey::new(guid, sub),
            ))
        })??;
    parsed.with_contract_summaries(CatalogRecordSelection::All, None, |summary| {
        verify_summary(&summary, &expected)
    })??;
    let expected = parsed.with_contract(key, None, |snapshot| full_rows(&snapshot))??;
    session.with_contract_summary(key, None, |summary| verify_summary(&summary, &expected))??;
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled =
        parsed.with_contract_summaries(CatalogRecordSelection::All, Some(&cancel), |_| true);
    assert!(
        matches!(cancelled, Err(p4kx_export::CatalogError::Cancelled)),
        "summaries use the session's cancellation boundary"
    );
    session
        .with_provenance_mode(full_proof::MODE)
        .with_contract_summaries(CatalogRecordSelection::All, None, |summary| {
            verify_title_origin(&summary)
        })??;
    Ok(())
}

#[test]
fn contract_summaries_keep_locations_while_omitting_ship_spawn_tables() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let expected = parsed.with_contracts(CatalogRecordSelection::All, None, |snapshot| {
        assert!(
            !snapshot.ship_spawns.is_empty(),
            "the control publishes actual ship spawn descriptions"
        );
        assert!(
            !snapshot.location_queries.is_empty(),
            "the control publishes authored location queries"
        );
        full_rows(&snapshot)
    })??;
    session.with_contract_summaries(CatalogRecordSelection::All, None, |summary| {
        assert!(
            summary
                .rows
                .iter()
                .any(|row| !row.location_queries.is_empty()),
            "a summary keeps authored location-query links"
        );
        verify_summary(&summary, &expected)
    })??;
    Ok(())
}

fn full_rows(snapshot: &ContractSnapshot<'_>) -> Result<Value, serde_json::Error> {
    let rows = snapshot
        .rows
        .iter()
        .zip(&snapshot.mission_titles)
        .map(|(row, title)| {
            let base = &row.base;
            json!({
                "sourceRecordIndex": base.source.record_index(),
                "guid": base.guid,
                "subContractGuid": base.sub_contract_guid,
                "blueprintRewards": base.blueprint_rewards,
                "giverFaction": base.giver_faction,
                "offeredAt": base.offered_at,
                "canBeShared": base.facts.layered.can_be_shared,
                "locationQueries": base.location_queries,
                "reputationGains": base.reputation_gains,
                "title": title.title,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_value((&rows, &snapshot.location_queries))
}

fn verify_summary(summary: &ContractSummaries<'_>, expected: &Value) -> TestResult {
    assert_eq!(
        &serde_json::to_value((&summary.rows, &summary.location_queries))?,
        expected,
        "the summary contains precisely the original typed fields in the canonical order"
    );
    for row in &summary.rows {
        if let Some(reference) = row.title.key.value() {
            assert!(
                summary
                    .game_texts
                    .iter()
                    .any(|text| text.id().value() == Some(reference)),
                "the original title key resolves in the summary's own protected text rows"
            );
        }
    }
    Ok(())
}

fn verify_title_origin(summary: &ContractSummaries<'_>) -> TestResult {
    let translation = summary
        .rows
        .iter()
        .flat_map(|row| row.title.translations.values())
        .find(|translation| translation.value().is_some())
        .ok_or("an actual localized title")?;
    let refs = translation.origin().source_uses_refs();
    assert!(
        !refs.is_empty(),
        "Full summaries keep the localized title's field-level proof"
    );
    assert!(
        refs.iter().all(|reference| summary
            .source_inventory
            .iter_uses()
            .any(|source| source.id() == reference)),
        "every localized title evidence reference is bound in the summary's own inventory"
    );
    Ok(())
}
