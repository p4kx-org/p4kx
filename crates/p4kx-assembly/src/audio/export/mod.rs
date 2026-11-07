// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exports the sounds of audio triggers as files named after the trigger.

pub(super) mod name;
mod plan;
mod write;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_audio::Loop;
use p4kx_extraction::{ArchiveWriteProtection, CancelToken, ConflictPolicy, ConversionError};

use self::write::Target;
use super::{ArchiveAudio, TriggerSounds};

/// The file format a sound is exported in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundFormat {
    /// Ogg Vorbis or Ogg Opus rebuilt without re-encoding; a codec without an Ogg form becomes WAV.
    Ogg,
    /// A 16-bit PCM WAV.
    Wav,
    /// The WEM bytes unchanged.
    Raw,
}

impl SoundFormat {
    /// Returns the file extension, without the dot: `ogg`, `wav` or `wem`.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Ogg => "ogg",
            Self::Wav => "wav",
            Self::Raw => "wem",
        }
    }
}

/// The settings of one sound export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    /// The format asked for; a sound that cannot be written in it falls back to WAV, then to raw.
    pub format: SoundFormat,
    /// What to do with a file that already exists.
    pub policy: ConflictPolicy,
}

/// The progress of a sound export and the request to stop it, shared with the thread that runs it.
///
/// A stopped export leaves out the sounds it had not started and drops the one it is writing,
/// leaving no partial file; the files it finished stay.
#[derive(Debug, Default)]
pub struct ExportWatch {
    token: CancelToken,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl ExportWatch {
    /// Creates a watch that also stops the export once `token` is cancelled.
    #[must_use]
    pub fn linked(token: CancelToken) -> Self {
        Self {
            token,
            ..Self::default()
        }
    }

    /// Asks the export to stop at the next write.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Returns whether the export was asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Returns how many sounds are handled so far, written, kept or failed.
    #[must_use]
    pub fn done(&self) -> usize {
        self.done.load(Ordering::Relaxed)
    }

    /// Returns how many sounds the export handles, once it is planned; zero before.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total.load(Ordering::Relaxed)
    }

    pub(super) fn plan(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
    }

    pub(super) fn advance(&self, count: usize) {
        self.done.fetch_add(count, Ordering::Relaxed);
    }
}

/// Whether an exported file was written or kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportState {
    /// The file was written, replacing an existing one.
    Written,
    /// The existing file was kept under the Conflict policy.
    Kept,
}

/// One sound file of an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedSound {
    /// The audio trigger the sound belongs to.
    pub trigger: String,
    /// The WEM ID.
    pub wem_id: u32,
    /// The file.
    pub path: PathBuf,
    /// The format the file is in.
    pub format: SoundFormat,
    /// The size of the file in bytes.
    pub size: u64,
    /// Whether the file was written or kept.
    pub state: ExportState,
    /// Why the sound is written unchanged although a conversion was asked for: it cannot be
    /// decoded, is plug-in media or is only the prefetched start of a streamed WEM.
    pub unconverted: Option<String>,
    /// The loop of the sound in frames, from the header of its WEM, whatever the format written;
    /// `None` for a sound without one, a WEM that cannot be parsed or only a prefetched start.
    pub loop_points: Option<Loop>,
    /// Whether the file holds only the prefetched start of a streamed WEM, not the whole sound.
    pub prefetch_only: bool,
}

/// A sound that was not exported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFailure {
    /// The audio trigger the sound belongs to.
    pub trigger: String,
    /// The WEM ID, or `None` for a source plug-in.
    pub wem_id: Option<u32>,
    /// Why.
    pub reason: String,
}

/// What an export did: the files in trigger and variant order, and the sounds without a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// The files written or kept.
    pub sounds: Vec<ExportedSound>,
    /// The sounds that got no file.
    pub failures: Vec<ExportFailure>,
    /// Whether the export was stopped before it handled every sound.
    pub cancelled: bool,
}

