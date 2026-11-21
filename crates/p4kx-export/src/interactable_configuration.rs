// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_point::{self, InteractionPoint};
use super::interaction_source::{InteractionList, RecordLink};
use super::interaction_task::{ActorActionTask, InteractionTask, ItemActionTask};
use super::shared_interaction::{self, SharedInteraction};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "SInteractableParams";

/// The original shared interactions, points, tasks and menu title of one component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractableConfiguration<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    shared_interactions: InteractionList<SharedInteraction<'source>>,
    interaction_points: InteractionList<InteractionPoint<'source>>,
    interaction_tasks: InteractionList<InteractionTask<'source>>,
    action_handler_tasks: InteractionList<ActorActionTask<'source>>,
    item_action_tasks: InteractionList<ItemActionTask<'source>>,
    menu_title: SourcedValue<InteractionMenuTitle<'source>>,
}

impl<'source> InteractableConfiguration<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let shared_interactions = source.strong_pointer_family_array_to(
            "SharedInteractions",
            shared_interaction::SOURCE_TYPE,
            notes,
            |source, notes| SharedInteraction::read(source, projection, notes),
        )?;
        let interaction_points = source.class_array_to(
            "InteractionPoints",
            interaction_point::SOURCE_TYPE,
            notes,
            |source, notes| InteractionPoint::read(source, projection, notes),
        )?;
        let interaction_tasks = source.strong_pointer_family_array_to(
            "interactionTasks",
            "SSequencerInteractionTask",
            notes,
            InteractionTask::read,
        )?;
        let action_handler_tasks = source.strong_pointer_family_array_to(
            "actionHandlerTasks",
            "SSequencerActorActionHandlerTask",
            notes,
            ActorActionTask::read,
        )?;
        let item_action_tasks = source.strong_pointer_family_array_to(
            "itemActionTasks",
            "SSequencerItemActionTask",
            notes,
            ItemActionTask::read,
        )?;
        let menu_title = source.strong_pointer_family_to(
            "menuTitle",
            "SInteractableMenuTitleParams",
            notes,
            InteractionMenuTitle::read,
        )?;
        let source = UnitSource::read(
            source,
            [
                ("SharedInteractions", shared_interactions.state()),
                ("InteractionPoints", interaction_points.state()),
                ("interactionTasks", interaction_tasks.state()),
                ("actionHandlerTasks", action_handler_tasks.state()),
                ("itemActionTasks", item_action_tasks.state()),
                ("menuTitle", menu_title.state()),
            ],
        )?;
        Ok(Self {
            source,
            shared_interactions,
            interaction_points,
            interaction_tasks,
            action_handler_tasks,
            item_action_tasks,
            menu_title,
        })
    }
}

/// The original menu color link and concrete title parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct InteractionMenuTitle<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    title_color: RecordLink<'source>,
}

impl<'source> InteractionMenuTitle<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let title_color = source.record_link_to("titleColor", "InnerThought_ColorParams", notes)?;
        let source = UnitSource::read(source, [("titleColor", title_color.state())])?;
        Ok(Self {
            source,
            title_color,
        })
    }
}
