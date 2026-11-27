// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Record, RecordTree, Value};

use crate::catalog_queries::crafting::BlueprintRewardSource;
use crate::catalog_read::original::{
    OriginalValueEdge, OriginalValuePart, OwnedValueEvent, OwnedValueWalk,
};
use crate::record_header_field::RecordHeaderField;
use crate::source_value_catalog::SourceDataType;

use super::super::verify::Graph;
use super::super::{TestResult, wire};
use super::owner_index;

#[derive(Default)]
pub(super) struct Sample {
    pub(super) bodies: usize,
    pub(super) slots: usize,
}

pub(super) fn sample(
    forge: &DataForge<'_>,
    rows: &[BlueprintRewardSource<'_>],
    owners: &[u32],
    target: &Record<'_>,
    graph: &Graph<'_>,
) -> TestResult<Sample> {
    let mut sample = Sample::default();
    let (Some(&first), Some(&last)) = (owners.first(), owners.last()) else {
        return Ok(sample);
    };
    for index in std::iter::once(first).chain((last != first).then_some(last)) {
        let owner = forge
            .record(index)
            .ok_or("sampled original owner missing")?;
        let tree = owner
            .tree()
            .map_err(|_| "sampled original owner is unreadable")?;
        let start = rows.partition_point(|row| owner_before(row, index));
        let remaining = rows
            .get(start..)
            .ok_or("sample owner row start unavailable")?;
        let count =
            remaining.partition_point(|row| row.source().record_index().value() == Some(&index));
        let selected = remaining
            .get(..count)
            .ok_or("sample owner rows unavailable")?;
        check_owner(&owner, &tree, target, selected, graph)?;
        sample.bodies += 1;
        sample.slots += selected.len();
    }
    Ok(sample)
}

fn owner_before(row: &BlueprintRewardSource<'_>, index: u32) -> bool {
    row.source()
        .record_index()
        .value()
        .is_some_and(|value| *value < index)
}

fn check_owner(
    owner: &Record<'_>,
    tree: &RecordTree<'_>,
    target: &Record<'_>,
    rows: &[BlueprintRewardSource<'_>],
    graph: &Graph<'_>,
) -> TestResult {
    let mut rows = rows.iter();
    let target_index = target.index();
    let mut walk = OwnedValueWalk::new(owner, tree, None)
        .map_err(|_| "sampled owner could not bind an original value walk")?;
    while let Some(event) = walk
        .next_event()
        .map_err(|_| "sampled owner value walk failed")?
    {
        let OwnedValueEvent::Value(visit) = event else {
            continue;
        };
        if let (OriginalValueEdge::Field { .. }, Value::Reference(reference)) =
            (visit.edge(), visit.value())
            && reference.location().is_none()
            && reference.guid() != tree.guid()
            && reference.guid() == target.guid()
            && reference.record_index() == target_index
        {
            let row = rows
                .next()
                .ok_or("sample owner is missing an original reward slot")?;
            check_slot(
                tree,
                visit.ordinal(),
                visit.field().storage().data_type(),
                row,
                graph,
            )?;
        }
    }
    if rows.next().is_some() {
        return Err("sample row has no original direct or root-Weak source occurrence".into());
    }
    Ok(())
}

fn check_slot(
    tree: &RecordTree<'_>,
    ordinal: u64,
    storage: DataType,
    row: &BlueprintRewardSource<'_>,
    graph: &Graph<'_>,
) -> TestResult {
    let index = owner_index(row)?;
    if index != tree.record_index()
        || row.source().guid().value().copied() != Some(tree.guid())
        || row.source().record_path().value().copied() != Some(tree.record_path())
        || row.source().source_type().value().copied() != Some(tree.type_name())
        || row.source_ordinal().value().map(|value| value.get()) != Some(ordinal)
    {
        return Err("sample row does not bind the actual original owner and slot".into());
    }
    if !matches!(
        (storage, row.pool_storage().value()),
        (DataType::Reference, Some(SourceDataType::Reference))
            | (DataType::WeakPointer, Some(SourceDataType::WeakPointer))
    ) {
        return Err("sample row does not preserve the original reference storage type".into());
    }
    check_evidence(row, graph, index, ordinal)
}

fn check_evidence(
    row: &BlueprintRewardSource<'_>,
    graph: &Graph<'_>,
    index: u32,
    ordinal: u64,
) -> TestResult {
    let wire = wire(row)?;
    graph.header(&wire, "guid", index, RecordHeaderField::Guid)?;
    graph.header(&wire, "recordIndex", index, RecordHeaderField::RecordIndex)?;
    let location = OriginalValuePart::Value.location(index, ordinal);
    for field in ["sourceOrdinal", "pool"] {
        let cell = wire.get(field).ok_or("sample row origin cell missing")?;
        graph.coordinate(cell, &location)?;
        graph.walk(cell)?;
    }
    graph.walk(
        wire.get("poolStorage")
            .ok_or("sample row storage cell missing")?,
    )?;
    Ok(())
}
