// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractKey,
    ContractSnapshot, ContractSummaries, ContractSummary,
};
use p4kx_extraction::{Archive, CancelToken, Guid};
use serde_json::{Map, Value, json};

use super::TestResult;
use crate::catalog_contracts_facts::SUB;
use crate::contracts_fixture;
use crate::facts_fixture;
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
            Ok::<_, Box<dyn Error>>((full_rows(&snapshot)?, ContractKey::new(guid, sub)))
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
            verify_text_origins(&summary)
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

#[test]
fn summaries_preserve_stated_run_terms_and_release_flags() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        facts_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let expected = parsed.with_contracts(CatalogRecordSelection::All, None, |snapshot| {
        full_rows(&snapshot)
    })??;
    parsed.with_contract_summaries(CatalogRecordSelection::All, None, |summary| {
        verify_summary(&summary, &expected)?;
        let terms = summary
            .rows
            .iter()
            .find(|row| row.buy_in_amount.value() == Some(&1500))
            .ok_or("stated contract buy-in")?;
        assert_eq!(
            terms.can_reaccept_after_abandoning.value.value(),
            Some(&true),
            "the authored positive reacceptance value survives"
        );
        assert_eq!(
            terms.can_reaccept_after_failing.value.value(),
            Some(&false),
            "explicit false is distinct from an absent reacceptance value"
        );
        verify_release_flags(&summary)
    })??;
    Ok(())
}

fn verify_release_flags(summary: &ContractSummaries<'_>) -> TestResult {
    let cases = [
        ("Bare", None, Some(true), Some(false)),
        ("Overridden", None, Some(false), Some(true)),
        ("Overridden", Some(SUB), Some(false), Some(true)),
        ("Stripped", None, None, None),
    ];
    for (name, sub_contract, not_for_release, work_in_progress) in cases {
        let sub_contract_guid = sub_contract.map(str::parse::<Guid>).transpose()?;
        let row = summary
            .rows
            .iter()
            .find(|row| {
                row.debug_name == Some(name)
                    && row.sub_contract_guid.value().map(|guid| guid.get()) == sub_contract_guid
            })
            .ok_or_else(|| format!("summary row for {name}, sub-contract {sub_contract:?}"))?;
        assert_eq!(
            row.not_for_release, not_for_release,
            "{name} uses the parent contract's notForRelease"
        );
        assert_eq!(
            row.work_in_progress, work_in_progress,
            "{name} uses the parent contract's workInProgress"
        );
    }
    Ok(())
}

#[test]
fn summary_schema_keeps_both_mission_texts_protected() -> TestResult {
    let schema = serde_json::to_value(schemars::schema_for!(ContractSummary<'_>))?;
    for text in ["title", "description"] {
        assert_eq!(
            schema.pointer(&format!("/properties/{text}/x-gameText")),
            Some(&Value::Bool(true)),
            "{text} remains a protected game text field"
        );
    }
    Ok(())
}

const PROJECTED_BASE_FIELDS: &[&str] = &[
    "guid",
    "subContractGuid",
    "debugName",
    "notForRelease",
    "workInProgress",
    "blueprintRewards",
    "fixedRewards",
    "giverFaction",
    "offeredAt",
    "canBeShared",
    "cooldownLayer",
    "cooldownLayers",
    "hasPersonalCooldown",
    "personalCooldownTime",
    "personalCooldownTimeVariation",
    "abandonedCooldownTime",
    "abandonedCooldownTimeVariation",
    "buyInAmount",
    "canReacceptAfterAbandoning",
    "canReacceptAfterFailing",
    "minStandingGuid",
    "maxStandingGuid",
    "standingRequirements",
    "locationQueries",
    "reputationGains",
];

fn full_rows(snapshot: &ContractSnapshot<'_>) -> Result<Value, Box<dyn Error>> {
    let rows = snapshot
        .rows
        .iter()
        .zip(&snapshot.mission_titles)
        .map(|(row, texts)| {
            let mut projected = serde_json::to_value(row)?;
            let fields = projected.as_object_mut().ok_or("detailed contract row")?;
            let mut summary = Map::new();
            for &field in PROJECTED_BASE_FIELDS {
                let value = fields
                    .remove(field)
                    .ok_or_else(|| format!("the detailed contract must include {field}"))?;
                summary.insert(field.to_owned(), value);
            }
            summary.insert(
                "sourceRecordIndex".to_owned(),
                json!(row.base.source.record_index()),
            );
            summary.insert("title".to_owned(), serde_json::to_value(&texts.title)?);
            summary.insert(
                "description".to_owned(),
                serde_json::to_value(&texts.description)?,
            );
            Ok::<_, Box<dyn Error>>(Value::Object(summary))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::to_value((&rows, &snapshot.location_queries))?)
}

fn verify_summary(summary: &ContractSummaries<'_>, expected: &Value) -> TestResult {
    assert_eq!(
        &serde_json::to_value((&summary.rows, &summary.location_queries))?,
        expected,
        "the summary contains precisely the original typed fields in the canonical order"
    );
    for row in &summary.rows {
        for field in [&row.title, &row.description] {
            if let Some(reference) = field.key.value() {
                assert!(
                    summary
                        .game_texts
                        .iter()
                        .any(|text| text.id().value() == Some(reference)),
                    "each original text key resolves in the summary's own protected text rows"
                );
            }
        }
    }
    Ok(())
}

fn verify_text_origins(summary: &ContractSummaries<'_>) -> TestResult {
    let title = summary
        .rows
        .iter()
        .flat_map(|row| row.title.translations.values())
        .find(|translation| translation.value().is_some())
        .ok_or("an actual localized title")?;
    let description = summary
        .rows
        .iter()
        .flat_map(|row| row.description.translations.values())
        .find(|translation| translation.value().is_some())
        .ok_or("an actual localized description")?;
    for (name, translation) in [("title", title), ("description", description)] {
        let refs = translation.origin().source_uses_refs();
        assert!(
            !refs.is_empty(),
            "Full summaries keep the localized {name}'s source proof"
        );
        assert!(
            refs.iter().all(|reference| summary
                .source_inventory
                .iter_uses()
                .any(|source| source.id() == reference)),
            "every localized {name} evidence reference is bound in its own inventory"
        );
    }
    Ok(())
}
