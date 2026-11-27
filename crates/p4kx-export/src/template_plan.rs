// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_flow_plan::FlowPlan;
use crate::template_meaning_plan::SelectedTemplateField;
use crate::template_objective_plan::TemplateObjectiveSelection;
use crate::template_output_plan::TemplateOutputSelection;
use crate::template_property_plan::SelectedProperty;
use crate::template_reference_index::TemplateReferences;
use crate::template_spawn_plan::TemplateSpawnPlan;
use crate::template_weak_plan::TemplateWeakSelection;

pub(crate) struct TemplatePlan<'fields, 'scope, 'read, 'source> {
    pub(crate) base: Vec<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(crate) flow: FlowPlan<'fields, 'scope, 'read, 'source>,
    pub(crate) flow_targets: Vec<SelectedTemplateField<'fields, 'scope, 'read, 'source>>,
    pub(crate) objectives: Vec<TemplateObjectiveSelection<'fields, 'scope, 'read, 'source>>,
    pub(crate) properties: Vec<SelectedProperty<'fields, 'scope, 'read, 'source>>,
    pub(crate) weak: Vec<TemplateWeakSelection<'fields, 'scope, 'read, 'source>>,
    pub(crate) outputs: Vec<TemplateOutputSelection<'fields, 'scope, 'read, 'source>>,
    pub(crate) spawns: TemplateSpawnPlan<'fields, 'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> TemplatePlan<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
        owner: FlowOwnerKind,
    ) -> Result<Self, CatalogError> {
        let base: Vec<_> = fields
            .facts()
            .iter()
            .filter(|fact| {
                fact.parent_ordinal().is_none()
                    && (matches!(owner, FlowOwnerKind::MissionBrokerEntry)
                        || retained(fact.field().name()))
            })
            .collect();
        let flow = FlowPlan::read(fields)?;
        let flow_targets = crate::template_meaning_plan::targets(&flow, references)?;
        let objectives = crate::template_objective_plan::read(fields)?;
        let properties = crate::template_property_plan::read(fields, &objectives, owner)?;
        let weak = crate::template_weak_plan::read(fields, &base, references, &properties)?;
        let outputs = crate::template_output_plan::read(fields, references, &properties)?;
        let spawns = TemplateSpawnPlan::read(fields, &properties, owner)?;
        Ok(Self {
            base,
            flow,
            flow_targets,
            objectives,
            properties,
            weak,
            outputs,
            spawns,
        })
    }

    pub(crate) fn roots(
        &self,
    ) -> impl Iterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>> + '_ {
        self.base
            .iter()
            .copied()
            .chain(self.flow_targets.iter().map(|target| target.field))
            .chain(crate::template_weak_plan::fields(&self.weak))
            .chain(crate::template_output_plan::fields(&self.outputs))
    }
}

fn retained(name: &str) -> bool {
    matches!(
        name,
        "missionFlow"
            | "contractProperties"
            | "objectiveTokens"
            | "notForRelease"
            | "workInProgress"
            | "contractDisplayInfo"
            | "contractClass"
            | "partialRewardPayout"
            | "modifiers"
            | "owner"
            | "contractStartCommsNotification"
            | "contractEndCommsNotifications"
    )
}
