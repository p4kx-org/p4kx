// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_alignment_options::AlignmentAxis;
pub(crate) use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

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

    pub(super) fn push(
        &mut self,
        kind: ItemAlignmentKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        options: &AlignmentOptions<'_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        match kind {
            ItemAlignmentKind::Port => {
                self.ports
                    .push(ItemPortContainer::read(owner, source, options, notes)?);
            }
            ItemAlignmentKind::Attachment => {
                self.attachments.push(ItemAttachmentComponent::read(
                    owner, source, options, notes,
                )?);
            }
        }
        Ok(())
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

#[derive(Clone, Copy)]
pub(super) enum ItemAlignmentKind {
    Port,
    Attachment,
}

impl ItemAlignmentKind {
    pub(super) fn of(
        value: &StructValue<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Option<Self> {
        match value.type_name() {
            CONTAINER => Some(Self::Port),
            ATTACHABLE if owns_item_definition(value, input) => Some(Self::Attachment),
            _ => None,
        }
    }
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
