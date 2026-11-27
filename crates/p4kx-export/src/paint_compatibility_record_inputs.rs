// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, RecordTree, SourceUse, SourceUseRole, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::record_fields;
use crate::{CatalogError, ship};

const PORTS: &str = "SItemPortContainerComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";

pub(super) struct RecordInputs {
    pub(super) origin: FieldOrigin,
    pub(super) has_vehicle: bool,
}

pub(super) fn observe<'source>(
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<RecordInputs, CatalogError> {
    let mut evidence = Evidence::default();
    let ports = component(tree, fields, PORTS, &mut evidence)?;
    let attachment = component(tree, fields, ATTACHABLE, &mut evidence)?;
    let vehicle = component(tree, fields, ship::VEHICLE_COMPONENT, &mut evidence)?;
    if let Some(ports) = ports {
        inspect_ports(fields, ports, &mut evidence)?;
    }
    if let Some(owner) = attachment
        && let Some(definition) = fact(fields, owner, "AttachDef", Shape::Struct, &mut evidence)?
    {
        fact(fields, definition, "Tags", Shape::Label, &mut evidence)?;
    }
    let mut has_vehicle = false;
    if let Some(owner) = vehicle {
        let definition = fact(
            fields,
            owner,
            "vehicleDefinition",
            Shape::Path,
            &mut evidence,
        )?;
        has_vehicle = definition.is_some_and(|value| match value.value() {
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                !text.trim().is_empty()
            }
            _ => false,
        });
        if has_vehicle {
            fact(fields, owner, "modification", Shape::Path, &mut evidence)?;
        }
    }
    Ok(RecordInputs {
        origin: fields.sources().origin(&evidence.uses, evidence.complete),
        has_vehicle,
    })
}

fn component<'fields, 'scope, 'read, 'source>(
    tree: &RecordTree<'source>,
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    name: &str,
    evidence: &mut Evidence,
) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
    let value = record_fields::component_occurrence(tree, name).map(|(value, _)| value);
    let prefix = fields.component_selection(value)?;
    evidence.complete &= prefix.complete;
    evidence.uses.extend(prefix.uses);
    value.map(|value| fields.fact(value)).transpose()
}

fn inspect_ports<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    component: &OriginalValueFact<'scope, 'read, 'source>,
    evidence: &mut Evidence,
) -> Result<(), CatalogError> {
    let Some(ports) = fact(fields, component, "Ports", Shape::Array, evidence)? else {
        return Ok(());
    };
    let Value::Array(entries) = ports.value() else {
        return Ok(());
    };
    for entry in entries {
        let port = fields.fact(entry)?;
        if !evidence.observe(port, Shape::Struct)? {
            continue;
        }
        inspect_port(fields, port, evidence)?;
    }
    Ok(())
}

fn inspect_port<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    port: &OriginalValueFact<'scope, 'read, 'source>,
    evidence: &mut Evidence,
) -> Result<(), CatalogError> {
    let Some(types) = fact(fields, port, "Types", Shape::Array, evidence)? else {
        return Ok(());
    };
    let Value::Array(entries) = types.value() else {
        return Ok(());
    };
    for entry in entries {
        let kind = fields.fact(entry)?;
        if evidence.observe(kind, Shape::Struct)? {
            fact(fields, kind, "Type", Shape::Label, evidence)?;
        }
    }
    if matches!(port.value(), Value::Struct(port) if ship::takes_paints(port)) {
        fact(fields, port, "PortTags", Shape::Label, evidence)?;
        fact(fields, port, "RequiredPortTags", Shape::Label, evidence)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Shape {
    Path,
    Label,
    Array,
    Struct,
}

struct Evidence {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl Default for Evidence {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }
}

impl Evidence {
    fn observe(
        &mut self,
        fact: &OriginalValueFact<'_, '_, '_>,
        shape: Shape,
    ) -> Result<bool, CatalogError> {
        let admissible = matches!(fact.value(), Value::Null)
            || matches!(
                (shape, fact.value()),
                (
                    Shape::Path,
                    Value::String(_) | Value::Locale(_) | Value::Enum(_)
                ) | (Shape::Label, Value::String(_) | Value::Enum(_))
                    | (Shape::Array, Value::Array(_))
                    | (Shape::Struct, Value::Struct(_))
            );
        let role = if admissible {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        self.uses
            .push(fact.observe(OriginalValuePart::Value, role)?);
        self.complete &= admissible;
        Ok(admissible && !matches!(fact.value(), Value::Null))
    }
}

fn fact<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
    name: &str,
    shape: Shape,
    evidence: &mut Evidence,
) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
    let Some((_, value)) = fields.field(Some(owner), name)? else {
        evidence.uses.push(fields.survey(Some(owner))?);
        return Ok(None);
    };
    if !evidence.observe(value, shape)? {
        return Ok(None);
    }
    Ok(Some(value))
}
