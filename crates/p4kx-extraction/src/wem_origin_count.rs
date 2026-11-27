// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::WemSampleCountSource;

use crate::SourceUseRole;

use super::{WemOriginError, WemOriginPlan, WemOriginSite};

pub(super) fn inputs(
    plan: &WemOriginPlan<'_>,
    sample_role: SourceUseRole,
) -> Result<Vec<WemOriginSite>, WemOriginError> {
    use SourceUseRole::{Conversion, Selector};

    let mut inputs = plan.codec(Selector)?;
    match plan.info.sample_count() {
        WemSampleCountSource::Pcm => {
            inputs.push(plan.format("bitsPerSample", Selector));
            inputs.push(plan.format("channels", Selector));
            frames(plan, &mut inputs, true);
        }
        WemSampleCountSource::Adpcm { supported_layout } => {
            inputs.push(plan.format("channels", Conversion));
            frames(plan, &mut inputs, supported_layout);
        }
        WemSampleCountSource::VorbisChunk { body_offset } => {
            inputs.push(WemOriginPlan::chunk(
                body_offset,
                "sampleCount",
                sample_role,
            ));
        }
        WemSampleCountSource::VorbisFormat {
            rejected_vorbis_body,
            accepted,
        } => {
            if let Some(body_offset) = rejected_vorbis_body {
                inputs.push(WemOriginPlan::chunk(body_offset, "bodyLength", Selector));
            }
            optional_count(plan, &mut inputs, accepted, sample_role);
        }
        WemSampleCountSource::Opus {
            rate_checked,
            accepted,
        } => {
            if rate_checked {
                inputs.push(plan.format("sampleRate", Selector));
            }
            optional_count(plan, &mut inputs, accepted, sample_role);
        }
        WemSampleCountSource::Unsupported => {}
        _ => return Err(WemOriginError::UnhandledDecision),
    }
    Ok(inputs)
}

fn frames(plan: &WemOriginPlan<'_>, inputs: &mut Vec<WemOriginSite>, uses_length: bool) {
    use SourceUseRole::Conversion;

    inputs.push(plan.format("blockAlign", Conversion));
    if uses_length {
        inputs.push(WemOriginPlan::chunk(
            plan.info.data_body_offset(),
            "bodyLength",
            Conversion,
        ));
    }
}

fn optional_count(
    plan: &WemOriginPlan<'_>,
    inputs: &mut Vec<WemOriginSite>,
    accepted: bool,
    role: SourceUseRole,
) {
    inputs.push(plan.format("bodyLength", SourceUseRole::Selector));
    if accepted {
        inputs.push(plan.format("sampleCount", role));
    }
}
