// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::equipment::ResourceCapacity;
use p4kx_export::catalog_queries::equipment::consumables::Consumable;
use p4kx_export::catalog_queries::equipment::mining_tools::AttachableModifier;
use p4kx_export::catalog_queries::{ResourceFloat32, SourceRecordReference};
use p4kx_extraction::{SourceValueState, SourcedValue, StructValue, Value as Original};

use super::{TestResult, Trace};

pub(super) fn record_link<'source>(
    cell: &SourcedValue<Box<SourceRecordReference<'_>>>,
    raw: Option<&Original<'source>>,
    family: &str,
    trace: &Trace<'_, 'source>,
) -> TestResult<Option<u32>> {
    match raw {
        Some(raw @ Original::Reference(target)) => {
            let link = cell.value().ok_or("the authored reference is present")?;
            assert!(
                target.location().is_none()
                    && target.type_name() == family
                    && link.resolved_index() == Some(target.record_index())
                    && link.target_type().value() == Some(&target.type_name())
                    && link.guid().value() == Some(&target.guid()),
                "the original link resolves the exact physical definition"
            );
            trace.check(&serde_json::to_value(cell)?, raw, "selector")?;
            Ok(Some(target.record_index()))
        }
        Some(Original::Null) => {
            assert!(
                cell.state() == SourceValueState::Null && cell.value().is_none(),
                "an explicit null reference has no dependency"
            );
            Ok(None)
        }
        None => {
            assert!(
                cell.state() == SourceValueState::Absent && cell.value().is_none(),
                "an absent reference has no dependency"
            );
            Ok(None)
        }
        _ => Err("unexpected reference storage in the selected witness".into()),
    }
}

pub(super) fn attachable_slots<'source>(
    row: &AttachableModifier<'_>,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Some(raw @ Original::Array(modifiers)) = original.field("modifiers") else {
        return Err("authored attachable modifier slots".into());
    };
    if matches!(original.field("charges"), Some(Original::Int32(_))) {
        integer(row.charges(), original.field("charges"), trace)?;
    }
    let slots = row
        .modifiers()
        .value()
        .ok_or("the SDK retains the attachment array")?;
    assert!(
        slots.len() == modifiers.len(),
        "one SDK slot per original effect occurrence"
    );
    trace.check(&serde_json::to_value(row.modifiers())?, raw, "selector")?;
    for (raw, slot) in modifiers.iter().zip(slots) {
        if matches!(raw, Original::Null) {
            assert!(
                slot.state() == SourceValueState::Null,
                "the null effect retains its position"
            );
            trace.check(&serde_json::to_value(slot)?, raw, "selector")?;
        }
    }
    Ok(())
}

pub(super) fn consumable<'source>(
    row: &Consumable<'_>,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
    subtypes: &mut BTreeSet<u32>,
) -> TestResult {
    verify_capacity(row, original, trace)?;
    let Some(raw @ Original::Array(contents)) = original.field("defaultContents") else {
        return Err("authored consumable contents slots".into());
    };
    let slots = row
        .default_contents()
        .value()
        .ok_or("the SDK retains the original contents")?;
    assert!(
        slots.len() == contents.len(),
        "one SDK slot per original contents occurrence"
    );
    trace.check(
        &serde_json::to_value(row.default_contents())?,
        raw,
        "selector",
    )?;
    for (raw, slot) in contents.iter().zip(slots) {
        let Original::Struct(original) = raw else {
            return Err("an original populated contents slot".into());
        };
        let item = slot
            .value()
            .ok_or("the SDK retains each original contents slot")?;
        trace.part(&serde_json::to_value(slot)?, raw, "$type", "selector")?;
        let Some(raw @ Original::Single(ratio)) = original.field("ratio") else {
            return Err("an original contents ratio".into());
        };
        assert!(
            item.ratio().value().map(ResourceFloat32::raw_bits) == Some(ratio.to_bits()),
            "the contents ratio retains the exact original bits"
        );
        trace.check(&serde_json::to_value(item.ratio())?, raw, "payload")?;
        subtypes.extend(record_link(
            item.consumable_subtype(),
            original.field("consumableSubtype"),
            "ConsumableSubtype",
            trace,
        )?);
    }
    Ok(())
}

fn verify_capacity<'source>(
    row: &Consumable<'_>,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Some(raw @ Original::Struct(volume)) = original.field("consumableVolume") else {
        return Err("original concrete consumable volume".into());
    };
    let capacity = row
        .consumable_volume()
        .value()
        .ok_or("the authored capacity is present")?;
    trace.part(
        &serde_json::to_value(row.consumable_volume())?,
        raw,
        "$type",
        "selector",
    )?;
    match (volume.type_name(), capacity) {
        ("SMicroCargoUnit", ResourceCapacity::MicroCargoUnit { micro_scu, .. }) => {
            integer(micro_scu, volume.field("microSCU"), trace)
        }
        ("SCentiCargoUnit", ResourceCapacity::CentiCargoUnit { centi_scu, .. }) => {
            integer(centi_scu, volume.field("centiSCU"), trace)
        }
        (
            "SStandardCargoUnit",
            ResourceCapacity::StandardCargoUnit {
                standard_cargo_units,
                ..
            },
        ) => {
            let Some(raw @ Original::Single(amount)) = volume.field("standardCargoUnits") else {
                return Err("an original standard cargo amount".into());
            };
            assert!(
                standard_cargo_units.value().map(ResourceFloat32::raw_bits)
                    == Some(amount.to_bits()),
                "the original standard cargo amount retains its authored bits"
            );
            trace.check(&serde_json::to_value(standard_cargo_units)?, raw, "payload")
        }
        _ => Err("the capacity must retain the concrete source unit".into()),
    }
}

fn integer<'source>(
    cell: &SourcedValue<i32>,
    raw: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Some(raw @ Original::Int32(amount)) = raw else {
        return Err("an original signed cargo amount".into());
    };
    assert!(
        cell.value() == Some(amount),
        "the integer amount stays in its source unit"
    );
    trace.check(&serde_json::to_value(cell)?, raw, "payload")
}
