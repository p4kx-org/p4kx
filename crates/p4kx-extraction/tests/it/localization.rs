// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the `global.ini` of a language from an Archive.

use std::error::Error;
use std::path::Path;

use p4kx_archive::DataError;
use p4kx_extraction::{
    Archive, Language, LocalizationEntry, LocalizationError, LocalizationLimits,
    LocalizationLoadError, LocalizationSet,
};
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

fn english() -> Vec<u8> {
    GlobalIniBuilder::new()
        .text("manufacturer_NameACAS", "Ace Astrogation")
        .build()
}

fn open(dir: &Path, builder: &ArchiveBuilder) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

#[test]
fn the_global_ini_of_a_language_is_read_and_resolves_keys() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::zstd("Data/LOCALIZATION/English/global.ini", english()).encrypted()),
    )?;
    let entry = LocalizationEntry::read(&archive, Language::English)?;
    assert_eq!(entry.language(), Language::English, "language");
    assert_eq!(
        entry.path(),
        "Data/LOCALIZATION/English/global.ini",
        "case and separator do not matter"
    );
    let ini = entry.parse()?;
    assert_eq!(
        ini.resolve("@manufacturer_NameACAS"),
        Some("Ace Astrogation"),
        "resolved"
    );
    Ok(())
}

#[test]
fn a_language_the_archive_does_not_hold_is_missing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(
            Language::English.global_ini_path(),
            english(),
        )),
    )?;
    let result = LocalizationEntry::read(&archive, Language::German);
    assert!(
        matches!(result, Err(LocalizationLoadError::Missing { .. })),
        "German is missing"
    );
    Ok(())
}

#[test]
fn an_entry_shorter_than_its_size_is_a_read_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = english();
    let actual = u64::try_from(text.len())?;
    let claimed = actual + 16;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(
            TestEntry::stored(Language::English.global_ini_path(), text)
                .with_uncompressed_size(claimed),
        ),
    )?;
    let Err(LocalizationLoadError::Read { source, .. }) =
        LocalizationEntry::read(&archive, Language::English)
    else {
        return Err("the shortened input must fail with an Archive read error".into());
    };
    assert!(
        matches!(source.get_ref().and_then(|error| error.downcast_ref::<DataError>()),
            Some(DataError::TooShort { actual: read, expected }) if *read == actual && *expected == claimed),
        "the original Archive length error survives: {source}"
    );
    Ok(())
}

#[test]
fn an_entry_longer_than_its_size_is_a_read_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = english();
    let claimed = u64::try_from(text.len())? - 4;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(
            TestEntry::zstd(Language::English.global_ini_path(), text)
                .with_uncompressed_size(claimed),
        ),
    )?;
    let Err(LocalizationLoadError::Read { source, .. }) = LocalizationEntry::read_with_limits(
        &archive,
        Language::English,
        LocalizationLimits::default().with_source_bytes(claimed),
    ) else {
        return Err(
            "the overlong source must fail independently of the admitted advertised size".into(),
        );
    };
    assert!(
        matches!(source.get_ref().and_then(|error| error.downcast_ref::<DataError>()),
            Some(DataError::TooLong { expected }) if *expected == claimed),
        "the original Archive overlong error survives: {source}"
    );
    Ok(())
}

#[test]
fn an_entry_over_the_caller_source_limit_is_not_read() -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = english();
    let size = u64::try_from(text.len())?;
    let limit = size - 1;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(Language::German.global_ini_path(), text)),
    )?;
    let result = LocalizationEntry::read_with_limits(
        &archive,
        Language::German,
        LocalizationLimits::default().with_source_bytes(limit),
    );
    assert!(
        matches!(result, Err(LocalizationLoadError::TooLarge { path, size: reported_size, limit: reported_limit })
            if path == Language::German.global_ini_path()
                && reported_size == size && reported_limit == limit),
        "the caller's source-byte limit reports the actual declared size and path"
    );
    let entry = LocalizationEntry::read_with_limits(
        &archive,
        Language::German,
        LocalizationLimits::default().with_source_bytes(size),
    )?;
    assert_eq!(
        entry.parse()?.resolve("@manufacturer_NameACAS"),
        Some("Ace Astrogation"),
        "the exact caller source-byte limit admits the complete text"
    );
    Ok(())
}

#[test]
fn the_decoded_byte_limit_remains_distinct_from_the_source_byte_limit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"k=\xff".as_slice(),
        )),
    )?;
    let result = LocalizationEntry::read_with_limits(
        &archive,
        Language::English,
        LocalizationLimits::default()
            .with_source_bytes(3)
            .with_decoded_bytes(4),
    );
    assert!(
        matches!(
            result,
            Err(LocalizationLoadError::Decode {
                source: LocalizationError::DecodedTooLarge { size: 5, limit: 4 },
                ..
            })
        ),
        "the replacement character expands a three-byte source to five decoded bytes"
    );
    Ok(())
}

