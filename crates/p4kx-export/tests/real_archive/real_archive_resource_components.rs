// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, RecordTree, StructValue, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, require_state};
use crate::real_archive_item_durability as proof;

#[path = "real_archive_resource_component_sample.rs"]
mod sample;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const TABLE: &str = "/configurations/resourceComponents/rows";
const SCALARS: [&str; 6] = [
    "isResourceNetworked",
    "isRelay",
    "isConnectedToRoom",
    "wirelessConnection",
    "defaultPriority",
    "repairRestoreRatio",
];
const EXPECTED: [&str; 5] = ["plant", "cooler", "shield", "controller", "tank"];

#[derive(Default)]
struct FieldCoverage {
    values: usize,
    missing: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn resource_states_trace_original_fields_across_representative_components() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built =
        build_catalog_from(&archive, &entry, &options).map_err(|_| "build resource catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let (seen, coverage) = trace_representatives(&entry, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during resource verification")?;
    for (field, count) in coverage {
        crate::real_archive_verification::print_line(&format!(
            "resource component {field}: {} stored values, {} missing slots",
            count.values, count.missing
        ));
    }
    let missing: Vec<_> = EXPECTED
        .into_iter()
        .filter(|role| !seen.contains(role))
        .collect();
    if !missing.is_empty() {
        return Err(
            format!("configured build is missing resource component roles {missing:?}").into(),
        );
    }
    Ok(())
}

type Coverage = BTreeMap<&'static str, FieldCoverage>;

fn trace_representatives(
    entry: &DataForgeEntry,
    catalog: &Value,
) -> TestResult<(BTreeSet<&'static str>, Coverage)> {
    let rows = components::component_rows(catalog, TABLE)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut trees = TreeBuilder::new();
    let mut seen = BTreeSet::new();
    let mut coverage = BTreeMap::new();
    for ((owner, slot), row) in rows {
        let original = forge
            .record(owner)
            .ok_or("physical resource owner Record")?;
        let tree = trees
            .build(&original)
            .map_err(|_| "read original resource owner")?;
        let Some(role) = representative(&tree) else {
            continue;
        };
        if !seen.insert(role) {
            continue;
        }
        let source = original_component(&tree, slot)?;
        if source.type_name() != "ItemResourceComponentParams"
            || proof::text(row, "/sourceType/value")? != source.type_name()
            || proof::text(row, "/ownerGuid/value")? != tree.guid().to_string()
        {
            return Err("resource component identity differs".into());
        }
        compare_component(source, row, &mut coverage)?;
        if seen.len() == EXPECTED.len() {
            break;
        }
    }
    Ok((seen, coverage))
}

