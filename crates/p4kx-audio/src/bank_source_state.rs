// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_object_fields::SeekObjectFields;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_state;
use crate::{BankError, BankObservedHeader, BankReadError, HircKind, StateObject};

use super::{BankHirc, BankHircObject};

/// One physical HIRC occurrence and its standalone State interpretation.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankHircState {
    /// The original object envelope.
    pub object: BankHircObject,
    /// The State fields or their disposition.
    pub status: BankStateRead,
}

/// What the reader can establish about an original standalone State.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankStateRead {
    /// The occurrence is another HIRC kind.
    NotState,
    /// The original State property bundle was read.
    Decoded(StateObject),
    /// The supported bundle did not fit the original object.
    Malformed(BankError),
    /// The standalone State layout is unsupported for this bank version.
    Unsupported {
        /// The original BKHD version.
        version: u32,
    },
}

impl<R: Read + Seek> BankHirc<'_, R> {
    /// Reads the next physical occurrence and interprets standalone State properties.
    ///
    /// The observed header must belong to the same bank input.
    pub fn next_state(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircState, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(cause) => return Some(Err(cause)),
        };
        let result = self.state(object, version);
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }

    pub(super) fn state(
        &mut self,
        object: BankHircObject,
        version: u32,
    ) -> Result<BankHircState, BankReadError> {
        let status = if object.kind(version) != HircKind::State {
            BankStateRead::NotState
        } else if !hirc_state::supports_standalone(version) {
            BankStateRead::Unsupported { version }
        } else {
            let mut fields = SeekObjectFields::after_id(
                self.input,
                object.body_offset(),
                object.body_len(),
                OBJECT_ID_LEN as u64,
            );
            match hirc_state::read_object(&mut fields, 0) {
                Ok(state) => BankStateRead::Decoded(state),
                Err(BankReadError::Format(cause)) => BankStateRead::Malformed(cause),
                Err(cause) => return Err(cause),
            }
        };
        Ok(BankHircState { object, status })
    }
}
