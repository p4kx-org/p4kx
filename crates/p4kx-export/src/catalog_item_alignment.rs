// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, SourceUse, SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::{AuthoredRestriction, Observed};
use super::item_alignment_options::AlignmentAxis;
pub(crate) use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use super::port_restrictions::PortRestrictions;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, TextOwner};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::PublicNameRef;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

#[path = "item_alignment_index.rs"]
mod index;
#[path = "item_port_staging.rs"]
mod staging;

pub(crate) use index::{
    AlignmentIndex, AttachmentDefinitionLink, PhysicalPort, PortDefinitionLink,
};
pub(crate) use staging::ItemPortStaging;

use self::staging::{PortContainerFacts, StagedItemPorts};

const CONTAINER: &str = "SItemPortContainerComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const PORT: &str = "SItemPortDef";
const DEFINITION: &str = "SItemDefinition";

pub(super) struct TopologyPortDefinition<'rows> {
    pub(super) name: &'rows str,
    pub(super) position: PhysicalPort,
    pub(super) name_use: Option<&'rows SourceUse>,
}

#[derive(Default)]
pub(crate) struct ItemAlignmentRows<'source> {
    ports: Vec<ItemPortContainer<'source>>,
    prepared_ports: Option<StagedItemPorts<'source>>,
    attachments: Vec<ItemAttachmentComponent<'source>>,
}

impl<'source> ItemAlignmentRows<'source> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.ports.extend(other.ports);
        self.attachments.extend(other.attachments);
    }

    pub(crate) fn stage_ports(
        &mut self,
        staging: &mut ItemPortStaging<'_, 'source>,
    ) -> Result<(), CatalogError> {
        for row in std::mem::take(&mut self.ports) {
            staging.append(row)?;
        }
        Ok(())
    }

    pub(crate) fn install_staged_ports(
        &mut self,
        staging: ItemPortStaging<'_, 'source>,
        forge: &DataForge<'_>,
    ) -> Result<(), CatalogError> {
        if !self.ports.is_empty() || self.prepared_ports.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.prepared_ports = Some(staging.finish(forge)?);
        Ok(())
    }

    fn port_facts(&self) -> &[PortContainerFacts<'source>] {
        self.prepared_ports
            .as_ref()
            .map_or(&[], StagedItemPorts::facts)
    }

    pub(super) fn push(
        &mut self,
        kind: ItemAlignmentKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        options: &AlignmentOptions<'_>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        match kind {
            ItemAlignmentKind::Port => {
                self.ports.push(ItemPortContainer::read(
                    owner, source, options, projection, notes,
                )?);
            }
            ItemAlignmentKind::Attachment => {
                self.attachments.push(ItemAttachmentComponent::read(
                    owner, source, options, notes,
                )?);
            }
        }
        Ok(())
    }

    pub(super) fn topology_ports(
        &self,
    ) -> impl Iterator<Item = Result<TopologyPortDefinition<'_>, CatalogError>> + '_ {
        self.port_facts().iter().flat_map(|row| {
            row.ports.iter().filter_map(move |port| {
                let name = port.name?;
                Some(
                    PhysicalPort::read(&row.owner, &port.port_ordinal).map(|position| {
                        TopologyPortDefinition {
                            name,
                            position,
                            name_use: port.name_use.as_ref(),
                        }
                    }),
                )
            })
        })
    }

    pub(super) fn attachment_type_uses(&self) -> impl Iterator<Item = ((u32, u32), &SourceUse)> {
        self.attachments.iter().filter_map(|row| {
            let (record, component) = row.owner.key();
            let original = row.definition.value()?.type_use.as_ref()?;
            Some(((record?, component?), original))
        })
    }

    pub(super) fn port_inventory_uses(&self) -> impl Iterator<Item = (u32, &SourceUse)> {
        self.port_facts()
            .iter()
            .filter_map(|row| Some((row.owner.key().0?, row.ports_use.as_ref()?)))
    }

    pub(super) fn incomplete_port_records(&self) -> impl Iterator<Item = u32> + '_ {
        self.port_facts()
            .iter()
            .filter(|row| row.incomplete_names)
            .filter_map(|row| row.owner.key().0)
    }

    pub(crate) fn into_tables(
        mut self,
        forge: &DataForge<'_>,
    ) -> Result<ItemAlignmentTables<'source>, CatalogError> {
        if !self.ports.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.attachments.sort_by_key(|row| row.owner.key());
        Ok(ItemAlignmentTables {
            item_ports: self.prepared_ports.map_or_else(
                || StagedCatalogTable::empty(staging::empty_availability(forge)),
                StagedItemPorts::into_table,
            ),
            item_attachment_constraints: CatalogTable::new(forge, &[ATTACHABLE], self.attachments),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemAlignmentTables<'source> {
    #[schemars(with = "CatalogTable<ItemPortContainer<'source>>")]
    pub(crate) item_ports: StagedCatalogTable,
    item_attachment_constraints: CatalogTable<ItemAttachmentComponent<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemPortContainer<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    ports: SourcedValue<Vec<SourcedValue<ItemPort<'source>>>>,
    #[serde(skip)]
    ports_use: Option<SourceUse>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemPort<'source> {
    port_ordinal: SourcedValue<u32>,
    name: SourcedValue<&'source str>,
    display_name: SourcedValue<PublicNameRef>,
    #[serde(skip)]
    name_use: Option<SourceUse>,
    restrictions: PortRestrictions<'source>,
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
    item_type: SourcedValue<AuthoredRestriction<'source, &'source str>>,
    #[serde(skip)]
    type_use: Option<SourceUse>,
    item_subtype: SourcedValue<AuthoredRestriction<'source, &'source str>>,
    size: SourcedValue<AuthoredRestriction<'source, ResourceNumber>>,
    tags: SourcedValue<&'source str>,
    required_tags: SourcedValue<&'source str>,
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
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let names = projection.for_text_owner(TextOwner::ItemPorts);
        let ports = source.class_array_to("Ports", PORT, notes, |port, notes| {
            Ok(ItemPort {
                port_ordinal: port.owned_element_ordinal()?,
                name: port.borrowed_string("Name", notes)?,
                display_name: port.port_display_name(&names, notes)?,
                name_use: port.selector_use("Name")?,
                restrictions: PortRestrictions::read(port, notes)?,
                attach_axes_to_align: port.alignment_axis("attachAxesToAlign", options, notes)?,
            })
        })?;
        Ok(Self {
            owner,
            ports,
            ports_use: source.selector_use("Ports")?,
        })
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
                item_type: item.restriction_label("Type", notes)?,
                type_use: item.selector_use("Type")?,
                item_subtype: item.restriction_label("SubType", notes)?,
                size: item.restriction_number("Size", notes)?,
                tags: item.borrowed_string("Tags", notes)?,
                required_tags: item.borrowed_string("RequiredTags", notes)?,
                attach_axes_to_align: item.alignment_axis("attachAxesToAlign", options, notes)?,
                ignored_attach_axis: item.alignment_axis("ignoredAttachAxis", options, notes)?,
                attach_check_center_of_mass: item.boolean("attachCheckCenterOfMass", notes)?,
            })
        })?;
        Ok(Self { owner, definition })
    }
}
