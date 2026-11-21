// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::{LocalizationError, LocalizationLimits};

use super::{IniEncoding, IniText};

pub(super) fn decode(
    body: &[u8],
    encoding: IniEncoding,
    limits: LocalizationLimits,
) -> Result<IniText<'static>, LocalizationError> {
    let mut decoder = Decoder {
        text: String::new(),
        lossy: false,
        source_size: u64::try_from(body.len()).unwrap_or(u64::MAX),
        limits,
    };
    match encoding {
        IniEncoding::Utf8 | IniEncoding::Utf8Bom => decoder.utf8_lossy(body)?,
        IniEncoding::Utf16Le => decoder.utf16(body, u16::from_le_bytes)?,
        IniEncoding::Utf16Be => decoder.utf16(body, u16::from_be_bytes)?,
    }
    Ok(IniText {
        text: Cow::Owned(decoder.text),
        encoding,
        lossy: decoder.lossy,
    })
}

struct Decoder {
    text: String,
    lossy: bool,
    source_size: u64,
    limits: LocalizationLimits,
}

impl Decoder {
    fn utf8_lossy(&mut self, body: &[u8]) -> Result<(), LocalizationError> {
        for chunk in body.utf8_chunks() {
            let prefix = chunk.valid();
            self.reserve(prefix.len())?;
            self.text.push_str(prefix);
            if !chunk.invalid().is_empty() {
                self.push_replacement()?;
            }
        }
        Ok(())
    }

    fn utf16(
        &mut self,
        body: &[u8],
        unit_of: impl Fn([u8; 2]) -> u16,
    ) -> Result<(), LocalizationError> {
        let (pairs, rest) = body.as_chunks::<2>();
        self.lossy = !rest.is_empty();
        for unit in char::decode_utf16(pairs.iter().copied().map(unit_of)) {
            match unit {
                Ok(character) => self.push(character)?,
                Err(_) => self.push_replacement()?,
            }
        }
        Ok(())
    }

    fn push_replacement(&mut self) -> Result<(), LocalizationError> {
        self.lossy = true;
        self.push(char::REPLACEMENT_CHARACTER)
    }

    fn push(&mut self, character: char) -> Result<(), LocalizationError> {
        self.reserve(character.len_utf8())?;
        self.text.push(character);
        Ok(())
    }

    fn reserve(&mut self, extra: usize) -> Result<(), LocalizationError> {
        let size = self
            .text
            .len()
            .checked_add(extra)
            .ok_or_else(|| self.capacity_error())?;
        self.limits.check_decoded_bytes(size)?;
        self.text
            .try_reserve(extra)
            .map_err(|_| self.capacity_error())
    }

    const fn capacity_error(&self) -> LocalizationError {
        LocalizationError::DecodeOutOfMemory {
            size: self.source_size,
        }
    }
}
