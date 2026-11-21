// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_source::{RecordLinks, WeakLink};
use super::shared_interaction;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const STATE: &str = "SInteractionState";
pub(super) const TRANSITION: &str = "SInteractionStateTransition";

/// The two independently authored `WeakPointer` slots of a state transition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionTransition<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    interaction: WeakLink<'source>,
    next_state: WeakLink<'source>,
}

impl<'source> InteractionTransition<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let interaction =
            source.weak_link_to("Interaction", shared_interaction::SOURCE_TYPE, notes)?;
        let next_state = source.weak_link_to("NextState", STATE, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("Interaction", interaction.state()),
                ("NextState", next_state.state()),
            ],
        )?;
        Ok(Self {
            source,
            interaction,
            next_state,
        })
    }
}

/// The authored automatic transition, timing and streamed-out flag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct StateAutoChange<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    next_state: WeakLink<'source>,
    delay: SourcedValue<ResourceFloat32>,
    run_when_streamed_out: SourcedValue<bool>,
}

impl<'source> StateAutoChange<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let next_state = source.weak_link_to("NextState", STATE, notes)?;
        let delay = source.single("Delay", notes)?;
        let run_when_streamed_out = source.boolean("RunWhenStreamedOut", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("NextState", next_state.state()),
                ("Delay", delay.state()),
                ("RunWhenStreamedOut", run_when_streamed_out.state()),
            ],
        )?;
        Ok(Self {
            source,
            next_state,
            delay,
            run_when_streamed_out,
        })
    }
}

/// One authored tag list and its ordered record references.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionTagList<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: RecordLinks<'source>,
}

impl<'source> InteractionTagList<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tags = source.record_link_array_to("tags", "Tag", notes)?;
        let source = UnitSource::read(source, [("tags", tags.state())])?;
        Ok(Self { source, tags })
    }
}
