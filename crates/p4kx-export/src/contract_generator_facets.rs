// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use super::contract_scenario_requirements::{self, ContractScenarioRequirements};
use super::local_contract_generator_configuration::{self, LocalContractGeneratorConfiguration};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::source_fields::{FieldCapture, FieldError};

#[derive(Default)]
pub(crate) struct AuthoredGeneratorFacets<'source> {
    pub(crate) scenario_requirements: Vec<ContractScenarioRequirements<'source>>,
    pub(crate) local_generators: Vec<LocalContractGeneratorConfiguration<'source>>,
    pub(crate) notes: Vec<FieldError>,
}

pub(crate) fn read<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    mut notes: Vec<FieldError>,
) -> Result<AuthoredGeneratorFacets<'source>, CatalogError> {
    let scenario_requirements =
        contract_scenario_requirements::read(input, tree, fields, capture, &mut notes)?;
    let local_generators =
        local_contract_generator_configuration::read(input, tree, fields, capture, &mut notes)?;
    Ok(AuthoredGeneratorFacets {
        scenario_requirements,
        local_generators,
        notes,
    })
}
