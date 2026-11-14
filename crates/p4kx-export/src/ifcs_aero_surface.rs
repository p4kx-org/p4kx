// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::{InventoryVector2, InventoryVector3};
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsAeroSurface<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    critical_angle: SourcedValue<ResourceFloat32>,
    #[schemars(extend("x-gameText" = true))]
    detach_parent_part_name: SourcedValue<&'source str>,
    flat_plate_multiplier: SourcedValue<ResourceFloat32>,
    flat_plate_surface: SourcedValue<bool>,
    fully_stalled_angle: SourcedValue<ResourceFloat32>,
    lift_level_flight: SourcedValue<ResourceFloat32>,
    min_lift: SourcedValue<ResourceFloat32>,
    #[schemars(extend("x-gameText" = true))]
    name: SourcedValue<&'source str>,
    negative_ao_a_multiplier: SourcedValue<ResourceFloat32>,
    offset: SourcedValue<InventoryVector3<'source>>,
    orientation: SourcedValue<InventoryVector3<'source>>,
    peak_lift: SourcedValue<ResourceFloat32>,
    size: SourcedValue<InventoryVector2<'source>>,
    torque_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[schemars(extend("x-gameText" = true))]
    transform_parent_part_name: SourcedValue<&'source str>,
}

impl<'source> IfcsAeroSurface<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let critical_angle = fields.single("criticalAngle", notes)?;
        let detach_parent_part_name = fields.borrowed_string("detachParentPartName", notes)?;
        let flat_plate_multiplier = fields.single("flatPlateMultiplier", notes)?;
        let flat_plate_surface = fields.boolean("flatPlateSurface", notes)?;
        let fully_stalled_angle = fields.single("fullyStalledAngle", notes)?;
        let lift_level_flight = fields.single("liftLevelFlight", notes)?;
        let min_lift = fields.single("minLift", notes)?;
        let name = fields.borrowed_string("name", notes)?;
        let negative_ao_a_multiplier = fields.single("negativeAoAMultiplier", notes)?;
        let offset = fields.declared_class("offset", notes, InventoryVector3::read)?;
        let orientation = fields.declared_class("orientation", notes, InventoryVector3::read)?;
        let peak_lift = fields.single("peakLift", notes)?;
        let size = fields.declared_class("size", notes, InventoryVector2::read)?;
        let torque_multiplier =
            fields.declared_class("torqueMultiplier", notes, InventoryVector3::read)?;
        let transform_parent_part_name =
            fields.borrowed_string("transformParentPartName", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("criticalAngle", critical_angle.state()),
                ("detachParentPartName", detach_parent_part_name.state()),
                ("flatPlateMultiplier", flat_plate_multiplier.state()),
                ("flatPlateSurface", flat_plate_surface.state()),
                ("fullyStalledAngle", fully_stalled_angle.state()),
                ("liftLevelFlight", lift_level_flight.state()),
                ("minLift", min_lift.state()),
                ("name", name.state()),
                ("negativeAoAMultiplier", negative_ao_a_multiplier.state()),
                ("offset", offset.state()),
                ("orientation", orientation.state()),
                ("peakLift", peak_lift.state()),
                ("size", size.state()),
                ("torqueMultiplier", torque_multiplier.state()),
                (
                    "transformParentPartName",
                    transform_parent_part_name.state(),
                ),
            ],
        )?;
        Ok(Self {
            source,
            critical_angle,
            detach_parent_part_name,
            flat_plate_multiplier,
            flat_plate_surface,
            fully_stalled_angle,
            lift_level_flight,
            min_lift,
            name,
            negative_ao_a_multiplier,
            offset,
            orientation,
            peak_lift,
            size,
            torque_multiplier,
            transform_parent_part_name,
        })
    }
}
