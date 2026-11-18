// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, RecordText, RecordTree, SourceValueState, SourcedValue, Step,
    StructValue, Value, ValueError,
};
use p4kx_testkit::watchdog;
use serde_json::Value as Json;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_queries::crafting::{
    BlueprintPoolDefinition, BlueprintPoolEntry, BlueprintPoolSnapshot, observe_pool_query,
};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ResourceFloat32,
};
use crate::catalog_read::original::OriginalValuePart;
use crate::definition_reference::DefinitionStatus;
use crate::record_fields;
use crate::record_header_field::RecordHeaderField;
use crate::resource_number::ResourceNumber;

use super::selection::original_pool_record;
use super::verify::{self, Graph, Proof};
use super::{TestResult, binding, open_archive, scope, wire};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_blueprint_pool_sdk_matches_one_original_record() -> TestResult {
    watchdog::start();
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let session = CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)
        .map_err(|_| "installed pool SDK session open failed")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "installed pool DataForge open failed")?;
    binding(&archive, &session, &parsed)?;
    let original = original_pool_record(&parsed.forge)?
        .tree()
        .map_err(|_| "selected original pool body is unreadable")?;
    let index = original.record_index();
    let proof = Proof::read(session.dataforge_origin())?;
    let (checked, reads) = observe_pool_query(
        &parsed,
        CatalogRecordSelection::RecordIndex(index),
        |snapshot| check_snapshot(&archive, &parsed.forge, &original, &proof, &snapshot),
    )?;
    checked?;
    if reads != [(index, true)] {
        return Err(
            "pool query observed Record bodies outside its single physical selection".into(),
        );
    }
    watch
        .check()
        .map_err(|_| "installed Archive changed during the Blueprint-pool witness")?;
    Ok(())
}

fn check_snapshot(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &RecordTree<'_>,
    proof: &Proof,
    snapshot: &BlueprintPoolSnapshot<'_>,
) -> TestResult {
    if snapshot.build() != archive.build()
        || !matches!(snapshot.availability(), DatasetAvailability::Available)
    {
        return Err("pool result Build or availability differs from the selected original".into());
    }
    verify::coverage(snapshot.coverage(), 0)?;
    let ([definition], [weights]) = (snapshot.rows(), snapshot.weights()) else {
        return Err("physical pool selection must return one definition and one weight row".into());
    };
    let index = original.record_index();
    let inventory = snapshot.source_inventory();
    let leaf = proof.mapped_leaf(inventory)?;
    let graph = Graph::read(inventory)?;
    graph.physical_closure(archive, proof)?;
    scope::validate_record_scope(archive, forge, inventory, leaf.id(), &[index])?;

    let pool = weights.pool();
    if pool.input_ref() != leaf.id()
        || pool.record_index().value().copied() != Some(index)
        || pool.guid().value().map(|guid| guid.0) != Some(original.guid())
    {
        return Err("weight projection identity differs from the original pool owner".into());
    }

    let definition_wire = wire(definition)?;
    let weights_wire = wire(weights)?;
    check_header(original, definition.source(), &definition_wire, &graph)?;
    let pool_wire = weights_wire
        .get("pool")
        .ok_or("weight projection owner missing")?;
    graph.header(pool_wire, "guid", index, RecordHeaderField::Guid)?;
    graph.header(
        pool_wire,
        "recordIndex",
        index,
        RecordHeaderField::RecordIndex,
    )?;
    let expected_total = check_slots(original, definition)?;
    if weights
        .total_weight()
        .value()
        .map(ResourceFloat32::raw_bits)
        != Some(expected_total.to_bits())
    {
        return Err("pool total differs from its original positive finite f32 weight sum".into());
    }
    check_reward_coordinates(original, &definition_wire, &graph)?;
    scope::check_targets(forge, &definition_wire)?;
    graph.walk(&definition_wire)?;
    graph.walk(&weights_wire)?;
    for source in inventory.iter_sources() {
        graph.walk(&wire(source.kind())?)?;
    }
    Ok(())
}

