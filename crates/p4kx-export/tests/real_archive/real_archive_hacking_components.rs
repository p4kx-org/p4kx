// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, DataType, Filter,
    InputScope, RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Deserialize;
use serde_json::Value;

use crate::real_archive_item_components::{self as components, Rows};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::{real_archive_shop_inventory_v3, real_archive_verification};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ROWS: &str = "/configurations/hackingChips/rows";
const CHARGES: &str = "HackingChipParams";
const REMOVABLE: &str = "RemovableChipParams";
const MODIFIER: &str = "RemovableChipValue";

#[derive(Default)]
struct Counts {
    charge_slots: usize,
    removable_slots: usize,
    modifier_slots: usize,
    original_scalars: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn hacking_chip_components_match_actual_item_slots_and_modifier_sources() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let filter = Filter::new(components::FILTER);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build selected Item catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let counts = compare(&forge, entry.origin(), &catalog, &filter, &watch)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    if std::env::var_os("P4KX_EXPECT_HACKING_COMPONENTS").is_some() {
        assert!(
            counts.charge_slots > 0 && counts.removable_slots > 0 && counts.modifier_slots > 0,
            "the configured Build must exercise both chip families and an original modifier"
        );
    }
    real_archive_verification::print_line(&format!(
        "hacking chips: {} charge slots, {} removable slots, {} modifier slots, {} original scalar fields",
        counts.charge_slots, counts.removable_slots, counts.modifier_slots, counts.original_scalars,
    ));
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    filter: &Filter,
    watch: &ArchiveWatch,
) -> TestResult<Counts> {
    let mut rows = components::component_rows(catalog, ROWS)?;
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("expected an observed Archive DataForge".into());
    };
    let sources = SourceUses::new();
    let scope = sources
        .observe(SourceInput::Archive(entry), InputScope::Payload)
        .map_err(|_| "observe actual DataForge origin")?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let probes: Vec<_> = [CHARGES, REMOVABLE]
        .into_iter()
        .filter_map(|name| forge.pointer_probe("Components", name))
        .collect();
    let mut trees = TreeBuilder::new();
    let mut counts = Counts::default();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if !components::selected_item(&record, &rule, filter)?
            || !components::has_component(&mut trees, &record, &probes)?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original chip owner")?;
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        compare_slots(forge, &tree, &trace, &mut rows, &mut counts)?;
    }
    assert!(
        rows.is_empty(),
        "every exported chip has its independently selected physical original slot"
    );
    if counts.charge_slots + counts.removable_slots > 0 {
        assert!(
            proof::text(catalog, "/configurations/hackingChips/availability")? == "available",
            "original chip instances prove the source family is available"
        );
    }
    Ok(counts)
}

fn compare_slots<'source>(
    forge: &DataForge<'_>,
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    rows: &mut Rows<'_>,
    counts: &mut Counts,
) -> TestResult {
    let Some(Original::Array(slots)) = tree.field("Components") else {
        return Err("the original chip owner must have a readable Components array".into());
    };
    for (slot, original) in slots.iter().enumerate() {
        let Original::Struct(component) = original else {
            continue;
        };
        let variant = match component.type_name() {
            CHARGES => "hackingChip",
            REMOVABLE => "removableChip",
            _ => continue,
        };
        let row = rows
            .remove(&(tree.record_index(), slot))
            .ok_or("one exported row for each original chip slot")?;
        components::compare_identity(tree, trace, original, component, variant, row)?;
        let config = row
            .pointer("/configuration/value/value")
            .ok_or("typed chip configuration")?;
        if component.type_name() == CHARGES {
            counts.charge_slots += 1;
            counts.original_scalars +=
                compare_scalar(trace, original, component, config, Scalar::Charges)?;
        } else {
            counts.removable_slots += 1;
            compare_modifiers(forge, trace, original, component, config, counts)?;
        }
    }
    Ok(())
}

fn compare_modifiers<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    component: &StructValue<'source>,
    config: &Value,
    counts: &mut Counts,
) -> TestResult {
    let cell = components::cell(config, "values")?;
    let Some(field) = component
        .fields()
        .iter()
        .find(|field| field.name() == "values")
    else {
        return absent(trace, owner, cell);
    };
    let raw = field.value();
    trace.part(cell, raw, "$dataType", "selector")?;
    trace.part(cell, raw, "$isArray", "selector")?;
    if matches!(raw, Original::Error(_)) {
        components::require_state(cell, "unreadable")?;
        return trace.attempted_value(cell, raw);
    }
    let storage = field.storage();
    if storage.data_type() != DataType::Class || !storage.is_array() {
        return retained(trace, raw, cell, "invalid", "selector");
    }
    match raw {
        Original::Array(values) if values.is_empty() => {
            compare_empty(forge, trace, raw, cell, storage.target())
        }
        Original::Array(values) => {
            retained(trace, raw, cell, "present", "selector")?;
            let exported = proof::rows(cell, "/value")?;
            assert!(
                exported.len() == values.len(),
                "all original removable modifier slots survive in order"
            );
            for (raw, row) in values.iter().zip(exported) {
                compare_modifier(trace, raw, row, counts)?;
                counts.modifier_slots += 1;
            }
            Ok(())
        }
        Original::Null => retained(trace, raw, cell, "null", "selector"),
        _ => retained(trace, raw, cell, "invalid", "selector"),
    }
}

