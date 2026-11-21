// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    AudioIndex, BreakReason, BrokenLink, IndexedSoundbank, PathStep, SoundVariant, TriggerSounds,
    Unmatched,
};

use crate::audio::location as location_shape;
pub(crate) use crate::audio::location::location;
use crate::audio_shapes::{
    BreakShape, CountsShape, IssueShape, SoundbankRef, StepShape, UnmatchedShape, VariantShape,
};
use crate::budget::Budget;
use crate::budget_text;
use crate::error::ToolError;

/// The most issues one result lists.
const MAX_ISSUES: usize = 20;

/// Returns the retained index issues and their original total.
pub(crate) fn issues(
    index: &AudioIndex,
    text: &mut Budget,
) -> Result<(Vec<IssueShape>, usize), ToolError> {
    let listed = index
        .issues()
        .iter()
        .take(MAX_ISSUES)
        .map(|issue| {
            Ok(IssueShape {
                path: text.text(issue.path())?,
                kind: issue.error().kind(),
                message: budget_text::display(text, issue.error())?,
            })
        })
        .collect::<Result<_, ToolError>>()?;
    Ok((listed, index.issues().len()))
}

pub(crate) fn soundbank(
    bank: &IndexedSoundbank,
    text: &mut Budget,
) -> Result<SoundbankRef, ToolError> {
    Ok(SoundbankRef {
        path: text.text(bank.path())?,
        language: bank.language().map(|value| text.text(value)).transpose()?,
    })
}

pub(crate) fn unmatched(reason: Unmatched, text: &mut Budget) -> Result<UnmatchedShape, ToolError> {
    Ok(UnmatchedShape {
        kind: reason.kind(),
        message: budget_text::display(text, &reason)?,
    })
}

pub(crate) fn counts(sounds: &TriggerSounds<'_>) -> CountsShape {
    let count = |keep: fn(&SoundVariant<'_>) -> bool| {
        sounds
            .variants()
            .iter()
            .filter(|variant| keep(variant))
            .count()
    };
    CountsShape {
        variants: sounds.variants().len(),
        located: count(|variant| variant.location().is_some()),
        unlocated: count(|variant| variant.location().is_none() && variant.wem_id().is_some()),
        plugins: count(|variant| variant.wem_id().is_none()),
        breaks: sounds.breaks().len(),
    }
}

pub(crate) fn variant(variant: &SoundVariant<'_>) -> Result<VariantShape, ToolError> {
    let mut text = Budget::new();
    Ok(VariantShape {
        event: text.text(variant.event().name())?,
        soundbank: soundbank(variant.soundbank(), &mut text)?,
        sound_id: variant.sound_id(),
        wem_id: variant.wem_id(),
        source: variant.source_kind().name(),
        localised: variant.is_localised(),
        language: variant
            .language()
            .map(|value| text.text(value))
            .transpose()?,
        fallback: variant.used_fallback(),
        location: variant
            .location()
            .map(|place| location_shape::bounded(place, &mut text))
            .transpose()?
            .flatten(),
        path: steps(variant.path()),
    })
}

pub(crate) fn broken_link(
    link: &BrokenLink<'_>,
    text: &mut Budget,
) -> Result<BreakShape, ToolError> {
    Ok(BreakShape {
        event: text.text(link.event().name())?,
        soundbank: link
            .soundbank()
            .map(|bank| soundbank(bank, text))
            .transpose()?,
        object: link.object().map(step),
        target: match link.reason() {
            BreakReason::Missing { id }
            | BreakReason::OtherSoundbank { id, .. }
            | BreakReason::Cycle { id } => Some(*id),
            _ => None,
        },
        path: steps(link.path()),
        kind: link.reason().kind(),
        message: budget_text::display(text, link.reason())?,
    })
}

fn steps(path: &[PathStep]) -> Vec<StepShape> {
    path.iter().copied().map(step).collect()
}

const fn step(step: PathStep) -> StepShape {
    StepShape {
        kind: step.kind.name(),
        id: step.id,
    }
}
