// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::Entities;
use p4kx_assembly::audio::{
    ArchiveAudio, AudioIndex, EntityAudio, EntityTrigger, EventMatch, ExportReport,
    IndexedSoundbank, LanguageChoice, ResolveOptions, TriggerSounds, Unmatched, trigger_references,
};
use p4kx_extraction::{CancelToken, ConversionError, DataForgeEntry, Record};
use serde::Serialize;

use crate::cli::EntityAudioArgs;
use crate::extract::cancel_on_interrupt;
use crate::query_table::one_line;
use crate::wwise::{break_line, language_choice, variant_line};
use crate::wwise_extract;
use crate::wwise_json::{self, BreakJson, VariantJson};

#[path = "entity_audio_origins.rs"]
mod origins;
#[path = "entity_audio_json.rs"]
mod presentation;

const CLASS_PREFIX: &str = "EntityClassDefinition.";

#[derive(Serialize)]
struct EntityJson<'a> {
    guid: String,
    record_path: &'a str,
    name: &'a str,
    class_name: &'a str,
}

#[derive(Serialize)]
struct TriggerJson<'a> {
    name: &'a str,
    location: &'a str,
    references: u32,
    definition: Option<&'a str>,
    events: Vec<EventJson<'a>>,
    unmatched: Option<UnmatchedJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sounds: Option<SoundsJson<'a>>,
}

#[derive(Serialize)]
struct SoundsJson<'a> {
    truncated_by_limits: bool,
    variants: Vec<VariantJson<'a>>,
    breaks: Vec<BreakJson<'a>>,
}

#[derive(Serialize)]
struct EventJson<'a> {
    name: &'a str,
    id: u32,
    localised: bool,
    soundbanks: Vec<SoundbankJson<'a>>,
}

#[derive(Serialize)]
struct SoundbankJson<'a> {
    path: &'a str,
    language: Option<&'a str>,
}

#[derive(Serialize)]
struct UnmatchedJson {
    kind: &'static str,
    message: String,
}

#[derive(Serialize)]
pub(crate) struct IssueJson<'a> {
    path: &'a str,
    kind: &'static str,
    message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct Counts {
    triggers: usize,
    matched: usize,
    unmatched: usize,
}

impl Counts {
    fn of(audio: &EntityAudio<'_>) -> Self {
        let triggers = audio.triggers().len();
        let matched = audio.matched_count();
        Self {
            triggers,
            matched,
            unmatched: triggers.saturating_sub(matched),
        }
    }
}

/// Prints the audio triggers of an Entity with their Wwise events and soundbanks.
pub(crate) fn run(args: &EntityAudioArgs, as_json: bool) -> Result<ExitCode> {
    let resolve_variants = args.sounds || args.extract.is_some();
    let cancel = resolve_variants
        .then(cancel_on_interrupt)
        .transpose()?
        .unwrap_or_default();
    let archive = crate::key::open(&args.archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let entity = Entities::new(&forge).find(&args.entity)?;
    let (references, inspection) = if as_json {
        let (references, original) = origins::inspect(&entity)?;
        (references, Some(original))
    } else {
        (trigger_references(&entity.tree()?), None)
    };
    let audio = ArchiveAudio::new(&archive);
    let index = audio
        .index_until(&cancel)
        .ok_or(ConversionError::Cancelled)?;
    let choice = language_choice(index, &args.resolve)?;
    let resolved = index.resolve(references);
    let options = resolve_variants.then(|| {
        let options = args.resolve.options(&cancel);
        if as_json {
            options.with_path_sources()
        } else {
            options
        }
    });
    let sounds = resolve_sounds(&audio, &resolved, &choice, options)?;
    let counts = Counts::of(&resolved);
    let report = export_sounds(&audio, &sounds, args, &cancel)?;
    let code = report
        .as_ref()
        .map_or(ExitCode::SUCCESS, wwise_extract::exit_code);
    if let Some(inspection) = &inspection {
        let input = origins::Input {
            archive: &archive,
            index,
            original: entry.origin(),
            record: &entity,
            inspection,
            resolved: &resolved,
            sounds: &sounds,
            language: resolve_variants.then_some(&choice),
        };
        presentation::print(&input, entry.path(), counts, args, report.as_ref())?;
        return Ok(code);
    }
    print_completion(
        index,
        &entity,
        &resolved,
        &sounds,
        counts,
        report.as_ref().zip(args.extract.as_deref()),
    )?;
    Ok(code)
}

fn print_completion(
    index: &AudioIndex,
    entity: &Record<'_>,
    resolved: &EntityAudio<'_>,
    sounds: &[Option<TriggerSounds<'_>>],
    counts: Counts,
    export: Option<(&ExportReport, &Path)>,
) -> Result<()> {
    if !warn_open(index)? {
        return Ok(());
    }
    match print_text(entity, resolved, sounds, counts) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => return Err(error.into()),
        Err(_) => return Ok(()),
        Ok(()) => {}
    }
    if let Some((report, out)) = export {
        wwise_extract::print_report(report, out)?;
    }
    Ok(())
}

fn resolve_sounds<'i>(
    audio: &'i ArchiveAudio<'_>,
    resolved: &EntityAudio<'i>,
    choice: &LanguageChoice,
    options: Option<ResolveOptions<'_>>,
) -> Result<Vec<Option<TriggerSounds<'i>>>, ConversionError> {
    let Some(options) = options else {
        return Ok(Vec::new());
    };
    resolved
        .triggers()
        .iter()
        .map(|trigger| audio.resolve_trigger_with_options(trigger.name(), choice, options))
        .collect()
}

