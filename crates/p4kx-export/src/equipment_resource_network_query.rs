// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::rows::{Family, Projector, ReadOptions};
use super::{record_query, rows};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_resource_network_read::resource_network_global;
use crate::record_observation::RecordObservation;

pub use resource_network_global::{
    DefaultPowerDistribution, PowerModifier, PowerParameters, ResourceNetworkGlobal,
};

pub use crate::catalog_v3::catalog_entity_configurations::{
    CompositionValue, ConsumptionFields, DeltaSource, HealthLink, HeatCooling, HeatPerformance,
    HeatTransfer, LinkedInteraction, ParameterSource, PowerMisfires, PowerPerformance,
    PowerPresentation, PowerRange, PowerSupply, ResourceComponent, ResourceComposition,
    ResourceConnection, ResourceConnectionFields, ResourceControlParameter, ResourceControls,
    ResourceDelta, ResourceDeltaAmount, ResourceFlowUnit, ResourceHeatConnection,
    ResourcePowerConnection, ResourcePowerRanges, ResourceSignatureParams, ResourceState,
    SelfRepair, SignatureEntry,
};

record_query::unlocalized!(
    rows::ResourceComponentRows,
    ResourceComponent,
    /// Original resource component rows and their original evidence.
    ResourceComponentSnapshot,
    with_resource_components,
    with_resource_components_from
);

record_query::unlocalized!(
    rows::ResourceConnectionRows,
    ResourceConnection,
    /// Original power and heat connection rows and their original evidence.
    ResourceConnectionSnapshot,
    with_resource_connections,
    with_resource_connections_from
);

struct GlobalRows;

impl Projector for GlobalRows {
    type Row<'source> = ResourceNetworkGlobal<'source>;
    const FAMILY: Family = Family::ResourceNetworkGlobals;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = resource_network_global::read_selected(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row]))
    }
}

record_query::unlocalized!(
    GlobalRows,
    ResourceNetworkGlobal,
    /// Selected physical global resource-network definitions and their source evidence.
    ResourceNetworkGlobalSnapshot,
    with_resource_network_globals,
    with_resource_network_globals_from
);
