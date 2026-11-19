// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};

pub use super::authored_link::HealthLink;
pub use super::catalog_entity_component_read::catalog_flight_controller::{
    CollisionDetection, FlightController, Interactions as FlightControllerInteractions,
    OperatingSettings as FlightControllerSettings, ShipRecall,
};
pub use super::catalog_entity_component_read::catalog_ifcs::{
    Ifcs, IfcsAcceleration, IfcsAeroSurface, IfcsAfterburner, IfcsAtmosphericFlight,
    IfcsBurnCapacitor, IfcsBurnResponse, IfcsBurnTiming, IfcsCore, IfcsDamageHandling,
    IfcsDynamics, IfcsEnvironment, IfcsFuelUsage, IfcsJerk, IfcsLimits, IfcsMotionLimits,
    IfcsNoFuel, IfcsPartDamage, IfcsPrecision, IfcsSpeedProfile, IfcsSystems,
    IfcsThrusterEfficiency, IfcsTurbulence,
};
pub use super::catalog_entity_component_read::catalog_thruster::{
    Backwash as ThrusterBackwash, Gimbal, GimbalAxis, GimbalOverride, IntegrityCurve,
    Operating as ThrusterOperating, SignatureEmitter, Thruster,
};
pub use super::catalog_resource_flow_unit::ResourceFlowUnit;
pub use super::degradation_curve::{CurvePoint, DegradationCurve};

use super::catalog_entity_component_read;
use super::catalog_independent_configurations::flight_configuration_tables::{
    FLIGHT_CONTROLLERS, IFCS, THRUSTERS,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_table::CatalogTable;

pub(crate) trait FlightComponent {
    type Row<'source>;

    fn query<'source>(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        owners: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<(CatalogTable<Self::Row<'source>>, CatalogRecordCoverage), CatalogError>;
}

macro_rules! flight_component {
    ($name:ident, $row:ident, $kind:ident) => {
        pub(crate) struct $name;

        impl FlightComponent for $name {
            type Row<'source> = $row<'source>;

            fn query<'source>(
                forge: &'source DataForge<'source>,
                input: &OriginalCatalogInput<'_, 'source>,
                owners: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
            ) -> Result<(CatalogTable<Self::Row<'source>>, CatalogRecordCoverage), CatalogError>
            {
                let (rows, coverage) = catalog_entity_component_read::read_selected(
                    forge,
                    input,
                    owners,
                    cancel,
                    &$kind,
                    $row::read,
                )?;
                Ok((
                    CatalogTable::new(forge, &[$kind.source_type], rows),
                    coverage,
                ))
            }
        }
    };
}

flight_component!(Controllers, FlightController, FLIGHT_CONTROLLERS);
flight_component!(IfcsComponents, Ifcs, IFCS);
flight_component!(Thrusters, Thruster, THRUSTERS);
