// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, Filter, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::real_archive_item_durability::{Trace, indexed, number, rows, text};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn hackable_device_keeps_original_target_values_and_state_links() -> Result {
    let path = std::env::var("P4KX_HACKABLE_DEVICE_RECORD")
        .map_err(|_| "set P4KX_HACKABLE_DEVICE_RECORD to an exact hackable Entity Record")?;
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut matches = forge.records_at_path(&path);
    let record = matches.next().ok_or("matching original hackable Record")?;
    if matches.next().is_some() || record.type_name().ok() != Some("EntityClassDefinition") {
        return Err("expected one original hackable Entity Record".into());
    }
    let tree = record.tree().map_err(|_| "read original hackable Entity")?;
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, options.with_full_provenance())
        .map_err(|_| "build selected hackable Device catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let uses = indexed(&catalog, "/provenance/uses", "id")?;
    let origin = entry.origin();
    let DataForgeOrigin::Archive(physical) = origin else {
        return Err("expected an observed Archive DataForge Entry".into());
    };
    let source_uses = SourceUses::with_compact_coordinates();
    let scope = source_uses.observe(SourceInput::Archive(physical), InputScope::Payload)?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(&tree, &uses, &scope, encrypted);
    verify(&catalog, &tree, &trace)?;
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    Ok(())
}

fn verify<'source>(
    catalog: &Value,
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> Result {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Entity Components".into());
    };
    let mut devices = 0;
    let mut links = 0;
    for (slot, value) in components.iter().enumerate() {
        let Original::Struct(component) = value else {
            continue;
        };
        let source_type = component.type_name();
        let table = match source_type {
            "HackableParams" => "/configurations/hackableDevices/rows",
            "SInteractionStateMachineParams" => "/configurations/interactions/rows",
            _ => continue,
        };
        let row = owner(catalog, table, tree.record_index(), u64::try_from(slot)?)?;
        assert_eq!(
            text(row, "/sourceType/value")?,
            source_type,
            "component type"
        );
        trace.check(&row["componentOrdinal"], value, "conversion")?;
        if source_type == "HackableParams" {
            verify_device(row, component, trace)?;
            devices += 1;
        } else {
            links += verify_states(row, component, trace)?;
        }
    }
    if devices == 0 || links == 0 {
        return Err(
            "select an Entity with a hackable device and a resolved hacking outcome".into(),
        );
    }
    Ok(())
}

fn owner<'catalog>(
    catalog: &'catalog Value,
    table: &str,
    record: u32,
    slot: u64,
) -> Result<&'catalog Value> {
    let mut rows = rows(catalog, table)?.iter().filter(|row| {
        row.pointer("/ownerRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(u64::from(record))
            && row
                .pointer("/componentOrdinal/value")
                .and_then(Value::as_u64)
                == Some(slot)
    });
    let row = rows.next().ok_or("missing original physical component")?;
    assert!(
        rows.next().is_none(),
        "one row per original physical component"
    );
    Ok(row)
}

fn verify_device<'source>(
    row: &Value,
    source: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> Result {
    for field in [
        "baseDuration",
        "baseErrorChance",
        "maxPauseDuration",
        "numErrorChecks",
        "resetAfterHackSuccess",
        "updateProgressDeltaTime",
    ] {
        compare_scalar(row, source, field, trace)?;
    }
    for (field, value) in [
        ("audioRtpcOverallProgress", "rtpc"),
        ("audioRtpcPartProgress", "rtpc"),
        ("audioRtpcRemainingDuration", "rtpc"),
        ("audioRtpcTotalDuration", "rtpc"),
        ("audioTriggerError", "audioTrigger"),
        ("audioTriggerPartSuccess", "audioTrigger"),
        ("audioTriggerPause", "audioTrigger"),
        ("audioTriggerProgressLoopStart", "audioTrigger"),
        ("audioTriggerProgressLoopStop", "audioTrigger"),
        ("audioTriggerReset", "audioTrigger"),
        ("audioTriggerResume", "audioTrigger"),
        ("audioTriggerStart", "audioTrigger"),
        ("audioTriggerSuccess", "audioTrigger"),
    ] {
        let Some(Original::Struct(declaration)) = source.field(field) else {
            return Err("original audio class".into());
        };
        let exported = row
            .get(field)
            .and_then(|cell| cell.get("value"))
            .ok_or("catalog audio class value")?;
        assert_eq!(
            text(exported, "/sourceType/value")?,
            declaration.type_name(),
            "original audio declaration type"
        );
        compare_scalar(exported, declaration, value, trace)?;
    }
    Ok(())
}

