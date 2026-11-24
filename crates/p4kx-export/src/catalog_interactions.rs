// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interactable_configuration::{self, InteractableConfiguration};
use super::interaction_source::InteractionList;
use super::interaction_state_type::{self, InteractionStateType};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_rows::staged::OrderedComponentRows;
use crate::catalog_table::DatasetAvailability;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const INTERACTABLE: &str = "SEntityInteractableParams";
const STATE_MACHINE: &str = "SInteractionStateMachineParams";
const KIOSK: &str = "KioskInteractionEntityComponentParams";
const KIOSK_VR_ALIGNMENT: &str = "supportWalkToAlignInVR";
pub(crate) const SOURCE_TYPES: &[&str] = &[INTERACTABLE, STATE_MACHINE, KIOSK];

#[derive(Clone, Copy)]
pub(super) enum InteractionKind {
    Interactable,
    StateMachine,
    Kiosk,
}

impl InteractionKind {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            INTERACTABLE => Some(Self::Interactable),
            STATE_MACHINE => Some(Self::StateMachine),
            KIOSK => Some(Self::Kiosk),
            _ => None,
        }
    }
}

/// One physical interaction component and its authored declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct InteractionConfiguration<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    fields: InteractionFields<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum InteractionFields<'source> {
    Interactable {
        interactable: Box<SourcedValue<InteractableConfiguration<'source>>>,
    },
    StateMachine {
        state_types: InteractionList<InteractionStateType<'source>>,
    },
    Kiosk {
        #[serde(rename = "supportWalkToAlignInVR")]
        support_walk_to_align_in_vr: SourcedValue<bool>,
    },
}

impl<'source> InteractionConfiguration<'source> {
    /// Borrows the physical owner and original Components position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the concrete interaction component's original fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the authored interactable cell for an interactable component.
    #[must_use]
    pub fn interactable(&self) -> Option<&SourcedValue<InteractableConfiguration<'source>>> {
        match &self.fields {
            InteractionFields::Interactable { interactable } => Some(interactable.as_ref()),
            InteractionFields::StateMachine { .. } | InteractionFields::Kiosk { .. } => None,
        }
    }

    /// Borrows the ordered state-type cell for a state machine.
    #[must_use]
    pub const fn state_types(
        &self,
    ) -> Option<&SourcedValue<Vec<SourcedValue<InteractionStateType<'source>>>>> {
        match &self.fields {
            InteractionFields::Interactable { .. } | InteractionFields::Kiosk { .. } => None,
            InteractionFields::StateMachine { state_types } => Some(state_types),
        }
    }

    /// Borrows the original walk-to-align flag for a kiosk component.
    #[must_use]
    pub const fn support_walk_to_align_in_vr(&self) -> Option<&SourcedValue<bool>> {
        match &self.fields {
            InteractionFields::Kiosk {
                support_walk_to_align_in_vr,
            } => Some(support_walk_to_align_in_vr),
            InteractionFields::Interactable { .. } | InteractionFields::StateMachine { .. } => None,
        }
    }

    pub(super) fn read(
        kind: InteractionKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let (fields, projected) = match kind {
            InteractionKind::Interactable => {
                let field = source.declared_class_to(
                    "Interactable",
                    interactable_configuration::SOURCE_TYPE,
                    notes,
                    |source, notes| InteractableConfiguration::read(source, projection, notes),
                )?;
                let projected = ("Interactable", field.state());
                (
                    InteractionFields::Interactable {
                        interactable: Box::new(field),
                    },
                    projected,
                )
            }
            InteractionKind::StateMachine => {
                let state_types = source.class_array_to(
                    "StateTypes",
                    interaction_state_type::SOURCE_TYPE,
                    notes,
                    InteractionStateType::read,
                )?;
                let projected = ("StateTypes", state_types.state());
                (InteractionFields::StateMachine { state_types }, projected)
            }
            InteractionKind::Kiosk => {
                let support_walk_to_align_in_vr = source.boolean(KIOSK_VR_ALIGNMENT, notes)?;
                let projected = (KIOSK_VR_ALIGNMENT, support_walk_to_align_in_vr.state());
                (
                    InteractionFields::Kiosk {
                        support_walk_to_align_in_vr,
                    },
                    projected,
                )
            }
        };
        let source = UnitSource::read(source, [projected])?;
        Ok(Self {
            owner,
            source,
            fields,
        })
    }
}

impl InteractionConfiguration<'_> {
    pub(super) fn stage(&self, rows: &mut OrderedComponentRows<'_>) -> Result<(), CatalogError> {
        rows.append(self.owner.key(), self)
    }
}

pub(super) fn empty_availability(forge: &DataForge<'_>) -> DatasetAvailability {
    DatasetAvailability::from_source_declarations(forge, |name| SOURCE_TYPES.contains(&name))
}

#[cfg(test)]
#[path = "interaction_staging_tests.rs"]
mod staging_tests;
