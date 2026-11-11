// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_source_sound::{self, BankHircSound, BankSoundRead};
use crate::hirc_source::NODE_BASE_VERSION;
use crate::read_at::SeekSource;
use crate::{BankError, BankObservedHeader, BankReadError, HircKind};

use super::{BankHirc, BankHircObject};

/// A physical HIRC occurrence and its initial Sound RTPC disposition.
#[non_exhaustive]
pub struct BankHircRtpc<'input, R> {
    /// Original object envelope.
    pub object: BankHircObject,
    /// Original RTPC observations or their disposition.
    pub status: BankRtpcRead<'input, R>,
}

/// Availability of a Sound's initial RTPC curves.
#[non_exhaustive]
pub enum BankRtpcRead<'input, R> {
    /// The occurrence is another kind.
    NotSound,
    /// The Sound layout is outside the proved version.
    Unsupported {
        /// Original bank version.
        version: u32,
    },
    /// The supported Sound has no node base.
    Absent,
    /// A format error prevented reading the original curve count.
    Unavailable(BankError),
    /// The count and all declared curves fit the original body.
    Readable(BankRtpcRows<'input, R>),
    /// The declared count and complete prefix precede a format error.
    Malformed {
        /// Count and complete curves in source order.
        rows: BankRtpcRows<'input, R>,
        /// Original format failure.
        error: BankError,
    },
}

impl<R: Read + Seek> BankHirc<'_, R> {
    /// Reads the next physical occurrence with its version-154 Sound RTPC rows.
    ///
    /// The observed header must belong to the same bank input.
    pub fn next_sound_rtpc(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircRtpc<'_, R>, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(cause) => return Some(Err(cause)),
        };
        let status = if object.kind(version) != HircKind::Sound {
            BankRtpcRead::NotSound
        } else if version != NODE_BASE_VERSION {
            BankRtpcRead::Unsupported { version }
        } else {
            let sound = match bank_source_sound::observe(self.input, object, version) {
                Ok(sound) => sound,
                Err(cause) => {
                    self.stopped = true;
                    return Some(Err(cause));
                }
            };
            read(self.input, &sound)
        };
        Some(Ok(BankHircRtpc { object, status }))
    }
}

pub(super) fn read<'input, R>(
    input: &'input mut SeekSource<R>,
    sound: &BankHircSound,
) -> BankRtpcRead<'input, R> {
    let extent = sound
        .fields
        .node
        .as_ref()
        .and_then(|node| node.base.as_ref())
        .and_then(|base| base.rtpc_chunk.as_ref());
    match (extent, &sound.status) {
        (Some(extent), BankSoundRead::Malformed(error))
            if extent.checked_curves != extent.curve_count.value =>
        {
            BankRtpcRead::Malformed {
                rows: BankRtpcRows::new(input, extent),
                error: error.clone(),
            }
        }
        (Some(extent), _) => BankRtpcRead::Readable(BankRtpcRows::new(input, extent)),
        (None, BankSoundRead::Decoded(_)) => BankRtpcRead::Absent,
        (None, BankSoundRead::Malformed(error) | BankSoundRead::Unsupported(error)) => {
            BankRtpcRead::Unavailable(error.clone())
        }
    }
}
