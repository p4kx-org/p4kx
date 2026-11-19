// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// Original collision warning time and speed settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CollisionDetection<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original collision danger close warn time field.
    pub collision_danger_close_warn_time: SourcedValue<ResourceFloat32>,
    /// Original collision warn speed field.
    pub collision_warn_speed: SourcedValue<ResourceFloat32>,
    /// Original collision warn time field.
    pub collision_warn_time: SourcedValue<ResourceFloat32>,
}

impl<'source> CollisionDetection<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let collision_danger_close_warn_time =
            fields.single("collisionDangerCloseWarnTime", notes)?;
        let collision_warn_speed = fields.single("collisionWarnSpeed", notes)?;
        let collision_warn_time = fields.single("collisionWarnTime", notes)?;
        let source = UnitSource::read(
            fields,
            [
                (
                    "collisionDangerCloseWarnTime",
                    collision_danger_close_warn_time.state(),
                ),
                ("collisionWarnSpeed", collision_warn_speed.state()),
                ("collisionWarnTime", collision_warn_time.state()),
            ],
        )?;
        Ok(Self {
            source,
            collision_danger_close_warn_time,
            collision_warn_speed,
            collision_warn_time,
        })
    }
}

/// Original physical recall parameters and its referenced AI module tag.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ShipRecall<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    #[serde(rename = "AIModuleTag")]
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original ai module tag field.
    pub ai_module_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "BrakingDistanceOffset")]
    /// Original braking distance offset field.
    pub braking_distance_offset: SourcedValue<ResourceFloat32>,
    #[serde(rename = "DefaultPlatformDetectionRange")]
    /// Original default platform detection range field.
    pub default_platform_detection_range: SourcedValue<ResourceFloat32>,
    #[serde(rename = "ForwardOffset")]
    /// Original forward offset field.
    pub forward_offset: SourcedValue<ResourceFloat32>,
    #[serde(rename = "HoverHeightAtDestination")]
    /// Original hover height at destination field.
    pub hover_height_at_destination: SourcedValue<ResourceFloat32>,
    #[serde(rename = "MinimumRecallDistance")]
    /// Original minimum recall distance field.
    pub minimum_recall_distance: SourcedValue<ResourceFloat32>,
    #[serde(rename = "ObstructionDetectionRange")]
    /// Original obstruction detection range field.
    pub obstruction_detection_range: SourcedValue<ResourceFloat32>,
}

impl<'source> ShipRecall<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let ai_module_tag = fields.record_link("AIModuleTag", notes)?;
        let braking_distance_offset = fields.single("BrakingDistanceOffset", notes)?;
        let default_platform_detection_range =
            fields.single("DefaultPlatformDetectionRange", notes)?;
        let forward_offset = fields.single("ForwardOffset", notes)?;
        let hover_height_at_destination = fields.single("HoverHeightAtDestination", notes)?;
        let minimum_recall_distance = fields.single("MinimumRecallDistance", notes)?;
        let obstruction_detection_range = fields.single("ObstructionDetectionRange", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("AIModuleTag", ai_module_tag.state()),
                ("BrakingDistanceOffset", braking_distance_offset.state()),
                (
                    "DefaultPlatformDetectionRange",
                    default_platform_detection_range.state(),
                ),
                ("ForwardOffset", forward_offset.state()),
                (
                    "HoverHeightAtDestination",
                    hover_height_at_destination.state(),
                ),
                ("MinimumRecallDistance", minimum_recall_distance.state()),
                (
                    "ObstructionDetectionRange",
                    obstruction_detection_range.state(),
                ),
            ],
        )?;
        Ok(Self {
            source,
            ai_module_tag,
            braking_distance_offset,
            default_platform_detection_range,
            forward_offset,
            hover_height_at_destination,
            minimum_recall_distance,
            obstruction_detection_range,
        })
    }
}
