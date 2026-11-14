// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::behavior::DeviceBehavior;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference, SourceText};

const TIMER: &str = "STriggerableDevicesTriggerTimerParams";
const IMPACT: &str = "STriggerableDevicesTriggerImpactParams";
const LASER_FIELD_COUNT: usize = 9;
const LASER_TRIP: &str = "STriggerableDevicesTriggerLaserTripParams";

/// Concrete trigger selection with the original common settings and owned behavior.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct DeviceTrigger<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    common: TriggerSource<'source>,
    #[serde(flatten)]
    details: TriggerDetails<'source>,
}

impl<'source> DeviceTrigger<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let common = TriggerSource::read(source, notes)?;
        let details = TriggerDetails::read(source, notes)?;
        let selected = details.projected();
        let source = UnitSource::read(source, common.projected().into_iter().chain(selected))?;
        Ok(Self {
            source,
            common,
            details,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TriggerSource<'source> {
    behavior: SourcedValue<DeviceBehavior<'source>>,
    blinking_params: SourcedValue<SourceArenaOutput<'source>>,
    effect_tags: SourcedValue<SourceArenaOutput<'source>>,
    is_authoritative: SourcedValue<bool>,
    marker_model_path: SourcedValue<SourceText<'source>>,
    marker_shape_path: SourcedValue<SourceText<'source>>,
    name: SourcedValue<SourceText<'source>>,
}

impl<'source> TriggerSource<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type()?;
        Ok(Self {
            behavior: source.declared_pointer("behavior", notes, DeviceBehavior::read)?,
            blinking_params: source.authored_member("blinkingParams", notes)?,
            effect_tags: source.authored_member("effectTags", notes)?,
            is_authoritative: source.boolean("isAuthoritative", notes)?,
            marker_model_path: source
                .borrowed_string("markerModelPath", notes)?
                .map(|text| SourceText::read(source_type, "markerModelPath", text)),
            marker_shape_path: source
                .borrowed_string("markerShapePath", notes)?
                .map(|text| SourceText::read(source_type, "markerShapePath", text)),
            name: source
                .borrowed_string("name", notes)?
                .map(|text| SourceText::read(source_type, "name", text)),
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("behavior", self.behavior.state()),
            ("blinkingParams", self.blinking_params.state()),
            ("effectTags", self.effect_tags.state()),
            ("isAuthoritative", self.is_authoritative.state()),
            ("markerModelPath", self.marker_model_path.state()),
            ("markerShapePath", self.marker_shape_path.state()),
            ("name", self.name.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum TriggerDetails<'source> {
    Timer {
        duration: SourcedValue<ResourceFloat32>,
    },
    Impact {
        ignore_water_collision: SourcedValue<bool>,
    },
    LaserTrip {
        value: Box<LaserTrip<'source>>,
    },
    Other,
}

impl<'source> TriggerDetails<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            TIMER => Self::Timer {
                duration: source.single("duration", notes)?,
            },
            IMPACT => Self::Impact {
                ignore_water_collision: source.boolean("ignoreWaterCollision", notes)?,
            },
            LASER_TRIP => Self::LaserTrip {
                value: Box::new(LaserTrip::read(source, notes)?),
            },
            _ => Self::Other,
        })
    }

    fn projected(&self) -> impl Iterator<Item = (&'static str, SourceValueState)> {
        let (outer, nested) = match self {
            Self::Timer { duration } => (Some(("duration", duration.state())), None),
            Self::Impact {
                ignore_water_collision,
            } => (
                Some(("ignoreWaterCollision", ignore_water_collision.state())),
                None,
            ),
            Self::LaserTrip { value } => (None, Some(value.projected())),
            Self::Other => (None, None),
        };
        outer.into_iter().chain(nested.into_iter().flatten())
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LaserTrip<'source> {
    helper_name: SourcedValue<SourceText<'source>>,
    laser_corner_duration: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    laser_effect_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    laser_impact_effect_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    laser_impact_offset_parameter: SourcedValue<Box<SourceRecordReference<'source>>>,
    laser_length: SourcedValue<ResourceFloat32>,
    laser_radius: SourcedValue<ResourceFloat32>,
    laser_sweep_angle: SourcedValue<ResourceFloat32>,
    laser_sweep_duration: SourcedValue<ResourceFloat32>,
}

impl<'source> LaserTrip<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type()?;
        Ok(Self {
            helper_name: source
                .borrowed_string("helperName", notes)?
                .map(|text| SourceText::read(source_type, "helperName", text)),
            laser_corner_duration: source.single("laserCornerDuration", notes)?,
            laser_effect_tag: source.record_link("laserEffectTag", notes)?,
            laser_impact_effect_tag: source.record_link("laserImpactEffectTag", notes)?,
            laser_impact_offset_parameter: source
                .record_link("laserImpactOffsetParameter", notes)?,
            laser_length: source.single("laserLength", notes)?,
            laser_radius: source.single("laserRadius", notes)?,
            laser_sweep_angle: source.single("laserSweepAngle", notes)?,
            laser_sweep_duration: source.single("laserSweepDuration", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); LASER_FIELD_COUNT] {
        [
            ("helperName", self.helper_name.state()),
            ("laserCornerDuration", self.laser_corner_duration.state()),
            ("laserEffectTag", self.laser_effect_tag.state()),
            ("laserImpactEffectTag", self.laser_impact_effect_tag.state()),
            (
                "laserImpactOffsetParameter",
                self.laser_impact_offset_parameter.state(),
            ),
            ("laserLength", self.laser_length.state()),
            ("laserRadius", self.laser_radius.state()),
            ("laserSweepAngle", self.laser_sweep_angle.state()),
            ("laserSweepDuration", self.laser_sweep_duration.state()),
        ]
    }
}
