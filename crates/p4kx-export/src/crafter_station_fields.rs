// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

#[path = "crafter_station_rules.rs"]
mod rules;

pub use rules::{
    CrafterCategoryRule, CrafterCategoryRuleKind, CrafterModifier, CrafterModifierKind,
    CrafterModifierType,
};

type Slots<T> = SourcedValue<Vec<SourcedValue<T>>>;

/// The selected crafting component with its authored limits, queues and modifiers.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterComponent<'source> {
    component_ordinal: SourcedValue<u32>,
    #[serde(flatten)]
    source: UnitSource<'source>,
    max_jobs_in_history: SourcedValue<ResourceNumber>,
    base_crafting_time: SourcedValue<ResourceNumber>,
    queues: Slots<CrafterQueue<'source>>,
    modifiers: Slots<CrafterModifier<'source>>,
}

/// One authored job queue at its original array position.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterQueue<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    name: SourcedValue<&'source str>,
    max_jobs_in_progress: SourcedValue<ResourceNumber>,
    max_jobs_waiting: SourcedValue<ResourceNumber>,
    blueprint_category_availability: SourcedValue<CrafterCategoryRule<'source>>,
    process_type_availability: Slots<bool>,
}

/// The selected attachable component used to match a station to a port.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterAttachment<'source> {
    component_ordinal: SourcedValue<u32>,
    #[serde(flatten)]
    source: UnitSource<'source>,
    attach_def: SourcedValue<CrafterAttachmentDefinition<'source>>,
}

/// The authored attachment labels and size of a crafting station.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterAttachmentDefinition<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    item_type: SourcedValue<&'source str>,
    item_subtype: SourcedValue<&'source str>,
    size: SourcedValue<ResourceNumber>,
}

impl<'source> CrafterComponent<'source> {
    /// Borrows the selected original Components slot.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        &self.component_ordinal
    }

    /// Borrows the component's concrete type and additional authored members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the authored global history limit.
    #[must_use]
    pub const fn max_jobs_in_history(&self) -> &SourcedValue<ResourceNumber> {
        &self.max_jobs_in_history
    }

    /// Borrows the authored base crafting time in its original numeric storage.
    #[must_use]
    pub const fn base_crafting_time(&self) -> &SourcedValue<ResourceNumber> {
        &self.base_crafting_time
    }

    /// Borrows the original queue slots in stored order, including their source states.
    #[must_use]
    pub const fn queues(&self) -> &SourcedValue<Vec<SourcedValue<CrafterQueue<'source>>>> {
        &self.queues
    }

    /// Borrows the original modifier slots in stored order, including their source states.
    #[must_use]
    pub const fn modifiers(&self) -> &SourcedValue<Vec<SourcedValue<CrafterModifier<'source>>>> {
        &self.modifiers
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let component_ordinal = fields.array_ordinal()?;
        let max_jobs_in_history = fields.authored_integer("maxJobsInHistory", notes)?;
        let base_crafting_time = fields.authored_number("baseCraftingTime", notes)?;
        let queues = fields.declared_owned_array("queues", notes, CrafterQueue::read)?;
        let modifiers = fields.declared_owned_array("modifiers", notes, CrafterModifier::read)?;
        let source = UnitSource::read(
            fields,
            [
                ("maxJobsInHistory", max_jobs_in_history.state()),
                ("baseCraftingTime", base_crafting_time.state()),
                ("queues", complete_slots(&queues)),
                ("modifiers", complete_slots(&modifiers)),
            ],
        )?;
        Ok(Self {
            component_ordinal,
            source,
            max_jobs_in_history,
            base_crafting_time,
            queues,
            modifiers,
        })
    }
}

impl<'source> CrafterQueue<'source> {
    /// Borrows the queue's concrete type and additional authored members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original queue label.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// Borrows the authored count of jobs that can run in parallel.
    #[must_use]
    pub const fn max_jobs_in_progress(&self) -> &SourcedValue<ResourceNumber> {
        &self.max_jobs_in_progress
    }

