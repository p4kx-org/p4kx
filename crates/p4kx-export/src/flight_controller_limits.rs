// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::{RecordLink, Single};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original collision warning time and speed settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct CollisionDetection<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    collision_danger_close_warn_time: Single,
    collision_warn_speed: Single,
    collision_warn_time: Single,
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
pub(super) struct ShipRecall<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(rename = "AIModuleTag")]
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    ai_module_tag: RecordLink<'source>,
    #[serde(rename = "BrakingDistanceOffset")]
    braking_distance_offset: Single,
    #[serde(rename = "DefaultPlatformDetectionRange")]
    default_platform_detection_range: Single,
    #[serde(rename = "ForwardOffset")]
    forward_offset: Single,
    #[serde(rename = "HoverHeightAtDestination")]
    hover_height_at_destination: Single,
    #[serde(rename = "MinimumRecallDistance")]
    minimum_recall_distance: Single,
    #[serde(rename = "ObstructionDetectionRange")]
    obstruction_detection_range: Single,
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
