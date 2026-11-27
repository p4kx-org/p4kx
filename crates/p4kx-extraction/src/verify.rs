// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::io::Read;

use p4kx_archive::{Archive, Entry};
use rayon::prelude::*;

use crate::observed_verification::{ObservedVerification, VerificationPositions};
use crate::source::SourceEntry;
use crate::tracker::Tracker;
use crate::{ArchiveWatch, CancelToken, EntryFailure, ExtractionError, Progress};

const CHUNK_LEN: usize = 1 << 20;

/// The settings of one verification run.
#[derive(Clone, Copy, Default)]
pub struct VerifyOptions<'a> {
    /// Stops the run once cancelled.
    pub cancel: Option<&'a CancelToken>,
    /// Called from any worker thread whenever an Entry ends or a chunk of data is read; `bytes_done`
    /// counts the decoded bytes read.
    pub progress: Option<&'a (dyn Fn(Progress) + Sync)>,
    /// Checked before the run, periodically during it and after it; a change stops the run.
    pub watch: Option<&'a ArchiveWatch>,
}

impl fmt::Debug for VerifyOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifyOptions")
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("watch", &self.watch)
            .finish()
    }
}

/// The outcome of a verification run.
#[derive(Debug, Default)]
pub struct VerificationReport {
    verified: usize,
    failures: Vec<EntryFailure>,
    cancelled: Vec<String>,
    archive_changed: bool,
}

impl VerificationReport {
    /// Returns the number of Entries whose data matched its checksum.
    #[must_use]
    pub const fn verified(&self) -> usize {
        self.verified
    }

    /// Returns the Entries that failed, in the order they were given: a checksum mismatch, or
    /// data that could not be read or decoded.
    #[must_use]
    pub fn failures(&self) -> &[EntryFailure] {
        &self.failures
    }

    /// Returns the Entries among the [`failures`](Self::failures) whose data did not match the
    /// checksum of the directory.
    pub fn checksum_mismatches(&self) -> impl Iterator<Item = &EntryFailure> {
        self.failures
            .iter()
            .filter(|failure| failure.checksum_mismatch().is_some())
    }

    /// Returns the paths of Entries not checked because the run was cancelled.
    #[must_use]
    pub fn cancelled(&self) -> &[String] {
        &self.cancelled
    }

    /// Returns whether the Archive changed on disk before or during the run, which then stopped.
    #[must_use]
    pub const fn archive_changed(&self) -> bool {
        self.archive_changed
    }

    /// Returns whether every Entry was checked and none failed, and the Archive did not change.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.cancelled.is_empty() && !self.archive_changed
    }
}

#[derive(Default)]
struct Tally {
    verified: usize,
    failures: Vec<(usize, EntryFailure)>,
    cancelled: Vec<usize>,
}

impl Tally {
    fn merge(mut self, mut other: Self) -> Self {
        self.verified += other.verified;
        self.failures.append(&mut other.failures);
        self.cancelled.append(&mut other.cancelled);
        self
    }

    fn report(
        mut self,
        entries: &[&Entry],
        positions: Option<&mut VerificationPositions>,
    ) -> VerificationReport {
        self.failures
            .sort_unstable_by_key(|(position, _)| *position);
        self.cancelled.sort_unstable();
        let cancelled = self
            .cancelled
            .iter()
            .filter_map(|&position| entries.get(position))
            .map(|entry| entry.path().to_owned())
            .collect();
        let failures = if let Some(positions) = positions {
            let (failed, failures) = self.failures.into_iter().unzip();
            positions.failed = failed;
            positions.cancelled = self.cancelled;
            failures
        } else {
            self.failures
                .into_iter()
                .map(|(_, failure)| failure)
                .collect()
        };
        VerificationReport {
            verified: self.verified,
            failures,
            cancelled,
            archive_changed: false,
        }
    }
}

enum Verdict {
    Verified,
    Failed(ExtractionError),
    Cancelled,
}

