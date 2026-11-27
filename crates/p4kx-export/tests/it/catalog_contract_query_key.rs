// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractKey,
    ContractSnapshot, with_contract,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

use super::query_source_fixture as fixture;

fn parent_key() -> TestResult<ContractKey> {
    Ok(ContractKey::new(fixture::CONTRACT_TEXT.parse()?, None))
}

fn sub_key() -> TestResult<ContractKey> {
    Ok(ContractKey::new(
        fixture::CONTRACT_TEXT.parse()?,
        Some(guid(fixture::SUB).parse()?),
    ))
}

fn verify_single(snapshot: &ContractSnapshot<'_>, key: ContractKey, owner: u32) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one exact logical row".into());
    };
    assert_eq!(
        row.base.guid.value().map(|guid| guid.get()),
        Some(key.contract_guid()),
        "the key is the logical parent identity"
    );
    assert_eq!(
        row.base.sub_contract_guid.value().map(|guid| guid.get()),
        key.sub_contract_guid(),
        "an absent sub identity selects only the parent"
    );
    assert_eq!(
        row.base.source.record_index(),
        owner,
        "the global winner retains its physical owner"
    );
    assert_eq!(
        snapshot.mission_titles.len(),
        1,
        "the title projection contains the same single occurrence"
    );
    Ok(())
}

#[test]
fn contract_sdk_logical_parent_matches_complete_catalog_and_preserves_owner_api() -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(false, false), DcbOrigin::Plain)?;
    let mut options = commodity_v3_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let expected = proof::rows(&catalog, "/projections/contracts/rows")?
        .iter()
        .find(|row| {
            row.pointer("/guid/value").and_then(Value::as_str) == Some(fixture::CONTRACT_TEXT)
                && row
                    .pointer("/subContractGuid/value")
                    .and_then(Value::as_str)
                    .is_none()
        })
        .ok_or("the canonical parent")?;
    let session = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_contract(parent_key()?, None, |snapshot| {
        verify_single(&snapshot, parent_key()?, 1)?;
        let [actual] = snapshot.rows.as_slice() else {
            return Err("one parent row".into());
        };
        assert_eq!(
            serde_json::to_value(actual)?,
            *expected,
            "the logical lookup uses the complete producer's canonical row and intro join"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    parsed.with_contracts(CatalogRecordSelection::RecordIndex(1), None, |snapshot| {
        assert_eq!(
            snapshot.rows.len(),
            2,
            "the physical-owner API still includes its parent and sub-contract"
        );
    })?;
    Ok(())
}

#[test]
fn contract_sdk_logical_subcontract_reports_only_its_exact_shadow_candidates() -> TestResult {
    let bytes = fixture::database_with(fixture::Options {
        outside_subcontract: true,
        ..fixture::Options::default()
    });
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    with_contract(
        &archive,
        sub_key()?,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            verify_single(&snapshot, sub_key()?, 1)?;
            let [shadowed] = snapshot.coverage.shadowed_candidates.as_slice() else {
                return Err("one exact sub-contract loser without the parent loser".into());
            };
            assert_eq!(
                shadowed.contract_guid,
                sub_key()?.contract_guid(),
                "the losing parent identity is retained"
            );
            assert_eq!(
                shadowed.sub_contract_guid,
                sub_key()?.sub_contract_guid(),
                "only this sub-contract's collision is reported"
            );
            assert_eq!(
                (shadowed.selected_record_index, shadowed.winner_record_index),
                (2, 1),
                "global physical duplicate ordering is preserved"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

#[test]
fn contract_sdk_logical_key_validates_all_owners_before_choosing_a_winner() -> TestResult {
    let bytes = fixture::database_with(fixture::Options {
        root_handler: fixture::HandlerState::Malformed,
        ..fixture::Options::default()
    });
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    session.with_contract(parent_key()?, None, |snapshot| {
        verify_single(&snapshot, parent_key()?, 2)?;
        assert!(
            snapshot
                .coverage
                .projection_failures
                .iter()
                .any(|failure| failure.record().index() == 1),
            "the earlier nominal winner failed its whole-owner transaction"
        );
        assert!(
            snapshot
                .coverage
                .dependencies
                .survey_failures
                .iter()
                .any(|failure| failure.record().index() == 1),
            "the source-only target-link guard remains observable"
        );
        assert!(
            snapshot.coverage.shadowed_candidates.is_empty(),
            "a failed owner supplies no positive contender"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    Ok(())
}

#[test]
fn contract_sdk_logical_parent_does_not_activate_a_subcontracts_intro_owner() -> TestResult {
    let bytes = fixture::database_with(fixture::Options {
        include_collision: false,
        intro: fixture::IntroRequirements {
            parent: false,
            sub: true,
        },
        unrelated_owner: fixture::UnrelatedOwner::Malformed,
        ..fixture::Options::default()
    });
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    parsed.with_contract(parent_key()?, None, |snapshot| {
        verify_single(&snapshot, parent_key()?, 1)?;
        assert!(
            snapshot.dependency_generators.is_empty(),
            "a different logical row's requirement does not wake its intro provider"
        );
        assert!(
            snapshot.coverage.projection_failures.is_empty(),
            "unrelated malformed owners are not projected"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    parsed.with_contract(sub_key()?, None, |snapshot| {
        verify_single(&snapshot, sub_key()?, 1)?;
        assert_eq!(
            snapshot.coverage.dependencies.related_generators,
            [2],
            "the exact sub-contract's own requirement activates the outside provider"
        );
        let [row] = snapshot.rows.as_slice() else {
            return Err("the selected sub-contract".into());
        };
        let requirements = row
            .required_intros
            .value()
            .ok_or("the sub-contract requirement")?;
        assert!(
            requirements
                .iter()
                .any(
                    |required| required.intro_contracts.value().is_some_and(|values| values
                        .iter()
                        .any(|intro| intro
                            .value()
                            .is_some_and(|guid| guid.get().to_string() == fixture::INTRO_TEXT)))
                ),
            "the exact sub-contract retains the actual outside intro identity"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    Ok(())
}

#[test]
fn contract_sdk_unknown_logical_key_keeps_source_gaps_without_loading_locales() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("unknown.p4k");
    let bytes = fixture::database_with(fixture::Options {
        outside_instance: Some(u16::MAX),
        ..fixture::Options::default()
    });
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes))
        .entry(TestEntry::stored(Language::English.global_ini_path(), b"locked=text\n").encrypted())
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let key = ContractKey::new(guid(fixture::ROOT).parse()?, None);
    with_contract(
        &archive,
        key,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            assert!(
                snapshot.rows.is_empty(),
                "a Generator GUID is not a logical contract identity"
            );
            assert_eq!(
                snapshot.coverage.dependencies.survey_failures.len(),
                1,
                "unreadable reverse data cannot establish authoritative absence"
            );
            assert!(
                snapshot.available_languages.is_empty(),
                "no readable logical match needs a localization payload"
            );
        },
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "the absent key does not read locked localization"
    );
    Ok(())
}
