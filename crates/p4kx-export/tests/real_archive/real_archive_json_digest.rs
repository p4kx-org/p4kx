// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Updates a SHA-256 digest with the JSON serialization of a value.
pub(crate) fn update_json_digest(
    hash: &mut Sha256,
    value: &impl Serialize,
) -> serde_json::Result<()> {
    serde_json::to_writer(DigestWrite(hash), value)
}

struct DigestWrite<'hash>(&'hash mut Sha256);

impl Write for DigestWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn streamed_digest_matches_the_json_bytes() -> serde_json::Result<()> {
    let value =
        serde_json::json!({"z": [null, "Quittung\n雪", 1e-12], "a": {"text": "x\u{0000}" }});
    let mut streamed = Sha256::new();
    update_json_digest(&mut streamed, &value)?;
    assert_eq!(
        streamed.finalize(),
        Sha256::digest(serde_json::to_vec(&value)?)
    );
    Ok(())
}
