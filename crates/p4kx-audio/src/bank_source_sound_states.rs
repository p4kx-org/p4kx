// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_inline_state_rows::BankInlineStateRows;
use crate::bank_source_sound::{self, BankSoundRead};
use crate::hirc_source::NODE_BASE_VERSION;
use crate::{BankError, BankObservedHeader, BankReadError, HircKind};

use super::{BankHirc, BankHircObject};

/// A physical HIRC occurrence and the disposition of its v154 Sound State chunk.
#[non_exhaustive]
pub struct BankHircSoundStates<'input, R> {
    /// The original physical envelope.
    pub object: BankHircObject,
    /// The observed State disposition.
    pub status: BankSoundStateRead<'input, R>,
}

/// The availability of a Sound's inline State chunk.
#[non_exhaustive]
pub enum BankSoundStateRead<'input, R> {
    /// The occurrence is another kind.
    NotSound,
    /// The Sound layout is outside the supported inline version.
    Unsupported {
        /// Original BKHD version.
        version: u32,
    },
    /// The supported Sound has no node base.
    Absent,
    /// A source format error prevented establishing the State extent.
    Unavailable(BankError),
    /// The reader can replay the checked State extent.
    Readable(BankInlineStateRows<'input, R>),
}

impl<R: Read + Seek> BankHirc<'_, R> {
    /// Reads the next physical occurrence with its version-154 Sound State rows.
    ///
    /// The observed header must belong to the same bank input.
    pub fn next_sound_states(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSoundStates<'_, R>, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(cause) => return Some(Err(cause)),
        };
        let status = if object.kind(version) != HircKind::Sound {
            BankSoundStateRead::NotSound
        } else if version != NODE_BASE_VERSION {
            BankSoundStateRead::Unsupported { version }
        } else {
            let sound = match bank_source_sound::observe(self.input, object, version) {
                Ok(sound) => sound,
                Err(cause) => {
                    self.stopped = true;
                    return Some(Err(cause));
                }
            };
            let extent = sound
                .fields
                .node
                .and_then(|node| node.base)
                .and_then(|base| base.state_chunk);
            match (extent, sound.status) {
                (Some(range), _) => {
                    BankSoundStateRead::Readable(BankInlineStateRows::new(self.input, range))
                }
                (None, BankSoundRead::Decoded(_)) => BankSoundStateRead::Absent,
                (None, BankSoundRead::Malformed(error) | BankSoundRead::Unsupported(error)) => {
                    BankSoundStateRead::Unavailable(error)
                }
            }
        };
        Some(Ok(BankHircSoundStates { object, status }))
    }
}
