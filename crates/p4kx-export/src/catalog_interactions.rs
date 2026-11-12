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
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const INTERACTABLE: &str = "SEntityInteractableParams";
const STATE_MACHINE: &str = "SInteractionStateMachineParams";

#[derive(Clone, Copy)]
pub(super) enum InteractionKind {
    Interactable,
    StateMachine,
}

impl InteractionKind {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            INTERACTABLE => Some(Self::Interactable),
            STATE_MACHINE => Some(Self::StateMachine),
            _ => None,
        }
    }
}

/// One physical interaction component and its authored declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionConfiguration<'source> {
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
}

impl<'source> InteractionConfiguration<'source> {
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
        };
        let source = UnitSource::read(source, [projected])?;
        Ok(Self {
            owner,
            source,
            fields,
        })
    }
}

pub(super) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<InteractionConfiguration<'source>>,
) -> CatalogTable<InteractionConfiguration<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[INTERACTABLE, STATE_MACHINE], rows)
}
