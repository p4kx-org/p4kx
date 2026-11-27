// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use crate::broker_entry::BrokerProjection;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_rows::mission_definitions::MissionDefinitionOutput;
use crate::contract_missions::ContractWithTitles;
use crate::crafting_warning::CraftingWarning;
use crate::location_template::LocationTemplates;
use crate::record_read::{KindRead, KindReadAccounting};
use crate::spawn_pools::SpawnPools;
use crate::template_issues::{UnknownVocabularyReport, Unknowns};
use crate::template_rows::TemplateConfigurations;
use crate::{CatalogError, CatalogReadFailure, Kind};

use super::Collected;

impl<'f> Collected<'f> {
    pub(crate) fn take_configuration_numbers(&mut self) -> Vec<CapturedNumber> {
        let mut numbers = std::mem::take(&mut self.entity_configurations.numbers);
        numbers.append(&mut self.blueprint_reward_numbers);
        for global in &mut self.crafting_global {
            numbers.append(&mut global.numbers);
        }
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

    /// Stores location templates and distinct spawn pools and projects their embedded names.
    pub(crate) fn add_spawns(
        &mut self,
        templates: LocationTemplates<'f>,
        pools: SpawnPools<'f>,
        projection: &CatalogProjection<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let pools = pools.publish(projection, cancel)?;
        self.contract_npc_spawns = pools.npcs;
        self.ledger.merge(templates.ledger);
        self.mission_location_templates = templates.items;
        if let Some(slot) = self.skipped.get_mut(Kind::MissionLocationTemplates.index()) {
            *slot = templates.skipped;
        }
        self.contract_ship_spawns = pools.ships;
        self.contract_entity_spawns = pools.entities;
        self.contract_location_queries = pools.locations;
        self.add_failures(templates.failures);
        Ok(())
    }

    /// Collects contract templates, configurations and source warnings.
    pub(crate) fn add_contract_templates(
        &mut self,
        templates: KindReadAccounting<'f>,
        definitions: MissionDefinitionOutput,
        configurations: TemplateConfigurations<'f>,
        unknowns: Unknowns,
    ) -> Result<(), CatalogError> {
        self.ledger.merge(templates.ledger);
        self.contract_templates = definitions;
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
        definitions: MissionDefinitionOutput,
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