fn representative(tree: &RecordTree<'_>) -> Option<&'static str> {
    let Original::Array(entries) = tree.field("Components")? else {
        return None;
    };
    for entry in entries {
        let Original::Struct(component) = entry else {
            continue;
        };
        match component.type_name() {
            "SCItemShieldGeneratorParams" => return Some("shield"),
            "SCItemFlightControllerParams" => return Some("controller"),
            "SCItemFuelTankParams" => return Some("tank"),
            "SAttachableComponentParams" => {
                if let Some(Original::Struct(attach)) = component.field("AttachDef")
                    && let Some(Original::Enum(kind) | Original::String(kind)) =
                        attach.field("Type")
                {
                    if kind.eq_ignore_ascii_case("PowerPlant") {
                        return Some("plant");
                    }
                    if kind.eq_ignore_ascii_case("Cooler") {
                        return Some("cooler");
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn original_component<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    slot: usize,
) -> TestResult<&'tree StructValue<'source>> {
    let Some(Original::Array(entries)) = tree.field("Components") else {
        return Err("original Components array".into());
    };
    let Some(Original::Struct(component)) = entries.get(slot) else {
        return Err("original resource component slot".into());
    };
    Ok(component)
}

fn compare_component(
    source: &StructValue<'_>,
    row: &Value,
    coverage: &mut BTreeMap<&'static str, FieldCoverage>,
) -> TestResult {
    for field in SCALARS {
        compare_scalar(source, row, field, coverage)?;
    }
    if let Some(Original::Struct(repair)) = source.field("selfRepair") {
        let row = row
            .pointer("/selfRepair/value")
            .ok_or("original self-repair row")?;
        for field in ["maxRepairCount", "timeToRepair", "healthRatio"] {
            compare_scalar(repair, row, field, coverage)?;
        }
    }
    compare_slots(source, row, "states", |state, row| {
        compare_scalar(state, row, "name", coverage)?;
        compare_slots(state, row, "deltas", |delta, row| {
            compare_delta(delta, row, coverage)
        })
    })
}

fn compare_slots(
    source: &StructValue<'_>,
    row: &Value,
    field: &str,
    mut compare: impl FnMut(&StructValue<'_>, &Value) -> TestResult,
) -> TestResult {
    let cell = components::cell(row, field)?;
    let originals = match source.field(field) {
        None => return require_state(cell, "absent"),
        Some(Original::Null) => return require_state(cell, "null"),
        Some(Original::Array(values)) => values,
        _ => return Err("unreadable original resource array".into()),
    };
    let slots = proof::rows(cell, "/value")?;
    if originals.len() != slots.len() {
        return Err("original resource array cardinality differs".into());
    }
    for (index, (original, slot)) in originals.iter().zip(slots).enumerate() {
        match original {
            Original::Null => require_state(slot, "null")?,
            Original::Struct(value) => {
                if proof::number(slot, "/value/sourcePosition/value")? != u64::try_from(index)?
                    || proof::text(slot, "/value/sourceType/value")? != value.type_name()
                {
                    return Err("original resource slot position or type differs".into());
                }
                compare(value, slot.get("value").ok_or("resource slot value")?)?;
            }
            _ => return Err("unreadable original resource array slot".into()),
        }
    }
    Ok(())
}

fn compare_delta(
    source: &StructValue<'_>,
    row: &Value,
    coverage: &mut BTreeMap<&'static str, FieldCoverage>,
) -> TestResult {
    if row["kind"] == "unknownDelta" {
        return Ok(());
    }
    for field in ["generation", "consumption"] {
        let Some(Original::Struct(side)) = source.field(field) else {
            continue;
        };
        let row = components::cell(row, field)?
            .get("value")
            .ok_or("resource delta side")?;
        compare_scalar(side, row, "resource", coverage)?;
        let Some(Original::Struct(unit)) = side.field("resourceAmountPerSecond") else {
            continue;
        };
        let output = row
            .pointer("/resourceAmountPerSecond/value")
            .ok_or("resource delta unit")?;
        if proof::text(output, "/sourceType/value")? != unit.type_name() {
            return Err("original resource unit differs".into());
        }
        if let Some(field) = named_amount(unit.type_name()) {
            compare_number(unit, output, field, coverage)?;
        }
    }
    Ok(())
}

pub(super) fn known_generation_amount(
    catalog: &Value,
    item: &Value,
    resource: &str,
) -> Option<f64> {
    let guid = item.get("guid")?;
    let component = catalog
        .pointer(TABLE)?
        .as_array()?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value") == Some(guid))?;
    let generation = component
        .pointer("/states/value")?
        .as_array()?
        .iter()
        .filter_map(|state| state.pointer("/value/deltas/value")?.as_array())
        .flatten()
        .filter_map(|delta| delta.pointer("/value/generation/value"))
        .find(|side| {
            side.pointer("/resource/value")
                .and_then(Value::as_str)
                .is_some_and(|label| label.eq_ignore_ascii_case(resource))
        })?;
    let unit = generation.pointer("/resourceAmountPerSecond/value")?;
    let field = named_amount(unit.pointer("/sourceType/value")?.as_str()?)?;
    let number = unit.get(field)?.get("value")?;
    number
        .get("finite")
        .or_else(|| number.get("value"))?
        .as_f64()
}

fn named_amount(source_type: &str) -> Option<&'static str> {
    match source_type {
        "SCentiResourceUnit" => Some("centiResourceUnits"),
        "SMicroResourceUnit" => Some("microResourceUnits"),
        "SStandardResourceUnit" => Some("standardResourceUnits"),
        "SPowerSegmentResourceUnit" => Some("units"),
        _ => None,
    }
}

fn compare_scalar(
    source: &StructValue<'_>,
    row: &Value,
    field: &'static str,
    coverage: &mut BTreeMap<&'static str, FieldCoverage>,
) -> TestResult {
    let cell = components::cell(row, field)?;
    let value = cell.get("value").ok_or("resource scalar payload")?;
    let count = coverage.entry(field).or_default();
    let matches = match source.field(field) {
        None => {
            count.missing += 1;
            return require_state(cell, "absent");
        }
        Some(Original::Null) => {
            count.missing += 1;
            return require_state(cell, "null");
        }
        Some(Original::Bool(expected)) => value.as_bool() == Some(*expected),
        Some(Original::Int32(expected)) => value.as_i64() == Some(i64::from(*expected)),
        Some(Original::Single(expected)) => bits_match(value, *expected),
        Some(Original::String(expected) | Original::Enum(expected)) => {
            value.as_str() == Some(*expected)
        }
        _ => return Err("unreadable original resource scalar".into()),
    };
    if !matches {
        return Err(format!("original resource field {field} differs").into());
    }
    count.values += 1;
    require_state(cell, "present")
}

fn compare_number(
    source: &StructValue<'_>,
    row: &Value,
    field: &'static str,
    coverage: &mut BTreeMap<&'static str, FieldCoverage>,
) -> TestResult {
    let cell = components::cell(row, field)?;
    let value = cell.get("value").ok_or("resource unit amount")?;
    let count = coverage.entry(field).or_default();
    let matches = match source.field(field) {
        None => {
            count.missing += 1;
            return require_state(cell, "absent");
        }
        Some(Original::Int32(expected)) => {
            value["type"] == "int32" && value["value"].as_i64() == Some(i64::from(*expected))
        }
        Some(Original::Single(expected)) => value["type"] == "f32" && bits_match(value, *expected),
        _ => return Err("original resource numeric unit storage".into()),
    };
    if !matches {
        return Err(format!("original resource amount {field} differs").into());
    }
    count.values += 1;
    require_state(cell, "present")
}

fn bits_match(value: &Value, original: f32) -> bool {
    value["rawBits"].as_str() == Some(format!("{:08x}", original.to_bits()).as_str())
}
