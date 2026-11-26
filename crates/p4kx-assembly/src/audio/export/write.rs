// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_archive::ObservedEntry;
use p4kx_audio::{Codec, Soundbank, Wem};
use p4kx_extraction::sound::{
    SoundOriginInput, WemOutput, WemOutputTarget, WemWrittenSizeOrigin, publish_wem_output,
};
use p4kx_extraction::{
    ArchiveWriteProtection, ConflictPolicy, ConversionError, MAX_SOUND_LEN, check_path_length,
};
use rayon::prelude::*;

#[path = "write_input.rs"]
mod input;

use self::input::{GroupInput, SoundInput};
use super::loop_origins::{CaptureOrigins, CapturedSoundOrigin};
use super::plan::Job;
use super::source::SourceEntry;
use super::{ExportFailure, ExportOptions, ExportState, ExportWatch, ExportedSound, SoundFormat};
use crate::audio::read::entry_bytes;
use crate::audio::{ArchiveAudio, MAX_SOUNDBANK_LEN};

type Outcome = Result<WrittenSound, ExportFailure>;

pub(super) struct WrittenSound {
    pub(super) sound: ExportedSound,
    pub(super) origin: Option<CapturedSoundOrigin>,
}

/// The folder the sounds go into.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target<'a> {
    /// The folder as given, which the reported paths start with.
    pub(super) folder: &'a Path,
    /// The absolute path of the folder, on which the path limit of ADR-0248 is measured.
    pub(super) absolute: &'a Path,
    /// Immutable source evidence retained by this Archive job.
    pub(super) protection: &'a ArchiveWriteProtection,
    /// Whether to retain the original contributors to measured byte lengths.
    pub(super) report_origins: bool,
}

impl<'archive> Target<'archive> {
    /// Returns the path the sound of `job` is written to in `format`, or the failure when that
    /// path exceeds the limit of ADR-0248.
    fn file(self, job: &Job, format: SoundFormat) -> Result<PathBuf, ExportFailure> {
        let name = format!("{}.{}", job.stem, format.extension());
        check_path_length(&self.absolute.join(&name))
            .map_err(|limit| failure(job, format!("{name}: {limit}")))?;
        Ok(self.folder.join(name))
    }

    const fn sound_target<'path>(
        self,
        path: &'path Path,
        policy: ConflictPolicy,
        watch: &'path ExportWatch,
    ) -> WemOutputTarget<'path>
    where
        'archive: 'path,
    {
        let target =
            WemOutputTarget::new(path, policy, Some(self.protection), watch.cancel_token());
        if self.report_origins {
            target.with_report_origins()
        } else {
            target
        }
    }
}