fn check_header(
    original: &RecordTree<'_>,
    source: &CanonicalRecordHeader<'_>,
    row: &Json,
    graph: &Graph<'_>,
) -> TestResult {
    let metadata = original.metadata();
    if source.record_index().value().copied() != Some(original.record_index())
        || source.guid().value().copied() != Some(original.guid())
        || source.source_type().value().copied() != Some(original.type_name())
        || source.record_path().value().copied() != Some(original.record_path())
        || source.file_name().value().copied() != Some(metadata.file_name())
        || source.class_name().value().copied() != Some(record_fields::class_name(original))
        || !record_text_matches(source.record_name(), metadata.name())
        || !record_text_matches(source.metadata_tag(), metadata.metadata_tag())
    {
        return Err("pool header differs from the original physical Record metadata".into());
    }
    for (name, field) in [
        ("guid", RecordHeaderField::Guid),
        ("recordName", RecordHeaderField::RecordName),
        ("className", RecordHeaderField::RecordName),
        ("recordPath", RecordHeaderField::FileName),
        ("recordIndex", RecordHeaderField::RecordIndex),
        ("sourceType", RecordHeaderField::SourceType),
        ("fileName", RecordHeaderField::FileName),
        ("metadataTag", RecordHeaderField::MetadataTag),
    ] {
        graph.header(row, name, original.record_index(), field)?;
    }
    Ok(())
}

fn record_text_matches(cell: &SourcedValue<&str>, expected: RecordText<'_>) -> bool {
    expected.fold(
        || cell.state() == SourceValueState::Absent,
        || cell.state() == SourceValueState::Null,
        |text| cell.value().copied() == Some(text),
    )
}

fn check_slots(
    original: &RecordTree<'_>,
    definition: &BlueprintPoolDefinition<'_>,
) -> TestResult<f32> {
    let Some(Value::Array(original_slots)) = original.field("blueprintRewards") else {
        return Err("selected original pool has no readable authored reward array".into());
    };
    let slots = definition
        .blueprint_rewards()
        .value()
        .ok_or("SDK reward array missing")?;
    if original_slots.len() != slots.len() {
        return Err("SDK reward slots do not preserve original positions".into());
    }
    let mut has_weight = false;
    for (original_slot, slot) in original_slots.iter().zip(slots) {
        match (original_slot, slot.value()) {
            (Value::Null, None) if slot.state() == SourceValueState::Null => {}
            (Value::Struct(original_entry), Some(entry)) => {
                check_entry(original_entry, entry)?;
                has_weight |= entry.weight().value().is_some();
            }
            _ => return Err("SDK reward slot differs from its original type or presence".into()),
        }
    }
    if !has_weight {
        return Err("selected original pool contains no numeric weight to witness".into());
    }
    Ok(original_slots
        .iter()
        .filter_map(|slot| match slot {
            Value::Struct(entry) => entry.field("weight"),
            _ => None,
        })
        .filter_map(record_fields::number_value)
        .filter(|weight| *weight > 0.0)
        .sum())
}

fn check_entry(original: &StructValue<'_>, entry: &BlueprintPoolEntry<'_>) -> TestResult {
    if entry.source_type().value().copied() != Some(original.type_name()) {
        return Err("SDK reward entry type differs from its original position".into());
    }
    let weight = entry.weight();
    let valid = match (original.field("weight"), weight.value()) {
        (None, None) => weight.state() == SourceValueState::Absent,
        (Some(Value::Null), None) => weight.state() == SourceValueState::Null,
        (Some(value), Some(number)) => exact_number(value, number),
        _ => false,
    };
    if !valid {
        return Err("SDK reward weight differs from original numeric storage".into());
    }
    check_reference(original, entry)
}

fn exact_number(value: &Value<'_>, number: &ResourceNumber) -> bool {
    match (value, number) {
        (Value::Int8(expected), ResourceNumber::Int8 { value }) => value == expected,
        (Value::Int16(expected), ResourceNumber::Int16 { value }) => value == expected,
        (Value::Int32(expected), ResourceNumber::Int32 { value }) => value == expected,
        (Value::Int64(expected), ResourceNumber::Int64 { value }) => value.get() == *expected,
        (Value::UInt8(expected), ResourceNumber::UInt8 { value }) => value == expected,
        (Value::UInt16(expected), ResourceNumber::UInt16 { value }) => value == expected,
        (Value::UInt32(expected), ResourceNumber::UInt32 { value }) => value == expected,
        (Value::UInt64(expected), ResourceNumber::UInt64 { value }) => value.get() == *expected,
        (Value::Single(expected), ResourceNumber::Single(value)) => {
            value.raw_bits() == expected.to_bits()
        }
        (Value::Double(expected), ResourceNumber::Double(value)) => {
            value.raw_bits() == expected.to_bits()
        }
        _ => false,
    }
}

