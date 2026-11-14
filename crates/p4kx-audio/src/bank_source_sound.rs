// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_object_fields::SeekObjectFields;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc;
use crate::hirc_source;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError, BankSoundFields, HircKind, SoundSource};

/// One original physical Sound occurrence and its owner-bound interpretation.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankHircSound {
    /// Original object envelope.
    pub object: BankHircObject,
    /// Sound interpretation made from the admitted body.
    pub status: BankSoundRead,
    /// Original readable source and node fields, when the layout is supported.
    pub fields: BankSoundFields,
}

/// Interpretation of an original Sound body for the observed bank version.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankSoundRead {
    /// The known Sound layout was traversed.
    Decoded(SoundSource),
    /// Format failure relative to the original object body.
    Malformed(BankError),
    /// The Sound layout is unknown for the original bank version.
    Unsupported(BankError),
}

pub(crate) fn observe<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: BankHircObject,
    version: u32,
) -> Result<BankHircSound, BankReadError> {
    let mut fields = BankSoundFields::default();
    let status = if hirc::has_unknown_layout(HircKind::Sound, version) {
        BankSoundRead::Unsupported(BankError::UnsupportedObjectLayout {
            kind: HircKind::Sound,
            version,
        })
    } else {
        let mut reader = SeekObjectFields::after_id(
            input,
            object.body_offset(),
            object.body_len(),
            OBJECT_ID_LEN as u64,
        );
        match hirc_source::read(version, &mut reader, &mut fields) {
            Ok(source) => BankSoundRead::Decoded(source),
            Err(BankReadError::Format(error)) => BankSoundRead::Malformed(error),
            Err(error) => return Err(error),
        }
    };
    Ok(BankHircSound {
        object,
        status,
        fields,
    })
}
