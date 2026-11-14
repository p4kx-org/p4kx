// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, RecordTree, StructValue, Value as Original};
use serde_json::Value;

use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};

#[path = "item_alignment_sample.rs"]
mod sample;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const AXIS_ENUM: &str = "EItemPortAttachRotationLimitAxis";
const PORTS: &str = "/configurations/itemPorts/rows";
const ITEMS: &str = "/configurations/itemAttachmentConstraints/rows";
const CONTAINER: &str = "SItemPortContainerComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";

fn check_ports(
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
) -> TestResult<usize> {
    let locations = Locations::read(tree.root());
    let mut port_count = 0;
    for (source, row) in component_rows(tree, catalog, PORTS, CONTAINER)? {
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
                source,
                slot,
                "/value",
                "attachAxesToAlign",
                &AxisProof {
                    forge,
                    origins,
                    locations: &locations,
                    owner: tree.record_index(),
                },
            )?;
            port_count += 1;
        }
    }
    Ok(port_count)
}

fn check_items(
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
) -> TestResult {
    let locations = Locations::read(tree.root());
    let proof = AxisProof {
        forge,
        origins,
        locations: &locations,
        owner: tree.record_index(),
    };
    for (component, row) in component_rows(tree, catalog, ITEMS, ATTACHABLE)? {
        let Some(Original::Struct(source)) = component.field("AttachDef") else {
            return Err("original attachable definition".into());
        };
        if source.type_name() != "SItemDefinition" {
            return Err("original attached item type".into());
        }
        for field in ["attachAxesToAlign", "ignoredAttachAxis"] {
            check_axis(source, row, "/definition/value", field, &proof)?;
        }
        let cell = at(row, "/definition/value/attachCheckCenterOfMass")?;
        match source.field("attachCheckCenterOfMass") {
            Some(raw @ Original::Bool(expected))
                if at(cell, "/value")?.as_bool() == Some(*expected) =>
            {
                origins.occurrence(cell, tree.record_index(), locations.ordinal(raw)?, "$value")?;
            }
            None if at(cell, "/state")? == "absent" => {}
            _ => return Err("stored center-of-mass declaration differs".into()),
        }
    }
    Ok(())
}

fn component_rows<'read, 'source>(
    tree: &'read RecordTree<'source>,
    catalog: &'read Value,
    path: &str,
    kind: &str,
) -> TestResult<Vec<(&'read StructValue<'source>, &'read Value)>> {
    let mut rows = crate::real_archive_item_components::component_rows(catalog, path)?;
    let owner = tree.record_index();
    let guid = tree.guid().to_string();
    let mut matched = Vec::new();
    for (slot, component) in original_components(tree).filter(|(_, slot)| slot.type_name() == kind)
    {
        let row = rows
            .remove(&(owner, slot))
            .ok_or("missing original alignment component")?;
        if at(row, "/ownerGuid/value")?.as_str() != Some(guid.as_str()) {
            return Err("alignment component source identity differs".into());
        }
        matched.push((component, row));
    }
    if rows.keys().any(|(index, _)| *index == owner) {
        return Err("export has an extra alignment component for the original owner".into());
    }
    Ok(matched)
}

fn original_components<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> impl Iterator<Item = (usize, &'read StructValue<'source>)> {
    structures(tree.root(), "Components")
}

fn structures<'read, 'source>(
    source: &'read StructValue<'source>,
    name: &str,
) -> impl Iterator<Item = (usize, &'read StructValue<'source>)> {
    let array: &[_] = match source.field(name) {
        Some(Original::Array(values)) => values,
        _ => &[],
    };
    array
        .iter()
        .enumerate()
        .filter_map(|(slot, raw)| match raw {
            Original::Struct(value) => Some((slot, value)),
            _ => None,
        })
}

struct AxisProof<'read, 'catalog, 'source> {
    forge: &'read DataForge<'source>,
    origins: &'read Origins<'catalog>,
    locations: &'read Locations<'source>,
    owner: u32,
}

fn check_axis<'source>(
    source: &StructValue<'source>,
    row: &Value,
    parent: &str,
    field: &str,
    proof: &AxisProof<'_, '_, 'source>,
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
    let ordinal = proof.locations.ordinal(original.value())?;
    proof
        .origins
        .occurrence(label, proof.owner, ordinal, "$value")?;
    let declaration = proof
        .forge
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
    proof
        .origins
        .occurrence(status, proof.owner, ordinal, "$value")
}

fn at<'a>(value: &'a Value, path: &str) -> TestResult<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("required alignment catalog field {path}").into())
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