/// Writes the sounds of `jobs` into `target`: the jobs of one Entry share one read of it, and the
/// Entries are handled in parallel. The outcomes come in job order.
pub(super) fn run(
    audio: &ArchiveAudio<'_>,
    jobs: &[Job],
    target: Target<'_>,
    options: ExportOptions,
    watch: &ExportWatch,
    capture: CaptureOrigins,
) -> Vec<Outcome> {
    let mut groups: Vec<(SourceEntry, Vec<usize>)> = Vec::new();
    let mut slots: HashMap<SourceEntry, usize> = HashMap::new();
    for (position, job) in jobs.iter().enumerate() {
        let entry = job.source.entry();
        let slot = *slots.entry(entry).or_insert_with(|| {
            groups.push((entry, Vec::new()));
            groups.len() - 1
        });
        if let Some((_, members)) = groups.get_mut(slot) {
            members.push(position);
        }
    }
    let mut outcomes: Vec<(usize, Outcome)> = groups
        .par_iter()
        .flat_map_iter(|(origin, members)| {
            run_group(
                audio,
                jobs,
                (*origin, members),
                (target, options),
                watch,
                capture,
            )
        })
        .collect();
    outcomes.sort_unstable_by_key(|(position, _)| *position);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

fn run_group(
    audio: &ArchiveAudio<'_>,
    jobs: &[Job],
    (origin, members): (SourceEntry, &[usize]),
    (target, options): (Target<'_>, ExportOptions),
    watch: &ExportWatch,
    capture: CaptureOrigins,
) -> Vec<(usize, Outcome)> {
    if watch.is_cancelled() {
        return Vec::new();
    }
    let failed_all = |reason: String| -> Vec<(usize, Outcome)> {
        watch.advance(members.len());
        members
            .iter()
            .filter_map(|&position| {
                Some((position, Err(failure(jobs.get(position)?, reason.clone()))))
            })
            .collect()
    };
    let (index, limit) = match origin {
        SourceEntry::Soundbank(index) => (index, MAX_SOUNDBANK_LEN),
        SourceEntry::Loose(index) => (index, MAX_SOUND_LEN),
    };
    let Some(entry) = audio.archive().entries().get(index) else {
        return failed_all("the Archive no longer holds the Entry".to_owned());
    };
    let observed = audio.archive().observed_entry(entry);
    let bytes = match entry_bytes(audio.archive(), entry, limit) {
        Ok(bytes) => bytes,
        Err(error) => return failed_all(format!("{}: {error}", entry.path())),
    };
    let bank = match origin {
        SourceEntry::Soundbank(_) => match Soundbank::parse(bytes.as_slice()) {
            Ok(bank) => Some(bank),
            Err(error) => return failed_all(format!("{}: {error}", entry.path())),
        },
        SourceEntry::Loose(_) => None,
    };
    let binding = observed
        .as_ref()
        .filter(|_| capture.loops)
        .map(ObservedEntry::binding);
    let input = GroupInput {
        bytes: &bytes,
        bank: bank.as_ref(),
        observed,
        binding: binding.as_ref(),
    };
    run_members(
        jobs,
        members,
        &input,
        entry.path(),
        (target, options, watch),
    )
}

fn run_members(
    jobs: &[Job],
    members: &[usize],
    input: &GroupInput<'_>,
    entry_path: &str,
    (target, options, watch): (Target<'_>, ExportOptions, &ExportWatch),
) -> Vec<(usize, Outcome)> {
    let mut outcomes: Vec<(usize, Outcome)> = members
        .par_iter()
        .filter_map(|&position| {
            if watch.is_cancelled() {
                return None;
            }
            let job = jobs.get(position)?;
            let outcome = match input.for_wem(job.wem_id, job.source) {
                Some(wem) => write_sound(&wem, job, target, (options, watch))?,
                None => Err(failure(
                    job,
                    format!("{entry_path} no longer embeds the WEM"),
                )),
            };
            watch.advance(1);
            Some((position, outcome))
        })
        .collect();
    outcomes.sort_unstable_by_key(|(position, _)| *position);
    outcomes
}

fn failure(job: &Job, reason: String) -> ExportFailure {
    ExportFailure {
        trigger: job.trigger.clone(),
        wem_id: Some(job.wem_id),
        reason,
    }
}

/// What writing one form produced.
struct Emitted {
    size: u64,
    state: ExportState,
    origin_receipt: Option<PathBuf>,
    receipt_failure: Option<String>,
    size_origin: Option<WemWrittenSizeOrigin>,
}

impl Emitted {
    const fn kept(size: u64) -> Self {
        Self {
            size,
            state: ExportState::Kept,
            origin_receipt: None,
            receipt_failure: None,
            size_origin: None,
        }
    }
}

/// Parses the available WEM once for loop metadata and any requested conversion.
fn parse_for(
    bytes: &[u8],
    prefetch: bool,
    format: SoundFormat,
) -> (Option<Wem<&[u8]>>, Option<String>) {
    if prefetch {
        let note = (format != SoundFormat::Raw)
            .then(|| "only the prefetched start of a streamed WEM is available".to_owned());
        return (None, note);
    }
    match Wem::parse(bytes) {
        Ok(wem) => (Some(wem), None),
        Err(_) if format == SoundFormat::Raw => (None, None),
        Err(error) => (None, Some(error.to_string())),
    }
}

fn write_sound(
    input: &SoundInput<'_>,
    job: &Job,
    target: Target<'_>,
    (options, watch): (ExportOptions, &ExportWatch),
) -> Option<Outcome> {
    let (wem, mut note) = parse_for(input.bytes, input.prefetch, options.format);
    let loop_points = wem.as_ref().and_then(Wem::loop_points);
    let chain = formats(wem.as_ref(), options.format);
    for &format in chain {
        let path = match target.file(job, format) {
            Ok(path) => path,
            Err(failed) => return Some(Err(failed)),
        };
        let sound = |emitted: Emitted, note: Option<String>| WrittenSound {
            sound: ExportedSound {
                trigger: job.trigger.clone(),
                wem_id: job.wem_id,
                source: job.source,
                selection: job.selection,
                path: path.clone(),
                origin_receipt: emitted.origin_receipt,
                origin_receipt_failure: emitted.receipt_failure,
                format,
                size: emitted.size,
                state: emitted.state,
                unconverted: note
                    .filter(|_| format == SoundFormat::Raw && options.format != SoundFormat::Raw),
                loop_points,
                prefetch_only: input.prefetch,
                size_origin: emitted.size_origin,
            },
            origin: if emitted.state == ExportState::Written {
                input.origin(job.wem_id, wem.as_ref())
            } else {
                None
            },
        };
        if options.policy == ConflictPolicy::Skip
            && let Ok(existing) = fs::symlink_metadata(&path)
        {
            return Some(Ok(sound(Emitted::kept(existing.len()), note)));
        }
        match emit(
            format,
            wem.as_ref(),
            input.bytes,
            input.observed_source(),
            target.sound_target(&path, options.policy, watch),
            job,
        ) {
            Ok(emitted) => return Some(Ok(sound(emitted, note))),
            Err(
                error @ (ConversionError::OwnedOutput(_) | ConversionError::CleanupFailed { .. }),
            ) => {
                return Some(Err(failure(job, chain_text(&error))));
            }
            Err(_) if watch.is_cancelled() => return None,
            Err(ConversionError::Audio { source, .. }) if format != SoundFormat::Raw => {
                note = Some(source.to_string());
            }
            Err(error) => return Some(Err(failure(job, chain_text(&error)))),
        }
    }
    Some(Err(failure(job, "no form could be written".to_owned())))
}

const fn formats(wem: Option<&Wem<&[u8]>>, requested: SoundFormat) -> &'static [SoundFormat] {
    match (wem, requested) {
        (None, _) | (_, SoundFormat::Raw) => &[SoundFormat::Raw],
        (Some(wem), SoundFormat::Ogg) if matches!(wem.codec(), Codec::Vorbis | Codec::Opus) => {
            &[SoundFormat::Ogg, SoundFormat::Wav, SoundFormat::Raw]
        }
        (Some(_), _) => &[SoundFormat::Wav, SoundFormat::Raw],
    }
}

fn emit(
    format: SoundFormat,
    wem: Option<&Wem<&[u8]>>,
    bytes: &[u8],
    source: Option<SoundOriginInput<'_>>,
    target: WemOutputTarget<'_>,
    job: &Job,
) -> Result<Emitted, ConversionError> {
    let output = match (format, wem) {
        (SoundFormat::Ogg, Some(wem)) => WemOutput::Ogg(wem),
        (SoundFormat::Wav, Some(wem)) => WemOutput::Wav(wem),
        _ => WemOutput::Raw(bytes),
    };
    let published = publish_wem_output(output, source, target, || {
        format!("{} WEM {}", job.trigger, job.wem_id)
    })?;
    Ok(Emitted {
        size: published.artifact.len,
        state: if published.artifact.changed {
            ExportState::Written
        } else {
            ExportState::Kept
        },
        origin_receipt: published.origin_receipt,
        receipt_failure: published.receipt_failure.as_ref().map(chain_text),
        size_origin: published.size_origin,
    })
}

fn chain_text(error: &ConversionError) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