fn check_reference(original: &StructValue<'_>, entry: &BlueprintPoolEntry<'_>) -> TestResult {
    let cell = entry.blueprint_record();
    let matches = match (original.field("blueprintRecord"), cell.value()) {
        (None, None) => cell.state() == SourceValueState::Absent,
        (Some(Value::Null), None) => cell.state() == SourceValueState::Null,
        (Some(Value::Reference(expected)), Some(link)) => {
            link.guid().value().copied() == Some(expected.guid())
                && link.target_record_index().value().copied() == Some(expected.record_index())
                && link.record_path().value().copied() == Some(expected.record_path())
                && link.target_type().value().copied() == Some(expected.type_name())
        }
        (Some(Value::Error(ValueError::DanglingReference { guid })), Some(link)) => {
            link.guid().value().copied() == Some(*guid)
                && link.target_record_index().value().is_none()
                && matches!(link.status(), DefinitionStatus::Dangling)
        }
        (Some(Value::Error(ValueError::UnexpectedReferenceType { guid, .. })), Some(link)) => {
            link.guid().value().copied() == Some(*guid)
                && matches!(link.status(), DefinitionStatus::UnexpectedTargetType)
        }
        _ => false,
    };
    if !matches {
        return Err("SDK Blueprint link differs from its original reward occurrence".into());
    }
    Ok(())
}

fn check_reward_coordinates(
    original: &RecordTree<'_>,
    row: &Json,
    graph: &Graph<'_>,
) -> TestResult {
    let array = row
        .get("blueprintRewards")
        .ok_or("SDK reward array cell missing")?;
    let Some(raw_array @ Value::Array(original_slots)) = original.field("blueprintRewards") else {
        return Err("original reward array missing during coordinate verification".into());
    };
    let mut checked = Ok(());
    original.root().visit_owned_values(|visit| {
        if checked.is_err() {
            return;
        }
        match visit.steps() {
            [Step::Field("blueprintRewards")] if std::ptr::eq(visit.value(), raw_array) => {
                checked = coordinate(original.record_index(), visit.ordinal(), array, graph);
            }
            [
                Step::Field("blueprintRewards"),
                Step::Element(index),
                Step::Field("weight"),
            ] if first_weight(original_slots, *index)
                .is_some_and(|weight| std::ptr::eq(weight, visit.value()))
                && ResourceNumber::read(visit.value()).is_some() =>
            {
                checked = check_weight_coordinate(
                    original.record_index(),
                    visit.ordinal(),
                    *index,
                    array,
                    graph,
                );
            }
            _ => {}
        }
    });
    checked
}

fn first_weight<'record, 'source>(
    slots: &'record [Value<'source>],
    index: u32,
) -> Option<&'record Value<'source>> {
    let Value::Struct(entry) = slots.get(usize::try_from(index).ok()?)? else {
        return None;
    };
    entry.field("weight")
}

fn check_weight_coordinate(
    record: u32,
    ordinal: usize,
    slot: u32,
    array: &Json,
    graph: &Graph<'_>,
) -> TestResult {
    let value = array.get("value").ok_or("SDK reward array value missing")?;
    let cell = value
        .get(usize::try_from(slot)?)
        .and_then(|entry| entry.pointer("/value/weight"))
        .ok_or("SDK reward weight cell missing at its original slot")?;
    coordinate(record, ordinal, cell, graph)
}

fn coordinate(record: u32, ordinal: usize, cell: &Json, graph: &Graph<'_>) -> TestResult {
    let location = OriginalValuePart::Value.location(record, u64::try_from(ordinal)?);
    graph.coordinate(cell, &location)
}
