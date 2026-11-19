// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{record_query, rows};

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
