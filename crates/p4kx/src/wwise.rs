// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::audio::{
    ArchiveAudio, AudioIndex, BrokenLink, IndexedSoundbank, LanguageChoice, PathStep, SoundVariant,
    SourceKind, TriggerSounds, WemLocation,
};
use p4kx_extraction::ConversionError;
use thiserror::Error;

use crate::cli::{AudioResolveArgs, WwiseAction, WwiseArgs, WwiseResolveArgs};
use crate::entity_audio::{issues_json, warn};
use crate::extract::cancel_on_interrupt;
use crate::json;
use crate::query_table::one_line;
use crate::wwise_json::{self, Counts, LanguageJson, ResolveJson};

/// No ATL definition of the Archive has the requested audio trigger.
#[derive(Debug, Error)]
#[error("no ATL definition has the audio trigger {0}")]
pub(crate) struct UnknownTrigger(String);

impl UnknownTrigger {
    pub(crate) fn new(name: &str) -> Self {
        Self(name.to_owned())
    }
}

/// The requested audio language is no language folder of the Archive.
#[derive(Debug, Error)]
#[error("the Archive holds no audio language {language}; its language folders are: {available}")]
pub(crate) struct UnknownAudioLanguage {
    language: String,
    available: String,
}

/// Runs a `p4kx wwise` subcommand.
pub(crate) fn run(args: &WwiseArgs, as_json: bool) -> Result<ExitCode> {
    match &args.action {
        WwiseAction::Resolve(args) => resolve(args, as_json).map(|()| ExitCode::SUCCESS),
        WwiseAction::Events(args) => crate::wwise_events::run(args, as_json),
        WwiseAction::Extract(args) => crate::wwise_extract::run(args, as_json),
    }
}

/// Returns the language choice of the command line: the languages must be language folders of the
/// Archive, and the fallback defaults to the game's default language.
pub(crate) fn language_choice(
    index: &AudioIndex,
    args: &AudioResolveArgs,
) -> Result<LanguageChoice, UnknownAudioLanguage> {
    let known = |name: &str| {
        index.language(name).ok_or_else(|| UnknownAudioLanguage {
            language: name.to_owned(),
            available: if index.languages().is_empty() {
                "none".to_owned()
            } else {
                index.languages().join(", ")
            },
        })
    };
    let preferred = args.lang.as_deref().map(known).transpose()?;
    let fallback = match args.fallback_lang.as_deref() {
        Some(name) => Some(known(name)?),
        None => index.default_language(),
    };
    Ok(LanguageChoice::new(preferred, fallback))
}

/// Prints every sound variant of an audio trigger and every broken link.
fn resolve(args: &WwiseResolveArgs, as_json: bool) -> Result<()> {
    let cancel = cancel_on_interrupt()?;
    let archive = crate::key::open(&args.archive)?;
    let audio = ArchiveAudio::new(&archive);
    let index = audio
        .index_until(&cancel)
        .ok_or(ConversionError::Cancelled)?;
    let choice = language_choice(index, &args.resolve)?;
    let mut options = args.resolve.options(&cancel);
    if as_json {
        options = options.with_path_sources();
    }
    let sounds = audio
        .resolve_trigger_with_options(&args.trigger, &choice, options)?
        .ok_or_else(|| UnknownTrigger::new(&args.trigger))?;
    let counts = Counts::of(&sounds);
    if as_json {
        let (field_origins, provenance) = wwise_json::capture(&archive, &sounds)?;
        json::print(&ResolveJson {
            archive: json::path(archive.path()),
            language: LanguageJson::of(&choice),
            trigger: wwise_json::trigger(&sounds),
            truncated_by_limits: sounds.truncated_by_limits(),
            variants: sounds.variants().iter().map(wwise_json::variant).collect(),
            breaks: sounds
                .breaks()
                .iter()
                .map(wwise_json::broken_link)
                .collect(),
            counts,
            issues: issues_json(index),
            field_origins,
            provenance,
        })?;
        return Ok(());
    }
    warn(index)?;
    match print_text(&sounds, counts) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

fn print_text(sounds: &TriggerSounds<'_>, counts: Counts) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    let header = format!(
        "{}  {}",
        sounds.trigger().name(),
        sounds.definition_path().unwrap_or_default()
    );
    writeln!(out, "{}", one_line(&header))?;
    for variant in sounds.variants() {
        writeln!(out, "  {}", one_line(&variant_line(variant)))?;
    }
    for link in sounds.breaks() {
        writeln!(out, "  ! {}", one_line(&break_line(link)))?;
    }
    writeln!(
        out,
        "{} variants: {} located, {} not located, {} without a file; {} broken links",
        counts.variants, counts.located, counts.unlocated, counts.plugins, counts.breaks
    )?;
    out.flush()
}

pub(crate) fn variant_line(variant: &SoundVariant<'_>) -> String {
    let location = match variant.location() {
        Some(WemLocation::Soundbank {
            soundbank,
            offset,
            len,
        }) => {
            let start = if variant.source_kind() == SourceKind::PrefetchStreamed {
                "start "
            } else {
                ""
            };
            format!(
                "{start}in {} at {offset} ({len} bytes)",
                soundbank_name(soundbank)
            )
        }
        Some(WemLocation::Entry { path, len, .. }) => format!("at {path} ({len} bytes)"),
        Some(_) => "at an unknown location".to_owned(),
        None => "not located".to_owned(),
    };
    let media = variant.wem_id().map_or_else(
        || {
            format!(
                "{} {}, no file",
                variant.source_kind().label(),
                variant.source().source_id
            )
        },
        |wem| {
            format!(
                "WEM {wem} {}{}, {location}",
                variant.source_kind().name(),
                localisation(variant)
            )
        },
    );
    format!(
        "{} in {}: {media}; {}",
        variant.event().name(),
        soundbank_name(variant.soundbank()),
        path_text(variant.path())
    )
}

fn localisation(variant: &SoundVariant<'_>) -> String {
    if !variant.is_localised() {
        return String::new();
    }
    match (variant.language(), variant.used_fallback()) {
        (Some(language), true) => format!(" localised {language} (fallback)"),
        (Some(language), false) => format!(" localised {language}"),
        (None, _) => " localised".to_owned(),
    }
}

pub(crate) fn break_line(link: &BrokenLink<'_>) -> String {
    let bank = link
        .soundbank()
        .map_or_else(String::new, |bank| format!(" in {}", soundbank_name(bank)));
    let object = link
        .object()
        .map_or_else(String::new, |object| format!(" at {}", step_text(object)));
    format!(
        "{}{bank}: broken{object}: {}; {}",
        link.event().name(),
        link.reason(),
        path_text(link.path())
    )
}

fn soundbank_name(bank: &IndexedSoundbank) -> String {
    bank.language().map_or_else(
        || bank.file_name().to_owned(),
        |language| format!("{} ({language})", bank.file_name()),
    )
}

fn path_text(path: &[PathStep]) -> String {
    path.iter()
        .map(|step| step_text(*step))
        .collect::<Vec<_>>()
        .join(" > ")
}

fn step_text(step: PathStep) -> String {
    format!("{} {}", step.kind.name(), step.id)
}
