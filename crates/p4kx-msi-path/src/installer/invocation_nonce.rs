// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::installer::fault::Fault;

const BYTE_COUNT: usize = 32;

/// A fresh random identity shared only by the phases of one MSI invocation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvocationNonce {
    bytes: [u8; BYTE_COUNT],
}

/// Failure to obtain a nonzero invocation identity from the operating system.
#[derive(Debug)]
#[non_exhaustive]
pub enum NonceError {
    /// The operating system did not provide the requested random bytes.
    Unavailable(getrandom::Error),
    /// The random source returned the reserved all-zero value.
    ReservedValue,
}

impl InvocationNonce {
    /// Generates a new invocation identity using the operating system random source.
    ///
    /// # Errors
    /// Returns the original random-source failure or a reserved all-zero result.
    pub fn generate() -> Result<Self, NonceError> {
        Self::generate_with(getrandom::fill)
    }

    /// Encodes the identity as exactly 64 lowercase hexadecimal ASCII characters.
    #[must_use]
    pub fn encode(self) -> String {
        self.bytes
            .iter()
            .fold(String::with_capacity(BYTE_COUNT * 2), |mut text, byte| {
                text.push(hex_digit(byte >> 4));
                text.push(hex_digit(byte & 15));
                text
            })
    }

    pub(crate) fn from_bytes(bytes: [u8; BYTE_COUNT]) -> Result<Self, Fault> {
        if bytes == [0; BYTE_COUNT] {
            return Err(Fault::InvalidNonce);
        }
        Ok(Self { bytes })
    }

    pub(crate) const fn bytes(&self) -> &[u8; BYTE_COUNT] {
        &self.bytes
    }

    pub(crate) fn parse(words: &[u16]) -> Result<Self, Fault> {
        if words.len() != BYTE_COUNT * 2 {
            return Err(Fault::InvalidNonce);
        }
        let (pairs, []) = words.as_chunks::<2>() else {
            return Err(Fault::InvalidNonce);
        };
        let mut bytes = [0; BYTE_COUNT];
        for (target, [high, low]) in bytes.iter_mut().zip(pairs) {
            *target = digit(*high)?.checked_mul(16).ok_or(Fault::InvalidNonce)? | digit(*low)?;
        }
        Self::from_bytes(bytes)
    }

    fn generate_with(
        fill: impl FnOnce(&mut [u8]) -> Result<(), getrandom::Error>,
    ) -> Result<Self, NonceError> {
        let mut bytes = [0; BYTE_COUNT];
        fill(&mut bytes).map_err(NonceError::Unavailable)?;
        Self::from_bytes(bytes).map_err(|_| NonceError::ReservedValue)
    }
}

impl fmt::Debug for InvocationNonce {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InvocationNonce")
    }
}

impl fmt::Display for NonceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(_) => {
                formatter.write_str("operating system random source unavailable")
            }
            Self::ReservedValue => {
                formatter.write_str("operating system returned a reserved invocation identity")
            }
        }
    }
}

impl std::error::Error for NonceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(error) => Some(error),
            Self::ReservedValue => None,
        }
    }
}

fn digit(word: u16) -> Result<u8, Fault> {
    match u8::try_from(word).map_err(|_| Fault::InvalidNonce)? {
        digit @ b'0'..=b'9' => Ok(digit - b'0'),
        digit @ b'a'..=b'f' => Ok(digit - b'a' + 10),
        _ => Err(Fault::InvalidNonce),
    }
}

fn hex_digit(nibble: u8) -> char {
    char::from(if nibble < 10 {
        b'0' + nibble
    } else {
        b'a' + nibble - 10
    })
}

#[cfg(test)]
#[path = "nonce_tests.rs"]
mod tests;
