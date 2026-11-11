// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{WemLoopEnd, WemLoopSource};

use crate::SourceUseRole;

use super::{WemOriginError, WemOriginField, WemOriginPlan, WemOriginSite, count};

pub(super) fn inputs(
    plan: &WemOriginPlan<'_>,
    field: WemOriginField,
) -> Result<Vec<WemOriginSite>, WemOriginError> {
    use SourceUseRole::{Conversion, Selector};

    let decision = plan.info.loop_points();
    if matches!(decision, WemLoopSource::NoSampleChunk) {
        return if matches!(field, WemOriginField::Loop) {
            Ok(vec![plan.routing()])
        } else {
            Err(WemOriginError::MissingLoop)
        };
    }
    let mut inputs = count::inputs(plan, Conversion)?;
    let selected = match decision {
        WemLoopSource::NoSampleCount => None,
        WemLoopSource::MissingCount { body_offset } => {
            inputs.push(WemOriginPlan::chunk(body_offset, "bodyLength", Selector));
            None
        }
        WemLoopSource::MissingEntries { body_offset } => {
            loop_header(&mut inputs, body_offset);
            None
        }
        WemLoopSource::Entries {
            body_offset,
            examined,
            end,
        } => {
            loop_header(&mut inputs, body_offset);
            if examined > 0 {
                inputs.push(WemOriginSite::pair(
                    "wemLoopPrefix",
                    [body_offset, examined],
                    "forwardAndFit",
                    Selector,
                ));
            }
            end.map(|end| (body_offset, examined, end))
        }
        _ => return Err(WemOriginError::UnhandledDecision),
    };
    if matches!(field, WemOriginField::Loop) {
        return Ok(inputs);
    }
    let (body_offset, examined, end) = selected.ok_or(WemOriginError::MissingLoop)?;
    let index = examined.checked_sub(1).ok_or(WemOriginError::MissingLoop)?;
    loop_value(&mut inputs, [body_offset, index], end, field)?;
    Ok(inputs)
}

fn loop_header(inputs: &mut Vec<WemOriginSite>, body_offset: u64) {
    for name in ["bodyLength", "loopCount"] {
        inputs.push(WemOriginPlan::chunk(
            body_offset,
            name,
            SourceUseRole::Selector,
        ));
    }
}

fn loop_value(
    inputs: &mut Vec<WemOriginSite>,
    indices: [u64; 2],
    end: WemLoopEnd,
    field: WemOriginField,
) -> Result<(), WemOriginError> {
    use SourceUseRole::{Conversion, Payload, Selector};

    let start = |role| WemOriginSite::pair("wemLoop", indices, "start", role);
    if matches!(field, WemOriginField::LoopStart) {
        inputs.push(start(Payload));
        return Ok(());
    }
    let end_role = match end {
        WemLoopEnd::LastFrame => Conversion,
        WemLoopEnd::SampleCount => Selector,
        _ => return Err(WemOriginError::UnhandledDecision),
    };
    inputs.push(WemOriginSite::pair(
        "wemLoop",
        indices,
        "lastFrame",
        end_role,
    ));
    if matches!(field, WemOriginField::LoopFrames) {
        inputs.push(start(Conversion));
    }
    Ok(())
}
