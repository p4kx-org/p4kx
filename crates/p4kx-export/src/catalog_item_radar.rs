// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const COMPONENT: &str = "SCItemRadarComponentParams";

/// One original radar component installed on a physical entity record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ItemRadar<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    aim_assist: SourcedValue<AimAssist<'source>>,
    ping_properties: SourcedValue<PingProperties<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> ItemRadar<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let aim_assist =
            source.declared_class_to("aimAssist", "SRadarAimAssistData", notes, AimAssist::read)?;
        let ping_properties = source.declared_class_to(
            "pingProperties",
            "SCItemRadarPingProperties",
            notes,
            PingProperties::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("aimAssist", aim_assist.state()),
                ("pingProperties", ping_properties.state()),
            ],
        )?;
        Ok(Self {
            owner,
            aim_assist,
            ping_properties,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AimAssist<'source> {
    distance_min_assignment: SourcedValue<ResourceFloat32>,
    distance_max_assignment: SourcedValue<ResourceFloat32>,
    outside_range_buffer_distance: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> AimAssist<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let distance_min_assignment = source.single("distanceMinAssignment", notes)?;
        let distance_max_assignment = source.single("distanceMaxAssignment", notes)?;
        let outside_range_buffer_distance = source.single("outsideRangeBufferDistance", notes)?;
        let original = UnitSource::read(
            source,
            [
                ("distanceMinAssignment", distance_min_assignment.state()),
                ("distanceMaxAssignment", distance_max_assignment.state()),
                (
                    "outsideRangeBufferDistance",
                    outside_range_buffer_distance.state(),
                ),
            ],
        )?;
        Ok(Self {
            distance_min_assignment,
            distance_max_assignment,
            outside_range_buffer_distance,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PingProperties<'source> {
    cooldown_time: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> PingProperties<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let cooldown_time = source.single("cooldownTime", notes)?;
        let original = UnitSource::read(source, [("cooldownTime", cooldown_time.state())])?;
        Ok(Self {
            cooldown_time,
            original,
        })
    }
}
