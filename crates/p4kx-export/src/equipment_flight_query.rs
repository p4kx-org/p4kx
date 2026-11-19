// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::component_sources::flight::{Controllers, IfcsComponents, Thrusters};

pub use crate::catalog_v3::catalog_independent_configurations::OrderedCurve;
pub use crate::catalog_v3::component_sources::flight::{
    CollisionDetection, CurvePoint, DegradationCurve, FlightController,
    FlightControllerInteractions, FlightControllerSettings, Gimbal, GimbalAxis, GimbalOverride,
    HealthLink, Ifcs, IfcsAcceleration, IfcsAeroSurface, IfcsAfterburner, IfcsAtmosphericFlight,
    IfcsBurnCapacitor, IfcsBurnResponse, IfcsBurnTiming, IfcsCore, IfcsDamageHandling,
    IfcsDynamics, IfcsEnvironment, IfcsFuelUsage, IfcsJerk, IfcsLimits, IfcsMotionLimits,
    IfcsNoFuel, IfcsPartDamage, IfcsPrecision, IfcsSpeedProfile, IfcsSystems,
    IfcsThrusterEfficiency, IfcsTurbulence, IntegrityCurve, ResourceFlowUnit, ShipRecall,
    SignatureEmitter, Thruster, ThrusterBackwash, ThrusterOperating,
};

equipment_component_query!(
    Controllers,
    FlightController,
    FlightControllerSnapshot,
    with_flight_controllers,
    with_flight_controllers_from
);
equipment_component_query!(
    IfcsComponents,
    Ifcs,
    IfcsSnapshot,
    with_ifcs,
    with_ifcs_from
);
equipment_component_query!(
    Thrusters,
    Thruster,
    ThrusterSnapshot,
    with_thrusters,
    with_thrusters_from
);
