// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shared descriptive encryption rules against original parsed declarations.

use std::fs;

use p4kx_extraction::{
    Archive, SourceDeclarationKind, SourceDetails, SourceEncryptionState, SourceRecordFormat,
    SourceUses,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

const ZIP_CENTRAL_SIGNATURE: &[u8; 4] = b"PK\x01\x02";
const ZIP_FLAGS_OFFSET: usize = 8;

#[derive(Clone, Copy)]
enum Layout {
    Zip(u16),
    Fixed,
}

#[test]
fn pure_zip_controls_match_the_original_observed_source_state() -> TestResult {
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    for (word, flags, expected) in [
        (0, 0, Unencrypted),
        (0, 1, Unknown),
        (0, 64, Unknown),
        (1, 0, Encrypted),
        (1, 1, Encrypted),
        (1, 64, Encrypted),
        (7, 0, Unknown),
        (7, 1, Unknown),
        (7, 64, Unknown),
    ] {
        observed(word, Layout::Zip(flags), expected)?;
    }
    Ok(())
}

#[test]
fn pure_fixed_directory_controls_match_the_original_observed_source_state() -> TestResult {
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    for (word, expected) in [(0, Unencrypted), (1, Encrypted), (7, Unknown)] {
        observed(word, Layout::Fixed, expected)?;
    }
    Ok(())
}

fn observed(word: u16, layout: Layout, expected: SourceEncryptionState) -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("controls.p4k");
    let builder = match layout {
        Layout::Zip(_) => ArchiveBuilder::new(),
        Layout::Fixed => ArchiveBuilder::v2(),
    };
    let mut bytes = builder
        .entry(TestEntry::stored("fact.bin", b"fact").with_encryption_field(word))
        .build();
    let (format, kind, block, flags) = match layout {
        Layout::Zip(flags) => {
            let at = bytes
                .windows(ZIP_CENTRAL_SIGNATURE.len())
                .position(|value| value == ZIP_CENTRAL_SIGNATURE)
                .ok_or("central directory record")?;
            let flags_start = at + ZIP_FLAGS_OFFSET;
            bytes
                .get_mut(flags_start..flags_start + size_of::<u16>())
                .ok_or("central directory flags")?
                .copy_from_slice(&flags.to_le_bytes());
            (
                SourceRecordFormat::ZipCentralDirectory,
                SourceDeclarationKind::CigExtraBlock,
                Some(6),
                Some(flags),
            )
        }
        Layout::Fixed => (
            SourceRecordFormat::P4kV2,
            SourceDeclarationKind::P4kV2Field,
            None,
            None,
        ),
    };
    fs::write(&path, bytes)?;
    let archive = Archive::open(path)?;
    let sources = SourceUses::new();
    let scope = support::source(&sources, &archive, SourceDetails::Full)?;
    let actual = support::used(&scope)?.encryption();
    assert_eq!(
        actual, expected,
        "the original parser/observed source follows the declared policy for word {word}"
    );
    let declared = kind.interpret(block, Some(word));
    assert_eq!(
        SourceEncryptionState::from_declarations(format, flags, [declared]),
        expected,
        "descriptive validation uses the same policy without original-input authority"
    );
    Ok(())
}

#[test]
fn malformed_unsupported_and_unknown_fields_never_establish_plain_encryption() {
    use SourceDeclarationKind::{CigExtraBlock, P4kV2Field, Unknown as UnknownKind};
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    for (kind, length, word, expected) in [
        (CigExtraBlock, Some(6), Some(0), Unencrypted),
        (CigExtraBlock, Some(6), Some(1), Encrypted),
        (CigExtraBlock, Some(6), Some(7), Unknown),
        (CigExtraBlock, Some(5), Some(1), Unknown),
        (CigExtraBlock, Some(7), Some(1), Unknown),
        (CigExtraBlock, Some(6), None, Unknown),
        (P4kV2Field, None, Some(0), Unencrypted),
        (P4kV2Field, None, Some(1), Encrypted),
        (P4kV2Field, Some(6), Some(1), Unknown),
        (UnknownKind, Some(6), Some(1), Unknown),
    ] {
        assert_eq!(
            kind.interpret(length, word),
            expected,
            "only complete supported raw declarations establish a known state"
        );
    }
}

#[test]
fn declaration_consensus_is_distinct_from_encrypted_ancestor_dominance() {
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    let cases: &[(&[SourceEncryptionState], SourceEncryptionState)] = &[
        (&[], Unknown),
        (&[Encrypted, Encrypted], Encrypted),
        (&[Unencrypted, Unencrypted], Unencrypted),
        (&[Encrypted, Unencrypted], Unknown),
        (&[Unencrypted, Encrypted], Unknown),
        (&[Encrypted, Unknown], Unknown),
        (&[Unknown, Encrypted], Unknown),
        (&[Unknown, Unknown], Unknown),
    ];
    for &(states, expected) in cases {
        assert_eq!(
            SourceEncryptionState::from_declarations(
                SourceRecordFormat::P4kV2,
                None,
                states.iter().copied()
            ),
            expected,
            "all stored declarations must agree; a conflicting encrypted word cannot override another field"
        );
    }
    assert_eq!(
        SourceEncryptionState::from_declarations(SourceRecordFormat::Unknown, None, [Unencrypted]),
        Unknown,
        "an unknown directory format cannot establish that no other encryption declaration applies"
    );
    assert_eq!(
        SourceEncryptionState::from_declarations(SourceRecordFormat::Unknown, None, [Encrypted]),
        Encrypted,
        "an actual complete encrypted declaration remains a known witness"
    );
}
