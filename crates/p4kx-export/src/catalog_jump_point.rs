// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

#[path = "jump_point_effects.rs"]
mod effects;

use effects::JumpPointEffects;

pub(super) const COMPONENT: &str = "SJumpPointParams";

/// Settings stored on one physical jump-point Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpPointConfiguration<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    required_fuel: SourcedValue<ResourceFloat32>,
    linking_range: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    largest_ship_size: SourcedValue<Box<SourceRecordReference<'source>>>,
    jump_gate_super_guid: SourcedValue<&'source str>,
    push_area_radar_geometry_name: SourcedValue<&'source str>,
    entrance_push_area_params: SourcedValue<EntrancePushArea<'source>>,
    effect_params: SourcedValue<JumpPointEffects<'source>>,
}

impl<'source> JumpPointConfiguration<'source> {
    /// Borrows the original Entity and Components position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the authored fuel requirement.
    #[must_use]
    pub const fn required_fuel(&self) -> &SourcedValue<ResourceFloat32> {
        &self.required_fuel
    }

    /// Borrows the authored linking range.
    #[must_use]
    pub const fn linking_range(&self) -> &SourcedValue<ResourceFloat32> {
        &self.linking_range
    }

    pub(super) fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let required_fuel = fields.single("requiredFuel", notes)?;
        let linking_range = fields.single("linkingRange", notes)?;
        let largest_ship_size =
            fields.record_link_to("largestShipSize", "LandingPadSize", notes)?;
        let jump_gate_super_guid = fields.borrowed_string("jumpGateSuperGuid", notes)?;
        let push_area_radar_geometry_name =
            fields.borrowed_string("pushAreaRadarGeometryName", notes)?;
        let entrance_push_area_params = fields.declared_class_to(
            "entrancePushAreaParams",
            "SJumpPointPushAreaParams",
            notes,
            EntrancePushArea::read,
        )?;
        let effect_params = fields.declared_class_to(
            "effectParams",
            "JumpPointEffectParams",
            notes,
            JumpPointEffects::read,
        )?;
        let source = UnitSource::read(
            fields,
            [
                ("requiredFuel", required_fuel.state()),
                ("linkingRange", linking_range.state()),
                ("largestShipSize", largest_ship_size.state()),
                ("jumpGateSuperGuid", jump_gate_super_guid.state()),
                (
                    "pushAreaRadarGeometryName",
                    push_area_radar_geometry_name.state(),
                ),
                ("entrancePushAreaParams", entrance_push_area_params.state()),
                ("effectParams", effect_params.state()),
            ],
        )?;
        Ok(Self {
            owner,
            source,
            required_fuel,
            linking_range,
            largest_ship_size,
            jump_gate_super_guid,
            push_area_radar_geometry_name,
            entrance_push_area_params,
            effect_params,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EntrancePushArea<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    area_radius: SourcedValue<ResourceFloat32>,
    soft_push_area_distance: SourcedValue<ResourceFloat32>,
    strength: SourcedValue<ResourceFloat32>,
    damping: SourcedValue<ResourceFloat32>,
    falloff_ratio: SourcedValue<ResourceFloat32>,
}

impl<'source> EntrancePushArea<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let area_radius = fields.single("areaRadius", notes)?;
        let soft_push_area_distance = fields.single("softPushAreaDistance", notes)?;
        let strength = fields.single("strength", notes)?;
        let damping = fields.single("damping", notes)?;
        let falloff_ratio = fields.single("falloffRatio", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("areaRadius", area_radius.state()),
                ("softPushAreaDistance", soft_push_area_distance.state()),
                ("strength", strength.state()),
                ("damping", damping.state()),
                ("falloffRatio", falloff_ratio.state()),
            ],
        )?;
        Ok(Self {
            source,
            area_radius,
            soft_push_area_distance,
            strength,
            damping,
            falloff_ratio,
        })
    }
}
