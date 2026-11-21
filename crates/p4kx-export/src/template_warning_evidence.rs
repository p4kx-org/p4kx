// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ExportAccountingError;
use crate::export_diagnostic::{ExportDiagnosticStage, ExportDiagnosticUnit};
use crate::export_health_warning;
use crate::health_diagnostic_observation::{DiagnosticObservation, ExampleObservation};
use crate::health_example_samples;
use crate::mission_flow_owner::FlowOwnerKind;

#[derive(Debug, Default)]
pub(crate) struct TemplateWarningEvidence {
    templates: KindWarnings,
    brokers: KindWarnings,
}

#[derive(Debug, Default)]
struct KindWarnings {
    count: usize,
    examples: Vec<ExampleObservation>,
}

impl TemplateWarningEvidence {
    pub(crate) fn observe(
        &mut self,
        owner: FlowOwnerKind,
        example: Option<ExampleObservation>,
    ) -> Result<(), ExportAccountingError> {
        let warnings = match owner {
            FlowOwnerKind::ContractTemplate => &mut self.templates,
            FlowOwnerKind::MissionBrokerEntry => &mut self.brokers,
        };
        warnings.count = warnings
            .count
            .checked_add(1)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        if let Some(example) = example {
            warnings.examples.push(example);
            warnings.examples =
                health_example_samples::retain(std::mem::take(&mut warnings.examples))?;
        }
        Ok(())
    }

    pub(crate) fn into_diagnostics(
        self,
        warning_code: &str,
    ) -> impl Iterator<Item = DiagnosticObservation> {
        [
            (FlowOwnerKind::ContractTemplate, self.templates),
            (FlowOwnerKind::MissionBrokerEntry, self.brokers),
        ]
        .into_iter()
        .filter(|(_, warnings)| warnings.count > 0)
        .map(move |(owner, warnings)| {
            export_health_warning::source_warning(
                owner.catalog_kind(),
                ExportDiagnosticStage::Projection,
                warning_code,
                warnings.count,
                ExportDiagnosticUnit::Values,
                warnings.examples,
            )
        })
    }
}
