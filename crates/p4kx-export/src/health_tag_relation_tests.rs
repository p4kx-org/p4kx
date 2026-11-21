// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    DataForgeError, Record, RecordTree, SourceEvidenceError, SourceLocation, SourceUseRole,
    TagBuildEvent, TagPathEdgeKind, TagRelationIssue, TagTree, Value,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::catalog_read::original::{ExampleBodyReceipt, OriginalFlowFields};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

use super::TagRelationReceipts;

#[path = "health_tag_relation_fixture.rs"]
mod fixture;

#[test]
fn original_tag_relation_bodies_keep_target_words_and_header_roles_until_retention() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let captured = fixture::bodies(forge, input)?;
        let before = input.origins().uses.freeze()?;
        for body in captured.bodies {
            let retained = RetainedExampleBody::read(body, input)?;
            for field in retained.texts() {
                assert!(
                    field
                        .contributions()
                        .0
                        .iter()
                        .all(|used| used.role() != SourceUseRole::Payload),
                    "display field paths and SourceType headers do not borrow target-word payload roles"
                );
            }
        }
        let after = input.origins().uses.freeze()?;
        let payloads: Vec<_> = after
            .uses()
            .iter()
            .filter(|used| used.role() == SourceUseRole::Payload && !before.uses().contains(used))
            .collect();
        assert_eq!(
            payloads.len(),
            4,
            "only the four retained actual relation GUID words add payload"
        );
        for expected in &captured.target_positions {
            assert!(
                payloads.iter().any(|used| used.location() == expected),
                "the retained GUID word keeps the exact original skipped array occurrence"
            );
        }
        for record_index in [0, 1] {
            for field in ["$value", "$error.guid"] {
                assert!(payloads.iter().any(|used| matches!(used.location(),
                    SourceLocation::Subresource { namespace, indices, field: actual }
                        if namespace == "dataforgeValue" && indices.first() == Some(&record_index) && actual == field)),
                    "each real owner retains its resolved and dangling GUID occurrence separately");
            }
        }
        Ok(())
    })
}

#[test]
fn genuine_foreign_relation_events_cannot_rebind_or_consume_an_owned_receipt() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        test_support::with_input(&fixture::bytes(), |foreign_forge, foreign| {
            let foreign_record = foreign_forge
                .record(0)
                .ok_or("the separate actual database")?;
            let foreign_subject = RecordObservation::capture(foreign, &foreign_record)?;
            let mut receipts = TagRelationReceipts::default();
            let mut rejected = 0;
            TagTree::build_observed(forge, &|| false, &mut |event| {
                match event {
                    TagBuildEvent::CanonicalRead { record, outcome } => {
                        let _positions = fixture::capture(&mut receipts, input, record, outcome.map_err(DataForgeError::clone)?)?;
                    }
                    TagBuildEvent::RelationSkipped { incoming, target, .. } if incoming.record().index() == 0 => {
                        let before = foreign.origins().uses.freeze()?;
                        assert!(matches!(receipts.take(foreign, foreign_subject.subject(), incoming, target),
                            Err(CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence))),
                            "an actual foreign physical Record cannot authorize the equal-index report");
                        assert_eq!(before, foreign.origins().uses.freeze()?, "foreign refusal registers no contribution");
                        let own = RecordObservation::capture(input, incoming.record())?;
                        let receipt = receipts.take(input, own.subject(), incoming, target)?;
                        assert_eq!(receipt.description()?.2, target, "foreign rejection did not consume the actual owner's body");
                        rejected += 1;
                    }
                    _ => {}
                }
                Ok::<_, Box<dyn Error>>(())
            })?.ok_or("the actual original build must complete")?;
            assert_eq!(
                rejected, 2,
                "both original database relation failures are independently protected"
            );
            Ok(())
        })
    })
}

