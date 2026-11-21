// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc_music_track;
use crate::hirc_source::{self, BankMediaSourceFields, SourceFlags};
use crate::{BankError, HircObject, HircValue, SectionId, SoundSource, Soundbank};

/// Original terminal media source fields in physical source order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankMediaSourceTable {
    /// The authored source count for a `MusicTrack`. A Sound has no source count word.
    pub source_count: Option<BankHircField<u32>>,
    /// Exact bank-relative fields for each admitted source occurrence.
    pub sources: Vec<BankMediaSourceFields>,
}

impl<B: AsRef<[u8]>> Soundbank<B> {
    /// Reads original media field spans of a Sound or `MusicTrack` borrowed from this bank.
    ///
    /// Returns `None` when the object has no decoded terminal media layout.
    ///
    /// # Errors
    ///
    /// Returns an error if a terminal object belongs to another bank or its bytes disagree with
    /// the admitted sources.
    pub fn media_source_fields(
        &self,
        object: &HircObject,
    ) -> Result<Option<BankMediaSourceTable>, BankError> {
        let version = self.header().version;
        let offset = object.body.start as u64;
        let table = match &object.value {
            HircValue::Sound(admitted) => {
                let mut reader = self.media_source_input(object)?;
                let mut fields = BankMediaSourceFields::default();
                let read = hirc_source::read_source(
                    version,
                    &mut reader,
                    &mut fields,
                    SourceFlags::Optional,
                )?;
                let expected = SoundSource {
                    parent_id: None,
                    ..*admitted
                };
                if read != expected {
                    return Err(inconsistent());
                }
                BankMediaSourceTable {
                    source_count: None,
                    sources: vec![fields.with_bank_offset(offset)],
                }
            }
            HircValue::MusicTrack(track) => {
                let mut reader = self.media_source_input(object)?;
                track_sources(version, &mut reader, offset, &track.sources)?
            }
            _ => return Ok(None),
        };
        Ok(Some(table))
    }

    fn media_source_input(&self, object: &HircObject) -> Result<SectionReader<'_>, BankError> {
        let objects = self.objects();
        let resident = objects
            .binary_search_by_key(&object.body.start, |candidate| candidate.body.start)
            .ok()
            .and_then(|ordinal| objects.get(ordinal));
        if !resident.is_some_and(|candidate| ptr::eq(candidate, object)) {
            return Err(inconsistent());
        }
        let mut input = SectionReader::new(SectionId::HIRC, self.object_body(object));
        if input.u32()? != object.id {
            return Err(inconsistent());
        }
        Ok(input)
    }
}

fn track_sources(
    version: u32,
    input: &mut SectionReader<'_>,
    offset: u64,
    admitted: &[SoundSource],
) -> Result<BankMediaSourceTable, BankError> {
    let count = input.read_u32()?;
    if usize::try_from(count.value).ok() != Some(admitted.len()) {
        return Err(inconsistent());
    }
    let mut sources = Vec::with_capacity(admitted.len());
    for expected in admitted {
        let read = hirc_music_track::source(version, input)?;
        if read.value != *expected {
            return Err(inconsistent());
        }
        sources.push(read.fields.with_bank_offset(offset));
    }
    Ok(BankMediaSourceTable {
        source_count: Some(count.with_bank_offset(offset)),
        sources,
    })
}

impl BankMediaSourceFields {
    fn with_bank_offset(self, offset: u64) -> Self {
        Self {
            plugin_id: self.plugin_id.map(|word| word.with_bank_offset(offset)),
            storage_code: self.storage_code.map(|word| word.with_bank_offset(offset)),
            source_id: self.source_id.map(|word| word.with_bank_offset(offset)),
            cache_id: self.cache_id.map(|word| word.with_bank_offset(offset)),
            in_memory_size: self
                .in_memory_size
                .map(|word| word.with_bank_offset(offset)),
            source_bits: self.source_bits.map(|word| word.with_bank_offset(offset)),
        }
    }
}

const fn inconsistent() -> BankError {
    BankError::invalid(
        SectionId::HIRC,
        "the original media source differs from the decoded HIRC object",
    )
}
