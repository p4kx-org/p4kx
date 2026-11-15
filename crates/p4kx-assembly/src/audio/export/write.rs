// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_archive::ObservedEntry;
use p4kx_audio::{AudioError, Codec, Soundbank, Wem};
use p4kx_extraction::{
    ArchiveWriteProtection, ConflictPolicy, ConversionError, MAX_SOUND_LEN, check_path_length,
    write_replacing_counted_protected,
};
use rayon::prelude::*;

#[path = "write_input.rs"]
mod input;

use self::input::{GroupInput, SoundInput};
use super::loop_origins::CaptureLoopOrigins;
use super::plan::{Job, Origin};
use super::{
    ExportFailure, ExportLoopOrigin, ExportOptions, ExportState, ExportWatch, ExportedSound,
    SoundFormat,
};
use crate::audio::read::entry_bytes;
use crate::audio::{ArchiveAudio, MAX_SOUNDBANK_LEN};

type Outcome = Result<WrittenSound, ExportFailure>;

pub(super) struct WrittenSound {
    pub(super) sound: ExportedSound,
    pub(super) loop_origin: Option<ExportLoopOrigin>,
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
}

impl Target<'_> {
    /// Returns the path the sound of `job` is written to in `format`, or the failure when that
    /// path exceeds the limit of ADR-0248.
    fn file(self, job: &Job, format: SoundFormat) -> Result<PathBuf, ExportFailure> {
        let name = format!("{}.{}", job.stem, format.extension());
        check_path_length(&self.absolute.join(&name))
            .map_err(|limit| failure(job, format!("{name}: {limit}")))?;
        Ok(self.folder.join(name))
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
    capture: CaptureLoopOrigins,
) -> Vec<Outcome> {
    let mut groups: Vec<(Origin, Vec<usize>)> = Vec::new();
    let mut slots: HashMap<Origin, usize> = HashMap::new();
    for (position, job) in jobs.iter().enumerate() {
        let slot = *slots.entry(job.origin).or_insert_with(|| {
            groups.push((job.origin, Vec::new()));
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
    (origin, members): (Origin, &[usize]),
    (target, options): (Target<'_>, ExportOptions),
    watch: &ExportWatch,
    capture: CaptureLoopOrigins,
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
        Origin::Soundbank(index) => (index, MAX_SOUNDBANK_LEN),
        Origin::Loose(index) => (index, MAX_SOUND_LEN),
    };
    let Some(entry) = audio.archive().entries().get(index) else {
        return failed_all("the Archive no longer holds the Entry".to_owned());
    };
    let bytes = match entry_bytes(audio.archive(), entry, limit) {
        Ok(bytes) => bytes,
        Err(error) => return failed_all(format!("{}: {error}", entry.path())),
    };
    let bank = match origin {
        Origin::Soundbank(_) => match Soundbank::parse(bytes.as_slice()) {
            Ok(bank) => Some(bank),
            Err(error) => return failed_all(format!("{}: {error}", entry.path())),
        },
        Origin::Loose(_) => None,
    };
    let binding = match capture {
        CaptureLoopOrigins::Omit => None,
        CaptureLoopOrigins::Retain => audio
            .archive()
            .observed_entry(entry)
            .as_ref()
            .map(ObservedEntry::binding),
    };
    let input = GroupInput {
        bytes: &bytes,
        bank: bank.as_ref(),
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
            let outcome = match input.for_wem(job.wem_id) {
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
                path: path.clone(),
                format,
                size: emitted.size,
                state: emitted.state,
                unconverted: note
                    .filter(|_| format == SoundFormat::Raw && options.format != SoundFormat::Raw),
                loop_points,
                prefetch_only: input.prefetch,
            },
            loop_origin: if emitted.state == ExportState::Written {
                input.loop_origin(job.wem_id, wem.as_ref())
            } else {
                None
            },
        };
        if options.policy == ConflictPolicy::Skip
            && let Ok(existing) = fs::symlink_metadata(&path)
        {
            let emitted = Emitted {
                size: existing.len(),
                state: ExportState::Kept,
            };
            return Some(Ok(sound(emitted, note)));
        }
        match emit(
            format,
            wem.as_ref(),
            input.bytes,
            &path,
            Some(target.protection),
            (job, options, watch),
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

/// Fails a write once the export is cancelled, so the file being written is dropped.
struct Guarded<'w, 'o> {
    inner: &'w mut dyn Write,
    watch: &'o ExportWatch,
}

impl Write for Guarded<'_, '_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.watch.is_cancelled() {
            return Err(io::Error::other("the export was cancelled"));
        }
        self.inner.write(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn emit(
    format: SoundFormat,
    wem: Option<&Wem<&[u8]>>,
    bytes: &[u8],
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    (job, options, watch): (&Job, ExportOptions, &ExportWatch),
) -> Result<Emitted, ConversionError> {
    let convert = |source: AudioError, path: &Path| match source {
        AudioError::Write(source) => ConversionError::Write {
            path: path.to_path_buf(),
            source,
        },
        source => ConversionError::Audio {
            sound: format!("{} WEM {}", job.trigger, job.wem_id),
            source,
        },
    };
    let replaced = write_replacing_counted_protected(
        path,
        options.policy == ConflictPolicy::IfChanged,
        protection,
        Some(&watch.token),
        |out| {
            let mut guarded = Guarded { inner: out, watch };
            match (format, wem) {
                (SoundFormat::Ogg, Some(wem)) => wem.write_ogg(&mut guarded).map(drop),
                (SoundFormat::Wav, Some(wem)) => wem.write_wav(&mut guarded).map(drop),
                _ => guarded.write_all(bytes).map_err(AudioError::Write),
            }
            .map_err(|source| convert(source, path))
        },
    )?;
    Ok(Emitted {
        size: replaced.len,
        state: if replaced.changed {
            ExportState::Written
        } else {
            ExportState::Kept
        },
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
