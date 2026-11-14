// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::binding::{self, Binding};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;

fn sample() -> Result<Binding, Fault> {
    Ok(Binding {
        sequence: u64::MAX,
        product: 1,
        package: 2,
        nonce: InvocationNonce::from_bytes([7; 32])?,
    })
}

#[test]
fn binding_has_one_ascii_registry_spelling_and_exactly_one_terminator() -> Result<(), Fault> {
    let binding = sample()?;
    let text = binding.encode();
    let words: Vec<u16> = text.encode_utf16().collect();
    assert_eq!(
        words.len(),
        binding::MAX_WORDS,
        "maximum sequence fits the declared bound"
    );
    assert!(
        !text.starts_with('#') && text.is_ascii(),
        "RegLocator does not escape the canonical REG_SZ prefix"
    );
    assert_eq!(
        Binding::parse(&words)?,
        binding,
        "all identities and nonce round trip"
    );
    assert_eq!(
        Binding::from_registry(&binding.registry_bytes())?,
        binding,
        "raw registry transport has one exact spelling"
    );
    let mut doubled = binding.registry_bytes();
    doubled.extend_from_slice(&[0, 0]);
    assert!(
        Binding::from_registry(&doubled).is_err(),
        "an alternate terminator cannot alias the canonical transport"
    );
    Ok(())
}

#[test]
fn malformed_or_alternate_binding_spellings_never_resolve() -> Result<(), Fault> {
    let original = sample()?.encode();
    for changed in [
        original.replace("v1|", "v2|"),
        original.replace("v1|", "#v1|"),
        original.replace("v1|", "v1|0"),
        original.replace(
            "00000000000000000000000000000001",
            "00000000000000000000000000000000",
        ),
        original.replace("0707", "ABAB"),
        format!("{original}|extra"),
        format!("{original}\n"),
    ] {
        let words: Vec<u16> = changed.encode_utf16().collect();
        assert_eq!(
            Binding::parse(&words),
            Err(Fault::InvalidBinding),
            "only canonical fields can identify an invocation"
        );
    }
    for bytes in [
        vec![],
        vec![1],
        vec![0, 0],
        vec![0; binding::MAX_REGISTRY_BYTES + 1],
    ] {
        assert!(
            Binding::from_registry(&bytes).is_err(),
            "bounds and terminator are checked before accepting Registry bytes"
        );
    }
    Ok(())
}
