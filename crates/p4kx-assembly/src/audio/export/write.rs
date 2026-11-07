// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_audio::{AudioError, Codec, Loop, Soundbank, Wem};
use p4kx_extraction::{
    ArchiveWriteProtection, ConflictPolicy, ConversionError, MAX_SOUND_LEN, check_path_length,
    write_replacing_counted_protected,
};
use rayon::prelude::*;

use super::plan::{Job, Origin};
use super::{ExportFailure, ExportOptions, ExportState, ExportWatch, ExportedSound, SoundFormat};
use crate::audio::read::entry_bytes;
use crate::audio::{ArchiveAudio, MAX_SOUNDBANK_LEN};

type Outcome = Result<ExportedSound, ExportFailure>;

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
            run_group(audio, jobs, (*origin, members), (target, options), watch)
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
    let mut outcomes: Vec<(usize, Outcome)> = members
        .par_iter()
        .filter_map(|&position| {
            if watch.is_cancelled() {
                return None;
            }
            let job = jobs.get(position)?;
            let outcome = match bank.as_ref().map(|bank| bank.wem(job.wem_id)) {
                None => write_sound(&bytes, false, job, target, (options, watch))?,
                Some(Some(wem)) => write_sound(
                    wem.bytes(),
                    wem.is_prefetch(),
                    job,
                    target,
                    (options, watch),
                )?,
                Some(None) => Err(failure(
                    job,
                    format!("{} no longer embeds the WEM", entry.path()),
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

/// Returns the loop of `wem`, or, when `raw` and no WEM was parsed for a conversion, of the WEM
/// `bytes` hold.
fn loop_of(wem: Option<&Wem<&[u8]>>, bytes: &[u8], raw: bool) -> Option<Loop> {
    match wem {
        Some(wem) => wem.loop_points(),
        None if raw => Wem::parse(bytes).ok().and_then(|wem| wem.loop_points()),
        None => None,
    }
}

/// Parses `bytes` for a conversion to `format`, or returns why they are written unchanged: the
/// format is raw (no reason), only the start of a streamed WEM is there, or the WEM cannot be parsed.
fn parse_for(
    bytes: &[u8],
    prefetch: bool,
    format: SoundFormat,
) -> (Option<Wem<&[u8]>>, Option<String>) {
    if format == SoundFormat::Raw {
        return (None, None);
    }
    if prefetch {
        let note = "only the prefetched start of a streamed WEM is available".to_owned();
        return (None, Some(note));
    }
    match Wem::parse(bytes) {
        Ok(wem) => (Some(wem), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

fn write_sound(
    bytes: &[u8],
    prefetch: bool,
    job: &Job,
    target: Target<'_>,
    (options, watch): (ExportOptions, &ExportWatch),
) -> Option<Outcome> {
    let (wem, mut note) = parse_for(bytes, prefetch, options.format);
    let loop_points = loop_of(
        wem.as_ref(),
        bytes,
        options.format == SoundFormat::Raw && !prefetch,
    );
    let chain = formats(wem.as_ref(), options.format);
    for &format in chain {
        let path = match target.file(job, format) {
            Ok(path) => path,
            Err(failed) => return Some(Err(failed)),
        };
        let sound = |emitted: Emitted, note: Option<String>| ExportedSound {
            trigger: job.trigger.clone(),
            wem_id: job.wem_id,
            path: path.clone(),
            format,
            size: emitted.size,
            state: emitted.state,
            unconverted: note
                .filter(|_| format == SoundFormat::Raw && options.format != SoundFormat::Raw),
            loop_points,
            prefetch_only: prefetch,
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
            bytes,
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
        (None, _) => &[SoundFormat::Raw],
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
