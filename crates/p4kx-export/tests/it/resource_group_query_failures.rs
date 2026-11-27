// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::CatalogPublicationMode;
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceGroupParentSnapshot};
use p4kx_extraction::{SourceLocation, SourceUseDescription, SourceUseRole};
use serde_json::Value;

use super::{fixture, parity, walk};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn parent_prefix_knowledge_distinguishes_earlier_and_later_field_failures() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        for before in [false, true] {
            let (_folder, archive) = common::open(&fixture::fault(before), origin)?;
            let catalog = parity::catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
            full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
                .with_resource_group_parents(
                    CatalogRecordSelection::Guid(common::guid(fixture::CHILD).parse()?),
                    None,
                    |snapshot| field_failure(&snapshot, &catalog, before, marker),
                )??;
        }
    }
    Ok(())
}

fn field_failure(
    snapshot: &ResourceGroupParentSnapshot,
    catalog: &Value,
    before: bool,
    marker: Option<bool>,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one selected physical child".into());
    };
    walk::parent(
        row,
        snapshot.source_inventory(),
        fixture::row(catalog, "projections/resourceGroupParents", 2)?,
    )?;
    assert_eq!(
        row.parent().origin().source_set_complete(),
        !before,
        "only an earlier failed group can hide a different winner"
    );
    let expected_marker = if before && marker == Some(false) {
        None
    } else {
        marker
    };
    assert_eq!(
        row.parent().origin().contains_encrypted_source(),
        expected_marker,
        "known encrypted contributors survive incomplete prefix knowledge"
    );
    let broken = u32::from(!before);
    assert_eq!(
        snapshot.dependency_coverage().incomplete_fields,
        [broken],
        "the exact original failed field remains visible"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "the selected child body is readable"
    );
    assert!(snapshot.source_inventory().iter_uses().any(|used| used.role() == SourceUseRole::Attempted && matches!(used.location(), SourceLocation::DataForge { record_index, field } if *record_index == broken && field == "groups")), "the failed original field read remains an actual observation");
    if !before {
        assert!(
            !snapshot.source_inventory().iter_uses().any(|used| row
                .parent()
                .origin()
                .source_uses_refs()
                .contains(used.id())
                && physical(used) == Some(broken)),
            "a later failed group contributes nothing to the established first parent"
        );
    }
    Ok(())
}

#[test]
fn reachable_unreadable_early_path_keeps_parent_choice_and_exact_failed_metadata_scope()
-> TestResult {
    let (_folder, archive) =
        common::open_bytes(fixture::unreadable_first_path()?, DcbOrigin::Plain)?;
    let catalog = parity::catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_resource_group_parents(
        CatalogRecordSelection::Guid(common::guid(fixture::CHILD).parse()?),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one selected readable child".into());
            };
            walk::parent(
                row,
                snapshot.source_inventory(),
                fixture::row(&catalog, "projections/resourceGroupParents", 2)?,
            )?;
            assert_eq!(
                row.parent()
                    .value()
                    .and_then(Option::as_ref)
                    .ok_or("later readable parent")?
                    .target_record_index()
                    .value(),
                Some(&1),
                "the readable later winner stays explicit"
            );
            assert!(
                !row.parent().origin().source_set_complete(),
                "unreadable earlier metadata prevents complete prefix knowledge"
            );
            assert_eq!(
                row.parent().origin().contains_encrypted_source(),
                None,
                "an incomplete plain prefix stays unknown"
            );
            assert_eq!(
                snapshot.dependency_coverage().unreadable_headers,
                [0],
                "the exact failed physical header is retained"
            );
            assert!(snapshot.source_inventory().iter_uses().any(|used| used.role() == SourceUseRole::Attempted && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field } if namespace == "dataforgeRecordHeader" && indices.as_slice() == [0] && field == "fileName")), "the actual failed lazy path getter is observed");
            Ok(())
        },
    )??;
    Ok(())
}

fn physical(used: &SourceUseDescription) -> Option<u32> {
    match used.location() {
        SourceLocation::DataForge { record_index, .. } => Some(*record_index),
        SourceLocation::Subresource {
            namespace, indices, ..
        } if namespace == "dataforgeRecordHeader" => indices
            .first()
            .copied()
            .and_then(|index| u32::try_from(index).ok()),
        _ => None,
    }
}
