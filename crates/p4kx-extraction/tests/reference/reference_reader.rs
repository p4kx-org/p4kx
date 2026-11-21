// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use sha2::{Digest as _, Sha256};

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::Fingerprint;

pub(crate) struct Fingerprinted<R> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
    limit: u64,
    ended: bool,
}

impl<R> Fingerprinted<R> {
    pub(crate) fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
            limit,
            ended: false,
        }
    }

    pub(crate) fn finish(self, expected: &Fingerprint) -> Result<()> {
        if !self.ended {
            return Err(ReferenceError::new(
                Code::Incomplete,
                "reference stream did not reach its end",
            ));
        }
        let actual = Fingerprint {
            bytes: self.bytes,
            digest: super::reference_digest::finish(self.hasher),
        };
        if actual != *expected {
            return Err(ReferenceError::new(
                Code::Changed,
                "reference bytes changed after initial validation",
            ));
        }
        Ok(())
    }
}

impl<R: Read> Read for Fingerprinted<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        if count == 0 && !buffer.is_empty() {
            self.ended = true;
        }
        self.bytes = self
            .bytes
            .checked_add(count as u64)
            .filter(|bytes| *bytes <= self.limit)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reference stream exceeds its bound",
                )
            })?;
        self.hasher.update(buffer.get(..count).unwrap_or_default());
        Ok(count)
    }
}
