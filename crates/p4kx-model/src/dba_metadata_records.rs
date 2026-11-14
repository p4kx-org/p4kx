// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::{Enumerate, FusedIterator};
use std::slice::Iter;

use p4kx_chunks::{ChunkFile, ivo};

use crate::animation_dba::{DBA_METADATA_VERSION, METADATA_HEADER_LEN, metadata_parts, next_name};
use crate::animation_read::{RECORD_LEN, corrupt};
use crate::animation_source::SourceChunk;
use crate::{AnimationError, AnimationMetadata, DbaAnimationName};

/// One declared DBA clip name and metadata record in physical order.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct DbaMetadataRecord<'a> {
    /// Original zero-based ordinal in the metadata records and name table.
    pub ordinal: usize,
    /// Exact bytes and position of the corresponding metadata record.
    pub metadata: AnimationMetadata<'a>,
    /// Exact bytes and position of the corresponding name.
    pub name: DbaAnimationName<'a>,
}

/// The declared records of a DBA, independent of its controller data.
pub struct DbaMetadataRecords<'a> {
    file_name: &'a str,
    metadata: SourceChunk<'a>,
    records: Enumerate<Iter<'a, [u8; RECORD_LEN]>>,
    count: usize,
    names: &'a [u8],
    name_at: u64,
    failed: bool,
}

impl<'a> DbaMetadataRecords<'a> {
    /// Opens only the versioned metadata chunk of a DBA.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing, unsupported or truncated metadata record table.
    pub fn read<B: AsRef<[u8]>>(
        file_name: &'a str,
        file: &'a ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        Self::from_chunk(file_name, Self::chunk(file_name, file)?)
    }

    pub(crate) fn chunk<B: AsRef<[u8]>>(
        file_name: &str,
        file: &'a ChunkFile<B>,
    ) -> Result<SourceChunk<'a>, AnimationError> {
        SourceChunk::read(
            file_name,
            file,
            ivo::DBA_METADATA,
            DBA_METADATA_VERSION,
            "DbaMetadata",
        )
    }

    pub(crate) fn from_chunk(
        file_name: &'a str,
        metadata: SourceChunk<'a>,
    ) -> Result<Self, AnimationError> {
        let (records, names) = metadata_parts(metadata.bytes)
            .map_err(|source| corrupt(file_name, "DbaMetadata", source))?;
        let name_at = metadata.at + METADATA_HEADER_LEN as u64 + records.len() as u64;
        let records = records.as_chunks::<RECORD_LEN>().0;
        Ok(Self {
            file_name,
            metadata,
            records: records.iter().enumerate(),
            count: records.len(),
            names,
            name_at,
            failed: false,
        })
    }

    fn record(
        &mut self,
        ordinal: usize,
        record: &'a [u8; RECORD_LEN],
    ) -> Result<DbaMetadataRecord<'a>, AnimationError> {
        let (name, remaining) = next_name(self.names, self.count)
            .map_err(|source| corrupt(self.file_name, "DbaMetadata", source))?;
        let record_at =
            self.metadata.at + METADATA_HEADER_LEN as u64 + ordinal as u64 * RECORD_LEN as u64;
        let name = DbaAnimationName::of(self.name_at, name);
        self.names = remaining;
        self.name_at = name.range().end;
        Ok(DbaMetadataRecord {
            ordinal,
            metadata: AnimationMetadata::of(
                self.metadata.index,
                record_at,
                self.metadata.version,
                record,
            ),
            name,
        })
    }
}

impl<'a> Iterator for DbaMetadataRecords<'a> {
    type Item = Result<DbaMetadataRecord<'a>, AnimationError>;

    /// Advances one declaration, stopping after an unterminated name.
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let (ordinal, record) = self.records.next()?;
        let record = self.record(ordinal, record);
        self.failed = record.is_err();
        Some(record)
    }
}

impl FusedIterator for DbaMetadataRecords<'_> {}
