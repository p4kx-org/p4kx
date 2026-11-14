// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

const TRIGGER_BASE: &str = "SandboxTriggerBaseDef";
const MANUAL: &str = "SandboxTriggerManualParams";
const TRIGGER_RECORD: &str = "SandboxTriggerRecord";
const INFRACTION_BASE: &str = "SandboxInfractionBaseDef";
const REWARD_BASE: &str = "ReputationRewardBaseDef";

type Link<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// An authored sandbox trigger occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SandboxTrigger<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    body: TriggerBody<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum TriggerBody<'source> {
    Manual {
        infraction_trigger: SourcedValue<InfractionTrigger<'source>>,
        trigger_on_innocents_only: SourcedValue<bool>,
        outcomes: SourcedValue<Vec<SourcedValue<RewardOutcome<'source>>>>,
    },
    Record {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        trigger_record: Link<'source>,
    },
    Authored,
}

impl<'source> SandboxTrigger<'source> {
    /// Reads the ordered trigger pointers owned by a faction reputation Record.
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<Self>>>, CatalogError> {
        fields.strong_pointer_family_array_to("sandboxTriggers", TRIGGER_BASE, notes, Self::read)
    }

    /// Reads the ordered inline definitions owned by a sandbox trigger Record.
    pub(super) fn read_record_members(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<Self>>>, CatalogError> {
        fields.class_array_to("triggerManualParams", MANUAL, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let body = TriggerBody::read(fields, notes)?;
        let authored_fields =
            fields.authored_members_except(body.projected().into_iter().flatten())?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            body,
            authored_fields,
        })
    }
}

impl<'source> TriggerBody<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match fields.source_type()? {
            MANUAL => Self::Manual {
                infraction_trigger: fields.strong_pointer_family_to(
                    "infractionTrigger",
                    INFRACTION_BASE,
                    notes,
                    InfractionTrigger::read,
                )?,
                trigger_on_innocents_only: fields.boolean("triggerOnInnocentsOnly", notes)?,
                outcomes: fields.strong_pointer_family_array_to(
                    "outcomes",
                    REWARD_BASE,
                    notes,
                    RewardOutcome::read,
                )?,
            },
            "SandboxTriggerRecordParams" => Self::Record {
                trigger_record: fields.record_link_to("triggerRecord", TRIGGER_RECORD, notes)?,
            },
            TRIGGER_BASE => Self::Authored,
            _ => {
                fields.note_retained_variant(notes)?;
                Self::Authored
            }
        })
    }

    const fn projected(&self) -> [Option<(&'static str, SourceValueState)>; 3] {
        match self {
            Self::Manual {
                infraction_trigger,
                trigger_on_innocents_only,
                outcomes,
            } => [
                Some(("infractionTrigger", infraction_trigger.state())),
                Some(("triggerOnInnocentsOnly", trigger_on_innocents_only.state())),
                Some(("outcomes", outcomes.state())),
            ],
            Self::Record { trigger_record } => {
                [Some(("triggerRecord", trigger_record.state())), None, None]
            }
            Self::Authored => [None; 3],
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct InfractionTrigger<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    body: InfractionBody<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum InfractionBody<'source> {
    Event {
        sandbox_trigger: SourcedValue<&'source str>,
    },
    Definition {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        sandbox_infraction_definition_trigger: Link<'source>,
    },
    Authored,
}

impl<'source> InfractionTrigger<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let body = match fields.source_type()? {
            "SandboxTrigger" => InfractionBody::Event {
                sandbox_trigger: fields.borrowed_enum_to(
                    "sandboxTrigger",
                    "EInfractionTrigger",
                    notes,
                )?,
            },
            "SandboxInfractionDefinitionTrigger" => InfractionBody::Definition {
                sandbox_infraction_definition_trigger: fields.record_link_to(
                    "sandboxInfractionDefinitionTrigger",
                    "InfractionDefinition",
                    notes,
                )?,
            },
            INFRACTION_BASE => InfractionBody::Authored,
            _ => {
                fields.note_retained_variant(notes)?;
                InfractionBody::Authored
            }
        };
        let projected = match &body {
            InfractionBody::Event { sandbox_trigger } => {
                Some(("sandboxTrigger", sandbox_trigger.state()))
            }
            InfractionBody::Definition {
                sandbox_infraction_definition_trigger,
            } => Some((
                "sandboxInfractionDefinitionTrigger",
                sandbox_infraction_definition_trigger.state(),
            )),
            InfractionBody::Authored => None,
        };
        Ok(Self {
            source_type: fields.source_type_value()?,
            body,
            authored_fields: fields.authored_members_except(projected)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct RewardOutcome<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    scope: Link<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    reward: Link<'source>,
    #[serde(flatten)]
    body: RewardTarget<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum RewardTarget<'source> {
    Base,
    Faction {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        faction_reputation: Link<'source>,
    },
    Authored,
}

impl<'source> RewardOutcome<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let scope = fields.record_link_to("scope", "SReputationScopeParams", notes)?;
        let reward = fields.record_link_to("reward", "SReputationRewardAmount", notes)?;
        let body = match fields.source_type()? {
            REWARD_BASE => RewardTarget::Base,
            "ReputationRewardFaction" => RewardTarget::Faction {
                faction_reputation: fields.record_link_to(
                    "factionReputation",
                    "FactionReputation",
                    notes,
                )?,
            },
            _ => {
                fields.note_retained_variant(notes)?;
                RewardTarget::Authored
            }
        };
        let faction = match &body {
            RewardTarget::Faction { faction_reputation } => {
                Some(("factionReputation", faction_reputation.state()))
            }
            RewardTarget::Base | RewardTarget::Authored => None,
        };
        let authored_fields = fields.authored_members_except(
            [("scope", scope.state()), ("reward", reward.state())]
                .into_iter()
                .chain(faction),
        )?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            scope,
            reward,
            body,
            authored_fields,
        })
    }
}
