// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, LanguageChoice, TriggerSounds};

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::TriggerParams;
use crate::audio_shapes::{BreakShape, LanguageShape, TriggerSoundsResult};
use crate::budget::{Budget, fill_within, json_len};
use crate::budget_text;
use crate::error::ToolError;
use crate::page::Page;

use super::event;

/// The most broken links listed with the first page.
const MAX_BREAKS: usize = 25;

/// Returns the language choice the call names; the languages must be language folders of the
/// Archive, and the fallback defaults to the game's default language.
fn language_choice(
    index: &AudioIndex,
    lang: Option<&str>,
    fallback: Option<&str>,
) -> Result<LanguageChoice, ToolError> {
    let known = |name: &str| {
        index
            .language(name)
            .ok_or_else(|| ToolError::UnknownLanguage {
                language: name.to_owned(),
                available: if index.languages().is_empty() {
                    "none".to_owned()
                } else {
                    index.languages().join(", ")
                },
            })
    };
    let preferred = lang.map(known).transpose()?;
    let fallback = match fallback {
        Some(name) => Some(known(name)?),
        None => index.default_language(),
    };
    Ok(LanguageChoice::new(preferred, fallback))
}

/// Returns complete retained metadata for one page of actual sound variants.
pub(crate) fn resolve(
    opened: &Opened,
    params: &TriggerParams,
    page: Page,
) -> Result<TriggerSoundsResult, ToolError> {
    let index = opened.audio_index();
    let audio = opened.audio(&index);
    let index = audio.index();
    let choice = language_choice(
        index,
        params.lang.as_deref(),
        params.fallback_lang.as_deref(),
    )?;
    let sounds = audio
        .resolve_trigger_in(&params.trigger, &choice)
        .ok_or_else(|| ToolError::UnknownTrigger(params.trigger.clone()))?;
    let total = sounds.variants().len();
    let mut result = envelope(opened, index, &sounds, &choice, page)?;
    let items = page.slice(sounds.variants()).iter().map(convert::variant);
    let (variants, next) = fill_within(page, total, json_len(&result), items, |variant| {
        format!("variant of the sound {}", variant.sound_id)
    })?;
    result.variants = variants;
    result.next_cursor = next;
    Ok(result)
}

fn envelope(
    opened: &Opened,
    index: &AudioIndex,
    sounds: &TriggerSounds<'_>,
    choice: &LanguageChoice,
    page: Page,
) -> Result<TriggerSoundsResult, ToolError> {
    let mut text = Budget::new();
    let breaks = breaks(sounds, page, &mut text)?;
    let (issues, issues_total) = convert::issues(index, &mut text)?;
    Ok(TriggerSoundsResult {
        archive: budget_text::display(&mut text, &opened.archive().path().display())?,
        language: LanguageShape {
            requested: choice
                .preferred()
                .map(|value| text.text(value))
                .transpose()?,
            fallback: choice
                .fallback()
                .map(|value| text.text(value))
                .transpose()?,
        },
        trigger: event::trigger(sounds, &mut text)?,
        counts: convert::counts(sounds),
        variants: Vec::new(),
        breaks,
        issues,
        issues_total,
        next_cursor: None,
    })
}

fn breaks(
    sounds: &TriggerSounds<'_>,
    page: Page,
    text: &mut Budget,
) -> Result<Vec<BreakShape>, ToolError> {
    if page.offset() != 0 {
        return Ok(Vec::new());
    }
    sounds
        .breaks()
        .iter()
        .take(MAX_BREAKS)
        .map(|link| convert::broken_link(link, text))
        .collect()
}