fn compare_scalar<'source>(
    row: &Value,
    source: &StructValue<'source>,
    field: &str,
    trace: &Trace<'_, 'source>,
) -> Result {
    let raw = source.field(field).ok_or("original device scalar field")?;
    let cell = row.get(field).ok_or("catalog device scalar cell")?;
    let equal = match raw {
        Original::Single(value) => {
            u32::from_str_radix(text(cell, "/value/rawBits")?, 16)? == value.to_bits()
        }
        Original::UInt32(value) => number(cell, "/value")? == u64::from(*value),
        Original::Bool(value) => cell["value"] == *value,
        Original::String(value) => text(cell, "/value")? == *value,
        _ => return Err("unexpected original device scalar".into()),
    };
    assert!(equal, "exact original device scalar {field}");
    assert_eq!(
        text(cell, "/state")?,
        "present",
        "original device scalar state"
    );
    trace.check(cell, raw, "payload")
}

fn verify_states<'source>(
    row: &Value,
    machine: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> Result<usize> {
    let Some(Original::Array(categories)) = machine.field("StateTypes") else {
        return Err("original interaction state categories".into());
    };
    let output = rows(row, "/stateTypes/value")?;
    assert_eq!(
        categories.len(),
        output.len(),
        "original state category count"
    );
    let mut links = 0;
    for (category, output) in categories.iter().zip(output) {
        let Original::Struct(category) = category else {
            continue;
        };
        let Some(Original::Array(modifiers)) = category.field("StateModifiers") else {
            continue;
        };
        let exported = rows(output, "/value/stateModifiers/value")?;
        assert_eq!(
            modifiers.len(),
            exported.len(),
            "original state modifier count"
        );
        for (modifier, output) in modifiers.iter().zip(exported) {
            let Original::Struct(modifier) = modifier else {
                continue;
            };
            if modifier.type_name() == "SHackableStateModifier" {
                links += verify_modifier(&output["value"], modifier, trace)?;
            }
        }
    }
    Ok(links)
}

fn verify_modifier<'source>(
    row: &Value,
    modifier: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> Result<usize> {
    assert_eq!(text(row, "/kind")?, "hacking", "hacking modifier tag");
    let mut links = 0;
    for field in [
        "onHackingErrorState",
        "onHackingResetState",
        "onHackingSuccessState",
    ] {
        let raw = modifier
            .field(field)
            .ok_or("original hacking outcome field")?;
        let cell = &row[field];
        match raw {
            Original::Reference(reference) => {
                let location = reference.location().ok_or("owned hacking state instance")?;
                if text(cell, "/value/target/location/value")? != location.as_str() {
                    return Err("original owned hacking state location".into());
                }
                assert_eq!(
                    number(cell, "/value/targetRecordIndex/value")?,
                    u64::from(reference.record_index()),
                    "original hacking state owner position"
                );
                trace.check(cell, raw, "selector")?;
                links += 1;
            }
            Original::Null => assert_eq!(
                text(cell, "/state")?,
                "null",
                "original empty hacking state"
            ),
            _ => return Err("unexpected original hacking outcome value".into()),
        }
    }
    Ok(links)
}
