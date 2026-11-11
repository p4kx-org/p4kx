// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_diagnostic_observation::ExampleObservation;
use crate::health_example_samples;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::{ExportAccountingError, Kind};

pub(crate) struct TemplateWarningExample {
    pub(crate) owner: FlowOwnerKind,
    pub(crate) example: ExampleObservation,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TemplateWarningExamples {
    templates: Vec<ExampleObservation>,
    brokers: Vec<ExampleObservation>,
}

impl TemplateWarningExamples {
    pub(crate) fn retain(
        &mut self,
        sample: TemplateWarningExample,
    ) -> Result<(), ExportAccountingError> {
        let examples = match sample.owner {
            FlowOwnerKind::ContractTemplate => &mut self.templates,
            FlowOwnerKind::MissionBrokerEntry => &mut self.brokers,
        };
        examples.push(sample.example);
        *examples = health_example_samples::retain(std::mem::take(examples))?;
        Ok(())
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.templates.extend(other.templates);
        self.brokers.extend(other.brokers);
    }

    pub(crate) fn for_kind(&self, kind: Kind) -> &[ExampleObservation] {
        match kind {
            Kind::ContractTemplates => &self.templates,
            Kind::MissionBrokerEntries => &self.brokers,
            _ => &[],
        }
    }
}
