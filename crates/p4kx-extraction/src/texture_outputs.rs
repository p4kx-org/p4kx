// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufWriter, Write};

use p4kx_archive::Checksum;
use p4kx_texture::merge;

use crate::ConflictPolicy;
use crate::ExtractionError;
use crate::buffers::Buffers;
use crate::texture_convert::{Source, Step};
use crate::texture_siblings::Capturing;
use crate::tracker::Tracker;
use crate::write::{Chunk, Destination, EntryWriter, Outcome, write_error};

const WRITE_BUFFER_LEN: usize = 256 * 1024;

impl EntryWriter<'_> {
    /// Writes the merged DDS of `source` to `destination` under the Conflict policy; while the
    /// parts stream through, keeps the data of part `capture` in `buffers.data` and reports the
    /// bytes written as progress within `counted`.
    pub(crate) fn write_merged(
        &self,
        (source, destination): (&Source<'_>, &Destination),
        capture: Option<u32>,
        counted: &mut Counted,
        buffers: &mut Buffers,
    ) -> Result<Step, ExtractionError> {
        if destination.existing.is_some() && self.policy == ConflictPolicy::Skip {
            return Ok(Step::Kept);
        }
        let (temporary, file, ownership) = self.temporary(&destination.folder)?;
        let decided = self
            .merge_to(file, source, (capture, counted), buffers)
            .map_err(|error| write_error_at(error, destination))
            .and_then(|(len, crc)| {
                self.decide(
                    destination,
                    || (len, Checksum::Crc32c(crc)),
                    &mut buffers.chunk,
                )
            });
        let step = match decided {
            Ok(None) => step_of(&self.put_in_place(
                &temporary,
                &ownership,
                &destination.file,
                Ok(Chunk::End),
                Outcome::Converted,
            )?),
            Ok(Some(outcome)) => crate::output_cleanup::extraction(
                Ok(step_of(&outcome)),
                &ownership,
                &temporary,
                self.cancel,
            )?,
            Err(error)
                if self.tracker.should_stop()
                    && !crate::guard_cancellation::retained_output(&error) =>
            {
                crate::output_cleanup::extraction(
                    Ok(Step::Cancelled),
                    &ownership,
                    &temporary,
                    self.cancel,
                )?
            }
            Err(error) => {
                return crate::output_cleanup::extraction(
                    Err(error),
                    &ownership,
                    &temporary,
                    self.cancel,
                );
            }
        };
        Ok(step)
    }

    /// Streams the merged DDS of `source` into `file` and returns its length and CRC-32C.
    fn merge_to(
        &self,
        file: File,
        source: &Source<'_>,
        (capture, counted): (Option<u32>, &mut Counted),
        buffers: &mut Buffers,
    ) -> Result<(u64, u32), MergeError> {
        let mut sink = MergeSink {
            writer: BufWriter::with_capacity(WRITE_BUFFER_LEN, file),
            tracker: self.tracker,
            counted,
            len: 0,
            crc: 0,
        };
        let siblings = Capturing {
            parts: source.texture.parts(),
            capture,
            data: RefCell::new(std::mem::take(&mut buffers.data)),
        };
        let merged = self.open_entry(source.header).and_then(|header| {
            merge(
                header,
                source.header.uncompressed_size(),
                &siblings,
                &mut sink,
            )
            .map_err(|error| source.texture_error(error))
        });
        buffers.data = siblings.data.into_inner();
        merged.map_err(MergeError::Merge)?;
        let (len, crc) = (sink.len, sink.crc);
        sink.writer
            .into_inner()
            .map_err(|error| MergeError::Close(error.into_error()))?;
        Ok((len, crc))
    }
}

/// Why streaming a merged DDS into its temporary file failed.
enum MergeError {
    Merge(ExtractionError),
    Close(io::Error),
}

fn write_error_at(error: MergeError, destination: &Destination) -> ExtractionError {
    match error {
        MergeError::Merge(error) => error,
        MergeError::Close(source) => write_error(destination.file.clone(), source),
    }
}

pub(crate) const fn step_of(outcome: &Outcome) -> Step {
    match outcome {
        Outcome::Kept => Step::Kept,
        Outcome::Cancelled => Step::Cancelled,
        _ => Step::Written,
    }
}

/// The progress a texture's merges reported, at most the size of the texture's Entries.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Counted {
    pub(crate) reported: u64,
    pub(crate) budget: u64,
}

/// The merged file being written: counts its length and CRC-32C, reports progress and stops once
/// the run stops.
struct MergeSink<'t, 'c> {
    writer: BufWriter<File>,
    tracker: &'t Tracker<'t>,
    counted: &'c mut Counted,
    len: u64,
    crc: u32,
}

impl Write for MergeSink<'_, '_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.tracker.should_stop() {
            return Err(io::Error::other("the Extraction stopped"));
        }
        let count = self.writer.write(data)?;
        let written = data.get(..count).unwrap_or_default();
        self.crc = crc32c::crc32c_append(self.crc, written);
        self.len = self.len.saturating_add(count as u64);
        let free = self.counted.budget.saturating_sub(self.counted.reported);
        let progress = free.min(count as u64);
        if progress > 0 {
            self.counted.reported += progress;
            self.tracker.wrote(progress);
        }
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
