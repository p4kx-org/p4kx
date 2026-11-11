// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::record_read::{KindRead, read_kind};
use crate::spawn_pools::SpawnPools;
use crate::template_canonical_read;
use crate::template_definition::TemplateDefinition;
use crate::template_issues::Unknowns;
use crate::template_rows::TemplateConfigurations;
use crate::{CatalogError, Kind};

/// Physical Template definitions and their required context rows from the same original reads.
pub(crate) struct ContractTemplates<'source> {
    pub(crate) read: KindRead<'source, TemplateDefinition<'source>>,
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
) -> Result<ContractTemplates<'source>, CatalogError> {
    let unknowns = Unknowns::default();
    let read = read_kind(
        forge,
        projection.original(),
        Kind::ContractTemplates,
        (filter, cancel),
        |record, tree| {
            template_canonical_read::read(projection, tags, record, tree, cancel, &unknowns)
                .map_err(|error| Box::new(error) as crate::record_read::ReadError)
        },
        |built| built.definition.physical_guid,
    )?;
    let KindRead {
        items,
        skipped,
        mut failures,
        mut ledger,
    } = read;
    let mut definitions = Vec::with_capacity(items.len());
    let mut configurations = TemplateConfigurations::default();
    let mut pools = SpawnPools::default();
    for built in items {
        definitions.push(built.definition);
        configurations.templates.push(built.projection);
        configurations.flows.extend(built.flows);
        configurations.properties.extend(built.properties);
        pools.merge(built.pools);
        ledger.merge(built.ledger);
        failures.extend(built.failures);
    }
    Ok(ContractTemplates {
        read: KindRead {
            items: definitions,
            skipped,
            failures,
            ledger,
        },
        configurations,
        pools,
        unknowns,
    })
}
