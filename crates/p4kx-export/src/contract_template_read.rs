// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_rows::mission_definitions::{MissionDefinitionOutput, MissionDefinitionWriter};
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_field_inspection::WholeInspectionFold;
use crate::export_ledger::ExportLedger;
use crate::record_read::{KindReadAccounting, visit_kind_by_record_guid};
use crate::spawn_pools::SpawnPools;
use crate::template_canonical_read;
use crate::template_issues::Unknowns;
use crate::template_rows::TemplateConfigurations;
use crate::{CatalogError, Kind};

/// Physical Template definitions and their required context rows from the same original reads.
pub(crate) struct ContractTemplates<'source> {
    pub(crate) read: KindReadAccounting<'source>,
    pub(crate) definitions: MissionDefinitionOutput,
    pub(crate) configurations: TemplateConfigurations<'source>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) unknowns: Unknowns,
}

/// Reads selected physical Template owners and their separate contextual interpretations.
///
/// # Errors
/// Returns cancellation, original-input or source-accounting failures.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    tags: &CapturedTags<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    inspections: &mut WholeInspectionFold<'_>,
) -> Result<ContractTemplates<'source>, CatalogError> {
    let unknowns = Unknowns::default();
    let scratch = std::env::temp_dir();
    let mut definitions = MissionDefinitionWriter::new(Kind::ContractTemplates, &scratch, cancel);
    let mut configurations = TemplateConfigurations::default();
    let mut pools = SpawnPools::default();
    let mut context_ledger = ExportLedger::default();
    let mut context_failures = Vec::new();
    let mut read = visit_kind_by_record_guid(
        forge,
        projection.original(),
        Kind::ContractTemplates,
        (filter, cancel, Some(inspections)),
        |record, tree| {
            template_canonical_read::read(projection, tags, record, tree, cancel, &unknowns)
                .map_err(|error| Box::new(error) as crate::record_read::ReadError)
        },
        |built| {
            definitions.append(built.definition, |_| built.definition_source)?;
            configurations.templates.push(built.projection);
            configurations.flows.extend(built.flows);
            configurations.properties.extend(built.properties);
            pools.merge(built.pools);
            context_ledger.merge(built.ledger);
            context_failures.extend(built.failures);
            Ok(())
        },
    )?;
    let definitions = definitions.finish(forge)?;
    read.ledger.merge(context_ledger);
    read.failures.extend(context_failures);
    Ok(ContractTemplates {
        read,
        definitions,
        configurations,
        pools,
        unknowns,
    })
}
