// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::raw_path::MAX_PATH_BYTES;

/// One installer-private directory, never a shared preexisting PATH element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lease {
    pub(crate) entry: Vec<u8>,
}

impl Lease {
    pub(crate) fn try_new(entry: Vec<u8>) -> Result<Self, Fault> {
        if entry.is_empty() || entry.len() > MAX_PATH_BYTES || !entry.len().is_multiple_of(2) {
            return Err(Fault::InvalidLease);
        }
        let (words, []) = entry.as_chunks::<2>() else {
            return Err(Fault::InvalidLease);
        };
        if words
            .iter()
            .any(|word| *word == [0, 0] || *word == [b';', 0])
        {
            return Err(Fault::InvalidLease);
        }
        Ok(Self { entry })
    }
}
