// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::link::HealthLink;
use super::observed::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const MAELSTROM_BASE: &str = "MaelstromBaseEvent";
const MAELSTROM_DESTROY: &str = "MaelstromDestroyEvent";
const MAELSTROM_STATE: &str = "MaelstromInteractionStateMachineEvent";
const SEQUENCER_BASE: &str = "EntityComponentHealth_SBaseSequencerTask";
const APPLY_DAMAGE_MAP: &str = "EntityComponentHealth_SApplyDamageMapTask";

/// Authored Health event and sequencer occurrences in their original slot order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EventActions<'source> {
    maelstrom_events: SourcedValue<Vec<SourcedValue<MaelstromEvent<'source>>>>,
    sequencer_tasks: SourcedValue<Vec<SourcedValue<SequencerTask<'source>>>>,
}

impl<'source> EventActions<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            maelstrom_events: source.array(
                "maelstromEvents",
                &[MAELSTROM_BASE, MAELSTROM_DESTROY, MAELSTROM_STATE],
                notes,
                MaelstromEvent::read,
            )?,
            sequencer_tasks: source.array(
                "sequencerTasks",
                &[SEQUENCER_BASE, APPLY_DAMAGE_MAP],
                notes,
                SequencerTask::read,
            )?,
        })
    }
}

/// The concrete Maelstrom event alternative on one Health component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum MaelstromEvent<'source> {
    Base { value: MaelstromEventTags<'source> },
    Destroy { value: MaelstromEventTags<'source> },
    InteractionStateMachine { value: MaelstromStateEvent<'source> },
}

impl<'source> MaelstromEvent<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tags = MaelstromEventTags::read(source, notes)?;
        Ok(match source.source_type()? {
            MAELSTROM_BASE => Self::Base { value: tags },
            MAELSTROM_DESTROY => Self::Destroy { value: tags },
            MAELSTROM_STATE => Self::InteractionStateMachine {
                value: MaelstromStateEvent {
                    tags,
                    state: source.link("state", notes)?,
                },
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Original event and group Tag links shared by the event alternatives.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MaelstromEventTags<'source> {
    event_tag: SourcedValue<HealthLink<'source>>,
    group_tag: SourcedValue<HealthLink<'source>>,
}

impl<'source> MaelstromEventTags<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            event_tag: source.link("eventTag", notes)?,
            group_tag: source.link("groupTag", notes)?,
        })
    }
}

/// The authored target of a Maelstrom interaction-state event.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MaelstromStateEvent<'source> {
    #[serde(flatten)]
    tags: MaelstromEventTags<'source>,
    state: SourcedValue<HealthLink<'source>>,
}

/// The concrete sequencer task authored by a Health component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum SequencerTask<'source> {
    Base,
    ApplyDamageMap { value: ApplyDamageMapTask<'source> },
}

impl<'source> SequencerTask<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            SEQUENCER_BASE => Self::Base,
            APPLY_DAMAGE_MAP => Self::ApplyDamageMap {
                value: ApplyDamageMapTask {
                    damage_map_path: source.borrowed_string("damageMapPath", notes)?,
                },
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// One original damage-map path without file resolution.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ApplyDamageMapTask<'source> {
    damage_map_path: SourcedValue<&'source str>,
}
