// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufWriter, Write};

use p4kx_archive::Checksum;
use p4kx_texture::MergeSpan;

use crate::buffers::Buffers;
use crate::output_file::StagedOutput;
use crate::texture_convert::{Source, Step};
use crate::texture_siblings::Capturing;
use crate::textures::{FileMergeInputs, FilePartReads};
use crate::tracker::Tracker;
use crate::write::{Destination, EntryWriter, Outcome, write_error};
use crate::{ConflictPolicy, ExtractionError, OriginArtifactDigest, OriginDigestWriter};

#[path = "texture_merge_publication.rs"]
mod publication;

pub(crate) use publication::MergedOutput;

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
        output: &mut MergedOutput,
    ) -> Result<(), ExtractionError> {
        if destination.existing.is_some() && self.policy == ConflictPolicy::Skip {
            output.step = Some(Step::Kept);
            return Ok(());
        }
        let (temporary, file, ownership) = self.temporary(&destination.folder)?;
        let staged = StagedOutput::new(
            &temporary,
            &destination.file,
            &ownership,
            self.protection,
            self.cancel,
        );
        let decided = self
            .merge_to(file, source, (capture, counted), buffers)
            .map_err(|error| write_error_at(error, destination))
            .and_then(|bytes| {
                self.decide(
                    destination,
                    || (bytes.len, Checksum::Crc32c(bytes.crc)),
                    &mut buffers.chunk,
                )
                .map(|decision| (decision, bytes.evidence))
            });
        let pending = match decided {
            Ok((None, evidence)) => {
                let published = self.publish_merge(source, &staged, evidence, output);
                if output.step == Some(Step::Written) {
                    return published;
                }
                if published.is_err() && destination.existing.is_some() {
                    output.step = Some(Step::Kept);
                }
                published
            }
            Ok((Some(outcome), _)) => {
                output.step = Some(step_of(&outcome));
                Ok(())
            }
            Err(error) => Err(error),
        };
        let resolved = self.resolve_uncommitted_merge(pending, output);
        crate::output_cleanup::extraction(resolved, &ownership, &temporary, self.cancel)
    }

    /// Streams the merged DDS of `source` into `file` and returns its length and CRC-32C.
    fn merge_to(
        &self,
        file: File,
        source: &Source<'_>,
        (capture, counted): (Option<u32>, &mut Counted),
        buffers: &mut Buffers,
    ) -> Result<MergedBytes, MergeError> {
        let mut sink = MergeSink {
            writer: BufWriter::with_capacity(WRITE_BUFFER_LEN, file),
            tracker: self.tracker,
            counted,
            len: 0,
            crc: 0,
        };
        let file_reads = source
            .header
            .archive_path()
            .is_none()
            .then(|| RefCell::new(FilePartReads::default()));
        let siblings = Capturing {
            parts: source.texture.parts(),
            capture,
            data: RefCell::new(std::mem::take(&mut buffers.data)),
            file_reads: file_reads.as_ref(),
        };
        let merged = self
            .open_entry(source.header)
            .map_err(MergeError::Merge)
            .and_then(|mut header| {
                let mut digest = OriginDigestWriter::new(&mut sink);
                let mut spans = Vec::new();
                p4kx_texture::merge_with_spans(
                    &mut header,
                    source.header.uncompressed_size(),
                    &siblings,
                    &mut digest,
                    |span| {
                        spans.try_reserve(1).map_err(io::Error::other)?;
                        spans.push(span);
                        Ok(())
                    },
                )
                .map_err(|error| MergeError::Merge(source.texture_error(error)))?;
                let digest = digest.finish().map_err(MergeError::Close)?;
                Ok((digest, spans, header.file_origin()))
            });
        buffers.data = siblings.data.into_inner();
        let (digest, spans, header) = merged?;
        let files = file_reads.map(|parts| FileMergeInputs {
            header,
            parts: parts.into_inner(),
        });
        let evidence = MergedEvidence {
            digest,
            spans,
            files,
        };
        let (len, crc) = (sink.len, sink.crc);
        sink.writer
            .into_inner()
            .map_err(|error| MergeError::Close(error.into_error()))?;
        Ok(MergedBytes { len, crc, evidence })
    }
}

struct MergedBytes {
    len: u64,
    crc: u32,
    evidence: MergedEvidence,
}

struct MergedEvidence {
    digest: OriginArtifactDigest,
    spans: Vec<MergeSpan>,
    files: Option<FileMergeInputs>,
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
