// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The original interaction selector, trigger and concrete task parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionTask<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    interaction: SourcedValue<&'source str>,
    trigger_type: SourcedValue<&'source str>,
}

impl<'source> InteractionTask<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let interaction = source.borrowed_string("interaction", notes)?;
        let trigger_type =
            source.borrowed_enum_to("triggerType", "EInteractionTriggerType", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("interaction", interaction.state()),
                ("triggerType", trigger_type.state()),
            ],
        )?;
        Ok(Self {
            source,
            interaction,
            trigger_type,
        })
    }
}

/// The original actor action discriminator and concrete task parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ActorActionTask<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    action_type: SourcedValue<&'source str>,
}

impl<'source> ActorActionTask<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let action_type =
            source.borrowed_enum_to("actionType", "EActorActionHandlerEventType", notes)?;
        let source = UnitSource::read(source, [("actionType", action_type.state())])?;
        Ok(Self {
            source,
            action_type,
        })
    }
}

/// The original item action discriminator and concrete task parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ItemActionTask<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    item_action_type: SourcedValue<&'source str>,
}

impl<'source> ItemActionTask<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let item_action_type =
            source.borrowed_enum_to("itemActionType", "EItemActionEventType", notes)?;
        let source = UnitSource::read(source, [("itemActionType", item_action_type.state())])?;
        Ok(Self {
            source,
            item_action_type,
        })
    }
}
