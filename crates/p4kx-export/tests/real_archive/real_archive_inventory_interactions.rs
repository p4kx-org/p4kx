// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, InputScope, SourceEncryption,
    SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, cell, owned, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn interaction_catalog_joins_original_component_and_target_slots() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    verify(&entry, &catalog)?;
    watch.check()?;
    crate::real_archive_verification::print_line(
        "interactions: original Interactable and StateMachine slots and links checked",
    );
    Ok(())
}

fn verify(entry: &DataForgeEntry, catalog: &Value) -> TestResult {
    let forge = entry.open()?;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("original DCB archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let rows = components::component_rows(catalog, "/configurations/interactions/rows")?;
    let mut interactable = false;
    let mut state_machine = false;
    for ((owner, slot), row) in rows {
        let (role, expected, checked) = match row["kind"].as_str() {
            Some("interactable") => (
                compare_interactable as ComponentCheck,
                "SEntityInteractableParams",
                &mut interactable,
            ),
            Some("stateMachine") => (
                compare_machine as ComponentCheck,
                "SInteractionStateMachineParams",
                &mut state_machine,
            ),
            _ => return Err("unknown interaction component kind".into()),
        };
        if *checked {
            continue;
        }
        let record = forge
            .record(owner)
            .ok_or("physical interaction owner Record")?;
        let tree = record.tree()?;
        let Some(Original::Array(originals)) = tree.field("Components") else {
            return Err("original interaction Components".into());
        };
        let Some(original @ Original::Struct(component)) = originals.get(slot) else {
            return Err("original interaction component slot".into());
        };
        if component.type_name() != expected || proof::text(row, "/sourceType/value")? != expected {
            return Err("interaction component concrete type differs".into());
        }
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, original, "conversion")?;
        trace.part(cell(row, "sourceType")?, original, "$type", "selector")?;
        role(component, row)?;
        *checked = true;
        if interactable && state_machine {
            return Ok(());
        }
    }
    Err("configured build lacks a traced Interactable or StateMachine".into())
}

type ComponentCheck = fn(&StructValue<'_>, &Value) -> TestResult;

fn compare_interactable(source: &StructValue<'_>, row: &Value) -> TestResult {
    let carrier = cell(row, "interactable")?;
    match source.field("Interactable") {
        None => require_state(carrier, "absent"),
        Some(Original::Null) => require_state(carrier, "null"),
        Some(Original::Struct(data)) => compare_interactable_data(data, carrier),
        _ => Err("unreadable original Interactable".into()),
    }
}

fn compare_interactable_data(data: &StructValue<'_>, carrier: &Value) -> TestResult {
    require_state(carrier, "present")?;
    let output = carrier.get("value").ok_or("original Interactable value")?;
    if proof::text(output, "/sourceType/value")? != data.type_name() {
        return Err("Interactable declaration type differs".into());
    }
    owned_array(
        data,
        "SharedInteractions",
        output,
        "sharedInteractions",
        compare_shared,
    )?;
    owned_array(
        data,
        "InteractionPoints",
        output,
        "interactionPoints",
        |point, row| {
            owned_array(point, "Interactions", row, "interactions", |link, row| {
                compare_link(link, "Interaction", row, "interaction")
            })
        },
    )
}

fn compare_shared(source: &StructValue<'_>, row: &Value) -> TestResult {
    if let Some(Original::String(name)) = source.field("Name")
        && proof::text(row, "/name/value")? != *name
    {
        return Err("shared interaction source name differs".into());
    }
    owned(source, row, "conditionList", |conditions, row| {
        owned_array(
            conditions,
            "conditionParams",
            row,
            "conditionParams",
            |_, _| Ok(()),
        )?;
        owned_array(
            conditions,
            "conditionToHideParams",
            row,
            "conditionToHideParams",
            |_, _| Ok(()),
        )
    })
}

fn compare_machine(source: &StructValue<'_>, row: &Value) -> TestResult {
    owned_array(source, "StateTypes", row, "stateTypes", |category, row| {
        compare_link(category, "DefaultState", row, "defaultState")?;
        owned_array(category, "States", row, "states", |state, row| {
            owned_array(
                state,
                "Interactions",
                row,
                "interactions",
                |transition, row| {
                    compare_link(transition, "Interaction", row, "interaction")?;
                    compare_link(transition, "NextState", row, "nextState")
                },
            )
        })
    })
}

fn owned_array(
    owner: &StructValue<'_>,
    field: &str,
    output: &Value,
    key: &str,
    compare: impl Fn(&StructValue<'_>, &Value) -> TestResult,
) -> TestResult {
    let output = cell(output, key)?;
    let original = match owner.field(field) {
        None => return require_state(output, "absent"),
        Some(Original::Null) => return require_state(output, "null"),
        Some(Original::Array(original)) => original,
        _ => return Err("unreadable original interaction array".into()),
    };
    require_state(output, "present")?;
    let rows = proof::rows(output, "/value")?;
    if rows.len() != original.len() {
        return Err("interaction array lost physical occurrences".into());
    }
    for (original, row) in original.iter().zip(rows) {
        match original {
            Original::Null => require_state(row, "null")?,
            Original::Struct(node) => {
                require_state(row, "present")?;
                let data = row
                    .get("value")
                    .ok_or("authored interaction array element")?;
                if proof::text(data, "/sourceType/value")? != node.type_name() {
                    return Err("interaction array element source type differs".into());
                }
                compare(node, data)?;
            }
            _ => return Err("unreadable original interaction element".into()),
        }
    }
    Ok(())
}

fn compare_link(owner: &StructValue<'_>, field: &str, row: &Value, key: &str) -> TestResult {
    let carrier = cell(row, key)?;
    let link = match owner.field(field) {
        None => return require_state(carrier, "absent"),
        Some(Original::Null) => return require_state(carrier, "null"),
        Some(Original::Reference(link)) => link,
        _ => return Err("unreadable original interaction link".into()),
    };
    require_state(carrier, "present")?;
    let target = if link.location().is_some() {
        "instance"
    } else {
        "record"
    };
    if proof::text(carrier, "/value/recordGuid/value")? != link.guid().to_string()
        || proof::number(carrier, "/value/targetRecordIndex/value")?
            != u64::from(link.record_index())
        || proof::text(carrier, "/value/target/type")? != target
    {
        return Err("authored interaction link target differs".into());
    }
    Ok(())
}
