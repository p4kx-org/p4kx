// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc;
use crate::hirc_event;
use crate::read_at::SeekSource;
use crate::{
    BankError, BankEventCountEncoding, BankHircField, BankHircObject, BankReadError, HircKind,
};

/// One original HIRC Event envelope with its decoded or diagnosed action list.
#[non_exhaustive]
pub struct BankHircEvent<'input, R> {
    /// Original physical HIRC object.
    pub object: BankHircObject,
    /// Owner-bound Event read state.
    pub status: BankEventRead<'input, R>,
}

/// The Event layout and its original action list, when readable.
#[non_exhaustive]
pub enum BankEventRead<'input, R> {
    /// The original count fits the admitted body.
    Decoded(BankEvent<'input, R>),
    /// The original count was incomplete or exceeded the admitted body.
    Malformed {
        /// Representation selected by the original bank version.
        encoding: BankEventCountEncoding,
        /// Count word when it was read completely.
        count: Option<BankHircField<u32>>,
        /// Original count bytes read before the failure.
        count_prefix: Range<u64>,
        /// Original format diagnosis.
        error: BankError,
    },
    /// The Event layout is unknown for the original bank version.
    Unsupported {
        /// Original unsupported layout diagnosis.
        error: BankError,
    },
}

/// Fixed-memory iterator over action ID words in one admitted Event body.
pub struct BankEvent<'input, R> {
    fields: SeekObjectFields<'input, R>,
    stopped: &'input mut bool,
    count: BankHircField<u32>,
    encoding: BankEventCountEncoding,
    ordinal: u32,
}

/// One original Event action occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankEventAction {
    /// Zero-based position within the Event.
    pub ordinal: u32,
    /// Original action ID word, including its exact range.
    pub action_id: BankHircField<u32>,
}

impl<R> BankEvent<'_, R> {
    /// Returns the original count word and range.
    #[must_use]
    pub const fn declared_count(&self) -> &BankHircField<u32> {
        &self.count
    }

    /// Returns the representation selected by the original version.
    #[must_use]
    pub const fn encoding(&self) -> BankEventCountEncoding {
        self.encoding
    }
}

impl<R: Read + Seek> BankEvent<'_, R> {
    /// Returns residual body bytes after a complete action scan, including an empty range.
    #[must_use]
    pub fn trailing_range(&self) -> Option<Range<u64>> {
        if *self.stopped || self.ordinal != self.count.value {
            return None;
        }
        let start = self.fields.offset();
        Some(start..start + self.fields.remaining_bytes())
    }
}

impl<R: Read + Seek> Iterator for BankEvent<'_, R> {
    type Item = Result<BankEventAction, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped || self.ordinal == self.count.value {
            return None;
        }
        let result = self.fields.read_u32().map(|action_id| BankEventAction {
            ordinal: self.ordinal,
            action_id,
        });
        if result.is_ok() {
            self.ordinal += 1;
        } else {
            *self.stopped = true;
        }
        Some(result)
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
) -> Result<BankHircEvent<'input, R>, BankReadError> {
    let status = if hirc::has_unknown_layout(HircKind::Event, version) {
        BankEventRead::Unsupported {
            error: BankError::UnsupportedObjectLayout {
                kind: HircKind::Event,
                version,
            },
        }
    } else {
        let fields = SeekObjectFields::after_id(
            input,
            object.body_offset(),
            object.body_len(),
            OBJECT_ID_LEN as u64,
        );
        read(
            fields,
            stopped,
            BankEventCountEncoding::for_version(version),
        )?
    };
    Ok(BankHircEvent { object, status })
}

fn read<'input, R: Read + Seek>(
    mut fields: SeekObjectFields<'input, R>,
    stopped: &'input mut bool,
    encoding: BankEventCountEncoding,
) -> Result<BankEventRead<'input, R>, BankReadError> {
    let count_start = fields.offset();
    let count = match hirc_event::count(encoding, &mut fields) {
        Ok(count) => count,
        Err(BankReadError::Format(error)) => {
            return Ok(BankEventRead::Malformed {
                encoding,
                count: None,
                count_prefix: count_start..fields.offset(),
                error,
            });
        }
        Err(error) => {
            *stopped = true;
            return Err(error);
        }
    };
    if let Err(error) = hirc_event::require_actions(count.value, fields.remaining_bytes()) {
        return Ok(BankEventRead::Malformed {
            encoding,
            count_prefix: count.range.clone(),
            count: Some(count),
            error,
        });
    }
    Ok(BankEventRead::Decoded(BankEvent {
        fields,
        stopped,
        count,
        encoding,
        ordinal: 0,
    }))
}
