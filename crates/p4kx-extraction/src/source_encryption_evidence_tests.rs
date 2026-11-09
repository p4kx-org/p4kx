// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    SourceDeclarationEvidence, SourceDeclarationKind, SourceEncryptionState, SourceRecordFormat,
    SourceValue,
};

#[test]
fn every_source_value_state_preserves_its_declared_length_meaning() {
    for (length, expected) in [
        (
            SourceValue::Present(6),
            &[SourceEncryptionState::Unknown][..],
        ),
        (
            SourceValue::Absent,
            &[SourceEncryptionState::Unencrypted][..],
        ),
        (SourceValue::Null, &[SourceEncryptionState::Unknown][..]),
        (
            SourceValue::Unreadable,
            &[
                SourceEncryptionState::Unencrypted,
                SourceEncryptionState::Unknown,
            ][..],
        ),
        (
            SourceValue::Unsupported,
            &[SourceEncryptionState::Unknown][..],
        ),
        (SourceValue::Invalid, &[SourceEncryptionState::Unknown][..]),
    ] {
        let declaration = SourceDeclarationEvidence::new(
            SourceDeclarationKind::P4kV2Field,
            length,
            SourceValue::Present(0),
        );
        assert_states(
            declaration,
            SourceRecordFormat::P4kV2,
            &SourceValue::Absent,
            expected,
        );
    }
}

#[test]
fn every_source_value_state_preserves_its_word_meaning() {
    for (word, expected) in [
        (
            SourceValue::Present(0),
            &[
                SourceEncryptionState::Unencrypted,
                SourceEncryptionState::Unknown,
            ][..],
        ),
        (SourceValue::Absent, &[SourceEncryptionState::Unknown][..]),
        (SourceValue::Null, &[SourceEncryptionState::Unknown][..]),
        (
            SourceValue::Unreadable,
            &[
                SourceEncryptionState::Encrypted,
                SourceEncryptionState::Unencrypted,
                SourceEncryptionState::Unknown,
            ][..],
        ),
        (
            SourceValue::Unsupported,
            &[SourceEncryptionState::Unknown][..],
        ),
        (SourceValue::Invalid, &[SourceEncryptionState::Unknown][..]),
    ] {
        let declaration = SourceDeclarationEvidence::new(
            SourceDeclarationKind::CigExtraBlock,
            SourceValue::Unreadable,
            word,
        );
        assert_states(
            declaration,
            SourceRecordFormat::ZipCentralDirectory,
            &SourceValue::Present(0),
            expected,
        );
    }
}

#[test]
fn every_source_value_state_preserves_its_zip_flag_meaning() {
    for (flags, expected) in [
        (
            SourceValue::Present(0),
            &[SourceEncryptionState::Unencrypted][..],
        ),
        (SourceValue::Absent, &[SourceEncryptionState::Unknown][..]),
        (SourceValue::Null, &[SourceEncryptionState::Unknown][..]),
        (
            SourceValue::Unreadable,
            &[
                SourceEncryptionState::Unencrypted,
                SourceEncryptionState::Unknown,
            ][..],
        ),
        (
            SourceValue::Unsupported,
            &[SourceEncryptionState::Unknown][..],
        ),
        (SourceValue::Invalid, &[SourceEncryptionState::Unknown][..]),
    ] {
        let declaration = SourceDeclarationEvidence::new(
            SourceDeclarationKind::P4kV2Field,
            SourceValue::Absent,
            SourceValue::Present(0),
        );
        assert_states(
            declaration,
            SourceRecordFormat::ZipCentralDirectory,
            &flags,
            expected,
        );
    }
}

fn assert_states(
    declaration: SourceDeclarationEvidence,
    format: SourceRecordFormat,
    flags: &SourceValue<u16>,
    expected: &[SourceEncryptionState],
) {
    for candidate in [
        SourceEncryptionState::Encrypted,
        SourceEncryptionState::Unencrypted,
        SourceEncryptionState::Unknown,
    ] {
        assert_eq!(
            candidate.is_possible_from_evidence(Some(format), flags.clone(), [declaration]),
            expected.contains(&candidate),
            "the six-state source contract preserves each exact possibility"
        );
    }
}
