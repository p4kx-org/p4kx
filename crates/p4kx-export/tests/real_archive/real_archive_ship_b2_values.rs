// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordText, RecordTree, StructValue, Value as Original};
use serde_json::{Value, json};

use super::TestResult;
use crate::real_archive_item_durability::Trace;

pub(super) fn header(tree: &RecordTree<'_>, ship: &Value, trace: &Trace<'_, '_>) -> TestResult {
    let metadata = tree.metadata();
    for (key, expected, origin_field, role) in [
        ("guid", json!(tree.guid().to_string()), "guid", "payload"),
        (
            "recordPath",
            json!(tree.record_path()),
            "fileName",
            "conversion",
        ),
        (
            "recordIndex",
            json!(tree.record_index()),
            "recordIndex",
            "selector",
        ),
        (
            "sourceType",
            json!(tree.type_name()),
            "sourceType",
            "selector",
        ),
        (
            "fileName",
            json!(metadata.file_name()),
            "fileName",
            "payload",
        ),
    ] {
        cell(&ship[key], key, "present", &expected);
        trace.header(&ship[key], origin_field, role)?;
    }
    record_text(&ship["recordName"], "recordName", metadata.name(), trace)?;
    record_text(
        &ship["metadataTag"],
        "metadataTag",
        metadata.metadata_tag(),
        trace,
    )?;
    let name = tree.name();
    let normalized = name
        .strip_prefix(tree.type_name())
        .and_then(|rest| rest.strip_prefix('.'))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(name);
    cell(
        &ship["className"],
        "className",
        "present",
        &json!(normalized),
    );
    if matches!(metadata.name(), RecordText::Present(_)) {
        trace.header(&ship["className"], "recordName", "conversion")?;
        trace.header(&ship["className"], "sourceType", "selector")?;
    } else {
        trace.header(&ship["className"], "recordName", "selector")?;
    }
    Ok(())
}

fn record_text(
    cell_value: &Value,
    label: &str,
    raw: RecordText<'_>,
    trace: &Trace<'_, '_>,
) -> TestResult {
    let (state, expected, role) = raw.fold(
        || ("absent", Value::Null, "selector"),
        || ("null", Value::Null, "payload"),
        |text| ("present", json!(text), "payload"),
    );
    cell(cell_value, label, state, &expected);
    trace.header(cell_value, label, role)
}

pub(super) fn body<'source>(
    tree: &RecordTree<'source>,
    ship: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let vehicle = first(tree, "Components", "VehicleComponentParams")?
        .ok_or("selected Ship needs a readable VehicleComponentParams")?
        .fields;
    number(trace, vehicle, "crewSize", &ship["crewSize"], "crewSize")?;
    text(
        trace,
        vehicle,
        "vehicleDefinition",
        &ship["vehicleDefinition"],
    )?;
    let attachable = first(tree, "Components", "SAttachableComponentParams")?
        .ok_or("selected Ship needs a readable SAttachableComponentParams")?
        .fields;
    let Some(Original::Struct(attach_def)) = attachable.field("AttachDef") else {
        return Err("selected Ship needs a readable AttachDef".into());
    };
    number(trace, attach_def, "Size", &ship["size"], "size")?;
    insurance(tree, &ship["insurance"], trace)
}

fn insurance<'source>(
    tree: &RecordTree<'source>,
    insurance: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Component {
        position,
        value: raw,
        fields: source,
    } = first(tree, "StaticEntityClassData", "SEntityInsuranceProperties")?
        .ok_or("selected Ship needs an original insurance declaration")?;
    let value = container(insurance, "insurance")?;
    trace.part(insurance, raw, "$type", "selector")?;
    let ordinal = &value["staticElementOrdinal"];
    cell(ordinal, "staticElementOrdinal", "present", &json!(position));
    trace.check(ordinal, raw, "conversion")?;
    let Some(restore @ Original::Bool(expected)) = source.field("restoreOnClaim") else {
        return Err("selected insurance needs an authored restoreOnClaim".into());
    };
    cell(
        &value["restoreOnClaim"],
        "restoreOnClaim",
        "present",
        &json!(expected),
    );
    trace.check(&value["restoreOnClaim"], restore, "payload")?;

    let Some(params @ Original::Struct(params_source)) = source.field("shipInsuranceParams") else {
        return Err("selected insurance needs readable shipInsuranceParams".into());
    };
    let params_cell = &value["shipInsuranceParams"];
    let params_value = container(params_cell, "shipInsuranceParams")?;
    trace.part(params_cell, params, "$type", "selector")?;
    text(
        trace,
        params_source,
        "shipEntityClassName",
        &params_value["shipEntityClassName"],
    )?;
    for (field, key) in [
        ("baseWaitTimeMinutes", "baseWaitTimeMinutes"),
        ("mandatoryWaitTimeMinutes", "mandatoryWaitTimeMinutes"),
        ("baseExpeditingFee", "expediteFee"),
    ] {
        number(trace, params_source, field, &params_value[key], key)?;
    }
    Ok(())
}

