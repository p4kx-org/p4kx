// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_dataforge::{Annotations, Record};

use crate::buffers::Buffers;
use crate::decoded_buffer::{AppendError, DecodedBuffer, MAX_ADDRESSABLE_BYTES};
use crate::record_export::{RecordRun, write_converted_record_xml};
use crate::source::SourceEntry;
use crate::write::{Chunk, EntryWriter, Outcome};
use crate::{DataForgeEntry, ExtractionError, RecordExportReport};

impl EntryWriter<'_> {
    /// Converts the DataForge Entry `entry` into one Record XML file per Record below the folder
    /// of its path, and reports its end to the tracker.
    ///
    /// Each Record file is handled by the Conflict policy on its own. The Entry's size is counted
    /// as progress, half while it is read and the rest in equal parts as the Records end. Once
    /// the run is to stop, the Records not yet written are cancelled and so is the Entry; Records
    /// written before stay.
    pub(crate) fn write_dataforge(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
    ) -> (Outcome, Option<RecordExportReport>) {
        let credited = AtomicU64::new(0);
        let (outcome, records) = match self.try_write_dataforge(entry, buffers, &credited) {
            Ok(done) => done,
            Err(error) if crate::guard_cancellation::retained_output(&error) => {
                (Outcome::Failed(error), None)
            }
            Err(_) if self.tracker.archive_file_changed() => (Outcome::Cancelled, None),
            Err(error) => (Outcome::Failed(error), None),
        };
        buffers.trim();
        self.tracker.ended(entry, credited.into_inner());
        (outcome, records)
    }

    fn try_write_dataforge(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        credited: &AtomicU64,
    ) -> Result<(Outcome, Option<RecordExportReport>), ExtractionError> {
        if self.tracker.should_stop() {
            return Ok((Outcome::Cancelled, None));
        }
        let relative = self.relative_path(entry)?;
        let size = entry.uncompressed_size();
        let Some(input) = self.read_dataforge(entry, buffers, credited)? else {
            return Ok((Outcome::Cancelled, None));
        };
        let forge = input.open().map_err(ExtractionError::DataForge)?;
        let folder = self.folder(relative.parent())?;
        let records: Vec<Record<'_>> = forge.records().collect();
        let rest = size.saturating_sub(credited.load(Ordering::Relaxed));
        let share = rest.checked_div(records.len() as u64).unwrap_or_default();
        let run = RecordRun {
            protection: self.protection,
            cancel: self.cancel,
            policy: self.policy,
            annotations: Annotations::none(),
            stop: &|| self.tracker.should_stop(),
            ended: &|| {
                credited.fetch_add(share, Ordering::Relaxed);
                self.tracker.wrote(share);
            },
        };
        let report = write_converted_record_xml(&input, &forge, &records, &folder, &run);
        let outcome = if report.cancelled() > 0 {
            Outcome::Cancelled
        } else {
            Outcome::Converted(None)
        };
        Ok((outcome, Some(report)))
    }

    fn read_dataforge(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        credited: &AtomicU64,
    ) -> Result<Option<DataForgeEntry>, ExtractionError> {
        let size = entry.uncompressed_size();
        let expected = DecodedBuffer::new(size).ok_or(ExtractionError::TooLargeToConvert {
            size,
            limit: MAX_ADDRESSABLE_BYTES,
        })?;
        let bytes = &mut buffers.data;
        bytes.clear();
        let mut reader = entry.open_exact(self.verify)?;
        if !self.read_all(&mut reader, bytes, expected, &mut buffers.chunk, credited)? {
            return Ok(None);
        }
        Ok(Some(DataForgeEntry::from_decoded_source(
            entry,
            &reader,
            std::mem::take(bytes),
        )))
    }

    /// Reads the declared Entry length, crediting half of each received chunk until cancellation.
    pub(crate) fn read_all(
        &self,
        reader: &mut impl Read,
        bytes: &mut Vec<u8>,
        expected: DecodedBuffer,
        chunk: &mut [u8],
        credited: &AtomicU64,
    ) -> Result<bool, ExtractionError> {
        loop {
            match self
                .read_chunk(reader, chunk)
                .map_err(ExtractionError::read)?
            {
                Chunk::Data(count) => {
                    let data = chunk.get(..count).unwrap_or_default();
                    expected.append(bytes, data).map_err(|error| match error {
                        AppendError::Overlong(source) => ExtractionError::read(source),
                        AppendError::Allocation { requested, source } => {
                            ExtractionError::ConversionAllocation { requested, source }
                        }
                    })?;
                    let half = count as u64 / 2;
                    credited.fetch_add(half, Ordering::Relaxed);
                    self.tracker.wrote(half);
                }
                Chunk::End => {
                    expected.finish(bytes).map_err(ExtractionError::read)?;
                    return Ok(true);
                }
                Chunk::Cancelled => return Ok(false),
            }
        }
    }
}
