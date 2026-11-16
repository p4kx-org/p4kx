// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    AudioIndex, BrokenLink, LanguageChoice, ResolveOptions, SoundVariant, TriggerSounds,
};
use p4kx_extraction::Archive;

use crate::archives::Opened;
use crate::audio_convert as convert;
use crate::audio_params::TriggerParams;
use crate::audio_shapes::{
    BreakShape, CountsShape, IssueShape, LanguageShape, TriggerShape, TriggerSoundsResult,
    VariantShape,
};
use crate::budget::{Budget, MAX_RESPONSE_LEN, fill_within, fit_prefix, json_len, structured_len};
use crate::budget_text;
use crate::error::ToolError;
use crate::page::Page;

use super::event;
use super::surveys::SurveyMode;

#[path = "audio_resolve_origins.rs"]
mod origins;

const MAX_BREAKS: usize = 25;

/// Returns a page of sound variants and the original inputs consumed by their resolution.
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
    let options = ResolveOptions::default().with_path_sources();
    let options = params
        .max_visits
        .map_or(options, |limit| options.with_max_visits(limit));
    let sounds = audio
        .resolve_trigger_with_options(&params.trigger, &choice, options)
        .map_err(|source| ToolError::AudioResolution {
            trigger: params.trigger.clone(),
            source: Box::new(source),
        })?
        .ok_or_else(|| ToolError::UnknownTrigger(params.trigger.clone()))?;
    let listed = listed_breaks(&sounds, page);
    let result = Envelope::read(opened, index, &sounds, &choice, &listed)?;
    let selection = Selection {
        archive: opened.archive(),
        index,
        sounds: &sounds,
        listed_breaks: &listed,
        variants: page.slice(sounds.variants()),
        page,
    };
    selection.fit(&result)
}

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

fn listed_breaks<'list, 'index>(
    sounds: &'list TriggerSounds<'index>,
    page: Page,
) -> Vec<&'list BrokenLink<'index>> {
    if page.offset() != 0 {
        return Vec::new();
    }
    let links = sounds.breaks();
    let late_cut = links
        .iter()
        .enumerate()
        .find(|(_, link)| link.reason().truncated_by_limit())
        .and_then(|(index, link)| (index >= MAX_BREAKS).then_some(link));
    links
        .iter()
        .take(MAX_BREAKS - usize::from(late_cut.is_some()))
        .chain(late_cut)
        .collect()
}

struct Selection<'list, 'index> {
    archive: &'list Archive,
    index: &'index AudioIndex,
    sounds: &'list TriggerSounds<'index>,
    listed_breaks: &'list [&'list BrokenLink<'index>],
    variants: &'list [SoundVariant<'index>],
    page: Page,
}

impl Selection<'_, '_> {
    fn fit(&self, envelope: &Envelope) -> Result<TriggerSoundsResult, ToolError> {
        let total = self.sounds.variants().len();
        let (rows, _) = fill_within(
            self.page,
            total,
            envelope.size(),
            self.variants.iter().map(convert::variant),
            |variant| format!("variant of the sound {}", variant.sound_id),
        )?;
        let response = |count, mode| self.response(envelope, &rows, count, mode);
        let (kept, witness) = fit_prefix(
            rows.len(),
            |count| response(count, SurveyMode::Witness),
            || "resolved audio variant".to_owned(),
        )?;
        let complete = response(kept, SurveyMode::Complete)?;
        if structured_len(&complete) <= MAX_RESPONSE_LEN {
            Ok(complete)
        } else {
            Ok(witness)
        }
    }

    fn response(
        &self,
        envelope: &Envelope,
        rows: &[VariantShape],
        count: usize,
        mode: SurveyMode,
    ) -> Result<TriggerSoundsResult, ToolError> {
        let rows = rows.get(..count).ok_or_else(position_error)?;
        let variants = self.variants.get(..count).ok_or_else(position_error)?;
        let next_cursor = self
            .page
            .next_cursor_after(count, self.sounds.variants().len())?;
        let origins = origins::capture(
            self,
            variants,
            rows,
            envelope.issues.len(),
            next_cursor.is_some(),
            mode,
        )?;
        Ok(TriggerSoundsResult {
            archive: envelope.archive.clone(),
            language: envelope.language.clone(),
            trigger: envelope.trigger.clone(),
            counts: envelope.counts,
            truncated_by_limits: envelope.truncated,
            variants: rows.to_vec(),
            breaks: envelope.breaks.clone(),
            issues: envelope.issues.clone(),
            issues_total: envelope.issues_total,
            next_cursor,
            origins,
        })
    }
}

struct Envelope {
    archive: String,
    language: LanguageShape,
    trigger: TriggerShape,
    counts: CountsShape,
    truncated: bool,
    breaks: Vec<BreakShape>,
    issues: Vec<IssueShape>,
    issues_total: usize,
}

impl Envelope {
    fn read(
        opened: &Opened,
        index: &AudioIndex,
        sounds: &TriggerSounds<'_>,
        choice: &LanguageChoice,
        listed: &[&BrokenLink<'_>],
    ) -> Result<Self, ToolError> {
        let mut text = Budget::new();
        let breaks = listed
            .iter()
            .map(|link| convert::broken_link(link, &mut text))
            .collect::<Result<_, _>>()?;
        let (issues, issues_total) = convert::issues(index, &mut text)?;
        Ok(Self {
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
            truncated: sounds.truncated_by_limits(),
            breaks,
            issues,
            issues_total,
        })
    }

    fn size(&self) -> usize {
        json_len(&(
            &self.archive,
            &self.language,
            &self.trigger,
            self.counts,
            self.truncated,
            &self.breaks,
            &self.issues,
            self.issues_total,
        ))
    }
}

fn position_error() -> ToolError {
    ToolError::Refused("Audio resolution output has inconsistent source positions".to_owned())
}
