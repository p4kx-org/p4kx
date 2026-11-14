// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_independent_configurations::ordered_curve::OrderedCurve;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[path = "ifcs_precision.rs"]
mod ifcs_precision;

#[path = "ifcs_dynamics.rs"]
mod ifcs_dynamics;

#[path = "ifcs_afterburner.rs"]
mod ifcs_afterburner;

#[path = "ifcs_environment.rs"]
mod ifcs_environment;

#[path = "ifcs_aero_surface.rs"]
mod ifcs_aero_surface;

#[path = "ifcs_systems.rs"]
mod ifcs_systems;

#[path = "ifcs_fuel.rs"]
mod ifcs_fuel;

use ifcs_dynamics::IfcsDynamics;
use ifcs_environment::IfcsEnvironment;
use ifcs_precision::{IfcsLimits, IfcsPrecision};
use ifcs_systems::IfcsSystems;

pub(in crate::catalog_v3) const COMPONENT: &str = "IFCSParams";

/// The original flight-control configuration of a physical Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct Ifcs<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    limits: IfcsLimits<'source>,
    precision: IfcsPrecision<'source>,
    dynamics: IfcsDynamics<'source>,
    environment: IfcsEnvironment<'source>,
    systems: IfcsSystems<'source>,
}

impl<'source> Ifcs<'source> {
    pub(in crate::catalog_v3) fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let limits = IfcsLimits::read(fields, notes)?;
        let precision = IfcsPrecision::read(fields, notes)?;
        let dynamics = IfcsDynamics::read(fields, notes)?;
        let environment = IfcsEnvironment::read(fields, notes)?;
        let systems = IfcsSystems::read(fields, notes)?;
        let source = UnitSource::read(
            fields,
            limits
                .projected()
                .into_iter()
                .chain(precision.projected())
                .chain(dynamics.projected())
                .chain(environment.projected())
                .chain(systems.projected()),
        )?;
        Ok(Self {
            owner,
            source,
            limits,
            precision,
            dynamics,
            environment,
            systems,
        })
    }

    pub(in crate::catalog_v3) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

#[cfg(test)]
#[path = "ifcs_tests.rs"]
mod tests;
