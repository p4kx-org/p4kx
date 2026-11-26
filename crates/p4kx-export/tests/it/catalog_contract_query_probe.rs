// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractSnapshot,
};
use p4kx_extraction::{Language, ProvenanceMode};

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin};

#[path = "catalog_contract_query_probe_fixture.rs"]
mod fixture;

#[test]
fn contract_sdk_dependency_probes_publish_only_complete_owner_transactions() -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(), DcbOrigin::Plain)?;
    for mode in [ProvenanceMode::MarkersOnly, ProvenanceMode::Full] {
        CatalogQuerySession::open(
            &archive,
            CatalogLanguageSelection::Selected(&[Language::English]),
            None,
        )?
        .with_provenance_mode(mode)
        .with_contracts(CatalogRecordSelection::RecordIndex(0), None, |snapshot| {
            verify(&snapshot)?;
            verify_text_origins(&snapshot, mode)
        })??;
    }
    Ok(())
}

fn verify(snapshot: &ContractSnapshot<'_>) -> TestResult {
    assert_eq!(
        snapshot.coverage.dependencies.forward_records,
        [1, 2],
        "only the two actual typed dependency bodies are read"
    );
    assert!(
        snapshot.coverage.dependencies.forward_failures.is_empty()
            && snapshot.coverage.projection_failures.is_empty(),
        "a preparation demand never becomes a CIG source or projection error"
    );
    assert!(
        snapshot.coverage.diagnostics.is_empty(),
        "private demand probes do not publish diagnostics"
    );
    assert_eq!(
        snapshot.rows.len(),
        3,
        "partial attempts never append duplicate contract rows"
    );
    verify_intro(snapshot)?;
    verify_texts(snapshot)
}

fn verify_intro(snapshot: &ContractSnapshot<'_>) -> TestResult {
    let consumer = snapshot
        .rows
        .iter()
        .find(|row| {
            row.base.guid.value().is_some_and(|guid| {
                guid.get().to_string() == commodity_v3_fixture::guid(fixture::CONSUMER)
            })
        })
        .ok_or("the final consumer contract")?;
    let [required] = consumer
        .required_intros
        .value()
        .ok_or("the completion-tag prerequisite")?
        .as_slice()
    else {
        return Err("one required completion tag".into());
    };
    let [intro] = required
        .intro_contracts
        .value()
        .ok_or("the completed intro join")?
        .as_slice()
    else {
        return Err("exactly one intro grant after all preparation attempts".into());
    };
    assert_eq!(
        intro
            .value()
            .ok_or("the source-backed intro identity")?
            .get()
            .to_string(),
        commodity_v3_fixture::guid(fixture::INTRO),
        "only the successful physical intro grant is joined"
    );
    Ok(())
}

fn verify_texts(snapshot: &ContractSnapshot<'_>) -> TestResult {
    let actual: std::collections::BTreeSet<_> = snapshot
        .game_texts
        .iter()
        .map(|text| {
            text.key()
                .value()
                .copied()
                .ok_or("an authored final text key")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        actual,
        std::collections::BTreeSet::from(["@shared", "@later_name"]),
        "only the final retained authored text definitions are published"
    );
    assert_eq!(
        snapshot.mission_titles.len(),
        3,
        "private probes cannot duplicate title projections"
    );
    for titles in &snapshot.mission_titles {
        let title = titles
            .title
            .translations
            .get(Language::English.code())
            .ok_or("the requested actual English title")?;
        assert!(
            title
                .value()
                .is_some_and(|text| text == "Shared text" || text == "Later name"),
            "the final projection uses the real locale text"
        );
        assert_eq!(
            title.origin().contains_encrypted_source(),
            Some(true),
            "the retained output carries the actual encrypted locale input"
        );
    }
    Ok(())
}

fn verify_text_origins(snapshot: &ContractSnapshot<'_>, mode: ProvenanceMode) -> TestResult {
    for text in &snapshot.game_texts {
        let refs = text.key().origin().source_uses_refs();
        assert_eq!(
            refs.is_empty(),
            mode == ProvenanceMode::MarkersOnly,
            "published text has reference evidence only in Full mode"
        );
        assert!(
            refs.iter().all(|reference| snapshot
                .source_inventory
                .iter_uses()
                .any(|source| source.id() == reference)),
            "the published text key has no dangling evidence reference after commit"
        );
    }
    Ok(())
}
