// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceWeakReference};

pub(super) const SOURCE_TYPE: &str = "SCItemSeatEjectParams";
const INTERACTION: &str = "SSharedInteractionParams";
const DEGREE_ANGLES: &str = "Deg3";

/// The original ejection motion parameters, interaction and offset.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SeatEjection<'source> {
    source_type: SourcedValue<&'source str>,
    max_linear_velocity: SourcedValue<ResourceFloat32>,
    max_linear_acceleration: SourcedValue<ResourceFloat32>,
    max_angular_velocity: SourcedValue<ResourceFloat32>,
    max_angular_acceleration: SourcedValue<ResourceFloat32>,
    ejection_loop_time: SourcedValue<ResourceFloat32>,
    ejection_interaction: SourcedValue<Box<SourceWeakReference<'source>>>,
    offset_angles: SourcedValue<SeatEjectionAngles<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> SeatEjection<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let max_linear_velocity = source.single("maxLinearVelocity", notes)?;
        let max_linear_acceleration = source.single("maxLinearAcceleration", notes)?;
        let max_angular_velocity = source.single("maxAngularVelocity", notes)?;
        let max_angular_acceleration = source.single("maxAngularAcceleration", notes)?;
        let ejection_loop_time = source.single("ejectionLoopTime", notes)?;
        let ejection_interaction =
            source.weak_link_to("ejectionInteraction", INTERACTION, notes)?;
        let offset_angles = source.declared_class_to(
            "offsetAngles",
            DEGREE_ANGLES,
            notes,
            SeatEjectionAngles::read,
        )?;
        let additional_fields = source.authored_members_except([
            ("maxLinearVelocity", max_linear_velocity.state()),
            ("maxLinearAcceleration", max_linear_acceleration.state()),
            ("maxAngularVelocity", max_angular_velocity.state()),
            ("maxAngularAcceleration", max_angular_acceleration.state()),
            ("ejectionLoopTime", ejection_loop_time.state()),
            ("ejectionInteraction", ejection_interaction.state()),
            ("offsetAngles", offset_angles.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            max_linear_velocity,
            max_linear_acceleration,
            max_angular_velocity,
            max_angular_acceleration,
            ejection_loop_time,
            ejection_interaction,
            offset_angles,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SeatEjectionAngles<'source> {
    source_type: SourcedValue<&'source str>,
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
    z: SourcedValue<ResourceFloat32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> SeatEjectionAngles<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let x = source.single("x", notes)?;
        let y = source.single("y", notes)?;
        let z = source.single("z", notes)?;
        let additional_fields = source.authored_members_except([
            ("x", x.state()),
            ("y", y.state()),
            ("z", z.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            x,
            y,
            z,
            additional_fields,
        })
    }
}