fn compare_empty<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    cell: &Value,
    target: u16,
) -> TestResult {
    trace.part(cell, raw, "$target", "selector")?;
    let name = forge
        .structs()
        .nth(usize::from(target))
        .and_then(|structure| structure.name().ok());
    let Some(name) = name else {
        components::require_state(cell, "unreadable")?;
        trace.part(cell, raw, "$value", "selector")?;
        return trace.attempted_struct_name(cell, target);
    };
    trace.struct_name(cell, target)?;
    if name == MODIFIER {
        retained(trace, raw, cell, "present", "selector")?;
        assert!(
            proof::rows(cell, "/value")?.is_empty(),
            "the actual empty Class array stays empty"
        );
        Ok(())
    } else {
        retained(trace, raw, cell, "unsupported", "selector")
    }
}

fn compare_modifier<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    cell: &Value,
    counts: &mut Counts,
) -> TestResult {
    match raw {
        Original::Struct(modifier) if modifier.type_name() == MODIFIER => {
            components::require_state(cell, "present")?;
            trace.part(cell, raw, "$type", "selector")?;
            let body = cell.get("value").ok_or("actual modifier fields")?;
            counts.original_scalars += compare_scalar(trace, raw, modifier, body, Scalar::Name)?;
            counts.original_scalars += compare_scalar(trace, raw, modifier, body, Scalar::Value)?;
            Ok(())
        }
        Original::Struct(_) => {
            components::require_state(cell, "unsupported")?;
            trace.part(cell, raw, "$type", "selector")
        }
        Original::Null => retained(trace, raw, cell, "null", "selector"),
        Original::Error(_) => {
            components::require_state(cell, "unreadable")?;
            trace.attempted_value(cell, raw)
        }
        _ => retained(trace, raw, cell, "invalid", "selector"),
    }
}

#[derive(Clone, Copy)]
enum Scalar {
    Charges,
    Name,
    Value,
}

impl Scalar {
    const fn field(self) -> &'static str {
        match self {
            Self::Charges => "maxCharges",
            Self::Name => "name",
            Self::Value => "value",
        }
    }

    fn equal(self, raw: &Original<'_>, cell: &Value) -> TestResult<Option<bool>> {
        Ok(match (self, raw) {
            (Self::Charges, Original::Int32(charges)) => {
                Some(cell.get("value").and_then(Value::as_i64) == Some(i64::from(*charges)))
            }
            (Self::Name, Original::String(name)) => Some(
                proof::text(cell, "/value/type")? == "gameText"
                    && proof::text(cell, "/value/sourceText")? == *name,
            ),
            (Self::Value, Original::Single(number)) => Some(single_equal(cell, *number)?),
            _ => None,
        })
    }
}

fn compare_scalar<'source>(
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    structure: &StructValue<'source>,
    row: &Value,
    kind: Scalar,
) -> TestResult<usize> {
    let cell = components::cell(row, kind.field())?;
    let Some(raw) = structure.field(kind.field()) else {
        absent(trace, owner, cell)?;
        return Ok(0);
    };
    match raw {
        Original::Null => retained(trace, raw, cell, "null", "selector")?,
        Original::Error(_) => {
            components::require_state(cell, "unreadable")?;
            trace.attempted_value(cell, raw)?;
        }
        _ => match kind.equal(raw, cell)? {
            Some(equal) => {
                assert!(
                    equal,
                    "the exported scalar preserves its actual original representation"
                );
                retained(trace, raw, cell, "present", "payload")?;
                return Ok(1);
            }
            None => retained(trace, raw, cell, "invalid", "selector")?,
        },
    }
    Ok(0)
}

fn single_equal(cell: &Value, original: f32) -> TestResult<bool> {
    let bits = proof::text(cell, "/value/rawBits")?;
    let raw = u32::from_str_radix(bits, 16).map_err(|_| "canonical Single bit pattern")?;
    let finite = cell
        .pointer("/value/finite")
        .ok_or("finite Single projection")?;
    let decimal_matches = if original.is_finite() {
        f32::deserialize(finite)
            .map_err(|_| "read finite Single projection")?
            .to_bits()
            == original.to_bits()
    } else {
        finite.is_null()
    };
    Ok(raw == original.to_bits() && decimal_matches)
}

fn absent<'source>(
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    cell: &Value,
) -> TestResult {
    components::require_state(cell, "absent")?;
    trace.part(cell, owner, "$fields", "selector")
}

fn retained<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    cell: &Value,
    state: &str,
    role: &str,
) -> TestResult {
    components::require_state(cell, state)?;
    trace.check(cell, raw, role)
}
