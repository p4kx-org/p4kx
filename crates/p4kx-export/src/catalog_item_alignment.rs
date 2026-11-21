// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid, SourceUse, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::{AuthoredRestriction, Observed};
use super::item_alignment_options::AlignmentAxis;
pub(crate) use super::item_alignment_options::AlignmentOptions;
pub use super::item_alignment_options::AttachmentAxisOptionStatus;
pub use super::item_component_owner::ComponentOwner;
pub use super::port_restrictions::{PortItemType, PortRestrictions};
use crate::catalog_projection::{CatalogProjection, TextOwner};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::PublicNameRef;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

#[path = "item_attachment_views.rs"]
mod attachment_views;
#[path = "item_alignment_index.rs"]
mod index;
#[path = "item_port_staging.rs"]
mod staging;

pub(crate) use index::AlignmentIndex;
pub use index::PhysicalPort;
pub use index::{AttachmentDefinitionLink, PortDefinitionLink};
pub(crate) use staging::ItemPortStaging;

use self::staging::{PortContainerFacts, StagedItemPorts};
pub(crate) use self::topology::ScopedTopologyAlignment;
pub(super) use self::topology::TopologyAlignment;

#[path = "item_alignment_topology.rs"]
mod topology;

pub(crate) const CONTAINER: &str = "SItemPortContainerComponentParams";
pub(crate) const ATTACHABLE: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const PORT: &str = "SItemPortDef";

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
    pub(crate) fn into_port_containers(self) -> Vec<ItemPortContainer<'source>> {
        self.ports
    }

    pub(crate) fn into_attachment_components(self) -> Vec<ItemAttachmentComponent<'source>> {
        self.attachments
    }

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

    pub(super) fn topology_facts(&self) -> TopologyAlignment<'_, 'source> {
        TopologyAlignment::new(self.port_facts(), &self.attachments)
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

/// One authored port-container component and its physical owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemPortContainer<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    ports: SourcedValue<Vec<SourcedValue<ItemPort<'source>>>>,
    #[serde(skip)]
    ports_use: Option<SourceUse>,
}

/// One authored slot in its port-container component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemPort<'source> {
    port_ordinal: SourcedValue<u32>,
    name: SourcedValue<&'source str>,
    display_name: SourcedValue<PublicNameRef>,
    #[serde(skip)]
    name_use: Option<SourceUse>,
    restrictions: PortRestrictions<'source>,
    attach_axes_to_align: AlignmentAxis<'source>,
}

/// One original attachment component and its physical owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemAttachmentComponent<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    definition: SourcedValue<ItemAttachment<'source>>,
}

/// The original restrictions and alignment fields of an attachment definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemAttachment<'source> {
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

#[path = "item_alignment_kind.rs"]
mod kind;
pub(super) use kind::{AttachmentTarget, ItemAlignmentKind, attachment_target};

impl<'source> ItemPortContainer<'source> {
    /// Returns the nominal owner GUID with its original header origin.
    #[must_use]
    pub fn owner_guid(&self) -> SourcedValue<Guid> {
        self.owner.guid().map_ref(|guid| guid.0)
    }

    /// Borrows the physical Record index in the queried DataForge.
    #[must_use]
    pub const fn owner_record_index(&self) -> &SourcedValue<u32> {
        self.owner.record_index()
    }

    /// Borrows the original Components array ordinal.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        self.owner.component_ordinal()
    }

    /// Borrows the authored ordered slots, including unavailable values.
    #[must_use]
    pub const fn ports(&self) -> &SourcedValue<Vec<SourcedValue<ItemPort<'source>>>> {
        &self.ports
    }

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

impl<'source> ItemPort<'source> {
    /// Borrows this port's authored position in the component.
    #[must_use]
    pub const fn port_ordinal(&self) -> &SourcedValue<u32> {
        &self.port_ordinal
    }

    /// Borrows the original port name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// Borrows the public-name reference for the display label.
    #[must_use]
    pub const fn display_name(&self) -> &SourcedValue<PublicNameRef> {
        &self.display_name
    }

    /// Borrows the original port constraints and rules.
    #[must_use]
    pub const fn restrictions(&self) -> &PortRestrictions<'source> {
        &self.restrictions
    }

    /// Borrows the authored alignment axis label.
    #[must_use]
    pub const fn alignment_axis(&self) -> &SourcedValue<&'source str> {
        &self.attach_axes_to_align.label
    }

    /// Borrows the enum-membership result when the authored axis label is readable.
    #[must_use]
    pub const fn alignment_axis_option_status(
        &self,
    ) -> Option<&SourcedValue<AttachmentAxisOptionStatus>> {
        self.attach_axes_to_align.option_status.as_ref()
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
