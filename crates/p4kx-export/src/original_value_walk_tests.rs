// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, RecordTree, SourceEvidenceError, SourceLocation, Value, ValueError,
};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};

use super::fixture;
use super::{OriginalValueEdge, OriginalValueEvent, OriginalValuePart, OriginalValueSite};

#[test]
fn original_walk_keeps_dfs_ordinals_equal_nodes_null_and_error_slots() -> TestResult {
    test_support::with_input(&fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("original Record")?;
        let tree = record.tree()?;
        assert_eq!(
            tree.fields().len(),
            5,
            "duplicate declarations are original fields"
        );
        assert_eq!(tree.error_count(), 1, "one genuine invalid pointer");
        let scope = input.record_scope(&record, &tree)?;
        let mut walk = scope.walk(None);
        let mut visits = 0;
        let mut ends = Vec::new();
        while let Some(event) = walk.next_event()? {
            let site = match event {
                OriginalValueEvent::Value(site) => site,
                OriginalValueEvent::EndContainer { ordinal, .. } => {
                    ends.push(ordinal);
                    continue;
                }
            };
            check_position(&site, visits)?;
            check_holes(&site, visits);
            visits += 1;
        }
        assert_eq!(visits, 13, "no equal-node interning or missing slots");
        assert_eq!(ends, [2, 6, 10, 5], "each actual container finalizes once");
        assert!(
            walk.next_event()?.is_none(),
            "completed walk stays complete"
        );
        Ok(())
    })
}

#[test]
fn original_walk_rejects_wrong_original_owners_and_latches_cancellation() -> TestResult {
    let bytes = fixture::database();
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("original Record")?;
        let other = forge.record(1).ok_or("other actual Record")?;
        let tree = record.tree()?;
        assert!(
            matches!(
                input.record_scope(&other, &tree),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "same Forge, wrong Record does not admit a tree"
        );
        check_copied_source(&bytes, input, &tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let cancel = CancelToken::new();
        let mut walk = scope.walk(Some(&cancel));
        assert!(
            matches!(walk.next_event()?, Some(OriginalValueEvent::Value(_))),
            "positive first actual value"
        );
        cancel.cancel();
        assert!(
            matches!(walk.next_event(), Err(CatalogError::Cancelled)),
            "cancel before next value"
        );
        assert!(
            matches!(walk.next_event(), Err(CatalogError::Cancelled)),
            "stopped is latched"
        );
        Ok(())
    })
}

fn check_position(site: &OriginalValueSite<'_, '_, '_, '_>, visits: usize) -> TestResult {
    let parents = [
        None,
        None,
        None,
        Some(2),
        Some(2),
        None,
        Some(5),
        Some(6),
        Some(5),
        Some(5),
        Some(5),
        Some(10),
        None,
    ];
    let edges = [0, 1, 2, 0, 1, 3, 0, 0, 1, 2, 3, 0, 4];
    assert_eq!(
        site.ordinal(),
        u64::try_from(visits)?,
        "actual preorder occurrence"
    );
    assert_eq!(
        site.parent_ordinal(),
        parents.get(visits).copied().ok_or("parent oracle")?,
        "the exact original parent ordinal"
    );
    let ordinal = *edges.get(visits).ok_or("edge oracle")?;
    let expected = if [6, 8, 9, 10].contains(&visits) {
        OriginalValueEdge::Element { ordinal }
    } else {
        OriginalValueEdge::Field { ordinal }
    };
    assert_eq!(
        site.edge(),
        expected,
        "the exact original field or element edge"
    );
    assert_eq!(
        site.fact().location(OriginalValuePart::Value),
        SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![0, u64::try_from(visits)?],
            field: "$value".to_owned(),
        },
        "the exact original value source coordinate"
    );
    Ok(())
}

fn check_holes(site: &OriginalValueSite<'_, '_, '_, '_>, visits: usize) {
    if visits == 8 {
        assert!(matches!(site.value(), Value::Null), "actual middle null");
    }
    if visits == 9 {
        assert!(
            matches!(
                site.value(),
                Value::Error(ValueError::InvalidPointer {
                    struct_index: 77,
                    instance_index: 99
                })
            ),
            "actual typed error words"
        );
        assert_eq!(
            site.fact().location(OriginalValuePart::DataType),
            SourceLocation::Subresource {
                namespace: "dataforgeValue".to_owned(),
                indices: vec![0, 5],
                field: "$dataType".to_owned(),
            },
            "array slot uses the actual declaring field metadata"
        );
    }
}

fn check_copied_source(
    bytes: &[u8],
    input: &OriginalCatalogInput<'_, '_>,
    tree: &RecordTree<'_>,
) -> TestResult {
    test_support::with_input(bytes, |foreign, _| {
        let copied = foreign.record(0).ok_or("foreign equal Record")?;
        let copied_tree = copied.tree()?;
        assert_eq!(*tree, copied_tree, "equal data is the negative control");
        assert!(!tree.is_from(&copied), "genuine owner identity differs");
        assert!(
            matches!(
                input.record_scope(&copied, &copied_tree),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "equal copied inputs cannot use this catalog authority"
        );
        Ok(())
    })
}
