// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, SourceLocation, SourcedValue, Value};

use crate::catalog_read::original::{OriginalValuePart, OwnedValueEvent, OwnedValueWalk};
use crate::catalog_v3::catalog_item_alignment::PhysicalPort;
use crate::contract_loadout::Loadout;

use super::TestResult;

pub(super) struct OriginalPort<'row> {
    pub(super) name: &'row SourcedValue<String>,
    pub(super) location: SourceLocation,
}

pub(super) fn read<'row>(
    record: &Record<'_>,
    row: &'row Loadout,
) -> TestResult<OriginalPort<'row>> {
    let tree = record
        .tree()
        .map_err(|_| "read the nominated original Loadout body")?;
    for port in row.ports() {
        if !matches!(port.source(), "unset" | "port_default") {
            continue;
        }
        let Some(position) = port.source_constraints().physical_port()? else {
            continue;
        };
        if position.owner_record_index != record.index() {
            return Err("root port definition points to another physical Record".into());
        }
        let raw = name_at(&tree, position)
            .ok_or("the selected physical port is absent from the original DCB")?;
        let Value::String(name) = raw else {
            return Err("the selected original port Name is not a readable String".into());
        };
        if name.is_empty() {
            continue;
        }
        if port.name().value().map(String::as_str) != Some(*name) {
            return Err("SDK Loadout port name differs from the original DCB Name".into());
        }
        return Ok(OriginalPort {
            name: port.name(),
            location: payload_location(record, &tree, raw)?,
        });
    }
    Err("nominate one owner with a root port defined by a readable original DCB Name".into())
}

fn name_at<'read, 'source>(
    tree: &'read RecordTree<'source>,
    position: PhysicalPort,
) -> Option<&'read Value<'source>> {
    let Some(Value::Array(components)) = tree.field("Components") else {
        return None;
    };
    let Value::Struct(container) =
        components.get(usize::try_from(position.component_ordinal).ok()?)?
    else {
        return None;
    };
    if container.type_name() != "SItemPortContainerComponentParams" {
        return None;
    }
    let Some(Value::Array(ports)) = container.field("Ports") else {
        return None;
    };
    let Value::Struct(port) = ports.get(usize::try_from(position.port_ordinal).ok()?)? else {
        return None;
    };
    (port.type_name() == "SItemPortDef")
        .then(|| port.field("Name"))
        .flatten()
}

fn payload_location(
    record: &Record<'_>,
    tree: &RecordTree<'_>,
    raw: &Value<'_>,
) -> TestResult<SourceLocation> {
    let mut walk = OwnedValueWalk::new(record, tree, None)?;
    while let Some(event) = walk.next_event()? {
        if let OwnedValueEvent::Value(site) = event
            && std::ptr::eq(site.value(), raw)
        {
            return Ok(OriginalValuePart::Value.location(record.index(), site.ordinal()));
        }
    }
    Err("the original DCB Name was not visited in its own physical body".into())
}