    /// Borrows the authored count of waiting jobs.
    #[must_use]
    pub const fn max_jobs_waiting(&self) -> &SourcedValue<ResourceNumber> {
        &self.max_jobs_waiting
    }

    /// Borrows the original category rule and its source state.
    #[must_use]
    pub const fn blueprint_category_availability(
        &self,
    ) -> &SourcedValue<CrafterCategoryRule<'source>> {
        &self.blueprint_category_availability
    }

    /// Borrows the ordered process availability flags without assigning process names.
    #[must_use]
    pub const fn process_type_availability(&self) -> &SourcedValue<Vec<SourcedValue<bool>>> {
        &self.process_type_availability
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.borrowed_label("debugName", notes)?;
        let max_jobs_in_progress = fields.authored_integer("maxJobsInProgress", notes)?;
        let max_jobs_waiting = fields.authored_integer("maxJobsWaiting", notes)?;
        let blueprint_category_availability = fields.declared_owned(
            "blueprintCategoryAvailability",
            notes,
            CrafterCategoryRule::read_whitelist,
        )?;
        let process_type_availability = fields.boolean_array("processTypeAvailability", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("debugName", name.state()),
                ("maxJobsInProgress", max_jobs_in_progress.state()),
                ("maxJobsWaiting", max_jobs_waiting.state()),
                (
                    "blueprintCategoryAvailability",
                    blueprint_category_availability.state(),
                ),
                (
                    "processTypeAvailability",
                    complete_slots(&process_type_availability),
                ),
            ],
        )?;
        Ok(Self {
            source,
            name,
            max_jobs_in_progress,
            max_jobs_waiting,
            blueprint_category_availability,
            process_type_availability,
        })
    }
}

impl<'source> CrafterAttachment<'source> {
    /// Borrows the original Components slot of the selected attachable component.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        &self.component_ordinal
    }

    /// Borrows the attachable component's concrete type and other original fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original attachment definition consulted for port matching.
    #[must_use]
    pub const fn attach_def(&self) -> &SourcedValue<CrafterAttachmentDefinition<'source>> {
        &self.attach_def
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let component_ordinal = fields.array_ordinal()?;
        let attach_def =
            fields.declared_owned("AttachDef", notes, CrafterAttachmentDefinition::read)?;
        let source = UnitSource::read(fields, [("AttachDef", attach_def.state())])?;
        Ok(Self {
            component_ordinal,
            source,
            attach_def,
        })
    }
}

impl<'source> CrafterAttachmentDefinition<'source> {
    /// Borrows the attachment definition's concrete type and additional authored members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the authored attachment type label.
    #[must_use]
    pub const fn item_type(&self) -> &SourcedValue<&'source str> {
        &self.item_type
    }

    /// Borrows the authored attachment subtype label.
    #[must_use]
    pub const fn item_subtype(&self) -> &SourcedValue<&'source str> {
        &self.item_subtype
    }

    /// Borrows the original attachment size.
    #[must_use]
    pub const fn size(&self) -> &SourcedValue<ResourceNumber> {
        &self.size
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let item_type = fields.borrowed_label("Type", notes)?;
        let item_subtype = fields.borrowed_label("SubType", notes)?;
        let size = fields.authored_integer("Size", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("Type", item_type.state()),
                ("SubType", item_subtype.state()),
                ("Size", size.state()),
            ],
        )?;
        Ok(Self {
            source,
            item_type,
            item_subtype,
            size,
        })
    }
}

fn complete_slots<T>(slots: &Slots<T>) -> SourceValueState {
    if slots.value().is_some_and(|values| {
        values.iter().any(|value| {
            !matches!(
                value.state(),
                SourceValueState::Present | SourceValueState::Null
            )
        })
    }) {
        SourceValueState::Unreadable
    } else {
        slots.state()
    }
}
