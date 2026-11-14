// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;

use p4kx_texture::TextureError;

use crate::buffers::Buffers;
use crate::extract::Ended;
use crate::source::SourceEntry;
use crate::texture_job::TextureJob;
use crate::texture_outputs::Counted;
use crate::texture_parts::Parts;
use crate::write::{EntryWriter, Outcome};
use crate::{ConversionError, EntryFailure, ExtractionError, Texture, TextureArtifacts};

/// How one output of a texture ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The file was written.
    Written,
    /// The file already there was kept under the Conflict policy.
    Kept,
    /// The run stopped.
    Cancelled,
}

/// What converting a texture got done before it ended or failed.
#[derive(Debug, Default)]
struct Done {
    merged: Option<Step>,
    merged_tail: Option<Step>,
    png: Option<Step>,
    counted: Counted,
    artifacts: Option<TextureArtifacts>,
}

impl Done {
    const fn steps(&self) -> [Option<Step>; 3] {
        [self.merged, self.merged_tail, self.png]
    }

    fn cancelled(&self) -> bool {
        self.steps().contains(&Some(Step::Cancelled))
    }

    fn outcome(&self) -> Outcome {
        if self.steps().contains(&Some(Step::Written)) {
            Outcome::Converted
        } else {
            Outcome::Kept
        }
    }

    /// Returns whether a merged file holds the path of `entry`, so its raw data is not written.
    fn holds_path_of(&self, job: &TextureJob<'_>, entry: &SourceEntry<'_>) -> bool {
        let in_place = |step: Option<Step>| matches!(step, Some(Step::Written | Step::Kept));
        (job.header.1.index() == entry.index() && in_place(self.merged))
            || (job
                .alpha_header
                .as_ref()
                .is_some_and(|tail| tail.index() == entry.index())
                && in_place(self.merged_tail))
    }

    /// Takes up to `size` bytes of the merge progress already reported for one Entry.
    fn take_counted(&mut self, size: u64) -> u64 {
        let taken = self.counted.reported.min(size);
        self.counted.reported -= taken;
        taken
    }
}

/// The texture being converted, with its alpha tail, its header Entry and its number of parts.
pub(crate) struct Source<'j> {
    pub(crate) texture: Texture<'j>,
    pub(crate) header: &'j SourceEntry<'j>,
    pub(crate) part_count: u32,
}

impl<'j> Source<'j> {
    fn new(texture: Texture<'j>, header: &'j SourceEntry<'j>) -> Result<Self, ExtractionError> {
        let part_count = texture.part_count().map_err(|error| {
            ExtractionError::Texture(ConversionError::of_texture(texture.name(), error))
        })?;
        Ok(Self {
            texture,
            header,
            part_count,
        })
    }

    pub(crate) fn texture_error(&self, error: TextureError) -> ExtractionError {
        ExtractionError::Texture(ConversionError::of_texture(self.texture.name(), error))
    }
}

