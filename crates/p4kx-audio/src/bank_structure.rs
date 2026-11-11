// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_reader::SectionReader;
use crate::{BankError, SectionId};

pub(crate) const SECTION_HEADER_LEN: usize = 8;
pub(crate) const DIDX_ENTRY_LEN: usize = 12;
pub(crate) const OBJECT_HEADER_LEN: usize = 5;
pub(crate) const OBJECT_ID_LEN: usize = 4;

const UNIQUE_SECTIONS: [SectionId; 5] = [
    SectionId::BKHD,
    SectionId::DIDX,
    SectionId::DATA,
    SectionId::HIRC,
    SectionId::STID,
];

pub(crate) struct SectionLayout {
    pub(crate) id: SectionId,
    pub(crate) body_len: u64,
}

pub(crate) fn section_layout(prefix: &[u8], remaining: u64) -> Result<SectionLayout, BankError> {
    let mut tag = [b'?'; 4];
    for (target, byte) in tag.iter_mut().zip(prefix) {
        *target = *byte;
    }
    let id = SectionId(tag);
    let size = prefix
        .get(4..SECTION_HEADER_LEN)
        .and_then(<[u8]>::first_chunk::<4>)
        .map(|field| u32::from_le_bytes(*field))
        .ok_or(BankError::Truncated {
            section: id,
            len: remaining,
            needed: SECTION_HEADER_LEN as u64,
        })?;
    let body_len = u64::from(size);
    let needed = SECTION_HEADER_LEN as u64 + body_len;
    if needed > remaining {
        return Err(BankError::Truncated {
            section: id,
            len: remaining,
            needed,
        });
    }
    Ok(SectionLayout { id, body_len })
}

#[derive(Default)]
pub(crate) struct SeenSections(u8);

impl SeenSections {
    pub(crate) fn repeat(&mut self, id: SectionId) -> bool {
        let Some(position) = UNIQUE_SECTIONS.iter().position(|&unique| unique == id) else {
            return false;
        };
        let flag = 1 << position;
        let repeated = self.0 & flag != 0;
        self.0 |= flag;
        repeated
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MediaTriplet {
    pub(crate) id: u32,
    pub(crate) offset: u32,
    pub(crate) length: u32,
}

impl MediaTriplet {
    pub(crate) fn read(reader: &mut SectionReader<'_>) -> Result<Self, BankError> {
        Ok(Self {
            id: reader.u32()?,
            offset: reader.u32()?,
            length: reader.u32()?,
        })
    }

    pub(crate) fn range_in(self, data_offset: u64, data_len: u64) -> Result<Range<u64>, BankError> {
        let offset = u64::from(self.offset);
        let end = offset + u64::from(self.length);
        if end > data_len {
            return Err(self.outside_data(data_len));
        }
        let start = data_offset
            .checked_add(offset)
            .ok_or_else(|| self.outside_data(data_len))?;
        let end = data_offset
            .checked_add(end)
            .ok_or_else(|| self.outside_data(data_len))?;
        Ok(start..end)
    }

    pub(crate) const fn outside_data(self, data_len: u64) -> BankError {
        BankError::MediaOutsideData {
            id: self.id,
            offset: self.offset as u64,
            end: self.offset as u64 + self.length as u64,
            data_len,
        }
    }
}

pub(crate) struct HircEnvelope {
    pub(crate) type_code: u8,
    pub(crate) body_len: u32,
}

impl HircEnvelope {
    pub(crate) fn read(reader: &mut SectionReader<'_>) -> Result<Self, BankError> {
        let type_code = reader.u8()?;
        let body_len = reader.u32()?;
        if u64::from(body_len) < OBJECT_ID_LEN as u64 {
            return Err(BankError::invalid(
                SectionId::HIRC,
                "an object is shorter than its ID",
            ));
        }
        Ok(Self {
            type_code,
            body_len,
        })
    }
}
