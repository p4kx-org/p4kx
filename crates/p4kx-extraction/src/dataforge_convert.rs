// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_dataforge::{Annotations, DataForge, Record, RecordXml};

use crate::buffers::Buffers;
use crate::record_export::{RecordRun, write_records};
use crate::record_files::RecordFiles;
use crate::record_format::RecordFileFormat;
use crate::source::SourceEntry;
use crate::write::{Chunk, EntryWriter, Outcome};
use crate::{ExtractionError, MAX_DATAFORGE_SIZE, RecordExportReport};

const INITIAL_RESERVE: u64 = 64 << 20;

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
        let too_large = || ExtractionError::TooLargeToConvert {
            size,
            limit: MAX_DATAFORGE_SIZE,
        };
        let len = usize::try_from(size)
            .ok()
            .filter(|_| size <= MAX_DATAFORGE_SIZE)
            .ok_or_else(too_large)?;
        let bytes = &mut buffers.data;
        bytes.clear();
        let reserve = usize::try_from(size.min(INITIAL_RESERVE)).unwrap_or_default();
        bytes.try_reserve_exact(reserve).map_err(|_| too_large())?;
        let mut reader = self.open_entry(entry)?;
        if !self.read_all(&mut reader, bytes, len, &mut buffers.chunk, credited)? {
            return Ok((Outcome::Cancelled, None));
        }
        let forge = DataForge::open(bytes).map_err(ExtractionError::DataForge)?;
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
        let plan = RecordFiles::plan(&forge, RecordXml::EXTENSION);
        let report = write_records::<RecordXml>(&forge, &plan, &records, &folder, &run);
        let outcome = if report.cancelled() > 0 {
            Outcome::Cancelled
        } else {
            Outcome::Converted
        };
        Ok((outcome, Some(report)))
    }

    /// Reads the Entry into `bytes` through `chunk`, up to `len` bytes, counting half of each chunk
    /// as progress in `credited`; returns `false` once the run is to stop.
    pub(crate) fn read_all(
        &self,
        reader: &mut impl Read,
        bytes: &mut Vec<u8>,
        len: usize,
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
                    if bytes.len().saturating_add(count) > len {
                        return Err(ExtractionError::Read(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "the Entry holds more data than its size",
                        )));
                    }
                    bytes.extend_from_slice(data);
                    let half = count as u64 / 2;
                    credited.fetch_add(half, Ordering::Relaxed);
                    self.tracker.wrote(half);
                }
                Chunk::End => return Ok(true),
                Chunk::Cancelled => return Ok(false),
            }
        }
    }
}
