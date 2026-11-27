// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlueprintCostMeaning, BlueprintCostPosition, BlueprintDefinition, BlueprintTier,
    CatalogLanguageSelection, CatalogRecordSelection, SourcePayload, with_blueprints,
};
use p4kx_extraction::{Archive, Guid};

use super::blueprint_query_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn blueprint_guid_duplicates_preserve_individually_queryable_physical_owners() -> TestResult {
    let database = fixture::database().record(fixture::duplicate_owner());
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let expected = common::guid(fixture::OWNER).parse()?;
    let indices = physical_indices(&archive, expected)?;
    assert!(
        indices.windows(2).all(|pair| pair[0] < pair[1]),
        "equal-GUID owners keep distinct physical order"
    );
    for index in indices {
        verify_physical(&archive, expected, index)?;
    }
    Ok(())
}

fn physical_indices(archive: &Archive, expected: Guid) -> TestResult<Vec<u32>> {
    with_blueprints(
        archive,
        CatalogRecordSelection::Guid(expected),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| -> TestResult<Vec<u32>> {
            assert_eq!(
                snapshot.definitions().len(),
                2,
                "both eligible physical Blueprint declarations survive"
            );
            snapshot
                .definitions()
                .iter()
                .map(|definition| {
                    assert_eq!(
                        definition.header().guid().value(),
                        Some(&expected),
                        "GUID selection retains exactly its owners"
                    );
                    definition
                        .header()
                        .record_index()
                        .value()
                        .copied()
                        .ok_or_else(|| "Blueprint physical index".into())
                })
                .collect()
        },
    )?
}

fn verify_physical(archive: &Archive, expected: Guid, index: u32) -> TestResult {
    with_blueprints(
        archive,
        CatalogRecordSelection::RecordIndex(index),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.definitions().len(),
                1,
                "physical selection retains exactly one owner"
            );
            for definition in snapshot.definitions() {
                assert_eq!(
                    definition.header().record_index().value(),
                    Some(&index),
                    "physical index is exact"
                );
                assert_eq!(
                    definition.header().guid().value(),
                    Some(&expected),
                    "physical and external identities stay distinct"
                );
            }
        },
    )?;
    Ok(())
}

#[test]
fn blueprint_quality_positions_use_original_arrays_despite_repeated_debug_names() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    with_blueprints(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.definitions().len(),
                1,
                "one physical owner is selected"
            );
            let owner = snapshot.definitions().first().ok_or("Blueprint owner")?;
            let indices: Vec<_> = snapshot
                .tiers()
                .iter()
                .map(|tier| tier.tier_index().value().copied())
                .collect();
            assert_eq!(
                indices,
                [Some(0), Some(1)],
                "tier identity follows original array positions"
            );
            for tier in snapshot.tiers() {
                verify_cost_positions(tier, owner)?;
            }
            Ok(())
        },
    )??;
    Ok(())
}

fn verify_cost_positions(tier: &BlueprintTier, owner: &BlueprintDefinition<'_>) -> TestResult {
    let selections = repeated_selections(tier, owner)?;
    let [(1, first), (2, second)] = selections.as_slice() else {
        return Err("original optional positions after a null sibling".into());
    };
    assert_ne!(
        first, second,
        "same debug text has different owner-local occurrences"
    );
    for (parent, expected) in [(first, &[1, 2][..]), (second, &[0][..])] {
        let actual: Vec<_> = tier
            .costs()
            .iter()
            .filter_map(|cost| match cost.position().value() {
                Some(BlueprintCostPosition::Option {
                    parent: source,
                    index,
                }) if source.value_index() == *parent => Some(*index),
                _ => None,
            })
            .collect();
        assert_eq!(
            actual, expected,
            "alternative identity uses its actual parent and original option position"
        );
    }
    Ok(())
}

fn repeated_selections(
    tier: &BlueprintTier,
    owner: &BlueprintDefinition<'_>,
) -> TestResult<Vec<(u32, u64)>> {
    let mut selections = Vec::new();
    for cost in tier.costs() {
        let Some(BlueprintCostPosition::Optional { index }) = cost.position().value() else {
            continue;
        };
        let BlueprintCostMeaning::Select { debug_name, .. } = cost.meaning() else {
            return Err("optional authored selection".into());
        };
        let node = owner
            .source()
            .value(debug_name.ok_or("original debug occurrence")?)
            .ok_or("debug occurrence in owner arena")?;
        let Some(SourcePayload::String { value }) = node.value().value() else {
            return Err("original debug text".into());
        };
        assert_eq!(
            value.as_str(),
            fixture::REPEATED,
            "the source really repeats the same debug text"
        );
        selections.push((*index, cost.source().value_index()));
    }
    Ok(selections)
}
