// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::broker_canonical_read::{self, BrokerEmission, CanonicalBroker, Sources};
use crate::broker_entry::BrokerProjection;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_row::CatalogRow;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_field_inspection::WholeInspectionFold;
use crate::export_ledger::ExportLedger;
use crate::mission_index::MissionIndex;
use crate::record_read::{KindRead, KindReadAccounting, visit_kind_by_record_guid};
use crate::selection::Kind;
use crate::spawn_pools::SpawnPools;
use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_issues::Unknowns;
use crate::template_number_capture::TemplateNumbers;
use crate::{CatalogError, CatalogReadFailure};

/// Source definitions, contextual rows, and the associated mission content.
pub(crate) struct BrokerEntries<'source> {
    pub(crate) read: KindRead<'source, BrokerProjection<'source>>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) definitions: BrokerDefinitions,
    pub(crate) flows: Vec<MissionFlowConfiguration>,
    pub(crate) properties: Vec<MissionPropertyConfiguration>,
}

/// Proof retained from a successfully staged canonical Broker owner.
pub(crate) struct BrokerDefinitionReceipt {
    pub(crate) row: CatalogRow,
    pub(crate) numbers: TemplateNumbers,
}

pub(crate) struct BrokerDefinitions {
    pub(crate) table: StagedCatalogTable,
    pub(crate) proof: Vec<BrokerDefinitionReceipt>,
}

impl BrokerDefinitions {
    pub(crate) const fn empty() -> Self {
        Self {
            table: StagedCatalogTable::empty(DatasetAvailability::Absent),
            proof: Vec::new(),
        }
    }
}

struct BrokerRows<'stage, 'source> {
    definitions: StagedCatalogRows<'stage>,
    proof: Vec<BrokerDefinitionReceipt>,
    projections: Vec<BrokerProjection<'source>>,
    flows: Vec<MissionFlowConfiguration>,
    properties: Vec<MissionPropertyConfiguration>,
    pools: SpawnPools<'source>,
    ledger: ExportLedger,
    failures: Vec<CatalogReadFailure<'source>>,
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
    inspections: &mut WholeInspectionFold<'_>,
) -> Result<BrokerEntries<'source>, CatalogError> {
    let sources = Sources {
        projection,
        tags,
        index,
        unknowns,
    };
    let scratch = std::env::temp_dir();
    let mut rows = BrokerRows {
        definitions: StagedCatalogRows::new(&scratch, cancel),
        proof: Vec::new(),
        projections: Vec::new(),
        flows: Vec::new(),
        properties: Vec::new(),
        pools: SpawnPools::default(),
        ledger: ExportLedger::default(),
        failures: Vec::new(),
    };
    let accounting = visit_kind_by_record_guid(
        forge,
        projection.original(),
        Kind::MissionBrokerEntries,
        (filter, cancel, Some(inspections)),
        |record, tree| {
            broker_canonical_read::read(&sources, record, tree, cancel)
                .map_err(|error| Box::new(error) as crate::record_read::ReadError)
        },
        |piece| rows.append(piece),
    )?;
    rows.finish(forge, accounting)
}

impl<'source> BrokerRows<'_, 'source> {
    fn append(&mut self, piece: CanonicalBroker<'source>) -> Result<(), CatalogError> {
        let definition = piece.definition;
        self.definitions.append(&definition)?;
        self.proof.push(BrokerDefinitionReceipt {
            row: CatalogRow {
                kind: Kind::MissionBrokerEntries,
                guid: definition.physical_guid.0,
                record_path: definition.physical_path.to_owned(),
            },
            numbers: definition.numbers,
        });
        self.flows.extend(piece.flows);
        self.properties.extend(piece.properties);
        match piece.projection {
            Ok(BrokerEmission {
                projection,
                pools: found,
                ledger: observations,
                failures: sources,
            }) => {
                self.projections.push(projection);
                self.pools.merge(found);
                self.ledger.merge(observations);
                self.failures.extend(sources);
            }
            Err(failed) => {
                self.ledger.merge(failed.ledger);
                self.failures.push(failed.failure);
            }
        }
        Ok(())
    }

    fn finish(
        self,
        forge: &DataForge<'_>,
        accounting: KindReadAccounting<'source>,
    ) -> Result<BrokerEntries<'source>, CatalogError> {
        let availability = if self.proof.is_empty() {
            DatasetAvailability::from_source_declarations(forge, |name| {
                name == Kind::MissionBrokerEntries.records().type_name
            })
        } else {
            DatasetAvailability::Available
        };
        let definitions = BrokerDefinitions {
            table: self.definitions.finish(availability)?,
            proof: self.proof,
        };
        let mut read = accounting.into_read(self.projections);
        crate::mission_pool_ledger::broker(&mut read.ledger);
        read.ledger.merge(self.ledger);
        read.failures.extend(self.failures);
        Ok(BrokerEntries {
            read,
            pools: self.pools,
            definitions,
            flows: self.flows,
            properties: self.properties,
        })
    }
}