fn export_sounds(
    audio: &ArchiveAudio<'_>,
    sounds: &[Option<TriggerSounds<'_>>],
    args: &EntityAudioArgs,
    cancel: &CancelToken,
) -> Result<Option<ExportReport>> {
    let Some(out) = &args.extract else {
        return Ok(None);
    };
    let found: Vec<&TriggerSounds<'_>> = sounds.iter().flatten().collect();
    Ok(Some(wwise_extract::export(
        audio,
        &found,
        out,
        &args.export,
        cancel,
    )?))
}

fn entity_json<'a>(entity: &Record<'a>) -> EntityJson<'a> {
    let name = entity.name().unwrap_or_default();
    EntityJson {
        guid: entity.guid().to_string(),
        record_path: entity.record_path().unwrap_or_default(),
        name,
        class_name: name.strip_prefix(CLASS_PREFIX).unwrap_or(name),
    }
}

fn trigger_json<'a>(
    trigger: &'a EntityTrigger<'a>,
    sounds: Option<&'a TriggerSounds<'a>>,
) -> TriggerJson<'a> {
    TriggerJson {
        sounds: sounds.map(|sounds| SoundsJson {
            truncated_by_limits: sounds.truncated_by_limits(),
            variants: sounds.variants().iter().map(wwise_json::variant).collect(),
            breaks: sounds
                .breaks()
                .iter()
                .map(wwise_json::broken_link)
                .collect(),
        }),
        name: trigger.name(),
        location: trigger.reference().location(),
        references: trigger.reference().count(),
        definition: trigger.definition_path(),
        events: trigger
            .events()
            .map(|event| EventJson {
                name: event.event().name(),
                id: event.event().id(),
                localised: event.event().is_localised(),
                soundbanks: event
                    .soundbanks()
                    .map(|bank| SoundbankJson {
                        path: bank.path(),
                        language: bank.language(),
                    })
                    .collect(),
            })
            .collect(),
        unmatched: trigger.unmatched().map(|reason| UnmatchedJson {
            kind: reason.kind(),
            message: reason.to_string(),
        }),
    }
}

pub(crate) fn issues_json(index: &AudioIndex) -> Vec<IssueJson<'_>> {
    index
        .issues()
        .iter()
        .map(|issue| IssueJson {
            path: issue.path(),
            kind: issue.error().kind(),
            message: issue.error().to_string(),
        })
        .collect()
}

/// Writes the warnings of [`warn`]; returns `false` when stderr is a closed pipe.
pub(crate) fn warn_open(index: &AudioIndex) -> io::Result<bool> {
    match warn(index) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(error) => Err(error),
        Ok(()) => Ok(true),
    }
}

pub(crate) fn warn(index: &AudioIndex) -> io::Result<()> {
    let mut err = io::stderr().lock();
    for issue in index.issues() {
        writeln!(
            err,
            "warning: {}",
            one_line(&format!("{}: {}", issue.path(), issue.error()))
        )?;
    }
    Ok(())
}

fn print_text(
    entity: &Record<'_>,
    audio: &EntityAudio<'_>,
    sounds: &[Option<TriggerSounds<'_>>],
    counts: Counts,
) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    let header = entity_json(entity);
    writeln!(
        out,
        "{}",
        one_line(&format!(
            "{}  {}  {}",
            header.class_name, header.guid, header.record_path
        ))
    )?;
    for (position, trigger) in audio.triggers().iter().enumerate() {
        writeln!(out, "  {}", one_line(&trigger_line(trigger)))?;
        if let Some(Some(sounds)) = sounds.get(position) {
            for variant in sounds.variants() {
                writeln!(out, "    {}", one_line(&variant_line(variant)))?;
            }
            for link in sounds.breaks() {
                writeln!(out, "    ! {}", one_line(&break_line(link)))?;
            }
        }
    }
    writeln!(
        out,
        "{} triggers: {} matched, {} unmatched",
        counts.triggers, counts.matched, counts.unmatched
    )?;
    out.flush()
}

fn trigger_line(trigger: &EntityTrigger<'_>) -> String {
    let events: Vec<String> = trigger.events().map(|event| event_text(&event)).collect();
    let mut line = format!("{}:", trigger.name());
    if !events.is_empty() {
        line.push(' ');
        line.push_str(&events.join("; "));
    }
    match trigger.unmatched() {
        None => {}
        Some(reason @ (Unmatched::NoDefinition | Unmatched::NoEvent)) => {
            let _infallible = write!(line, " unmatched: {reason}");
        }
        Some(reason) => {
            let _infallible = write!(line, " (unmatched: {reason})");
        }
    }
    line
}

fn event_text(event: &EventMatch<'_>) -> String {
    let banks: Vec<String> = event.soundbanks().map(bank_text).collect();
    if banks.is_empty() {
        format!("{} in no soundbank", event.event().name())
    } else {
        format!("{} in {}", event.event().name(), banks.join(", "))
    }
}

fn bank_text(bank: &IndexedSoundbank) -> String {
    bank.language().map_or_else(
        || bank.file_name().to_owned(),
        |language| format!("{} ({language})", bank.file_name()),
    )
}
