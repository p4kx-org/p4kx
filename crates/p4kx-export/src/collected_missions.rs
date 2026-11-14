// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::broker_definition::BrokerDefinition;
use crate::broker_entry::BrokerProjection;
use crate::catalog_number_capture::CapturedNumber;
use crate::contract_missions::ContractWithTitles;
use crate::crafting_warning::CraftingWarning;
use crate::location_template::LocationTemplates;
use crate::record_read::KindRead;
use crate::spawn_pools::SpawnPools;
use crate::template_definition::TemplateDefinition;
use crate::template_issues::{UnknownVocabularyReport, Unknowns};
use crate::template_rows::TemplateConfigurations;
use crate::texts::Texts;
use crate::{CatalogError, CatalogReadFailure, Kind};

use super::Collected;

impl<'f> Collected<'f> {
    pub(crate) fn take_configuration_numbers(&mut self) -> Vec<CapturedNumber> {
        let mut numbers = std::mem::take(&mut self.entity_configurations.numbers);
        for location in &mut self.mission_location_templates {
            numbers.append(&mut location.configuration_numbers);
        }
        numbers
    }

    /// Stores the contracts, counts the entries left out as `skipped` and adds the failures.
    pub(crate) fn add_contracts(
        &mut self,
        contracts: Vec<ContractWithTitles<'f>>,
        skipped: usize,
        failures: Vec<CatalogReadFailure<'f>>,
    ) {
        (self.contracts, self.mission_titles) = contracts
            .into_iter()
            .map(|contract| (contract.row, contract.titles))
            .unzip();
        if let Some(slot) = self.skipped.get_mut(Kind::Contracts.index()) {
            *slot = skipped;
        }
        self.add_failures(failures);
    }

    /// Stores the location templates and the distinct spawn descriptions and location queries of
    /// the contracts, resolving the texts of the descriptions with `texts`, counts the templates
    /// left out as `skipped` and adds the failures.
    pub(crate) fn add_spawns(
        &mut self,
        templates: LocationTemplates<'f>,
        mut pools: SpawnPools<'f>,
        texts: &Texts<'f>,
    ) {
        self.ledger.merge(templates.ledger);
        self.mission_location_templates = templates.items;
        if let Some(slot) = self.skipped.get_mut(Kind::MissionLocationTemplates.index()) {
            *slot = templates.skipped;
        }
        pools.resolve_texts(texts);
        self.contract_ship_spawns = pools.ships.into_values().collect();
        self.contract_npc_spawns = pools.npcs.into_values().collect();
        self.contract_entity_spawns = pools.entities.into_values().collect();
        self.contract_location_queries = pools.locations.into_values().collect();
        self.add_failures(templates.failures);
    }

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
            evidence,
        } = unknowns.into_report()?;
        if count > 0 {
            let warning = CraftingWarning::UnknownTemplateValues { count, details };
            for diagnostic in evidence.into_diagnostics(warning.kind()) {
                self.ledger.field_diagnostic(diagnostic);
            }
            self.warnings.push(warning);
        }
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
