// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::argument_identity;
use crate::installer::caller::Caller;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;

const CODE_WORDS: usize = 32;
const NONCE_WORDS: usize = 64;
const MAX_SEQUENCE_WORDS: usize = 20;
pub(crate) const MAX_WORDS: usize = 2 + 4 + MAX_SEQUENCE_WORDS + CODE_WORDS * 2 + NONCE_WORDS;
pub(crate) const MAX_REGISTRY_BYTES: usize = (MAX_WORDS + 1) * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) sequence: u64,
    pub(crate) product: u128,
    pub(crate) package: u128,
    pub(crate) nonce: InvocationNonce,
}

impl Binding {
    pub(crate) const fn for_caller(sequence: u64, caller: &Caller) -> Self {
        Self {
            sequence,
            product: caller.product,
            package: caller.package,
            nonce: caller.nonce,
        }
    }

    pub(crate) const fn validate(self) -> Result<(), Fault> {
        if self.product == 0 || self.package == 0 {
            return Err(Fault::InvalidBinding);
        }
        Ok(())
    }

    pub(crate) fn encode(self) -> String {
        format!(
            "v1|{}|{:032x}|{:032x}|{}",
            self.sequence,
            self.product,
            self.package,
            self.nonce.encode()
        )
    }

    pub(crate) fn parse(words: &[u16]) -> Result<Self, Fault> {
        if words.len() > MAX_WORDS {
            return Err(Fault::InvalidBinding);
        }
        let mut fields = words.split(|word| *word == u16::from(b'|'));
        let (Some(version), Some(sequence), Some(product), Some(package), Some(nonce), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            return Err(Fault::InvalidBinding);
        };
        if !version.iter().copied().eq("v1".encode_utf16()) {
            return Err(Fault::InvalidBinding);
        }
        Ok(Self {
            sequence: argument_identity::sequence(sequence).map_err(|_| Fault::InvalidBinding)?,
            product: code(product)?,
            package: code(package)?,
            nonce: InvocationNonce::parse(nonce).map_err(|_| Fault::InvalidBinding)?,
        })
    }

    pub(crate) fn registry_bytes(self) -> Vec<u8> {
        self.encode()
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    pub(crate) fn from_registry(bytes: &[u8]) -> Result<Self, Fault> {
        if bytes.len() > MAX_REGISTRY_BYTES {
            return Err(Fault::InvalidBinding);
        }
        let (pairs, []) = bytes.as_chunks::<2>() else {
            return Err(Fault::InvalidBinding);
        };
        let mut words: Vec<u16> = pairs.iter().copied().map(u16::from_le_bytes).collect();
        if words.pop() != Some(0) {
            return Err(Fault::InvalidBinding);
        }
        Self::parse(&words)
    }
}

fn code(words: &[u16]) -> Result<u128, Fault> {
    if words.len() != CODE_WORDS {
        return Err(Fault::InvalidBinding);
    }
    let mut value = 0u128;
    for word in words {
        let digit = if (u16::from(b'0')..=u16::from(b'9')).contains(word) {
            *word - u16::from(b'0')
        } else if (u16::from(b'a')..=u16::from(b'f')).contains(word) {
            *word - u16::from(b'a') + 10
        } else {
            return Err(Fault::InvalidBinding);
        };
        value = value
            .checked_mul(16)
            .and_then(|value| value.checked_add(u128::from(digit)))
            .ok_or(Fault::InvalidBinding)?;
    }
    if value == 0 {
        return Err(Fault::InvalidBinding);
    }
    Ok(value)
}

#[cfg(test)]
#[path = "binding_tests.rs"]
mod tests;
