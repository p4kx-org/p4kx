// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub use super::super::authored_link::HealthLink;
pub use super::super::catalog_entity_component_read::catalog_flight_controller::{
    CollisionDetection, FlightController, Interactions as FlightControllerInteractions,
    OperatingSettings as FlightControllerSettings, ShipRecall,
};
pub use super::super::catalog_entity_component_read::catalog_ifcs::{
    Ifcs, IfcsAcceleration, IfcsAeroSurface, IfcsAfterburner, IfcsAtmosphericFlight,
    IfcsBurnCapacitor, IfcsBurnResponse, IfcsBurnTiming, IfcsCore, IfcsDamageHandling,
    IfcsDynamics, IfcsEnvironment, IfcsFuelUsage, IfcsJerk, IfcsLimits, IfcsMotionLimits,
    IfcsNoFuel, IfcsPartDamage, IfcsPrecision, IfcsSpeedProfile, IfcsSystems,
    IfcsThrusterEfficiency, IfcsTurbulence,
};
pub use super::super::catalog_entity_component_read::catalog_thruster::{
    Backwash as ThrusterBackwash, Gimbal, GimbalAxis, GimbalOverride, IntegrityCurve,
    Operating as ThrusterOperating, SignatureEmitter, Thruster,
};
pub use super::super::catalog_resource_flow_unit::ResourceFlowUnit;
pub use super::super::degradation_curve::{CurvePoint, DegradationCurve};

use super::super::catalog_independent_configurations::flight_configuration_tables::{
    FLIGHT_CONTROLLERS, IFCS, THRUSTERS,
};
use super::physical_component;

physical_component!(Controllers, FlightController, FLIGHT_CONTROLLERS);
physical_component!(IfcsComponents, Ifcs, IFCS);
physical_component!(Thrusters, Thruster, THRUSTERS);
