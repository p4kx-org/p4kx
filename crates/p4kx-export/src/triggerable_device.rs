// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "triggerable_behavior.rs"]
mod behavior;
#[path = "triggerable_trigger.rs"]
mod trigger;

use behavior::DeviceBehavior;
use trigger::DeviceTrigger;

pub(super) const SOURCE_TYPE: &str = "EntityComponentTriggerableDevicesParams";

type TriggerSlots<'source> = SourcedValue<Vec<SourcedValue<DeviceTrigger<'source>>>>;

/// Authored trigger lists and death behavior of one original device component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct TriggerableDevice<'source> {
    additional_fields: SourceArenaOutput<'source>,
    triggers: TriggerSlots<'source>,
    ai_triggers: TriggerSlots<'source>,
    death_behavior: SourcedValue<DeviceBehavior<'source>>,
}

impl<'source> TriggerableDevice<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let triggers = source.declared_family_array("triggers", notes, DeviceTrigger::read)?;
        let ai_triggers = source.declared_family_array("aiTriggers", notes, DeviceTrigger::read)?;
        let death_behavior =
            source.declared_pointer("deathBehavior", notes, DeviceBehavior::read)?;
        let additional_fields = source.authored_members_except([
            ("triggers", triggers.state()),
            ("aiTriggers", ai_triggers.state()),
            ("deathBehavior", death_behavior.state()),
        ])?;
        Ok(Self {
            additional_fields,
            triggers,
            ai_triggers,
            death_behavior,
        })
    }
}
