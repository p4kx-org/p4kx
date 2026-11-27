// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::{InvocationNonce, NonceError};

#[test]
fn generated_identity_uses_exact_injected_bytes_and_canonical_transport()
-> Result<(), Box<dyn std::error::Error>> {
    let nonce = InvocationNonce::generate_with(|bytes| {
        bytes.fill(0xa3);
        Ok(())
    })?;
    let encoded = nonce.encode();
    assert_eq!(encoded, "a3".repeat(32), "all 256 random bits are retained");
    let words: Vec<_> = encoded.encode_utf16().collect();
    assert_eq!(
        InvocationNonce::parse(&words)?,
        nonce,
        "the queued identity round trips exactly"
    );
    assert_eq!(
        format!("{nonce:?}"),
        "InvocationNonce",
        "debug output does not expose the nonce"
    );
    Ok(())
}

#[test]
fn random_source_failure_remains_typed_and_partial_bytes_never_escape() {
    let result = InvocationNonce::generate_with(|bytes| {
        bytes.fill(7);
        Err(getrandom::Error::UNSUPPORTED)
    });
    assert!(
        matches!(result, Err(NonceError::Unavailable(error)) if error == getrandom::Error::UNSUPPORTED),
        "the original random-source error is retained"
    );
}

#[test]
fn reserved_zero_identity_is_rejected_without_a_predictable_fallback() {
    let generated = InvocationNonce::generate_with(|bytes| {
        bytes.fill(0);
        Ok(())
    });
    assert!(
        matches!(generated, Err(NonceError::ReservedValue)),
        "zero is not replaced with a counter, timestamp or product code"
    );
    assert_eq!(
        InvocationNonce::from_bytes([0; 32]),
        Err(Fault::InvalidNonce),
        "a persisted zero identity is invalid"
    );
}

#[test]
fn malformed_noncanonical_and_empty_identities_are_rejected() {
    for text in [
        String::new(),
        "0".repeat(64),
        "A".repeat(64),
        "g".repeat(64),
        "1".repeat(63),
        "1".repeat(65),
    ] {
        let words: Vec<_> = text.encode_utf16().collect();
        assert_eq!(
            InvocationNonce::parse(&words),
            Err(Fault::InvalidNonce),
            "only one exact canonical nonzero identity reaches the WAL"
        );
    }
}