impl EntryWriter<'_> {
    /// Converts the texture of `job` as the run's texture options say and reports the end of each
    /// of its Entries; a texture that fails is reported and its raw Entries are written instead,
    /// except those whose path a merged file already took.
    pub(crate) fn write_texture(&self, job: &TextureJob<'_>, buffers: &mut Buffers) -> Ended {
        let mut done = Done {
            counted: Counted {
                reported: 0,
                budget: job
                    .entries()
                    .map(|(_, entry)| entry.uncompressed_size())
                    .sum(),
            },
            ..Done::default()
        };
        let failure = match self.convert_texture(job, buffers, &mut done) {
            Ok(()) => None,
            Err(_) if self.tracker.archive_file_changed() => {
                done.merged = Some(Step::Cancelled);
                None
            }
            Err(error) => Some(error),
        };
        let raw = self.textures.keep_raw || failure.is_some();
        let failure = failure.filter(|error| !error.is_path_error());
        let cancelled = done.cancelled();
        let (raw_entries, converted): (Vec<_>, Vec<_>) = job
            .entries()
            .partition(|(_, entry)| !cancelled && raw && !done.holds_path_of(job, entry));
        let mut entries = Vec::with_capacity(job.requested.len() + 1);
        for (index, entry) in converted {
            let counted = done.take_counted(entry.uncompressed_size());
            self.tracker.ended(entry, counted);
            let outcome = if cancelled {
                Outcome::Cancelled
            } else {
                done.outcome()
            };
            entries.push((index, entry.path().to_owned(), outcome));
        }
        self.tracker.retract(done.counted.reported);
        for (index, entry) in raw_entries {
            let outcome = match self.write_raw(entry, buffers) {
                Outcome::Written if failure.is_none() => Outcome::Converted,
                outcome => outcome,
            };
            entries.push((index, entry.path().to_owned(), outcome));
        }
        buffers.trim();
        let unconverted = failure.map(|error| {
            let (index, header) = job.header;
            let path = header.path().to_owned();
            (index, EntryFailure { path, error })
        });
        Ended::Texture {
            entries,
            unconverted,
            artifacts: done.artifacts.map(|artifacts| (job.header.0, artifacts)),
        }
    }

    fn convert_texture(
        &self,
        job: &TextureJob<'_>,
        buffers: &mut Buffers,
        done: &mut Done,
    ) -> Result<(), ExtractionError> {
        if self.tracker.should_stop() {
            done.merged = Some(Step::Cancelled);
            return Ok(());
        }
        let (_, header) = job.header;
        let destination = self.destination(header)?;
        let source = self.open_source(job)?;
        let dds = source.texture.header();
        let level = self
            .textures
            .png
            .map(|level| level.min(dds.mip_count().saturating_sub(1)));
        let capture = level
            .filter(|&level| {
                level < source.part_count
                    && (dds.format().is_decodable() || dds.format().is_high_range())
            })
            .map(|level| source.part_count - level);
        buffers.data.clear();
        if self.textures.merge {
            let step =
                self.write_merged((&source, &destination), capture, &mut done.counted, buffers)?;
            done.merged = Some(step);
            if step == Step::Cancelled {
                return Ok(());
            }
        }
        let Some(level) = level else {
            return self.write_merged_tail(job, &source, buffers, done);
        };
        done.png =
            Some(self.write_images(&source, &destination, level, buffers, &mut done.artifacts)?);
        Ok(())
    }

    /// Writes the alpha tail merged like any split texture under the tail header's path, so the
    /// tail is not lost when no PNG carries it.
    fn write_merged_tail(
        &self,
        job: &TextureJob<'_>,
        source: &Source<'_>,
        buffers: &mut Buffers,
        done: &mut Done,
    ) -> Result<(), ExtractionError> {
        let (true, Some(tail_header)) = (self.textures.merge, job.alpha_header.as_ref()) else {
            return Ok(());
        };
        let Some(tail) = source.texture.alpha_tail() else {
            return Ok(());
        };
        let tail = Source::new(tail.map_err(ExtractionError::Texture)?, tail_header)?;
        let destination = self.destination(tail_header)?;
        done.merged_tail =
            Some(self.write_merged((&tail, &destination), None, &mut done.counted, buffers)?);
        Ok(())
    }

    fn open_source<'j>(&'j self, job: &'j TextureJob<'_>) -> Result<Source<'j>, ExtractionError> {
        let (_, header) = job.header;
        let colour = Parts::Source {
            header: *header,
            parts: job.parts.clone(),
            verify: self.verify,
        };
        let alpha = job.alpha_header.as_ref().map(|alpha_header| Parts::Source {
            header: *alpha_header,
            parts: job.alpha_parts.clone(),
            verify: self.verify,
        });
        if self.verify {
            Self::verify_header(header)?;
            if let Some(alpha_header) = &job.alpha_header {
                Self::verify_header(alpha_header)?;
            }
        }
        let texture = Texture::from_parts(colour, alpha).map_err(ExtractionError::Texture)?;
        Source::new(texture, header)
    }

    /// Reads a header Entry to its end through the verified reader, since a Conversion parses
    /// only a prefix of it.
    fn verify_header(header: &SourceEntry<'_>) -> Result<(), ExtractionError> {
        let fail = |source: io::Error| {
            ExtractionError::Texture(ConversionError::Read {
                path: PathBuf::from(header.path()),
                source,
            })
        };
        let mut reader = header
            .open(true)
            .map_err(|error| fail(io::Error::other(error)))?;
        io::copy(&mut reader, &mut io::sink())
            .map(drop)
            .map_err(fail)
    }
}
