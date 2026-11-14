// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::{Label, Single};
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_link::HealthLink;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original Pitch and Yaw gimbal controls without axis substitution.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Gimbal<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    is_flex: SourcedValue<bool>,
    pitch_axis: SourcedValue<GimbalAxis<'source>>,
    yaw_axis: SourcedValue<GimbalAxis<'source>>,
}

impl<'source> Gimbal<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let is_flex = fields.boolean("isFlex", notes)?;
        let pitch_axis = fields.declared_class("pitchAxis", notes, GimbalAxis::read)?;
        let yaw_axis = fields.declared_class("yawAxis", notes, GimbalAxis::read)?;
        let source = UnitSource::read(
            fields,
            [
                ("isFlex", is_flex.state()),
                ("pitchAxis", pitch_axis.state()),
                ("yawAxis", yaw_axis.state()),
            ],
        )?;
        Ok(Self {
            source,
            is_flex,
            pitch_axis,
            yaw_axis,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct GimbalAxis<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    accel: Single,
    angle_max: Single,
    angle_min: Single,
    animation: Label<'source>,
    joint_name: Label<'source>,
    speed: Single,
}

impl<'source> GimbalAxis<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let accel = fields.single("accel", notes)?;
        let angle_max = fields.single("angleMax", notes)?;
        let angle_min = fields.single("angleMin", notes)?;
        let animation = fields.borrowed_string("animation", notes)?;
        let joint_name = fields.borrowed_string("jointName", notes)?;
        let speed = fields.single("speed", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("accel", accel.state()),
                ("angleMax", angle_max.state()),
                ("angleMin", angle_min.state()),
                ("animation", animation.state()),
                ("jointName", joint_name.state()),
                ("speed", speed.state()),
            ],
        )?;
        Ok(Self {
            source,
            accel,
            angle_max,
            angle_min,
            animation,
            joint_name,
            speed,
        })
    }
}

/// One authored interaction-state override kept in its source array slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct GimbalOverride<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    interaction_state: SourcedValue<HealthLink<'source>>,
    gimbal_params: SourcedValue<Gimbal<'source>>,
}

impl<'source> GimbalOverride<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let interaction_state = fields.link("interactionState", notes)?;
        let gimbal_params = fields.declared_class("gimbalParams", notes, Gimbal::read)?;
        let source = UnitSource::read(
            fields,
            [
                ("interactionState", interaction_state.state()),
                ("gimbalParams", gimbal_params.state()),
            ],
        )?;
        Ok(Self {
            source,
            interaction_state,
            gimbal_params,
        })
    }
}
