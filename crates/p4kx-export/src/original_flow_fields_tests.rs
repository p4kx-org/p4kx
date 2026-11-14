// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, InputScope, SourceEvidenceError, SourceLocation, SourceUseRole, SourceUses,
    SourceValue, Value,
};

use super::super::owned::fixture as walk_fixture;
use super::OriginalFlowFields;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{
    OriginalValueEdge, OriginalValueEvent, OriginalValueFact, OriginalValuePart,
};
use crate::{CatalogError, ExportAccountingError};

#[path = "original_flow_fields_fixture.rs"]
mod fixture;
#[path = "original_flow_reference_tests.rs"]
mod reference_tests;

#[test]
fn captured_edges_restore_containers_and_keep_original_null_and_error_occurrences() -> TestResult {
    test_support::with_input(&walk_fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        assert_eq!(
            tree.error_count(),
            1,
            "the actual invalid pointer is readable"
        );
        let scope = input.record_scope(&record, &tree)?;
        let before = scope.sources().freeze()?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        assert_eq!(
            fields.facts.len(),
            13,
            "all actual preorder values retained"
        );
        occurrence_positions(&fields)?;
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "discarded cached candidates emit no uses"
        );
        Ok(())
    })
}

fn occurrence_positions(fields: &OriginalFlowFields<'_, '_, '_>) -> TestResult {
    let expected = [
        (None, 0),
        (None, 1),
        (None, 2),
        (Some(2), 0),
        (Some(2), 1),
        (None, 3),
        (Some(5), 0),
        (Some(6), 0),
        (Some(5), 1),
        (Some(5), 2),
        (Some(5), 3),
        (Some(10), 0),
        (None, 4),
    ];
    for (index, (parent, ordinal)) in expected.into_iter().enumerate() {
        let edge = if [6, 8, 9, 10].contains(&index) {
            OriginalValueEdge::Element { ordinal }
        } else {
            OriginalValueEdge::Field { ordinal }
        };
        let fact = fields.child(parent, edge)?;
        assert_eq!(
            fact.ordinal(),
            u64::try_from(index)?,
            "the real typed source edge"
        );
    }
    assert!(
        matches!(
            fields.facts.get(8).map(OriginalValueFact::value),
            Some(Value::Null)
        ),
        "the real middle null remains a distinct occurrence"
    );
    assert!(
        matches!(
            fields.facts.get(9).map(OriginalValueFact::value),
            Some(Value::Error(_))
        ),
        "the real invalid pointer remains a distinct occurrence"
    );
    Ok(())
}

#[test]
fn same_record_and_tree_do_not_make_two_scope_frames_interchangeable() -> TestResult {
    test_support::with_input(&walk_fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let other = input.record_scope(&record, &tree)?;
        assert!(
            std::ptr::eq(scope.tree(), other.tree()),
            "same actual materialized read"
        );
        assert!(
            !std::ptr::eq(&raw const scope, &raw const other),
            "two genuine separately admitted frames"
        );
        let mut fields = OriginalFlowFields::read(&scope, None)?;
        let before = scope.sources().freeze()?;
        let mut walk = other.walk(None);
        let Some(OriginalValueEvent::Value(site)) = walk.next_event()? else {
            return Err("actual foreign-frame first value".into());
        };
        assert!(
            matches!(
                fields.retain(site.fact()),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "a foreign frame cannot be attached even with identical physical input and tree"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "rejected facts emit no source claims"
        );
        Ok(())
    })
}

#[test]
fn original_index_cancellation_is_explicit_and_registers_no_candidate_uses() -> TestResult {
    test_support::with_input(&walk_fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let before = scope.sources().freeze()?;
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                OriginalFlowFields::read(&scope, Some(&cancel)),
                Err(CatalogError::Cancelled)
            ),
            "actual canonical walk cancellation is fatal"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "no invented candidate payload uses"
        );
        Ok(())
    })
}

#[test]
fn a_retained_fact_binds_the_same_exact_current_loot_coordinate() -> TestResult {
    test_support::with_input(&walk_fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let fact = fields.child(None, OriginalValueEdge::Field { ordinal: 0 })?;
        assert!(
            matches!(fact.value(), Value::Bool(false)),
            "genuine original false"
        );
        let cell = fact
            .bind(
                OriginalValuePart::Value,
                SourceValue::Present(false),
                SourceUseRole::Payload,
                true,
            )?
            .into_cell(scope.sources());
        let cell = serde_json::to_value(cell)?;
        assert_eq!(
            cell.get("containsEncryptedSource"),
            Some(&serde_json::json!(false)),
            "real plain original input remains proved"
        );
        assert_eq!(
            cell.get("value"),
            Some(&serde_json::json!(false)),
            "the actual value retained"
        );
        coordinate_uses(fact, scope.sources())?;
        Ok(())
    })
}

fn coordinate_uses(fact: &OriginalValueFact<'_, '_, '_>, sources: &SourceUses) -> TestResult {
    let expected = SourceLocation::Subresource {
        namespace: "dataforgeValue".to_owned(),
        indices: vec![0, 0],
        field: "$value".to_owned(),
    };
    assert_eq!(
        fact.location(OriginalValuePart::Value),
        expected,
        "the retained fact keeps its exact original coordinate"
    );
    let inventory = sources.freeze()?;
    let uses: Vec<_> = inventory
        .uses()
        .iter()
        .filter(|input| input.location() == &expected)
        .collect();
    assert_eq!(uses.len(), 1, "one genuine exact-coordinate observation");
    let input = uses.first().ok_or("actual retained source use")?;
    assert_eq!(
        input.role(),
        SourceUseRole::Payload,
        "the actual value contribution"
    );
    assert_eq!(
        input.scope(),
        InputScope::Payload,
        "the original body scope"
    );
    Ok(())
}

pub(super) fn conflicting<T>(result: &Result<T, CatalogError>) {
    assert!(
        matches!(
            result,
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "explicit genuine-index contradiction"
    );
}