fn german_utf16() -> Vec<u8> {
    let mut bytes = b"\xff\xfe".to_vec();
    bytes.extend(
        "manufacturer_NameACAS=Ace Astrogation DE\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    bytes
}

#[test]
fn the_languages_of_an_archive_are_listed_in_the_order_of_language_all() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::stored(
                Language::Japanese.global_ini_path(),
                english(),
            ))
            .entry(TestEntry::stored(
                "data/localization/GERMAN_(GERMANY)/Global.INI",
                german_utf16(),
            ))
            .entry(TestEntry::stored(
                "Data\\Localization\\klingon\\global.ini",
                english(),
            ))
            .entry(TestEntry::stored(
                Language::English.global_ini_path(),
                english(),
            )),
    )?;
    assert_eq!(
        LocalizationEntry::languages(&archive),
        [Language::English, Language::German, Language::Japanese],
        "known languages only"
    );
    Ok(())
}

#[test]
fn a_set_reads_the_wanted_languages_that_exist_and_resolves_across_them() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::zstd(
                Language::German.global_ini_path(),
                german_utf16(),
            ))
            .entry(TestEntry::zstd(
                Language::English.global_ini_path(),
                english(),
            )),
    )?;
    let set = LocalizationSet::read(&archive, &[Language::German, Language::French])?;
    assert_eq!(
        set.languages().collect::<Vec<_>>(),
        [Language::German],
        "French is absent, English not wanted"
    );
    let all = LocalizationSet::read_all(&archive)?;
    assert_eq!(
        all.languages().collect::<Vec<_>>(),
        [Language::English, Language::German],
        "both"
    );
    let texts = all.index()?;
    assert_eq!(
        texts
            .lookup(
                "@manufacturer_NameACAS",
                Language::German,
                Some(Language::English)
            )
            .text(),
        Some("Ace Astrogation DE"),
        "UTF-16 German"
    );
    assert!(
        all.entry(Language::German)
            .is_some_and(|entry| entry.text().encoding() == p4kx_extraction::IniEncoding::Utf16Le),
        "encoding is reported"
    );
    Ok(())
}

#[test]
fn a_set_over_an_archive_without_any_global_ini_is_empty() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored("Data\\a.txt", b"x".to_vec())),
    )?;
    let set = LocalizationSet::read_all(&archive)?;
    assert!(set.entries().is_empty(), "nothing read");
    assert!(set.index()?.keys().is_empty(), "nothing indexed");
    Ok(())
}

#[test]
fn a_set_applies_limits_independently_to_each_language() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::stored(
                Language::English.global_ini_path(),
                b"k=A\n".as_slice(),
            ))
            .entry(TestEntry::stored(
                Language::German.global_ini_path(),
                b"k=B\n".as_slice(),
            )),
    )?;
    let limits = LocalizationLimits::default()
        .with_source_bytes(4)
        .with_decoded_bytes(4)
        .with_keys(1);
    let set = LocalizationSet::read_all_with_limits(&archive, limits)?;
    let text = set.index()?;
    for (language, expected) in [(Language::English, "A"), (Language::German, "B")] {
        assert_eq!(
            text.lookup("k", language, None).text(),
            Some(expected),
            "each selected language owns its entire byte and key allowance"
        );
    }
    Ok(())
}

#[test]
fn an_entry_keeps_its_caller_key_limit_when_indexed_through_the_set() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = Language::French.global_ini_path();
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(path, b"a=A\nb=B\n".as_slice())),
    )?;
    let limits = LocalizationLimits::default().with_keys(1);
    let set = LocalizationSet::read_with_limits(&archive, &[Language::French], limits)?;
    let entry = set.entry(Language::French).ok_or("selected French Entry")?;
    assert!(
        matches!(
            entry.parse(),
            Err(LocalizationError::TooManyKeys { limit: 1 })
        ),
        "direct indexing of a selected Entry retains the caller policy"
    );
    assert!(
        matches!(set.index(), Err(LocalizationLoadError::Index {
            path: failed_path, source: LocalizationError::TooManyKeys { limit: 1 },
        }) if failed_path == path),
        "set indexing returns the exact expired allowance and physical Entry path"
    );
    let full = LocalizationSet::read_all(&archive)?;
    assert_eq!(
        full.index()?.keys().len(),
        2,
        "the default read and index resolve both keys"
    );
    Ok(())
}

#[test]
fn a_broken_entry_makes_the_set_fail_with_its_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = english();
    let claimed = u64::try_from(text.len())? + 16;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(
            TestEntry::stored(Language::English.global_ini_path(), text)
                .with_uncompressed_size(claimed),
        ),
    )?;
    let result = LocalizationSet::read_all(&archive);
    assert!(
        matches!(result, Err(LocalizationLoadError::Read { .. })),
        "truncated: {result:?}"
    );
    Ok(())
}

#[test]
fn an_entry_with_invalid_utf8_is_read_lossily() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"good=1\r\nbad=\xff\r\nlast=3\r\n".to_vec(),
        )),
    )?;
    let entry = LocalizationEntry::read(&archive, Language::English)?;
    assert!(entry.text().is_lossy(), "lossy");
    let ini = entry.parse()?;
    assert_eq!(ini.resolve("last"), Some("3"), "later lines survive");
    Ok(())
}
