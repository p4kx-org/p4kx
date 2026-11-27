// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Cursor;

use crate::bank_structure::OBJECT_HEADER_LEN;
use crate::{
    BankError, BankHircSemantic, BankObservedHeader, BankReadError, BankSource, BankSourceSection,
    HircObject, SectionId, Soundbank,
};

impl<B: AsRef<[u8]>> Soundbank<B> {
    /// Visits the checked original fields of one object in the owning bank.
    ///
    /// An index outside the decoded object directory has no observation.
    ///
    /// # Errors
    /// Returns a read or format error when the physical envelope disagrees with the bank directory.
    pub fn with_hirc_source_fields<'bank, T>(
        &'bank self,
        index: usize,
        visit: impl for<'read> FnOnce(
            BankObservedHeader,
            BankHircSemantic<'read, Cursor<&'bank [u8]>>,
        ) -> T,
    ) -> Result<Option<T>, BankReadError> {
        let Some(expected) = self.objects.get(index) else {
            return Ok(None);
        };
        let bytes = self.bytes.as_ref();
        let mut source = BankSource::open(Cursor::new(bytes), bytes.len() as u64)?;
        let (header, section) = sections(&mut source)?;
        let mut hierarchy = source.hirc(&section)?;
        let ordinal = u32::try_from(index).map_err(|_| inconsistent())?;
        let offset = expected
            .body
            .start
            .checked_sub(OBJECT_HEADER_LEN)
            .ok_or_else(inconsistent)?;
        hierarchy.start_at(ordinal, offset as u64)?;
        let selected = if header.header().is_supported_version() {
            hierarchy.next_semantic_with_source_fields(&header)
        } else {
            hierarchy
                .next()
                .map(|result| result.map(BankHircSemantic::Raw))
        }
        .ok_or_else(inconsistent)??;
        if !matches_directory(&header, &selected, expected, self.header.version) {
            return Err(inconsistent().into());
        }
        Ok(Some(visit(header, selected)))
    }
}

fn sections(
    source: &mut BankSource<Cursor<&[u8]>>,
) -> Result<(BankObservedHeader, BankSourceSection), BankReadError> {
    let mut directory = source.section_cursor();
    let header = source
        .next_section(&mut directory)
        .ok_or(BankError::NotSoundbank)??;
    let header = source.read_observed_header_for(&header)?;
    while let Some(section) = source.next_section(&mut directory) {
        let section = section?;
        if section.id() == SectionId::HIRC {
            return Ok((header, section));
        }
    }
    Err(inconsistent().into())
}

fn matches_directory<R>(
    header: &BankObservedHeader,
    semantic: &BankHircSemantic<'_, R>,
    expected: &HircObject,
    version: u32,
) -> bool {
    let physical = semantic.object();
    header.header().version == version
        && physical.kind(version) == expected.kind
        && physical.id() == expected.id
        && physical.body_offset() == expected.body.start as u64
        && physical.body_end_exclusive() == expected.body.end as u64
}

const fn inconsistent() -> BankError {
    BankError::invalid(
        SectionId::HIRC,
        "the original object differs from the bank directory",
    )
}