/// Checks the data of `entries` of `archive` against the checksums of its directory, in
/// parallel on the current rayon pool.
///
/// Every Entry is decrypted and decompressed and its checksum is computed while the data streams
/// by, without a second pass and without holding more than one chunk of an Entry. Nothing is
/// written. A change of the Archive seen by the watch of `options` stops the run like a cancel
/// and is reported.
#[must_use]
pub fn verify(
    archive: &Archive,
    entries: &[&Entry],
    options: &VerifyOptions<'_>,
) -> VerificationReport {
    verify_run(archive, entries, options, None)
}

/// Verifies the original selected Entries and preserves their physical positions in the report.
///
/// This performs the same reads and checks as [verify].
#[must_use]
pub fn verify_observed<'entry>(
    archive: &Archive,
    entries: &'entry [&'entry Entry],
    options: &VerifyOptions<'_>,
) -> ObservedVerification<'entry> {
    let mut positions = VerificationPositions::default();
    let report = verify_run(archive, entries, options, Some(&mut positions));
    ObservedVerification {
        report,
        entries,
        positions,
    }
}

fn verify_run(
    archive: &Archive,
    entries: &[&Entry],
    options: &VerifyOptions<'_>,
    positions: Option<&mut VerificationPositions>,
) -> VerificationReport {
    if options.watch.is_some_and(|watch| watch.check().is_err()) {
        if let Some(positions) = positions {
            positions.cancelled = (0..entries.len()).collect();
        }
        return VerificationReport {
            cancelled: entries
                .iter()
                .map(|entry| entry.path().to_owned())
                .collect(),
            archive_changed: true,
            ..VerificationReport::default()
        };
    }
    let files: Vec<SourceEntry<'_>> = entries
        .iter()
        .map(|entry| SourceEntry::of_archive(archive, entry))
        .collect();
    let tracker = Tracker::new(&files, options.cancel, options.watch, options.progress);
    let tally = entries
        .par_iter()
        .enumerate()
        .fold(
            || (Tally::default(), Vec::new()),
            |(mut tally, mut buffer), (position, entry)| {
                match check(archive, entry, &mut buffer, &tracker) {
                    Verdict::Verified => tally.verified += 1,
                    Verdict::Failed(error) => tally.failures.push((
                        position,
                        EntryFailure {
                            path: entry.path().to_owned(),
                            error,
                        },
                    )),
                    Verdict::Cancelled => tally.cancelled.push(position),
                }
                (tally, buffer)
            },
        )
        .map(|(tally, _)| tally)
        .reduce(Tally::default, Tally::merge);
    let mut report = tally.report(entries, positions);
    report.archive_changed = options.watch.is_some_and(|watch| watch.check().is_err());
    report
}

fn check(archive: &Archive, entry: &Entry, buffer: &mut Vec<u8>, tracker: &Tracker<'_>) -> Verdict {
    let mut read = 0;
    let verdict = read_all(archive, entry, buffer, tracker, &mut read);
    tracker.ended(&SourceEntry::of_archive(archive, entry), read);
    match verdict {
        Err(_) if tracker.archive_file_changed() => Verdict::Cancelled,
        Err(error) => Verdict::Failed(error),
        Ok(true) => Verdict::Verified,
        Ok(false) => Verdict::Cancelled,
    }
}

/// Reads `entry` to its end; returns `false` once the run is to stop. The buffer grows to the
/// size of the largest Entry read, up to one chunk.
fn read_all(
    archive: &Archive,
    entry: &Entry,
    buffer: &mut Vec<u8>,
    tracker: &Tracker<'_>,
    read: &mut u64,
) -> Result<bool, ExtractionError> {
    if tracker.should_stop() {
        return Ok(false);
    }
    let wanted = usize::try_from(entry.uncompressed_size())
        .map_or(CHUNK_LEN, |size| size.clamp(1, CHUNK_LEN));
    if buffer.len() < wanted {
        buffer.resize(wanted, 0);
    }
    let mut reader = archive.open_entry_verified(entry)?;
    loop {
        if tracker.should_stop() {
            return Ok(false);
        }
        match reader.read(buffer) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                *read += count as u64;
                tracker.wrote(count as u64);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(ExtractionError::read(error)),
        }
    }
}