impl ExportReport {
    /// Returns how many files hold a sound that was written unchanged because it could not be
    /// converted.
    #[must_use]
    pub fn unconverted_count(&self) -> usize {
        self.sounds
            .iter()
            .filter(|sound| sound.unconverted.is_some())
            .count()
    }

    /// Returns how many files were written.
    #[must_use]
    pub fn written_count(&self) -> usize {
        self.sounds
            .iter()
            .filter(|sound| sound.state == ExportState::Written)
            .count()
    }

    /// Returns how many existing files were kept.
    #[must_use]
    pub fn kept_count(&self) -> usize {
        self.sounds
            .iter()
            .filter(|sound| sound.state == ExportState::Kept)
            .count()
    }

    /// Returns whether every sound was exported and converted as asked.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        !self.cancelled && self.failures.is_empty() && self.unconverted_count() == 0
    }
}

impl ArchiveAudio<'_> {
    /// Writes the sounds of `triggers` into `folder` as `<trigger>.<ext>` for a trigger with one
    /// sound and `<trigger>_<WEM ID>.<ext>` for one with several; a WEM several variants lead to
    /// is written once.
    ///
    /// Names follow the portable file-name rule of ADR-0248: a character it forbids becomes `%`
    /// and two hex digits, a name too long for a file becomes `sound`, and a name already taken
    /// under the case fold of that rule gets `_2`, `_3`, … appended in the order of the triggers
    /// and variants, so the names do not depend on the order the files are written in. The sounds
    /// are read and converted in parallel, each soundbank once, and streamed to their files. A
    /// sound that cannot be converted is written unchanged and reported in its
    /// [`ExportedSound::unconverted`]; one that cannot be located, whose path exceeds the limit of
    /// ADR-0248, or that cannot be written is listed in [`ExportReport::failures`] and the others
    /// are still written.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] when `folder` is in a Star Citizen installation or cannot be
    /// created.
    pub fn export_sounds(
        &self,
        triggers: &[&TriggerSounds<'_>],
        folder: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, ConversionError> {
        self.export_sounds_watched(triggers, folder, options, &ExportWatch::default())
    }

    /// Writes the sounds like [`export_sounds`](Self::export_sounds), reporting its progress to
    /// `watch` and stopping when `watch` is cancelled; the report then lists what was handled.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] when `folder` is in a Star Citizen installation or cannot be
    /// created.
    pub fn export_sounds_watched(
        &self,
        triggers: &[&TriggerSounds<'_>],
        folder: &Path,
        options: ExportOptions,
        watch: &ExportWatch,
    ) -> Result<ExportReport, ConversionError> {
        let plan = plan::build(triggers);
        watch.plan(plan.jobs.len());
        let mut report = ExportReport {
            failures: plan.failures,
            ..ExportReport::default()
        };
        if watch.is_cancelled() {
            report.cancelled = !plan.jobs.is_empty();
            return Ok(report);
        }
        let protection = ArchiveWriteProtection::new(self.archive().path(), watch.token.flag())?;
        protection.ensure_writable(folder, watch.token.flag())?;
        if plan.jobs.is_empty() {
            return Ok(report);
        }
        fs::create_dir_all(folder).map_err(|source| ConversionError::Write {
            path: folder.to_path_buf(),
            source,
        })?;
        let absolute = std::path::absolute(folder).map_err(|source| ConversionError::Write {
            path: folder.to_path_buf(),
            source,
        })?;
        let target = Target {
            folder,
            absolute: &absolute,
            protection: &protection,
        };
        for outcome in write::run(self, &plan.jobs, target, options, watch) {
            match outcome {
                Ok(sound) => report.sounds.push(sound),
                Err(failure) => report.failures.push(failure),
            }
        }
        report.cancelled = watch.is_cancelled() && watch.done() < plan.jobs.len();
        Ok(report)
    }
}
