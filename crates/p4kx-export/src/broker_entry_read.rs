// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::CatalogError;
use crate::broker_canonical_read::{self, BrokerEmission, CanonicalBroker, Sources};
use crate::broker_definition::BrokerDefinition;
use crate::broker_entry::BrokerProjection;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::mission_index::MissionIndex;
use crate::record_read::{KindRead, read_kind};
use crate::selection::Kind;
use crate::spawn_pools::SpawnPools;
use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_issues::Unknowns;

/// Source definitions, contextual rows, and the associated mission content.
pub(crate) struct BrokerEntries<'source> {
    pub(crate) read: KindRead<'source, BrokerProjection<'source>>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) definitions: Vec<BrokerDefinition<'source>>,
    pub(crate) flows: Vec<MissionFlowConfiguration>,
    pub(crate) properties: Vec<MissionPropertyConfiguration>,
}

/// Reads the selected physical Broker records and their separate contextual interpretations.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    (tags, index, unknowns): (
        &CapturedTags<'_, 'source>,
        &MissionIndex<'source>,
        &Unknowns,
    ),
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<BrokerEntries<'source>, CatalogError> {
    let sources = Sources {
        projection,
        tags,
        index,
        unknowns,
    };
    let read = read_kind(
        forge,
        projection.original(),
        Kind::MissionBrokerEntries,
        (filter, cancel),
        |record, tree| {
            broker_canonical_read::read(&sources, record, tree, cancel)
                .map_err(|error| Box::new(error) as crate::record_read::ReadError)
        },
        |piece| piece.definition.physical_guid,
    )?;
    Ok(collect(read))
}

fn collect<'source>(read: KindRead<'source, CanonicalBroker<'source>>) -> BrokerEntries<'source> {
    let KindRead {
        items,
        skipped,
        mut failures,
        mut ledger,
    } = read;
    crate::mission_pool_ledger::broker(&mut ledger);
    let mut pools = SpawnPools::default();
    let mut definitions = Vec::new();
    let mut flows = Vec::new();
    let mut properties = Vec::new();
    let mut entries = Vec::new();
    for piece in items {
        definitions.push(piece.definition);
        flows.extend(piece.flows);
        properties.extend(piece.properties);
        match piece.projection {
            Ok(BrokerEmission {
                projection,
                pools: found,
                ledger: observations,
                failures: sources,
            }) => {
                entries.push(projection);
                pools.merge(found);
                ledger.merge(observations);
                failures.extend(sources);
            }
            Err(failed) => {
                ledger.merge(failed.ledger);
                failures.push(failed.failure);
            }
        }
    }
    BrokerEntries {
        read: KindRead {
            items: entries,
            skipped,
            failures,
            ledger,
        },
        pools,
        definitions,
        flows,
        properties,
    }
}
