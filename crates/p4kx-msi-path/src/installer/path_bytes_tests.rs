// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::path_edit;
use crate::installer::raw_path;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::tests::{self, ENTRY, PRODUCT, TestResult};

#[test]
fn removing_an_owned_entry_preserves_an_unterminated_foreign_value() -> TestResult {
    for kind in [StringKind::Plain, StringKind::Expandable] {
        let mut store = tests::memory(tests::path("A", StringKind::Plain));
        tests::install(&mut store)?;
        store.state.path = RawPath::Present {
            kind,
            bytes: tests::bytes(&format!("{ENTRY};B")),
        };
        tests::remove(&mut store, PRODUCT)?;
        assert_eq!(
            store.state.path,
            RawPath::Present {
                kind,
                bytes: vec![b'B', 0],
            },
            "an owned splice cannot add terminator bytes to a foreign value"
        );
    }
    Ok(())
}

#[test]
fn inserting_into_present_values_preserves_the_exact_empty_or_unterminated_tail() -> TestResult {
    for kind in [StringKind::Plain, StringKind::Expandable] {
        for body in ["", "B"] {
            let original = RawPath::Present {
                kind,
                bytes: tests::bytes(body),
            };
            let mut store = tests::memory(original.clone());
            tests::install(&mut store)?;
            assert_eq!(
                store.state.path,
                RawPath::Present {
                    kind,
                    bytes: tests::bytes(&format!("{ENTRY};{body}")),
                },
                "present values retain their authored absence of terminator bytes"
            );
            tests::remove(&mut store, PRODUCT)?;
            assert_eq!(store.state.path, original, "the original value is restored");
        }
    }
    Ok(())
}

#[test]
fn the_exact_unterminated_insertion_boundary_is_accepted_and_one_word_more_is_rejected()
-> TestResult {
    let lease = tests::lease()?;
    let body_bytes = raw_path::MAX_PATH_BYTES - lease.entry.len() - 2;
    let original = RawPath::Present {
        kind: StringKind::Expandable,
        bytes: [b'A', 0].repeat(body_bytes / 2),
    };
    let proof = path_edit::insert(&original, &lease)?;
    let RawPath::Present { bytes, .. } = &proof.after else {
        return Err("insertion did not produce a present value".into());
    };
    assert_eq!(
        bytes.len(),
        raw_path::MAX_PATH_BYTES,
        "an absent terminator consumes no invented budget"
    );
    assert!(
        path_edit::remove(&proof.after, &lease, &proof)? == original,
        "the exact-boundary value remains recoverable"
    );
    let oversized = RawPath::Present {
        kind: StringKind::Expandable,
        bytes: [b'A', 0].repeat(body_bytes / 2 + 1),
    };
    assert!(
        path_edit::insert(&oversized, &lease).is_err(),
        "one extra UTF16 word exceeds the output budget"
    );
    Ok(())
}
