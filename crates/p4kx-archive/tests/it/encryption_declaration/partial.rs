// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    EncryptionDeclarationEvidence, EncryptionDeclarationSource, EncryptionFieldEvidence,
    EncryptionFormatEvidence, EntryMetadata, EntryRecordFormat, OpenOptions, SourceEncryption,
    ZipHeaderMetadata,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::fixture::{self, FixtureResult, PAYLOAD};

#[test]
fn full_disclosure_collapses_to_the_actual_observed_archive_classifier() -> FixtureResult<()> {
    for bytes in classifier_corpus()? {
        assert_observed_classification(&bytes)?;
    }
    Ok(())
}

fn classifier_corpus() -> FixtureResult<Vec<Vec<u8>>> {
    let mut corpus = Vec::new();
    for word in [0, 1, 7] {
        let entry = TestEntry::zstd("entry.bin", PAYLOAD)
            .encrypted()
            .with_encryption_field(word);
        let original = fixture::v1(entry.clone());
        for flags in [0, 1, 64] {
            corpus.push(fixture::zip_flags(original.clone(), flags)?);
        }
        corpus.push(ArchiveBuilder::v2().entry(entry).build());
    }
    for bytes in [Vec::new(), vec![1]] {
        corpus.push(fixture::v1(
            TestEntry::zstd("entry.bin", PAYLOAD)
                .encrypted()
                .with_encryption_block(bytes),
        ));
    }
    corpus.push(fixture::missing_declaration(fixture::v1(
        TestEntry::stored("entry.bin", PAYLOAD),
    ))?);
    corpus.push(fixture::additional_declarations(
        TestEntry::zstd("entry.bin", PAYLOAD).encrypted(),
        &[0],
    )?);
    Ok(corpus)
}

fn assert_observed_classification(bytes: &[u8]) -> FixtureResult<()> {
    let (_folder, archive) = fixture::open(bytes, OpenOptions::default())?;
    let entry = fixture::entry(&archive, 0)?;
    let observed = archive
        .observed_entry(entry)
        .ok_or("actual original entry")?;
    let metadata = entry.metadata();
    let flags = field(
        metadata
            .zip_header()
            .map(ZipHeaderMetadata::general_purpose_flags),
    );
    let available: Vec<_> = metadata
        .encryption_declarations()
        .iter()
        .map(|declaration| {
            let word = declaration.raw_value().map_or(
                EncryptionFieldEvidence::Malformed,
                EncryptionFieldEvidence::Present,
            );
            EncryptionDeclarationEvidence::new(
                declaration.source(),
                field(declaration.declared_block_len()),
                word,
            )
        })
        .collect();
    let actual: Vec<_> = SourceEncryption::possible_from_evidence(
        EncryptionFormatEvidence::Known(metadata.record_format()),
        flags,
        available,
    )
    .collect();
    assert_eq!(
        actual,
        [observed.stored_encryption()],
        "complete real source evidence has exactly its original classification"
    );
    assert_unreadable_inner_facts(metadata, flags, observed.stored_encryption());
    Ok(())
}

fn assert_unreadable_inner_facts(
    metadata: &EntryMetadata,
    flags: EncryptionFieldEvidence<u16>,
    stored_encryption: SourceEncryption,
) {
    for erase_length in [false, true] {
        let partial: Vec<_> = metadata
            .encryption_declarations()
            .iter()
            .map(|declaration| {
                let length = if erase_length {
                    EncryptionFieldEvidence::Unavailable
                } else {
                    field(declaration.declared_block_len())
                };
                EncryptionDeclarationEvidence::new(
                    declaration.source(),
                    length,
                    EncryptionFieldEvidence::Unavailable,
                )
            })
            .collect();
        assert!(
            SourceEncryption::possible_from_evidence(
                EncryptionFormatEvidence::Known(metadata.record_format()),
                flags,
                partial
            )
            .any(|state| state == stored_encryption),
            "unreadable inner facts preserve a genuine original possibility"
        );
    }
}

#[test]
fn known_empty_and_unavailable_lists_remain_distinct() {
    let format = EncryptionFormatEvidence::Known(EntryRecordFormat::ZipCentralDirectory);
    assert_states(
        SourceEncryption::possible_from_evidence(format, EncryptionFieldEvidence::Present(0), []),
        &[SourceEncryption::Unknown],
    );
    assert_states(
        SourceEncryption::possible_from_evidence(
            format,
            EncryptionFieldEvidence::Present(0),
            [EncryptionDeclarationEvidence::unavailable()],
        ),
        &[
            SourceEncryption::Encrypted,
            SourceEncryption::Unencrypted,
            SourceEncryption::Unknown,
        ],
    );
    assert_states(
        SourceEncryption::possible_from_evidence(
            format,
            EncryptionFieldEvidence::Present(1),
            [EncryptionDeclarationEvidence::unavailable()],
        ),
        &[SourceEncryption::Encrypted, SourceEncryption::Unknown],
    );
}

