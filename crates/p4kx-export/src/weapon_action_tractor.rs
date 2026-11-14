// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// Stored force, distance, volume and angle inputs for a tractor beam action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TractorBeamAction<'source> {
    min_force: SourcedValue<ResourceFloat32>,
    max_force: SourcedValue<ResourceFloat32>,
    min_distance: SourcedValue<ResourceFloat32>,
    max_distance: SourcedValue<ResourceFloat32>,
    full_strength_distance: SourcedValue<ResourceFloat32>,
    max_volume: SourcedValue<ResourceFloat32>,
    max_angle: SourcedValue<ResourceFloat32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> TractorBeamAction<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        recoil: [(&'static str, SourceValueState); 3],
    ) -> Result<Self, CatalogError> {
        let min_force = source.single("minForce", notes)?;
        let max_force = source.single("maxForce", notes)?;
        let min_distance = source.single("minDistance", notes)?;
        let max_distance = source.single("maxDistance", notes)?;
        let full_strength_distance = source.single("fullStrengthDistance", notes)?;
        let max_volume = source.single("maxVolume", notes)?;
        let max_angle = source.single("maxAngle", notes)?;
        let authored = source.authored_members_except(
            [
                ("minForce", min_force.state()),
                ("maxForce", max_force.state()),
                ("minDistance", min_distance.state()),
                ("maxDistance", max_distance.state()),
                ("fullStrengthDistance", full_strength_distance.state()),
                ("maxVolume", max_volume.state()),
                ("maxAngle", max_angle.state()),
            ]
            .into_iter()
            .chain(recoil),
        )?;
        Ok(Self {
            min_force,
            max_force,
            min_distance,
            max_distance,
            full_strength_distance,
            max_volume,
            max_angle,
            authored,
        })
    }
}
