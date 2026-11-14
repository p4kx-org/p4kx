// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded verified reads for nested shader test discovery.

use std::io::Read;

use p4kx_extraction::{Archive, Entry};
use p4kx_media::{ContainerIdentity, Identity};
use sha2::{Digest as _, Sha256};

use super::super::TestResult;
use super::super::shader_expected::{ExpectedCounts, ExpectedSource};
use super::{
    Budget, CONTAINER_BYTES, CONTAINERS, DEFINITION_SOURCES, DISCOVERY_BYTES, ENTRY_BYTES, Family,
    SOURCE_BYTES,
};

impl Family<'_> {
    pub(super) fn read_source(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        chain: &[ContainerIdentity],
        budget: &Budget<'_>,
    ) -> TestResult {
        if self.sources.len() == DEFINITION_SOURCES || entry.uncompressed_size() > ENTRY_BYTES {
            return Err("selected shader definition exceeds verification source bounds".into());
        }
        self.source_bytes = self
            .source_bytes
            .checked_add(entry.uncompressed_size())
            .filter(|bytes| *bytes <= SOURCE_BYTES)
            .ok_or("shader source byte budget exceeded")?;
        let mut bytes = Vec::with_capacity(usize::try_from(entry.uncompressed_size())?);
        archive
            .open_entry_verified(entry)?
            .take(ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len())? != entry.uncompressed_size() {
            return Err("selected shader read is truncated or exceeds its declared size".into());
        }
        let (counts, failure) =
            match p4kx_shader::read_with(&bytes, p4kx_shader::Limits::default(), || {
                budget.checkpoint()
            }) {
                Ok(definition) => (Some(ExpectedCounts::of(&definition)?), None),
                Err(p4kx_shader::ReadError::Syntax(error)) => (None, Some(error)),
                Err(p4kx_shader::ReadError::Interrupted(error)) => return Err(error),
                Err(_) => return Err("unsupported shader reader result".into()),
            };
        let records = match &counts {
            Some(counts) => u64::try_from(counts.nodes.len())?
                .checked_add(1)
                .ok_or("shader record count overflow")?,
            None => 1,
        };
        self.expected_records = self
            .expected_records
            .checked_add(records)
            .filter(|count| *count <= super::super::shader_record_accounting::MAX_RECORDS)
            .ok_or("source declaration record budget exceeded")?;
        self.sources.push(ExpectedSource {
            identity: Identity::Entry {
                containers: chain.to_vec(),
                entry: ContainerIdentity::try_new_with_entry(archive, entry)?,
            },
            size: u64::try_from(bytes.len())?,
            digest: p4kx_extraction::Checksum::Sha256(Sha256::digest(&bytes).into()).hex(),
            counts,
            failure,
        });
        budget.checkpoint()
    }
}

impl Budget<'_> {
    pub(super) fn checkpoint(&self) -> TestResult {
        if self.cancel.is_cancelled() {
            return Err("shader verification cancelled".into());
        }
        self.watch.poll()?;
        Ok(())
    }

    pub(super) fn open(&mut self, parent: &Archive, entry: &Entry) -> TestResult<Archive> {
        self.checkpoint()?;
        if self.opened == CONTAINERS || entry.uncompressed_size() > CONTAINER_BYTES {
            return Err("selected shader container exceeds verification bounds".into());
        }
        self.bytes = self
            .bytes
            .checked_add(entry.uncompressed_size())
            .filter(|bytes| *bytes <= DISCOVERY_BYTES)
            .ok_or("shader discovery byte budget exceeded")?;
        self.opened += 1;
        let archive = parent.open_container(entry)?;
        self.checkpoint()?;
        Ok(archive)
    }
}
