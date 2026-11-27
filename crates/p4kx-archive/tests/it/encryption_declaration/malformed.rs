// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;

use p4kx_archive::{
    DeclaredEncryption, EntryIssue, MAX_ENTRY_ISSUES, OpenOptions, StoredEncryptionDeclaration,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::fixture::{self, FixtureResult, PAYLOAD};

struct MalformedCase {
    body: &'static [u8],
    declared_len: u16,
    word: Option<u16>,
    decoder_encrypted: bool,
    issues: &'static [EntryIssue],
}

const MALFORMED_CASES: &[MalformedCase] = &[
    MalformedCase {
        body: &[],
        declared_len: 4,
        word: None,
        decoder_encrypted: true,
        issues: &[EntryIssue::EncryptionBlockSize { size: 4 }],
    },
    MalformedCase {
        body: &[0],
        declared_len: 5,
        word: None,
        decoder_encrypted: true,
        issues: &[EntryIssue::EncryptionBlockSize { size: 5 }],
    },
    MalformedCase {
        body: &[1],
        declared_len: 5,
        word: None,
        decoder_encrypted: true,
        issues: &[EntryIssue::EncryptionBlockSize { size: 5 }],
    },
    MalformedCase {
        body: &[7, 0],
        declared_len: 6,
        word: Some(7),
        decoder_encrypted: true,
        issues: &[EntryIssue::UnknownEncryption { value: 7 }],
    },
    MalformedCase {
        body: &[0, 0, 9, 9],
        declared_len: 8,
        word: Some(0),
        decoder_encrypted: false,
        issues: &[EntryIssue::EncryptionBlockSize { size: 8 }],
    },
    MalformedCase {
        body: &[1, 0, 9, 9],
        declared_len: 8,
        word: Some(1),
        decoder_encrypted: true,
        issues: &[EntryIssue::EncryptionBlockSize { size: 8 }],
    },
];
#[test]
fn malformed_v1_fields_retain_raw_bytes_and_existing_decoder_decisions() -> FixtureResult<()> {
    for case in MALFORMED_CASES {
        let authored =
            TestEntry::stored("Data\\item.bin", PAYLOAD).with_encryption_block(case.body);
        let bytes = fixture::v1(authored);
        let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
        let entry = fixture::entry(&archive, 0)?;
        let declarations = entry.metadata().encryption_declarations();
        assert_eq!(declarations.state(), DeclaredEncryption::Unknown);
        assert_eq!(entry.is_encrypted(), case.decoder_encrypted);
        let declaration = declarations.iter().next().ok_or("declaration missing")?;
        assert_eq!(declaration.declared_block_len(), Some(case.declared_len));
        assert_eq!(declaration.raw_value(), case.word);
        assert_eq!(
            declaration.raw_value_bytes().collect::<Vec<_>>(),
            case.body.iter().copied().take(2).collect::<Vec<_>>()
        );
        assert_eq!(
            archive.issues(),
            case.issues
                .iter()
                .cloned()
                .map(|issue| (0, issue))
                .collect::<Vec<_>>()
        );
        if !case.decoder_encrypted {
            let mut decoded = Vec::new();
            archive
                .open_entry_verified(entry)?
                .read_to_end(&mut decoded)?;
            assert_eq!(
                decoded, PAYLOAD,
                "a readable zero does not establish valid provenance"
            );
        }
    }
    Ok(())
}

#[test]
fn duplicate_v1_declarations_keep_order_and_decoder_last_wins() -> FixtureResult<()> {
    let cases: &[(u16, &[u16], DeclaredEncryption, bool)] = &[
        (0, &[0, 0], DeclaredEncryption::Unencrypted, false),
        (1, &[1, 1], DeclaredEncryption::Encrypted, true),
        (1, &[0], DeclaredEncryption::Unknown, false),
        (0, &[1], DeclaredEncryption::Unknown, true),
        (0, &[7, 0], DeclaredEncryption::Unknown, false),
    ];
    for &(first, later, state, decoder_encrypted) in cases {
        let authored = TestEntry::stored("Data\\item.bin", PAYLOAD).with_encryption_field(first);
        let bytes = fixture::additional_declarations(authored, later)?;
        let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
        let entry = fixture::entry(&archive, 0)?;
        let declarations = entry.metadata().encryption_declarations();
        assert_eq!(declarations.state(), state);
        assert_eq!(entry.is_encrypted(), decoder_encrypted);
        let expected: Vec<_> = std::iter::once(first)
            .chain(later.iter().copied())
            .map(Some)
            .collect();
        assert_eq!(
            declarations
                .iter()
                .map(StoredEncryptionDeclaration::raw_value)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            declarations
                .iter()
                .map(StoredEncryptionDeclaration::value_offset)
                .collect::<Vec<_>>(),
            fixture::cig_offsets(&bytes)?
        );
    }
    let authored = TestEntry::stored("Data\\item.bin", PAYLOAD).with_encryption_block([0, 0, 9, 9]);
    let bytes = fixture::additional_declarations(authored, &[0])?;
    let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
    let entry = fixture::entry(&archive, 0)?;
    assert_eq!(
        entry.metadata().encryption_declarations().state(),
        DeclaredEncryption::Unknown
    );
    assert!(!entry.is_encrypted());
    Ok(())
}

#[test]
fn capture_continues_after_the_issue_sample_is_full() -> FixtureResult<()> {
    let mut builder = ArchiveBuilder::new();
    for index in 0..MAX_ENTRY_ISSUES {
        builder = builder.entry(
            TestEntry::stored(format!("Data\\bad-{index}.bin"), PAYLOAD).with_encryption_field(7),
        );
    }
    builder = builder
        .entry(TestEntry::stored("Data\\late.bin", PAYLOAD).with_encryption_block([0, 0, 9, 9]));
    let (_directory, archive) = fixture::open(&builder.build(), OpenOptions::new())?;
    assert_eq!(archive.issues().len(), MAX_ENTRY_ISSUES);
    assert_eq!(archive.issue_count(), MAX_ENTRY_ISSUES + 1);
    let entry = fixture::entry(&archive, MAX_ENTRY_ISSUES)?;
    let declarations = entry.metadata().encryption_declarations();
    assert_eq!(declarations.state(), DeclaredEncryption::Unknown);
    assert_eq!(
        declarations
            .iter()
            .next()
            .and_then(StoredEncryptionDeclaration::raw_value),
        Some(0)
    );
    assert!(!entry.is_encrypted());
    assert!(
        archive
            .issues()
            .iter()
            .all(|(index, _)| *index < MAX_ENTRY_ISSUES)
    );
    let mut decoded = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut decoded)?;
    assert_eq!(decoded, PAYLOAD);
    Ok(())
}

#[test]
fn one_entry_retains_more_declarations_than_the_issue_sample_limit() -> FixtureResult<()> {
    let count = MAX_ENTRY_ISSUES + 3;
    let bytes = fixture::v1(
        TestEntry::stored("Data\\many.bin", PAYLOAD).with_empty_encryption_blocks(count),
    );
    let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
    let entry = fixture::entry(&archive, 0)?;
    let declarations = entry.metadata().encryption_declarations();
    assert_eq!(declarations.iter().len(), count + 1);
    assert_eq!(
        declarations
            .iter()
            .filter(|value| value.raw_value().is_none())
            .count(),
        count
    );
    assert_eq!(declarations.state(), DeclaredEncryption::Unknown);
    assert!(entry.is_encrypted());
    assert_eq!(
        archive.issues(),
        [(0, EntryIssue::EncryptionBlockSize { size: 4 })]
    );
    Ok(())
}
