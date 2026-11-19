// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::localization::{
    CatalogLanguageSource, LanguageSourcesSnapshot, SourceIniEncoding, with_language_sources,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Entry, FieldOrigin, IniEncoding, IniText, InputScope, Language,
    PhysicalSourceKind, SourceEncryption, SourceInventory, SourceLocation, SourceUseRole,
    SourceValueState,
};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};

use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn language_source_sdk_matches_original_archive_entries() -> TestResult {
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    for (input, path) in std::iter::once(real_archive_path()?)
        .chain(real_archive_b_path())
        .enumerate()
    {
        let archive = Archive::open_with_key(path, Some(key.clone()))
            .map_err(|_| "open installed localization Archive")?;
        let watch = ArchiveWatch::new(&archive);
        let (present, absent) = with_language_sources(
            &archive,
            CatalogLanguageSelection::AllAvailable,
            None,
            |result| verify(&archive, &result),
        )??;
        watch.check().map_err(|_| "localization Archive changed")?;
        print_line(&format!(
            "language sources input {}: {present} verified original Entries, {absent} absent languages",
            input + 1
        ));
    }
    Ok(())
}

fn verify(archive: &Archive, result: &LanguageSourcesSnapshot<'_>) -> TestResult<(usize, usize)> {
    if archive.build().is_none()
        || result.build != archive.build()
        || result.sources.len() != Language::ALL.len()
    {
        return Err("unexpected Build or supported language scope".into());
    }
    let mut present = 0;
    let mut absent = 0;
    for language in Language::ALL {
        let path = language.global_ini_path();
        let expected = find_localization_entry(archive, path);
        let row = result
            .sources
            .get(language.code())
            .ok_or("requested language source state")?;
        if let Some(entry) = expected {
            let source = row.value().ok_or("present original localization Entry")?;
            verify_entry(archive, entry, source, &result.source_inventory)?;
            present += 1;
        } else {
            if row.state() != SourceValueState::Absent || !row.origin().source_set_complete() {
                return Err("absent original language requires complete directory evidence".into());
            }
            absent += 1;
        }
    }
    if present == 0 {
        return Err("no original localization Entries verified".into());
    }
    Ok((present, absent))
}

fn find_localization_entry<'archive>(
    archive: &'archive Archive,
    path: &str,
) -> Option<&'archive Entry> {
    archive.entries().iter().find(|entry| {
        let mut original_parts = entry.path().split(['\\', '/']);
        path.split('\\').all(|part| {
            original_parts
                .next()
                .is_some_and(|original| part.eq_ignore_ascii_case(original))
        }) && original_parts.next().is_none()
    })
}

fn verify_entry(
    archive: &Archive,
    entry: &Entry,
    source: &CatalogLanguageSource<'_>,
    inventory: &SourceInventory,
) -> TestResult {
    let mut original = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut original)?;
    let decoded = IniText::decode(&original)?;
    let encoding = match decoded.encoding() {
        IniEncoding::Utf8 => SourceIniEncoding::Utf8,
        IniEncoding::Utf8Bom => SourceIniEncoding::Utf8Bom,
        IniEncoding::Utf16Le => SourceIniEncoding::Utf16Le,
        IniEncoding::Utf16Be => SourceIniEncoding::Utf16Be,
        _ => SourceIniEncoding::Unknown,
    };
    if source.path().value().copied() != Some(entry.path())
        || source.encoding().value() != Some(&encoding)
        || source.lossy_decoding().value() != Some(&decoded.is_lossy())
        || source.skipped_malformed_lines().value() != Some(&skipped_lines(decoded.text()))
    {
        return Err("the SDK metadata differs from its verified original language Entry".into());
    }
    let original = archive
        .entry_origin(entry)
        .ok_or("original physical Entry evidence")?;
    let marker = match original.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    verify_metadata_path(inventory, source.path().origin(), entry.index())?;
    for (name, origin) in [
        ("encoding", source.encoding().origin()),
        ("lossyDecoding", source.lossy_decoding().origin()),
        (
            "skippedMalformedLines",
            source.skipped_malformed_lines().origin(),
        ),
    ] {
        if origin.contains_encrypted_source() != marker {
            return Err("the selected locale conversion has different encryption evidence".into());
        }
        verify_conversion(inventory, origin, entry.index(), name)?;
    }
    Ok(())
}

fn skipped_lines(text: &str) -> usize {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter(|line| {
            let Some((head, _)) = line.split_once('=') else {
                return true;
            };
            let key = head.split_once(',').map_or(head, |(key, _)| key);
            key.trim().is_empty()
        })
        .count()
}

fn verify_metadata_path(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    index: usize,
) -> TestResult {
    use_source(
        inventory,
        origin,
        index,
        InputScope::ArchiveDirectoryMetadata,
        SourceUseRole::Payload,
        |location| matches!(location, SourceLocation::Metadata { field } if field == "path"),
    )
}

fn verify_conversion(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    index: usize,
    wanted: &str,
) -> TestResult {
    use_source(
        inventory,
        origin,
        index,
        InputScope::Payload,
        SourceUseRole::Conversion,
        |location| {
            matches!(
                location,
                SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "globalIni" && indices.is_empty() && field == wanted
            )
        },
    )
}

fn use_source(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    index: usize,
    scope: InputScope,
    role: SourceUseRole,
    location_matches: impl FnOnce(&SourceLocation) -> bool,
) -> TestResult {
    let [reference] = origin.source_uses_refs() else {
        return Err("locale metadata needs one exact original use".into());
    };
    let source_use = inventory
        .iter_uses()
        .find(|used| used.id() == reference)
        .ok_or("no original locale use in the query inventory")?;
    let physical = inventory
        .iter_sources()
        .find(|source| source.id() == source_use.source())
        .ok_or("no physical locale source in the query inventory")?;
    let expected_entry = match physical.kind() {
        PhysicalSourceKind::ArchiveEntry(entry) => entry.entry_index() == index,
        PhysicalSourceKind::ArchiveEntryProof(entry) => {
            u64::try_from(index)? == entry.entry_index().get()
        }
        _ => false,
    };
    if !origin.source_set_complete()
        || !expected_entry
        || source_use.scope() != scope
        || source_use.role() != role
        || !location_matches(source_use.location())
    {
        return Err("the locale metadata lacks its exact physical Entry or observation".into());
    }
    Ok(())
}