pub(super) fn crew_manifest<'source>(
    forge: &DataForge<'source>,
    tree: &RecordTree<'source>,
    ship: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult<u32> {
    let Some(Component {
        position,
        value: raw,
        fields: component,
    }) = first(tree, "Components", "SEntityComponentCrewManifestParams")?
    else {
        return Ok(0);
    };
    let inputs = &ship["playabilityInputs"];
    let ordinal = &inputs["crewManifestComponentOrdinal"];
    cell(
        ordinal,
        "crewManifestComponentOrdinal",
        "present",
        &json!(position),
    );
    trace.check(ordinal, raw, "conversion")?;
    let Some(reference @ Original::Reference(link)) = component.field("crewManifest") else {
        return Ok(0);
    };
    let target = forge
        .record(link.record_index())
        .ok_or("physically referenced CrewManifest header")?;
    if target.guid() != link.guid() {
        return Err("the stored CrewManifest reference and target header disagree".into());
    }
    let link_cell = &inputs["crewManifest"];
    let value = container(link_cell, "crewManifest")?;
    trace.check(link_cell, reference, "selector")?;
    cell(
        &value["guid"],
        "CrewManifest GUID",
        "present",
        &json!(link.guid().to_string()),
    );
    cell(
        &value["targetRecordIndex"],
        "CrewManifest index",
        "present",
        &json!(target.index()),
    );
    let target_type = target
        .type_name()
        .map_err(|_| "original CrewManifest type")?;
    let path = target
        .record_path()
        .map_err(|_| "original CrewManifest Record path")?;
    cell(
        &value["targetType"],
        "CrewManifest type",
        "present",
        &json!(target_type),
    );
    cell(
        &value["recordPath"],
        "CrewManifest path",
        "present",
        &json!(path),
    );
    Ok(1)
}

struct Component<'read, 'source> {
    position: usize,
    value: &'read Original<'source>,
    fields: &'read StructValue<'source>,
}

fn first<'read, 'source>(
    tree: &'read RecordTree<'source>,
    field: &str,
    concrete_type: &str,
) -> TestResult<Option<Component<'read, 'source>>> {
    let Some(Original::Array(values)) = tree.field(field) else {
        return Err(format!("selected Ship needs a readable {field} array").into());
    };
    for (position, value) in values.iter().enumerate() {
        match value {
            Original::Struct(fields) if fields.type_name() == concrete_type => {
                return Ok(Some(Component {
                    position,
                    value,
                    fields,
                }));
            }
            Original::Error(_) => {
                return Err(format!("an earlier {field} slot is unreadable").into());
            }
            _ => {}
        }
    }
    Ok(None)
}

fn text<'source>(
    trace: &Trace<'_, 'source>,
    structure: &StructValue<'source>,
    field: &str,
    output: &Value,
) -> TestResult {
    let Some(raw @ (Original::String(expected) | Original::Enum(expected))) =
        structure.field(field)
    else {
        return Err(format!("selected Ship needs an original String or Enum {field}").into());
    };
    cell(output, field, "present", &json!(expected));
    trace.check(output, raw, "payload")
}

fn number<'source>(
    trace: &Trace<'_, 'source>,
    structure: &StructValue<'source>,
    field: &str,
    output: &Value,
    label: &str,
) -> TestResult {
    let raw = structure
        .field(field)
        .ok_or_else(|| format!("selected Ship needs authored {field}"))?;
    let (kind, key, expected) = match raw {
        Original::Int8(value) => ("int8", "value", json!(value)),
        Original::Int16(value) => ("int16", "value", json!(value)),
        Original::Int32(value) => ("int32", "value", json!(value)),
        Original::Int64(value) => ("int64", "value", json!(value.to_string())),
        Original::UInt8(value) => ("uint8", "value", json!(value)),
        Original::UInt16(value) => ("uint16", "value", json!(value)),
        Original::UInt32(value) => ("uint32", "value", json!(value)),
        Original::UInt64(value) => ("uint64", "value", json!(value.to_string())),
        Original::Single(value) => ("f32", "rawBits", json!(format!("{:08x}", value.to_bits()))),
        Original::Double(value) => ("f64", "rawBits", json!(format!("{:016x}", value.to_bits()))),
        _ => return Err(format!("selected Ship needs readable numeric {field}").into()),
    };
    let value = container(output, label)?;
    assert!(
        value["type"].as_str() == Some(kind) && value.get(key) == Some(&expected),
        "{label} must preserve its exact original storage and value"
    );
    trace.check(output, raw, "payload")
}

fn container<'read>(output: &'read Value, label: &str) -> TestResult<&'read Value> {
    let value = output
        .get("value")
        .ok_or_else(|| format!("missing {label} value"))?;
    assert!(
        output["state"].as_str() == Some("present")
            && output["sourceSetComplete"].as_bool() == Some(true)
            && value.is_object(),
        "{label} must be a complete original object"
    );
    Ok(value)
}

fn cell(output: &Value, label: &str, state: &str, expected: &Value) {
    assert!(
        output["state"].as_str() == Some(state)
            && output.get("value") == Some(expected)
            && output["sourceSetComplete"].as_bool() == Some(true),
        "{label} must preserve its exact original value and source state"
    );
}
