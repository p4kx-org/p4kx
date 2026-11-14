// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_effect_resource::{AudioResource, Rtpc};
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const OVERALL_PROGRESS: &str = "audioRtpcOverallProgress";
const PART_PROGRESS: &str = "audioRtpcPartProgress";
const REMAINING_DURATION: &str = "audioRtpcRemainingDuration";
const TOTAL_DURATION: &str = "audioRtpcTotalDuration";
const ERROR: &str = "audioTriggerError";
const PART_SUCCESS: &str = "audioTriggerPartSuccess";
const PAUSE: &str = "audioTriggerPause";
const LOOP_START: &str = "audioTriggerProgressLoopStart";
const LOOP_STOP: &str = "audioTriggerProgressLoopStop";
const RESET: &str = "audioTriggerReset";
const RESUME: &str = "audioTriggerResume";
const START: &str = "audioTriggerStart";
const SUCCESS: &str = "audioTriggerSuccess";

/// The authored sound events and parameters of a hackable Entity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[expect(
    clippy::struct_field_names,
    reason = "field names match the authored audio declarations"
)]
pub(super) struct HackableAudio<'source> {
    audio_rtpc_overall_progress: SourcedValue<HackableSound<'source, Rtpc>>,
    audio_rtpc_part_progress: SourcedValue<HackableSound<'source, Rtpc>>,
    audio_rtpc_remaining_duration: SourcedValue<HackableSound<'source, Rtpc>>,
    audio_rtpc_total_duration: SourcedValue<HackableSound<'source, Rtpc>>,
    audio_trigger_error: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_part_success: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_pause: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_progress_loop_start: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_progress_loop_stop: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_reset: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_resume: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_start: SourcedValue<HackableSound<'source, AudioResource>>,
    audio_trigger_success: SourcedValue<HackableSound<'source, AudioResource>>,
}

impl<'source> HackableAudio<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut parameter = |name| source.declared_class(name, notes, HackableSound::read_rtpc);
        let audio_rtpc_overall_progress = parameter(OVERALL_PROGRESS)?;
        let audio_rtpc_part_progress = parameter(PART_PROGRESS)?;
        let audio_rtpc_remaining_duration = parameter(REMAINING_DURATION)?;
        let audio_rtpc_total_duration = parameter(TOTAL_DURATION)?;
        let mut trigger = |name| source.declared_class(name, notes, HackableSound::read_trigger);
        Ok(Self {
            audio_rtpc_overall_progress,
            audio_rtpc_part_progress,
            audio_rtpc_remaining_duration,
            audio_rtpc_total_duration,
            audio_trigger_error: trigger(ERROR)?,
            audio_trigger_part_success: trigger(PART_SUCCESS)?,
            audio_trigger_pause: trigger(PAUSE)?,
            audio_trigger_progress_loop_start: trigger(LOOP_START)?,
            audio_trigger_progress_loop_stop: trigger(LOOP_STOP)?,
            audio_trigger_reset: trigger(RESET)?,
            audio_trigger_resume: trigger(RESUME)?,
            audio_trigger_start: trigger(START)?,
            audio_trigger_success: trigger(SUCCESS)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 13] {
        [
            (OVERALL_PROGRESS, self.audio_rtpc_overall_progress.state()),
            (PART_PROGRESS, self.audio_rtpc_part_progress.state()),
            (
                REMAINING_DURATION,
                self.audio_rtpc_remaining_duration.state(),
            ),
            (TOTAL_DURATION, self.audio_rtpc_total_duration.state()),
            (ERROR, self.audio_trigger_error.state()),
            (PART_SUCCESS, self.audio_trigger_part_success.state()),
            (PAUSE, self.audio_trigger_pause.state()),
            (LOOP_START, self.audio_trigger_progress_loop_start.state()),
            (LOOP_STOP, self.audio_trigger_progress_loop_stop.state()),
            (RESET, self.audio_trigger_reset.state()),
            (RESUME, self.audio_trigger_resume.state()),
            (START, self.audio_trigger_start.state()),
            (SUCCESS, self.audio_trigger_success.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct HackableSound<'source, Setting> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    setting: Setting,
}

impl<'source> HackableSound<'source, Rtpc> {
    fn read_rtpc(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let setting = Rtpc::read(source, notes)?;
        let source = UnitSource::read(source, [setting.projected_member()])?;
        Ok(Self { source, setting })
    }
}

impl<'source> HackableSound<'source, AudioResource> {
    fn read_trigger(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let setting = AudioResource::read(source, notes)?;
        let source = UnitSource::read(source, [setting.projected_member()])?;
        Ok(Self { source, setting })
    }
}
