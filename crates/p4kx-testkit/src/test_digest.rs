// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{self, Write as _};

use sha2::{Digest, Sha256};

pub(crate) fn sha256_hex(payload: &[u8]) -> Result<String, fmt::Error> {
    let digest = Sha256::digest(payload);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}")?;
    }
    Ok(output)
}
