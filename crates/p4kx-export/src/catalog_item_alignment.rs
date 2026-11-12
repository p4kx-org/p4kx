// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, DataType, RecordTree, SourcedValue, StructValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_alignment_options::AlignmentAxis;
pub(crate) use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[path = "item_alignment_index.rs"]
mod index;

pub(crate) use index::{AlignmentIndex, AttachmentDefinitionLink, PortDefinitionLink};

const CONTAINER: &str = "SItemPortContainerComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const PORT: &str = "SItemPortDef";
const DEFINITION: &str = "SItemDefinition";

#[derive(Default)]
pub(crate) struct ItemAlignmentRows<'source> {
    ports: Vec<ItemPortContainer<'source>>,
    attachments: Vec<ItemAttachmentComponent<'source>>,
}

impl<'source> ItemAlignmentRows<'source> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.ports.extend(other.ports);
        self.attachments.extend(other.attachments);
    }

    pub(crate) fn into_tables(mut self, forge: &DataForge<'_>) -> ItemAlignmentTables<'source> {
        self.ports.sort_by_key(|row| row.owner.key());
        self.attachments.sort_by_key(|row| row.owner.key());
        ItemAlignmentTables {
            item_ports: CatalogTable::new(forge, &[CONTAINER], self.ports),
            item_attachment_constraints: CatalogTable::new(forge, &[ATTACHABLE], self.attachments),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemAlignmentTables<'source> {
    item_ports: CatalogTable<ItemPortContainer<'source>>,
    item_attachment_constraints: CatalogTable<ItemAttachmentComponent<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemPortContainer<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    ports: SourcedValue<Vec<SourcedValue<ItemPort<'source>>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemPort<'source> {
    port_ordinal: SourcedValue<u32>,
    name: SourcedValue<&'source str>,
    attach_axes_to_align: AlignmentAxis<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemAttachmentComponent<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    definition: SourcedValue<ItemAttachment<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemAttachment<'source> {
    attach_axes_to_align: AlignmentAxis<'source>,
    ignored_attach_axis: AlignmentAxis<'source>,
    attach_check_center_of_mass: SourcedValue<bool>,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(ItemAlignmentRows<'source>, Vec<FieldError>), CatalogError> {
    let mut rows = ItemAlignmentRows::default();
    let mut notes = Vec::new();
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok((rows, notes));
    };
    let mut components = components
        .iter()
        .filter_map(|component| match component {
            Value::Struct(value) if value.type_name() == CONTAINER => Some((component, CONTAINER)),
            Value::Struct(value)
                if value.type_name() == ATTACHABLE && owns_item_definition(value, input) =>
            {
                Some((component, ATTACHABLE))
            }
            _ => None,
        })
        .peekable();
    if components.peek().is_none() {
        return Ok((rows, notes));
    }
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = input
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());
    for (component, kind) in components {
        originals.check_cancel()?;
        let component = originals.fact(component)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let source = Observed::new(&originals, input, component, &capture, &numbers);
        if kind == CONTAINER {
            rows.ports
                .push(ItemPortContainer::read(owner, source, options, &mut notes)?);
        } else {
            rows.attachments.push(ItemAttachmentComponent::read(
                owner, source, options, &mut notes,
            )?);
        }
    }
    Ok((rows, notes))
}

fn owns_item_definition(source: &StructValue<'_>, input: &OriginalCatalogInput<'_, '_>) -> bool {
    let Some(field) = source
        .fields()
        .iter()
        .find(|field| field.name() == "AttachDef")
    else {
        return false;
    };
    let storage = field.storage();
    if storage.is_array()
        || !matches!(
            storage.data_type(),
            DataType::Class | DataType::StrongPointer
        )
    {
        return false;
    }
    match field.value() {
        Value::Struct(definition) => definition.type_name() == DEFINITION,
        _ => input.struct_name(storage.target()) == Some(DEFINITION),
    }
}

impl<'source> ItemPortContainer<'source> {
    fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        options: &AlignmentOptions<'_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let ports = source.class_array_to("Ports", PORT, notes, |port, notes| {
            Ok(ItemPort {
                port_ordinal: port.owned_element_ordinal()?,
                name: port.borrowed_string("Name", notes)?,
                attach_axes_to_align: port.alignment_axis("attachAxesToAlign", options, notes)?,
            })
        })?;
        Ok(Self { owner, ports })
    }
}

impl<'source> ItemAttachmentComponent<'source> {
    fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        options: &AlignmentOptions<'_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let definition = source.owned("AttachDef", &[DEFINITION], notes, |item, notes| {
            Ok(ItemAttachment {
                attach_axes_to_align: item.alignment_axis("attachAxesToAlign", options, notes)?,
                ignored_attach_axis: item.alignment_axis("ignoredAttachAxis", options, notes)?,
                attach_check_center_of_mass: item.boolean("attachCheckCenterOfMass", notes)?,
            })
        })?;
        Ok(Self { owner, definition })
    }
}
