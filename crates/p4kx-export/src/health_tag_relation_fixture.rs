// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;

use p4kx_extraction::{
    DataForge, DataForgeError, Guid, Record, RecordTree, SourceLocation, TagBuildEvent,
    TagPathEdgeKind, TagPathIncoming, TagTree, Value, ValueError,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::{
    ExampleBodyReceipt, OriginalErrorParameter, OriginalFlowFields, OriginalValuePart,
};
use crate::record_observation::RecordObservation;

use super::TagRelationReceipts;

pub(super) fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::Reference),
        )
        .structure(TestStruct::new("TagDatabase").array("tags", TestType::Reference))
        .record(
            TestRecord::new(2, "TagDatabase", "Database", "database.xml").value(
                "tags",
                TestValue::Array(vec![
                    TestValue::Null,
                    TestValue::Reference(999),
                    TestValue::Reference(1),
                    TestValue::Reference(1),
                ]),
            ),
        )
        .record(
            TestRecord::new(1, "Tag", "One", "one.xml")
                .value("tagName", TestValue::text("One"))
                .value(
                    "children",
                    TestValue::Array(vec![
                        TestValue::Null,
                        TestValue::Reference(1),
                        TestValue::Reference(999),
                    ]),
                ),
        )
        .build()
}

pub(super) fn capture<'source>(
    receipts: &mut TagRelationReceipts,
    input: &OriginalCatalogInput<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> TestResult<Vec<(usize, SourceLocation)>> {
    let kind = if record.index() == 0 {
        TagPathEdgeKind::Root
    } else {
        TagPathEdgeKind::Child
    };
    let Some(Value::Array(items)) = tree.field(kind.field()) else {
        return Err("the genuine original Tag array".into());
    };
    assert_eq!(
        tree.error_count(),
        1,
        "one original dangling GUID word per source"
    );
    assert_eq!(
        items.len(),
        if record.index() == 0 { 4 } else { 3 },
        "all original array positions survive"
    );
    assert!(
        matches!(items.first(), Some(Value::Null)),
        "the actual null prefix is retained"
    );
    let admission = RecordObservation::capture(input, record)?;
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let body = fields.body_capture(input, &admission)?;
    let mut locations = Vec::new();
    for (ordinal, value) in items.iter().enumerate() {
        let part = match value {
            Value::Reference(_) => OriginalValuePart::Value,
            Value::Error(ValueError::DanglingReference { .. }) => {
                OriginalValuePart::Error(OriginalErrorParameter::Guid)
            }
            _ => continue,
        };
        locations.push((ordinal, fields.fact(value)?.location(part)));
    }
    let before = input.origins().uses.freeze()?;
    receipts.capture(kind, tree, &fields, &body)?;
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "discarded relation recipes emit no source uses"
    );
    Ok(locations)
}

pub(super) struct Captured {
    pub(super) bodies: Vec<ExampleBodyReceipt>,
    pub(super) target_positions: Vec<SourceLocation>,
}

pub(super) fn bodies<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
) -> TestResult<Captured> {
    let mut receipts = TagRelationReceipts::default();
    let mut bodies = Vec::new();
    let mut positions = HashMap::new();
    let mut retained_positions = Vec::new();
    let mut sources = 0;
    let tree = TagTree::build_observed(forge, &|| false, &mut |event| {
        match event {
            TagBuildEvent::CanonicalRead { record, outcome } => {
                let kind = if record.index() == 0 {
                    TagPathEdgeKind::Root
                } else {
                    TagPathEdgeKind::Child
                };
                for (ordinal, location) in capture(
                    &mut receipts,
                    input,
                    record,
                    outcome.map_err(DataForgeError::clone)?,
                )? {
                    positions.insert((record.index(), kind, ordinal), location);
                }
                sources += 1;
            }
            TagBuildEvent::RelationSkipped {
                incoming, target, ..
            } => {
                let body = skipped_body(&mut receipts, input, incoming, target)?;
                let position = positions
                    .get(&(
                        incoming.record().index(),
                        incoming.kind(),
                        incoming.ordinal(),
                    ))
                    .ok_or("the independently recorded original target position")?;
                retained_positions.push(position.clone());
                bodies.push(body);
            }
            _ => {}
        }
        Ok::<_, Box<dyn Error>>(())
    })?
    .ok_or("the actual tree must not be cancelled")?;
    assert_eq!(
        sources, 2,
        "both original sources were checked before body assertions"
    );
    assert_eq!(tree.len(), 1, "the same physical Tag remains selected");
    assert_eq!(
        bodies.len(),
        4,
        "all four actual skipped original relations have bodies"
    );
    Ok(Captured {
        bodies,
        target_positions: retained_positions,
    })
}

fn skipped_body<'source>(
    receipts: &mut TagRelationReceipts,
    input: &OriginalCatalogInput<'_, 'source>,
    incoming: TagPathIncoming<'_, 'source>,
    target: Option<Guid>,
) -> TestResult<ExampleBodyReceipt> {
    let admission = RecordObservation::capture(input, incoming.record())?;
    let before = input.origins().uses.freeze()?;
    let body = receipts.take(input, admission.subject(), incoming, target)?;
    assert_eq!(
        body.description()?.0.as_deref(),
        Some(incoming.kind().field()),
        "the field belongs to the actual skipped array"
    );
    assert_eq!(
        body.description()?.2,
        target,
        "the target is the original actual occurrence's GUID"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "unretained skipped-body selection emits no payload"
    );
    Ok(body)
}

pub(super) fn with_reports(
    check: impl for<'input> FnOnce(
        &'input DataForge<'input>,
        &'input OriginalCatalogInput<'input, 'input>,
        &'input OriginalCatalogInput<'input, 'input>,
    ) -> TestResult,
) -> TestResult {
    crate::build::original_input::test_support::with_same_forge_reports(&bytes(), check)
}

pub(super) fn malformed_bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::Int32),
        )
        .record(
            TestRecord::new(1, "Tag", "Original.Malformed", "malformed.xml")
                .value("tagName", TestValue::text("Malformed"))
                .value(
                    "children",
                    TestValue::Array(vec![TestValue::Int32(7), TestValue::Int32(8)]),
                ),
        )
        .build()
}
