// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractSnapshot,
    SourcePayload,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

use super::TestResult;

#[path = "../../src/mission_generator_entries_fixture.rs"]
mod query_source_fixture;
use crate::commodity_v3_fixture;
use crate::contracts_fixture;
use crate::health_origin_assertions as proof;

#[path = "catalog_contract_query_probe.rs"]
mod probe;

#[path = "catalog_contract_query_demands.rs"]
mod demands;

#[path = "catalog_contract_query_intro_evidence.rs"]
mod intro_evidence;

#[path = "catalog_contract_query_key.rs"]
mod key;

#[path = "catalog_contract_query_places.rs"]
mod places;

#[path = "catalog_contract_query_scope.rs"]
mod scope;

#[test]
fn contract_sdk_all_matches_canonical_rows_and_titles() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        contracts_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = commodity_v3_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let expected: Value = serde_json::from_slice(built.json())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_contracts(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            serde_json::to_value(&snapshot.rows)?,
            serde_json::to_value(proof::rows(&expected, "/projections/contracts/rows")?)?,
            "the SDK shares the canonical contract constructor, ordering and intro join"
        );
        assert_eq!(
            serde_json::to_value(&snapshot.mission_titles)?,
            serde_json::to_value(proof::rows(&expected, "/projections/missionTitles/rows")?)?,
            "the SDK shares the canonical per-language mission title projection"
        );
        verify_typed_rows(&snapshot)
    })??;
    Ok(())
}

fn verify_typed_rows(snapshot: &ContractSnapshot<'_>) -> TestResult {
    assert!(
        !snapshot.rows.is_empty(),
        "the fixture exports canonical contracts"
    );
    for row in &snapshot.rows {
        let owner = snapshot
            .generators
            .iter()
            .find(|source| {
                source.header().record_index().value() == Some(&row.base.source.record_index())
            })
            .ok_or("the physical source owner")?;
        let original = owner
            .source()
            .value(row.base.source.contract())
            .ok_or("the original contract occurrence")?;
        assert!(
            matches!(original.value().value(), Some(SourcePayload::Struct { .. })),
            "an external consumer can follow the derived row to typed original source values"
        );
        if let Some(required) = row.required_intros.value() {
            for intro in required {
                let _level = intro.level.value();
                let _tag = intro.tag_guid.value();
                let _matching_contracts = intro.intro_contracts.value();
            }
        }
        for reward in row.base.fixed_rewards.value().into_iter().flatten() {
            assert!(
                !reward.amount.origin().source_uses_refs().is_empty(),
                "payouts retain original field provenance"
            );
        }
    }
    for title in &snapshot.mission_titles {
        if let Some(reference) = title.title.key.value() {
            assert!(
                snapshot
                    .game_texts
                    .iter()
                    .any(|text| text.id().value() == Some(reference)),
                "every returned title key resolves in this result's protected text table"
            );
        }
        for tokens in title.title.item_tokens.values() {
            for token in tokens {
                let _name = token.name.value();
                let _outcome = token.outcome.value();
            }
        }
    }
    Ok(())
}

#[path = "catalog_contract_query_publication.rs"]
mod publication;

#[path = "catalog_contract_query_typed_groups.rs"]
mod typed_groups;
