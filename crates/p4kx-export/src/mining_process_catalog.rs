// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};

use super::prepared_definition::PreparedDefinition;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, Kind};

#[path = "mining_parameter_blocks.rs"]
mod blocks;
#[path = "mining_global_contract.rs"]
mod globals;
#[path = "refining_process_definition.rs"]
mod refining;

pub use blocks::{MiningExplosion, MiningHitConsistency, MiningInstability};
pub use globals::MiningGlobalConfiguration;
pub use refining::RefiningProcessDefinition;

pub(crate) type PreparedRefining<'source> =
    PreparedDefinition<'source, RefiningProcessDefinition<'source>>;
pub(crate) type PreparedMiningGlobal<'source> =
    PreparedDefinition<'source, MiningGlobalConfiguration<'source>>;

pub(crate) fn read_refining<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedRefining<'source>, Vec<FieldError>), CatalogError> {
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        RefiningProcessDefinition::project,
    )
}

pub(crate) fn read_globals<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedMiningGlobal<'source>, Vec<FieldError>), CatalogError> {
    PreparedDefinition::read(tree, projection, admission, cancel, globals::project)
}

pub(crate) fn order_refining(rows: &mut [PreparedRefining<'_>]) {
    PreparedDefinition::order(rows);
}
pub(crate) fn order_globals(rows: &mut [PreparedMiningGlobal<'_>]) {
    PreparedDefinition::order(rows);
}

pub(crate) fn refining_table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedRefining<'source>>,
) -> CatalogTable<RefiningProcessDefinition<'source>> {
    PreparedDefinition::table(forge, "RefiningProcess", rows)
}

pub(crate) fn globals_table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedMiningGlobal<'source>>,
) -> CatalogTable<MiningGlobalConfiguration<'source>> {
    PreparedDefinition::table(forge, "MiningGlobalParams", rows)
}

pub(crate) fn sources<'rows, 'source>(
    refining: &'rows [PreparedRefining<'source>],
    globals: &'rows [PreparedMiningGlobal<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::RefiningProcesses, refining).chain(
        PreparedDefinition::sources(Kind::MiningGlobalParams, globals),
    )
}
