// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::catalog_v3::component_sources::physical_component;
use crate::export_field_inspection::ConfigurationInspection;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[path = "hackable_audio.rs"]
mod hackable_audio;

use super::ComponentKind;
pub use hackable_audio::{AudioResource, HackableAudio, HackableSound, Rtpc};

pub(super) const KIND: ComponentKind = ComponentKind {
    source_type: "HackableParams",
    inspection: ConfigurationInspection::HackableDevice,
};

physical_component!(HackableDevices, HackableConfiguration, KIND);

/// The original timing, error and audio configuration of one hackable Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct HackableConfiguration<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    base_duration: SourcedValue<ResourceFloat32>,
    base_error_chance: SourcedValue<ResourceFloat32>,
    max_pause_duration: SourcedValue<ResourceFloat32>,
    num_error_checks: SourcedValue<u32>,
    reset_after_hack_success: SourcedValue<bool>,
    update_progress_delta_time: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    audio: HackableAudio<'source>,
}

impl<'source> HackableConfiguration<'source> {
    source_getters! {
        /// Borrows the physical owner and original Components ordinal.
        owner: ComponentOwner,
        /// Borrows the original source class and additional fields.
        source: UnitSource<'source>,
        /// Borrows the authored duration.
        base_duration: SourcedValue<ResourceFloat32>,
        /// Borrows the authored base error chance.
        base_error_chance: SourcedValue<ResourceFloat32>,
        /// Borrows the authored maximum pause duration.
        max_pause_duration: SourcedValue<ResourceFloat32>,
        /// Borrows the authored count of error checks.
        num_error_checks: SourcedValue<u32>,
        /// Borrows the authored reset flag.
        reset_after_hack_success: SourcedValue<bool>,
        /// Borrows the authored progress update interval.
        update_progress_delta_time: SourcedValue<ResourceFloat32>,
        /// Borrows the original audio triggers and parameters.
        audio: HackableAudio<'source>,
    }
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let base_duration = source.single("baseDuration", notes)?;
        let base_error_chance = source.single("baseErrorChance", notes)?;
        let max_pause_duration = source.single("maxPauseDuration", notes)?;
        let num_error_checks = source.uint32("numErrorChecks", notes)?;
        let reset_after_hack_success = source.boolean("resetAfterHackSuccess", notes)?;
        let update_progress_delta_time = source.single("updateProgressDeltaTime", notes)?;
        let audio = HackableAudio::read(source, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("baseDuration", base_duration.state()),
                ("baseErrorChance", base_error_chance.state()),
                ("maxPauseDuration", max_pause_duration.state()),
                ("numErrorChecks", num_error_checks.state()),
                ("resetAfterHackSuccess", reset_after_hack_success.state()),
                (
                    "updateProgressDeltaTime",
                    update_progress_delta_time.state(),
                ),
            ]
            .into_iter()
            .chain(audio.projected()),
        )?;
        Ok(Self {
            owner,
            source,
            base_duration,
            base_error_chance,
            max_pause_duration,
            num_error_checks,
            reset_after_hack_success,
            update_progress_delta_time,
            audio,
        })
    }
}