#[test]
fn unavailable_and_known_unsupported_formats_have_distinct_plain_possibilities() {
    let plain = v2_word(0);
    assert_states(
        SourceEncryption::possible_from_evidence(
            EncryptionFormatEvidence::Unavailable,
            EncryptionFieldEvidence::Present(0),
            [plain],
        ),
        &[SourceEncryption::Unencrypted, SourceEncryption::Unknown],
    );
    assert_states(
        SourceEncryption::possible_from_evidence(
            EncryptionFormatEvidence::Unsupported,
            EncryptionFieldEvidence::Present(0),
            [plain],
        ),
        &[SourceEncryption::Unknown],
    );
    assert_states(
        SourceEncryption::possible_from_evidence(
            EncryptionFormatEvidence::Known(EntryRecordFormat::P4kV2),
            EncryptionFieldEvidence::Absent,
            [plain],
        ),
        &[SourceEncryption::Unencrypted],
    );
}

#[test]
fn partial_consensus_keeps_only_states_compatible_with_every_available_occurrence() {
    for (word, expected) in [
        (
            0_u16,
            &[SourceEncryption::Unencrypted, SourceEncryption::Unknown][..],
        ),
        (
            1,
            &[SourceEncryption::Encrypted, SourceEncryption::Unknown][..],
        ),
        (7, &[SourceEncryption::Unknown][..]),
    ] {
        assert_states(
            SourceEncryption::possible_from_evidence(
                EncryptionFormatEvidence::Known(EntryRecordFormat::P4kV2),
                EncryptionFieldEvidence::Absent,
                [v2_word(word), EncryptionDeclarationEvidence::unavailable()],
            ),
            expected,
        );
    }
    assert_states(
        SourceEncryption::possible_from_evidence(
            EncryptionFormatEvidence::Unavailable,
            EncryptionFieldEvidence::Unavailable,
            [
                EncryptionDeclarationEvidence::unsupported(),
                EncryptionDeclarationEvidence::unavailable(),
            ],
        ),
        &[SourceEncryption::Unknown],
    );
}

#[test]
fn absent_malformed_and_unreadable_flags_keep_their_canonical_distinct_meanings() {
    let format = EncryptionFormatEvidence::Known(EntryRecordFormat::ZipCentralDirectory);
    for flags in [
        EncryptionFieldEvidence::Absent,
        EncryptionFieldEvidence::Malformed,
    ] {
        assert_states(
            SourceEncryption::possible_from_evidence(format, flags, [v2_word(0)]),
            &[SourceEncryption::Unknown],
        );
        assert_states(
            SourceEncryption::possible_from_evidence(format, flags, [v2_word(1)]),
            &[SourceEncryption::Encrypted],
        );
    }
    assert_states(
        SourceEncryption::possible_from_evidence(
            format,
            EncryptionFieldEvidence::Unavailable,
            [v2_word(0)],
        ),
        &[SourceEncryption::Unencrypted, SourceEncryption::Unknown],
    );
}

#[test]
fn declaration_field_absence_malformation_and_unreadability_are_not_interchangeable() {
    for length in [
        EncryptionFieldEvidence::Absent,
        EncryptionFieldEvidence::Malformed,
    ] {
        let declaration = EncryptionDeclarationEvidence::new(
            EncryptionDeclarationSource::CigExtraBlock,
            length,
            EncryptionFieldEvidence::Present(1),
        );
        assert_states(
            SourceEncryption::possible_from_evidence(
                EncryptionFormatEvidence::Unavailable,
                EncryptionFieldEvidence::Unavailable,
                [declaration],
            ),
            &[SourceEncryption::Unknown],
        );
    }
    let declaration = EncryptionDeclarationEvidence::new(
        EncryptionDeclarationSource::CigExtraBlock,
        EncryptionFieldEvidence::Unavailable,
        EncryptionFieldEvidence::Present(1),
    );
    assert_states(
        SourceEncryption::possible_from_evidence(
            EncryptionFormatEvidence::Unavailable,
            EncryptionFieldEvidence::Unavailable,
            [declaration],
        ),
        &[SourceEncryption::Encrypted, SourceEncryption::Unknown],
    );
    for word in [
        EncryptionFieldEvidence::Absent,
        EncryptionFieldEvidence::Malformed,
    ] {
        let declaration = EncryptionDeclarationEvidence::new(
            EncryptionDeclarationSource::CigExtraBlock,
            EncryptionFieldEvidence::Unavailable,
            word,
        );
        assert_states(
            SourceEncryption::possible_from_evidence(
                EncryptionFormatEvidence::Unavailable,
                EncryptionFieldEvidence::Unavailable,
                [declaration],
            ),
            &[SourceEncryption::Unknown],
        );
    }
}

fn v2_word(word: u16) -> EncryptionDeclarationEvidence {
    EncryptionDeclarationEvidence::new(
        EncryptionDeclarationSource::P4kV2Field,
        EncryptionFieldEvidence::Absent,
        EncryptionFieldEvidence::Present(word),
    )
}

fn field(value: Option<u16>) -> EncryptionFieldEvidence<u16> {
    value.map_or(
        EncryptionFieldEvidence::Absent,
        EncryptionFieldEvidence::Present,
    )
}

fn assert_states(states: impl Iterator<Item = SourceEncryption>, expected: &[SourceEncryption]) {
    let actual: Vec<_> = states.collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "each possible state is unique and no impossible state is added"
    );
    for expected in expected {
        assert!(
            actual.contains(expected),
            "the exact possible state is retained"
        );
    }
}
