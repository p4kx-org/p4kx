// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory, Value,
};

use super::read;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::mission_sources::{
    contract_generator_facets, contract_scenario_requirements,
    local_contract_generator_configuration,
};
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

pub use contract_scenario_requirements::ContractScenarioRequirements;
pub use local_contract_generator_configuration::LocalContractGeneratorConfiguration;

/// Selected authored Generator facets and their original source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContractGeneratorFacetSnapshot<'source> {
    /// The originating Archive Build, when available.
    pub build: Option<Build>,
    /// Original schema availability of local generation and board settings.
    pub local_generator_availability: DatasetAvailability,
    /// Canonical Handler configurations in physical source order.
    pub local_generators: Vec<LocalContractGeneratorConfiguration<'source>>,
    /// Original schema availability of the scenario requirement declarations.
    pub scenario_requirement_availability: DatasetAvailability,
    /// Canonical scenario reference arrays in physical source order.
    pub scenario_requirements: Vec<ContractScenarioRequirements<'source>>,
    /// Header, body and field coverage of the selected Generator Records.
    pub coverage: CatalogRecordCoverage,
    /// Sources and uses retained by the selected authored values.
    pub source_inventory: SourceInventory,
}

/// Supplies authored local generation and scenario requirements for selected Generator Records.
///
/// # Errors
/// Returns input, Archive-change, source-evidence or cancellation errors.
pub fn with_contract_generator_facets<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractGeneratorFacetSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_contract_generator_facets(selection, cancel, inspect)
}

/// Supplies the authored Generator facets using a decoded Entry from the same Archive.
///
/// # Errors
/// Returns query errors or an input-origin mismatch for a foreign Entry.
pub fn with_contract_generator_facets_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractGeneratorFacetSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_contract_generator_facets(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies the authored Generator facets using the session's original DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_generator_facets`].
    pub fn with_contract_generator_facets<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractGeneratorFacetSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_generator_facets(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the authored Generator facets using this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_generator_facets`].
    pub fn with_contract_generator_facets<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractGeneratorFacetSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractGeneratorFacetSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut local_generators = Vec::new();
    let mut scenario_requirements = Vec::new();
    let coverage = read::visit_selected(forge, selection, cancel, |record, tree| {
        let admission = RecordObservation::capture(&input, record)?;
        let scope = input.record_scope(record, tree)?;
        let fields = OriginalFlowFields::read(&scope, cancel)?;
        let capture = TemplateFieldCapture::new(&fields, &input, &admission)?;
        let rows = contract_generator_facets::read(&input, tree, &fields, &capture, Vec::new())?;
        let unreadable = fields
            .facts()
            .iter()
            .any(|fact| matches!(fact.value(), Value::Error(_)));
        let incomplete = unreadable
            || rows
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection());
        local_generators.extend(rows.local_generators);
        scenario_requirements.extend(rows.scenario_requirements);
        Ok(incomplete)
    })?;
    let local = local_contract_generator_configuration::table(forge, local_generators);
    let scenarios = contract_scenario_requirements::table(forge, scenario_requirements);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ContractGeneratorFacetSnapshot {
        build: archive.build().cloned(),
        local_generator_availability: local.availability,
        local_generators: local.rows,
        scenario_requirement_availability: scenarios.availability,
        scenario_requirements: scenarios.rows,
        coverage,
        source_inventory,
    }))
}
