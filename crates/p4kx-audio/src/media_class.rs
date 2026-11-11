// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

const RIFF: &[u8; 4] = b"RIFF";
const RIFX: &[u8; 4] = b"RIFX";
const WAVE: &[u8; 4] = b"WAVE";
const RMID: &[u8; 4] = b"RMID";
const PLUG: &[u8; 4] = b"PLUG";
const MIDI_HEADER: &[u8; 4] = b"MThd";
const FORM_OFFSET: usize = 8;

/// What a media item of a soundbank holds, told from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaClass {
    /// A RIFF or RIFX WAVE file: a WEM.
    Wem,
    /// A standard MIDI file or a RIFF MIDI file, the media of a Wwise MIDI source.
    Midi,
    /// Wwise plug-in data such as a convolution reverb's impulse response.
    Impulse,
    /// Anything else, including media too short to tell.
    Unknown,
}

/// The original fields consulted to classify a bounded media prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankMediaClassSource {
    /// The admitted range was shorter than the four-byte signature.
    TooShort,
    /// Only the four-byte signature was consulted.
    Signature,
    /// A RIFF signature was present, but the admitted range did not contain the form.
    MissingForm,
    /// The signature at 0..4 and the form at 8..12 determined the class.
    SignatureAndForm,
}

/// The original fields consulted for a bounded media's declared RIFF extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankDeclaredLengthSource {
    /// The admitted range was shorter than the signature and size prefix.
    TooShort,
    /// A non-RIFF signature ruled out a declared RIFF extent.
    Signature,
    /// The four-byte signature and size word supplied a declared extent.
    SignatureAndLength,
}

impl MediaClass {
    /// Returns the class of `bytes`; never reads past their end.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self::inspect(bytes).0
    }

    pub(crate) fn inspect(bytes: &[u8]) -> (Self, BankMediaClassSource) {
        use BankMediaClassSource::{MissingForm, Signature, SignatureAndForm, TooShort};

        let Some(magic) = bytes.first_chunk::<4>() else {
            return (Self::Unknown, TooShort);
        };
        if magic == MIDI_HEADER {
            return (Self::Midi, Signature);
        }
        if magic == PLUG {
            return (Self::Impulse, Signature);
        }
        if magic != RIFF && magic != RIFX {
            return (Self::Unknown, Signature);
        }
        match bytes.get(FORM_OFFSET..FORM_OFFSET + 4) {
            Some(form) if form == WAVE => (Self::Wem, SignatureAndForm),
            Some(form) if form == RMID && magic == RIFF => (Self::Midi, SignatureAndForm),
            Some(_) => (Self::Unknown, SignatureAndForm),
            None => (Self::Unknown, MissingForm),
        }
    }

    /// Returns a stable name for the class, e.g. `midi`, for scripts.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Wem => "wem",
            Self::Midi => "midi",
            Self::Impulse => "impulse",
            Self::Unknown => "unknown",
        }
    }

    /// Returns whether the media is no sound to decode: MIDI and plug-in data.
    #[must_use]
    pub const fn is_non_sound(self) -> bool {
        matches!(self, Self::Midi | Self::Impulse)
    }
}

impl fmt::Display for MediaClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Self::Wem => "WEM",
            Self::Midi => "MIDI",
            Self::Impulse => "plug-in media",
            Self::Unknown => "unknown",
        })
    }
}

pub(crate) fn declared_len(bytes: &[u8]) -> Option<u64> {
    declared_len_with_source(bytes).0
}

pub(crate) fn declared_len_with_source(bytes: &[u8]) -> (Option<u64>, BankDeclaredLengthSource) {
    use BankDeclaredLengthSource::{Signature, SignatureAndLength, TooShort};

    let Some(prefix) = bytes.first_chunk::<8>() else {
        return (None, TooShort);
    };
    let size = match prefix {
        [b'R', b'I', b'F', b'F', size @ ..] => u32::from_le_bytes(*size),
        [b'R', b'I', b'F', b'X', size @ ..] => u32::from_be_bytes(*size),
        _ => return (None, Signature),
    };
    (Some(u64::from(size) + 8), SignatureAndLength)
}

pub(crate) const fn is_prefetch(declared_len: Option<u64>, retained_len: u64) -> bool {
    matches!(declared_len, Some(declared) if declared > retained_len)
}

#[cfg(test)]
mod tests {
    use super::{
        BankDeclaredLengthSource, BankMediaClassSource, MediaClass, declared_len_with_source,
    };

    #[test]
    fn semantic_receipts_do_not_include_unconsumed_prefix_words() {
        for (bytes, class, class_source, size_source) in [
            (
                b"RIF".as_slice(),
                MediaClass::Unknown,
                BankMediaClassSource::TooShort,
                BankDeclaredLengthSource::TooShort,
            ),
            (
                b"RIFF\x04\0\0\0".as_slice(),
                MediaClass::Unknown,
                BankMediaClassSource::MissingForm,
                BankDeclaredLengthSource::SignatureAndLength,
            ),
            (
                b"RIFF\x04\0\0\0WAVE".as_slice(),
                MediaClass::Wem,
                BankMediaClassSource::SignatureAndForm,
                BankDeclaredLengthSource::SignatureAndLength,
            ),
            (
                b"PLUGabcdefgh".as_slice(),
                MediaClass::Impulse,
                BankMediaClassSource::Signature,
                BankDeclaredLengthSource::Signature,
            ),
        ] {
            assert_eq!(
                MediaClass::inspect(bytes),
                (class, class_source),
                "class only retains the actual selector inputs"
            );
            assert_eq!(
                declared_len_with_source(bytes).1,
                size_source,
                "length only retains actually consulted words"
            );
        }
    }
}
