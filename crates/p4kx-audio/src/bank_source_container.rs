// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc::{self, CONTAINER_VERSION};
use crate::hirc_container::{self, BankContainerFields, Layout};
use crate::read_at::SeekSource;
use crate::{BankError, BankHircField, BankHircObject, BankReadError, HircKind};

/// One original container occurrence with its bounded child reader.
#[non_exhaustive]
pub struct BankHircContainer<'input, R> {
    /// Physical HIRC object.
    pub object: BankHircObject,
    /// Layout interpretation made from the admitted body.
    pub status: BankContainerRead,
    /// Successfully observed original fields and ranges.
    pub fields: BankContainerFields,
    /// Child cursor when the original declared list fits the object.
    pub children: Option<BankContainerChildren<'input, R>>,
}

/// Interpretation of one original container body.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankContainerRead {
    /// The known layout was traversed completely.
    Decoded,
    /// The known layout failed after the readable prefix.
    Malformed(BankError),
    /// The observed bank version has an unsupported container layout.
    Unsupported(BankError),
}

/// One original container child occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankContainerChild {
    /// Zero-based position in the original child list.
    pub ordinal: u32,
    /// Original ID and byte range.
    pub child_id: BankHircField<u32>,
}

/// Fixed-memory reader restricted to one admitted child range.
pub struct BankContainerChildren<'input, R> {
    fields: SeekObjectFields<'input, R>,
    stopped: &'input mut bool,
    ordinal: u32,
    count: u32,
}

impl<R: Read + Seek> Iterator for BankContainerChildren<'_, R> {
    type Item = Result<BankContainerChild, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped || self.ordinal == self.count {
            return None;
        }
        let read = self.fields.read_u32().map(|child_id| BankContainerChild {
            ordinal: self.ordinal,
            child_id,
        });
        if read.is_ok() {
            self.ordinal += 1;
        } else {
            *self.stopped = true;
        }
        Some(read)
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
    kind: HircKind,
    layout: Layout,
) -> Result<BankHircContainer<'input, R>, BankReadError> {
    let mut fields = BankContainerFields::default();
    let status = if hirc::has_unknown_layout(kind, version) {
        BankContainerRead::Unsupported(BankError::UnsupportedObjectLayout { kind, version })
    } else {
        debug_assert_eq!(
            version, CONTAINER_VERSION,
            "the scanner reads the admitted container layout"
        );
        let result = scan(input, &object, layout, &mut fields);
        match result {
            Ok(()) => BankContainerRead::Decoded,
            Err(BankReadError::Format(error)) => BankContainerRead::Malformed(error),
            Err(error) => {
                *stopped = true;
                return Err(error);
            }
        }
    };
    let children = fields
        .children
        .as_ref()
        .zip(fields.child_count.as_ref())
        .map(|(range, count)| child_reader(input, stopped, range, count.value));
    Ok(BankHircContainer {
        object,
        status,
        fields,
        children,
    })
}

fn scan<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: &BankHircObject,
    layout: Layout,
    observed: &mut BankContainerFields,
) -> Result<(), BankReadError> {
    let mut fields = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    let header = hirc_container::head(layout, &mut fields, observed)?;
    fields.skip_bytes(header.child_bytes)?;
    hirc_container::tail(layout, &mut fields, observed)
}

const fn child_reader<'input, R>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    range: &Range<u64>,
    count: u32,
) -> BankContainerChildren<'input, R> {
    BankContainerChildren {
        fields: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
        stopped,
        ordinal: 0,
        count,
    }
}
