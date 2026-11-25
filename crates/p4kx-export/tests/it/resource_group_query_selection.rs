// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::CatalogPublicationMode;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordReference, CatalogRecordSelection, DefinitionStatus,
    ResourceGroupParentSnapshot, ResourceGroupSnapshot,
};
use p4kx_extraction::{DataForgeEntry, SourceValueState, SourcedValue};
use serde_json::Value;

use super::{fixture, parity, walk};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn direct_guid_aliases_and_physical_selection_match_their_canonical_rows() -> TestResult {
    let (_folder, archive) = common::open(&fixture::graph(), DcbOrigin::Plain)?;
    let catalog = parity::catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    let entry = DataForgeEntry::read(&archive)?;
    full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
        .with_resource_groups(
            CatalogRecordSelection::Guid(common::guid(fixture::CHILD).parse()?),
            None,
            |snapshot| selected_direct(&snapshot, &catalog, &[1, 5]),
        )??;
    full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_resource_groups(CatalogRecordSelection::RecordIndex(5), None, |snapshot| {
        selected_direct(&snapshot, &catalog, &[5])
    })??;
    Ok(())
}

#[test]
fn parent_owner_selection_retains_the_entire_required_family_and_first_parent() -> TestResult {
    let (_folder, archive) = common::open(&fixture::graph(), DcbOrigin::Plain)?;
    let catalog = parity::catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    let entry = DataForgeEntry::read(&archive)?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_resource_group_parents(
        CatalogRecordSelection::Guid(common::guid(fixture::CHILD).parse()?),
        None,
        |snapshot| selected_parents(&snapshot, &catalog, &[1, 5], 0),
    )??;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_resource_group_parents(
            CatalogRecordSelection::RecordIndex(1),
            None,
            |snapshot| selected_parents(&snapshot, &catalog, &[1], 0),
        )??;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_resource_group_parents(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| selected_parents(&snapshot, &catalog, &[0], 1),
    )??;
    Ok(())
}

fn selected_direct(
    snapshot: &ResourceGroupSnapshot<'_>,
    catalog: &Value,
    physical: &[u32],
) -> TestResult {
    let indices = snapshot
        .definitions()
        .iter()
        .map(|row| row.header().record_index().value().copied())
        .collect::<Vec<_>>();
    assert_eq!(
        indices,
        physical.iter().copied().map(Some).collect::<Vec<_>>(),
        "physical owner selection retains stable same-GUID aliases"
    );
    assert_eq!(
        snapshot.members().len(),
        physical.len(),
        "selected memberships have the same physical owners"
    );
    for ((definition, members), physical) in snapshot
        .definitions()
        .iter()
        .zip(snapshot.members())
        .zip(physical)
    {
        assert_eq!(
            members.record_index().value(),
            Some(physical),
            "declarations never merge nominal aliases"
        );
        walk::definition(
            definition,
            snapshot.source_inventory(),
            fixture::row(catalog, "definitions/resourceGroups", u64::from(*physical))?,
        )?;
        walk::members(
            members,
            snapshot.source_inventory(),
            fixture::row(
                catalog,
                "declarations/resourceGroupMembers",
                u64::from(*physical),
            )?,
        )?;
    }
    Ok(())
}

fn selected_parents(
    snapshot: &ResourceGroupParentSnapshot,
    catalog: &Value,
    physical: &[u32],
    expected_parent: u32,
) -> TestResult {
    assert_eq!(
        snapshot.rows().len(),
        physical.len(),
        "only requested child owners become output rows"
    );
    for (row, physical) in snapshot.rows().iter().zip(physical) {
        assert_eq!(
            row.record_index().value(),
            Some(physical),
            "physical child identities stay separate"
        );
        walk::parent(
            row,
            snapshot.source_inventory(),
            fixture::row(
                catalog,
                "projections/resourceGroupParents",
                u64::from(*physical),
            )?,
        )?;
        let parent = row
            .parent()
            .value()
            .and_then(Option::as_ref)
            .ok_or("actual first parent")?;
        assert_eq!(
            parent.target_record_index().value(),
            Some(&expected_parent),
            "parent record selection remains physical"
        );
    }
    Ok(())
}

#[test]
fn original_memberships_keep_nulls_repetitions_wrong_family_dangling_and_self_edges() -> TestResult
{
    let (_folder, archive) = common::open(&fixture::graph(), DcbOrigin::Plain)?;
    full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
        .with_resource_groups(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| -> TestResult {
                let [members] = snapshot.members() else {
                    return Err("one selected membership owner".into());
                };
                groups(members.groups().value().ok_or("original groups array")?)?;
                resources(
                    members
                        .resources()
                        .value()
                        .ok_or("original resource array")?,
                )?;
                Ok(())
            },
        )??;
    Ok(())
}

fn groups(slots: &[SourcedValue<CatalogRecordReference>]) -> TestResult {
    let [null, foreign, missing, first, repeated, self_edge] = slots else {
        return Err("every original group slot survives in source order".into());
    };
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "authored null slot stays null"
    );
    reference_states(foreign, missing)?;
    assert_ne!(
        first.origin().source_uses_refs(),
        repeated.origin().source_uses_refs(),
        "equal child GUIDs keep distinct physical source slots"
    );
    assert_eq!(
        first.value().ok_or("first child")?.guid().value(),
        repeated.value().ok_or("repeated child")?.guid().value(),
        "the original repeated child values match"
    );
    assert_eq!(
        self_edge
            .value()
            .ok_or("self edge")?
            .guid()
            .value()
            .map(ToString::to_string),
        Some(common::guid(fixture::FIRST)),
        "raw self edges are preserved"
    );
    Ok(())
}

fn reference_states(
    foreign: &SourcedValue<CatalogRecordReference>,
    missing: &SourcedValue<CatalogRecordReference>,
) -> TestResult {
    let foreign = foreign.value().ok_or("wrong-family source Reference")?;
    assert!(
        matches!(foreign.status(), DefinitionStatus::Present),
        "source resolution remains separate from catalog-family health"
    );
    assert_eq!(
        foreign.target_type().value().map(String::as_str),
        Some("Other"),
        "actual unexpected target family is retained"
    );
    let missing = missing.value().ok_or("dangling source Reference")?;
    assert!(
        matches!(missing.status(), DefinitionStatus::Dangling),
        "missing authored target stays dangling"
    );
    assert_eq!(
        missing.guid().value().map(ToString::to_string),
        Some(common::guid(fixture::MISSING)),
        "original missing GUID survives"
    );
    Ok(())
}

fn resources(slots: &[SourcedValue<CatalogRecordReference>]) -> TestResult {
    let [first, null, repeated] = slots else {
        return Err("every original resource slot survives in source order".into());
    };
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "resource null remains at its authored position"
    );
    assert_eq!(
        first.value().ok_or("first resource")?.guid().value(),
        repeated.value().ok_or("repeated resource")?.guid().value(),
        "the original repeated resource values match"
    );
    assert_ne!(
        first.origin().source_uses_refs(),
        repeated.origin().source_uses_refs(),
        "repeated resources keep separate original uses"
    );
    Ok(())
}
