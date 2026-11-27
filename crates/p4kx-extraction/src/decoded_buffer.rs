// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;
use std::io;

pub(crate) const MAX_ADDRESSABLE_BYTES: u64 = isize::MAX as u64;

#[derive(Clone, Copy)]
pub(crate) struct DecodedBuffer {
    size: usize,
}

pub(crate) enum AppendError {
    Overlong(io::Error),
    Allocation {
        requested: usize,
        source: TryReserveError,
    },
}

impl DecodedBuffer {
    pub(crate) fn new(size: u64) -> Option<Self> {
        if size > MAX_ADDRESSABLE_BYTES {
            return None;
        }
        usize::try_from(size).ok().map(|size| Self { size })
    }

    pub(crate) const fn size(self) -> usize {
        self.size
    }

    pub(crate) fn append(self, bytes: &mut Vec<u8>, chunk: &[u8]) -> Result<(), AppendError> {
        let Some(required) = bytes
            .len()
            .checked_add(chunk.len())
            .filter(|&len| len <= self.size)
        else {
            return Err(AppendError::Overlong(io::Error::new(
                io::ErrorKind::InvalidData,
                "the input holds more data than its declared size",
            )));
        };
        if required > bytes.capacity() {
            let doubled = bytes.capacity().saturating_mul(2);
            let requested = required.max(doubled).min(self.size);
            bytes
                .try_reserve_exact(requested - bytes.len())
                .map_err(|source| AppendError::Allocation { requested, source })?;
        }
        bytes.extend_from_slice(chunk);
        Ok(())
    }

    pub(crate) fn finish(self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() != self.size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the input ends before its declared size",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{DecodedBuffer, MAX_ADDRESSABLE_BYTES};

    #[test]
    fn a_large_declared_size_does_not_reserve_unobserved_bytes() -> Result<(), &'static str> {
        let buffer = DecodedBuffer::new(MAX_ADDRESSABLE_BYTES).ok_or("addressable size")?;
        let mut bytes = Vec::new();
        buffer
            .append(&mut bytes, b"abc")
            .map_err(|_| "first bytes")?;
        assert_eq!(bytes.as_slice(), b"abc", "only received bytes are retained");
        assert_eq!(
            bytes.capacity(),
            3,
            "the first reserve follows the first chunk"
        );
        buffer.append(&mut bytes, b"d").map_err(|_| "next bytes")?;
        assert_eq!(
            bytes.capacity(),
            6,
            "amortized spare follows the received prefix"
        );
        assert_eq!(
            buffer.finish(&bytes).err().map(|error| error.kind()),
            Some(io::ErrorKind::UnexpectedEof),
            "the validated prefix cannot stand in for the declared input"
        );
        Ok(())
    }

    #[test]
    fn addressability_uses_the_platform_byte_vector_limit() {
        assert!(
            DecodedBuffer::new(MAX_ADDRESSABLE_BYTES).is_some(),
            "the byte vector's representable boundary is admitted"
        );
        assert!(
            DecodedBuffer::new(MAX_ADDRESSABLE_BYTES + 1).is_none(),
            "lengths beyond the vector layout are refused without allocating"
        );
    }
}
