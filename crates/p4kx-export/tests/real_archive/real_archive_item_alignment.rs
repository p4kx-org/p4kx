// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, SourceEncryption, StructValue,
    TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const AXIS_ENUM: &str = "EItemPortAttachRotationLimitAxis";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn item_port_alignment_matches_original_physical_declarations() -> TestResult {
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
        build_catalog_from(&archive, &entry, &options).map_err(|_| "build alignment catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "decode catalog")?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let encryption = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let ports = rows(&catalog, "/configurations/itemPorts/rows")?;
    let items = rows(&catalog, "/configurations/itemAttachmentConstraints/rows")?;
    let mut trees = TreeBuilder::new();
    let port_count = check_ports(&forge, &mut trees, ports, encryption)?;
    check_items(&forge, &mut trees, items, encryption)?;
    watch
        .check()
        .map_err(|_| "Archive changed during alignment trace")?;
    if port_count == 0 || items.is_empty() {
        return Err("Archive must contain both alignment definition families".into());
    }
    crate::real_archive_verification::print_line(&format!(
        "item-port alignment: {port_count} physical ports and {} item definitions verified",
        items.len()
    ));
    Ok(())
}

fn check_ports<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    ports: &[Value],
    encryption: Option<bool>,
) -> TestResult<usize> {
    let mut port_count = 0;
    for row in ports {
        let tree = original_tree(forge, trees, row)?;
        let source = component(&tree, row, "SItemPortContainerComponentParams")?;
        let Some(Original::Array(original)) = source.field("Ports") else {
            return Err("original port array".into());
        };
        let projected = rows(row, "/ports/value")?;
        if original.len() != projected.len() {
            return Err("port cardinality differs from its physical array".into());
        }
        for (ordinal, (expected, slot)) in original.iter().zip(projected).enumerate() {
            let Original::Struct(source) = expected else {
                return Err("original physical port shape".into());
            };
            if source.type_name() != "SItemPortDef"
                || position(slot, "/value/portOrdinal/value")? != ordinal
            {
                return Err("physical port identity differs".into());
            }
            check_axis(
                forge,
                source,
                slot,
                "/value",
                "attachAxesToAlign",
                encryption,
            )?;
            port_count += 1;
        }
    }
    Ok(port_count)
}

fn check_items<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    items: &[Value],
    encryption: Option<bool>,
) -> TestResult {
    for row in items {
        let tree = original_tree(forge, trees, row)?;
        let component = component(&tree, row, "SAttachableComponentParams")?;
        let Some(Original::Struct(source)) = component.field("AttachDef") else {
            return Err("original attachable definition".into());
        };
        if source.type_name() != "SItemDefinition" {
            return Err("original attached item type".into());
        }
        for field in ["attachAxesToAlign", "ignoredAttachAxis"] {
            check_axis(forge, source, row, "/definition/value", field, encryption)?;
        }
        let cell = at(row, "/definition/value/attachCheckCenterOfMass")?;
        match source.field("attachCheckCenterOfMass") {
            Some(Original::Bool(expected)) if boolean(cell, "/value")? == *expected => {
                check_present(cell, encryption)?;
            }
            None if at(cell, "/state")? == "absent" => {}
            _ => return Err("stored center-of-mass declaration differs".into()),
        }
    }
    Ok(())
}

fn original_tree<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    row: &Value,
) -> TestResult<RecordTree<'source>> {
    let index = u32::try_from(position(row, "/ownerRecordIndex/value")?)?;
    let record = forge.record(index).ok_or("physical owner Record")?;
    trees
        .build(&record)
        .map_err(|_| "read physical owner Record".into())
}

fn component<'read, 'source>(
    tree: &'read RecordTree<'source>,
    row: &Value,
    expected_type: &str,
) -> TestResult<&'read StructValue<'source>> {
    let index = position(row, "/componentOrdinal/value")?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("owner Components array".into());
    };
    match components.get(index) {
        Some(Original::Struct(component)) if component.type_name() == expected_type => {
            Ok(component)
        }
        _ => Err("original physical component differs".into()),
    }
}

fn check_axis(
    forge: &DataForge<'_>,
    source: &StructValue<'_>,
    row: &Value,
    parent: &str,
    field: &str,
    encryption: Option<bool>,
) -> TestResult {
    let axis = at(row, &format!("{parent}/{field}"))?;
    let label = at(axis, "/label")?;
    let status = at(axis, "/optionStatus")?;
    let Some(original) = source
        .fields()
        .iter()
        .find(|original| original.name() == field)
    else {
        if at(label, "/state")? == "absent" && status.is_null() {
            return Ok(());
        }
        return Err("absent axis declaration was materialized".into());
    };
    let Original::Enum(value) = original.value() else {
        return Err("original scalar Enum is unreadable".into());
    };
    if at(label, "/value")?.as_str() != Some(*value) {
        return Err("original axis label differs".into());
    }
    check_present(label, encryption)?;
    let declaration = forge
        .enums()
        .nth(usize::from(original.storage().target()))
        .ok_or("original axis declaration")?;
    if declaration
        .name()
        .map_err(|_| "axis enum declaration name")?
        != AXIS_ENUM
    {
        return Err("original axis enum has a different declaration".into());
    }
    let mut found = false;
    for option in declaration.options() {
        found |= option.map_err(|_| "read original axis option")? == *value;
    }
    let expected = if found { "declared" } else { "undeclared" };
    if at(status, "/value")?.as_str() != Some(expected) {
        return Err("axis option membership differs".into());
    }
    check_present(status, encryption)
}

fn check_present(cell: &Value, encryption: Option<bool>) -> TestResult {
    if at(cell, "/state")? != "present"
        || at(cell, "/sourceSetComplete")? != true
        || at(cell, "/containsEncryptedSource")?.as_bool() != encryption
        || rows(cell, "/sourceUsesRefs")?.is_empty()
    {
        return Err("alignment source marker or reference set differs".into());
    }
    Ok(())
}

fn at<'a>(value: &'a Value, path: &str) -> TestResult<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| "required alignment catalog field".into())
}

fn rows<'a>(value: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    at(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "alignment array".into())
}

fn position(value: &Value, path: &str) -> TestResult<usize> {
    let number = at(value, path)?
        .as_u64()
        .ok_or("alignment physical ordinal")?;
    Ok(usize::try_from(number)?)
}

fn boolean(value: &Value, path: &str) -> TestResult<bool> {
    at(value, path)?
        .as_bool()
        .ok_or_else(|| "alignment Bool".into())
}