#[test]
fn same_archive_and_forge_do_not_admit_a_second_report_inventory() -> TestResult {
    fixture::with_reports(|forge, input, foreign| {
        let record = forge.record(0).ok_or("the one actual physical database")?;
        input.record_context(&record)?;
        foreign.record_context(&record)?;
        let foreign_subject = RecordObservation::capture(foreign, &record)?;
        let mut receipts = TagRelationReceipts::default();
        let mut checked = 0;
        TagTree::build_observed(forge, &|| false, &mut |event| {
            match event {
                TagBuildEvent::CanonicalRead { record, outcome } => {
                    let _positions = fixture::capture(
                        &mut receipts,
                        input,
                        record,
                        outcome.map_err(DataForgeError::clone)?,
                    )?;
                }
                TagBuildEvent::RelationSkipped {
                    incoming, target, ..
                } if incoming.record().index() == 0 => {
                    let before = foreign.origins().uses.freeze()?;
                    assert!(
                        matches!(
                            receipts.take(foreign, foreign_subject.subject(), incoming, target),
                            Err(CatalogError::Accounting(
                                ExportAccountingError::UnboundHealthSubject
                            ))
                        ),
                        "the same physical Record does not pair two genuine report inventories"
                    );
                    assert_eq!(
                        before,
                        foreign.origins().uses.freeze()?,
                        "cross-inventory refusal emits no source uses"
                    );
                    let own = RecordObservation::capture(input, incoming.record())?;
                    let _body = receipts.take(input, own.subject(), incoming, target)?;
                    checked += 1;
                }
                _ => {}
            }
            Ok::<_, Box<dyn Error>>(())
        })?
        .ok_or("the actual Tag build completes")?;
        assert_eq!(
            checked, 2,
            "both real database skip occurrences retain their owning report"
        );
        Ok(())
    })
}

#[test]
fn original_non_reference_values_produce_absent_targets_without_guid_or_payload_fallback()
-> TestResult {
    test_support::with_input(&fixture::malformed_bytes(), |forge, input| {
        let mut receipts = TagRelationReceipts::default();
        let mut source_controls = 0;
        let mut bodies = Vec::new();
        let tree = TagTree::build_observed(forge, &|| false, &mut |event| {
            match event {
                TagBuildEvent::CanonicalRead { record, outcome } => {
                    capture_malformed(
                        &mut receipts,
                        input,
                        record,
                        outcome.map_err(DataForgeError::clone)?,
                    )?;
                    source_controls += 1;
                }
                TagBuildEvent::RelationSkipped {
                    incoming,
                    target,
                    reason,
                } => {
                    assert_eq!(
                        reason,
                        TagRelationIssue::MalformedValue,
                        "the real reader retains its malformed decision"
                    );
                    assert!(
                        target.is_none(),
                        "readable non-reference data has no invented GUID"
                    );
                    assert_eq!(
                        incoming.ordinal(),
                        bodies.len(),
                        "the actual source slots remain ordered"
                    );
                    let admission = RecordObservation::capture(input, incoming.record())?;
                    bodies.push(receipts.take(input, admission.subject(), incoming, target)?);
                }
                _ => {}
            }
            Ok::<_, Box<dyn Error>>(())
        })?
        .ok_or("the actual malformed-source build completes")?;
        assert_eq!(
            source_controls, 1,
            "the genuine source control precedes both diagnostics"
        );
        assert_eq!(tree.len(), 1, "the Tag remains projected");
        check_absent_target_payload(bodies, input)
    })
}

fn check_absent_target_payload(
    bodies: Vec<ExampleBodyReceipt>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult {
    let before = input.origins().uses.freeze()?;
    for body in bodies {
        assert!(
            body.description()?.2.is_none(),
            "the source receipt retains target absence"
        );
        let _retained = RetainedExampleBody::read(body, input)?;
    }
    let after = input.origins().uses.freeze()?;
    assert!(
        after
            .uses()
            .iter()
            .filter(|used| !before.uses().contains(used))
            .all(|used| used.role() != SourceUseRole::Payload),
        "no target GUID payload is fabricated"
    );
    Ok(())
}

fn capture_malformed<'source>(
    receipts: &mut TagRelationReceipts,
    input: &OriginalCatalogInput<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> TestResult {
    let Some(Value::Array(values)) = tree.field("children") else {
        return Err("original malformed array".into());
    };
    assert_eq!(
        tree.error_count(),
        0,
        "the Int32 words are genuinely fully readable"
    );
    assert_eq!(
        values.as_ref(),
        [Value::Int32(7), Value::Int32(8)],
        "the original non-reference values"
    );
    let admission = RecordObservation::capture(input, record)?;
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let body = fields.body_capture(input, &admission)?;
    receipts.capture(TagPathEdgeKind::Child, tree, &fields, &body)?;
    Ok(())
}
