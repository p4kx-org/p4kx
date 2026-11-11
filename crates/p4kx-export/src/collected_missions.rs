// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::broker_definition::BrokerDefinition;
use crate::broker_entry::BrokerProjection;
use crate::crafting_warning::CraftingWarning;
use crate::record_read::KindRead;
use crate::template_definition::TemplateDefinition;
use crate::template_issues::{UnknownVocabularyReport, Unknowns};
use crate::template_rows::TemplateConfigurations;
use crate::{CatalogError, Kind};

use super::Collected;

impl<'f> Collected<'f> {
    /// Collects contract templates, configurations and source warnings.
    pub(crate) fn add_contract_templates(
        &mut self,
        templates: KindRead<'f, TemplateDefinition<'f>>,
        configurations: TemplateConfigurations<'f>,
        unknowns: Unknowns,
    ) -> Result<(), CatalogError> {
        self.ledger.merge(templates.ledger);
        self.contract_templates = templates.items;
        self.template_configurations = configurations;
        if let Some(slot) = self.skipped.get_mut(Kind::ContractTemplates.index()) {
            *slot = templates.skipped;
        }
        self.add_failures(templates.failures);
        let UnknownVocabularyReport {
            count,
            details,
            examples,
        } = unknowns.into_report()?;
        if count > 0 {
            self.warnings
                .push(CraftingWarning::UnknownTemplateValues { count, details });
        }
        self.ledger.template_warning_examples.extend(examples);
        Ok(())
    }

    /// Collects mission broker entries and their read accounting.
    pub(crate) fn add_broker_entries(
        &mut self,
        entries: KindRead<'f, BrokerProjection<'f>>,
        definitions: Vec<BrokerDefinition<'f>>,
    ) {
        self.ledger.merge(entries.ledger);
        self.broker_projections = entries.items;
        self.broker_definitions = definitions;
        if let Some(slot) = self.skipped.get_mut(Kind::MissionBrokerEntries.index()) {
            *slot = entries.skipped;
        }
        self.add_failures(entries.failures);
    }
}
