// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ContractSnapshot, with_contracts,
};
use p4kx_extraction::DataForge;

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin};

#[path = "catalog_contract_query_typed_group_fixture.rs"]
mod fixture;

#[test]
fn contract_sdk_same_guid_other_family_cannot_activate_its_unreadable_body() -> TestResult {
    let bytes = fixture::cross_family();
    let forge = DataForge::open(&bytes)?;
    let template = forge.record(1).ok_or("the actual Template")?;
    let place = forge.record(2).ok_or("the unrelated Place")?;
    assert_eq!(
        template.guid(),
        place.guid(),
        "both physical source families have the same nominal GUID"
    );
    template.tree()?;
    assert!(
        place.tree().is_err(),
        "the other family's body is genuinely unreadable"
    );
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    with_contracts(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage.dependencies.forward_records,
                [1],
                "the real Template lookup reads its own complete nominal group only"
            );
            assert!(
                snapshot.coverage.dependencies.forward_failures.is_empty()
                    && snapshot.coverage.projection_failures.is_empty(),
                "an equal nominal GUID cannot activate an unrelated family's failed body"
            );
            assert_eq!(
                snapshot.rows.len(),
                1,
                "the independent selected contract remains available"
            );
        },
    )?;
    Ok(())
}

#[test]
fn contract_sdk_nominal_reward_group_keeps_last_usable_amount_and_actual_failed_candidate()
-> TestResult {
    let bytes = fixture::rewards();
    let forge = DataForge::open(&bytes)?;
    let failure = forge
        .record(3)
        .ok_or("the original later Reward")?
        .tree()
        .expect_err("the final nominal candidate is truly unreadable");
    assert!(
        forge.record(4).ok_or("the unused Reward")?.tree().is_err(),
        "the unreferenced body is also genuinely unreadable"
    );
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    with_contracts(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage.dependencies.forward_records,
                [1, 2, 3],
                "all and only the demanded nominal Reward candidates are prepared in physical order"
            );
            let [actual] = snapshot.coverage.dependencies.forward_failures.as_slice() else {
                return Err("one actual nominal candidate body failure".into());
            };
            assert_eq!(
                actual.record().index(),
                3,
                "the real failed duplicate remains identified"
            );
            assert_eq!(
                actual.canonical_error(),
                Some(&failure),
                "coverage preserves the original body error"
            );
            verify_reward_amount(&snapshot)?;
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

fn verify_reward_amount(snapshot: &ContractSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one admitted contract row".into());
    };
    let [gain] = row
        .base
        .reputation_gains
        .value()
        .ok_or("the authored successful result")?
        .as_slice()
    else {
        return Err("one authored Reputation amount".into());
    };
    assert_eq!(
        gain.amount.value().map(|amount| amount.get()),
        Some(9),
        "the canonical last-usable rule survives an unreadable later duplicate"
    );
    assert!(
        !gain.amount.origin().source_set_complete(),
        "the actual failed selection candidate cannot certify complete amount provenance"
    );
    assert_eq!(
        gain.amount.origin().contains_encrypted_source(),
        None,
        "an incomplete plain source selection is not falsely labeled public"
    );
    Ok(())
}
