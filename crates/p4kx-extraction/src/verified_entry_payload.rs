// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::OnceLock;

use p4kx_archive::{Archive, Checksum, Entry, EntryReader, ObservedEntry, ObservedEntryBinding};

use crate::ExtractionError;
use crate::origin_artifact::{OriginArtifactDigest, OriginDigestWriter};
use crate::source::{EntrySource, OpenOptions, SourceEntry, SourceReader};

/// The exact decoded bytes of a physically selected Entry written by verified Extraction.
#[derive(Debug, Clone)]
pub struct VerifiedEntryPayload {
    binding: ObservedEntryBinding,
    digest: OriginArtifactDigest,
}

impl VerifiedEntryPayload {
    /// Returns whether the original receipt was for this exact Archive Entry.
    #[must_use]
    pub fn is_for(&self, archive: &Archive, entry: &Entry) -> bool {
        archive
            .observed_entry(entry)
            .is_some_and(|entry| self.binding.is_same_entry(&entry.binding()))
    }

    /// Borrows the measured length and SHA-256 of the original decoded bytes.
    #[must_use]
    pub const fn digest(&self) -> &OriginArtifactDigest {
        &self.digest
    }
}

#[derive(Debug)]
pub(crate) struct VerifiedPayloadSource<'archive> {
    archive: &'archive Archive,
    observed: ObservedEntry<'archive>,
    started: OnceLock<()>,
    measured: OnceLock<OriginArtifactDigest>,
}

impl<'archive> VerifiedPayloadSource<'archive> {
    pub(crate) fn new(archive: &'archive Archive, entry: &'archive Entry) -> Option<Self> {
        Some(Self {
            archive,
            observed: archive.observed_entry(entry)?,
            started: OnceLock::new(),
            measured: OnceLock::new(),
        })
    }

    pub(crate) fn source_entry(&self) -> SourceEntry<'_> {
        let entry = self.observed.entry();
        SourceEntry::new(
            self,
            (entry.path(), entry.uncompressed_size()),
            entry.index(),
        )
    }

    pub(crate) fn into_proof(self) -> Option<VerifiedEntryPayload> {
        let digest = self.measured.into_inner()?;
        (digest.byte_length() == self.observed.entry().uncompressed_size()).then(|| {
            VerifiedEntryPayload {
                binding: self.observed.binding(),
                digest,
            }
        })
    }

    const fn matches(&self, index: usize) -> bool {
        self.observed.entry().index() == index
    }
}

impl EntrySource for VerifiedPayloadSource<'_> {
    fn archive_path(&self) -> Option<&Path> {
        Some(self.archive.path())
    }

    fn observed_entry(&self, index: usize) -> Option<ObservedEntry<'_>> {
        self.matches(index).then_some(self.observed)
    }

    fn open(
        &self,
        index: usize,
        options: OpenOptions,
    ) -> Result<SourceReader<'_>, ExtractionError> {
        if !options.verify || !self.matches(index) || self.started.set(()).is_err() {
            return Err(ExtractionError::Read(io::Error::other(
                "a verified payload requires one original Entry read",
            )));
        }
        let reader = options.archive_reader(self.archive, self.observed.entry())?;
        Ok(SourceReader::Archive(Box::new(VerifiedPayloadReader {
            reader,
            measure: Some(OriginDigestWriter::new(io::sink())),
            completed: &self.measured,
        })))
    }

    fn checksum(&self, index: usize) -> Option<Checksum> {
        self.matches(index)
            .then(|| self.observed.entry().checksum())
    }

    fn raw_path(&self, index: usize) -> Option<&[u8]> {
        if self.matches(index) {
            self.archive.raw_path(index)
        } else {
            None
        }
    }

    fn path_of(&self, index: usize) -> Option<&str> {
        self.matches(index).then(|| self.observed.entry().path())
    }

    fn members(&self, wanted: &(dyn Fn(&str) -> bool + Sync)) -> Vec<SourceEntry<'_>> {
        if wanted(self.observed.entry().path()) {
            vec![self.source_entry()]
        } else {
            Vec::new()
        }
    }
}

struct VerifiedPayloadReader<'archive> {
    reader: EntryReader<'archive>,
    measure: Option<OriginDigestWriter<io::Sink>>,
    completed: &'archive OnceLock<OriginArtifactDigest>,
}

impl Read for VerifiedPayloadReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let count = self.reader.read(buffer)?;
        if count == 0 {
            if let Some(measure) = self.measure.take() {
                self.completed
                    .set(measure.finish()?)
                    .map_err(|_| io::Error::other("verified Entry read already completed"))?;
            }
        } else if let Some(measure) = &mut self.measure {
            let bytes = buffer
                .get(..count)
                .ok_or_else(|| io::Error::other("verified Entry returned an invalid byte count"))?;
            measure.write_all(bytes)?;
        }
        Ok(count)
    }
}

#[cfg(test)]
#[path = "verified_entry_payload_tests.rs"]
mod tests;
