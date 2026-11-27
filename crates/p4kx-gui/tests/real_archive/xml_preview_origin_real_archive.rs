// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;
use std::time::{Duration, Instant};

use p4kx_extraction::cryxml::{self, XmlFieldKind, XmlRenderKind};
use p4kx_extraction::{
    Archive, InputScope, ObservedEntry, PhysicalSourceKind, SourceEncryption, SourceLocation,
    SourceUseRole,
};
use p4kx_gui::{Browser, EntryIndex, Preview, XmlDocument, XmlPreview, XmlStatus};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::preview_physical_entry;

type TestResult = Result<(), Box<dyn Error>>;

const CANDIDATES: usize = 32;
const ENTRY_LIMIT: u64 = 64 * 1024;
const SETTLE_LIMIT: Duration = Duration::from_secs(45);

struct ExpectedXml {
    entry_index: u32,
    encrypted: Option<bool>,
    original_name: SourceLocation,
    rendered: Vec<u8>,
}

#[test]
#[ignore = "installed Archive: set P4KX_DATA_P4K and the Archive key"]
fn installed_cryxml_preview_keeps_the_original_root_name() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let expected = find_candidate(&archive)?;
    let index = EntryIndex::build(archive);
    let id = preview_physical_entry::browser_id(&index, expected.entry_index)
        .ok_or("the selected physical Entry has no GUI tree id")?;
    let mut browser = Browser::new(index);
    browser.select_entry(id, || {});
    let document = wait_for_document(&mut browser)?;
    assert!(
        document.bytes() == expected.rendered,
        "the normal XML tab shows the independently read and converted Entry"
    );
    assert!(
        !document.is_truncated(),
        "the selected small document is complete"
    );
    check_original_name(document, &expected)
}

fn find_candidate(archive: &Archive) -> Result<ExpectedXml, Box<dyn Error>> {
    let candidates = archive
        .entries()
        .iter()
        .filter(|entry| {
            (1..=ENTRY_LIMIT).contains(&entry.uncompressed_size())
                && entry
                    .path()
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("xml"))
        })
        .take(CANDIDATES);
    for entry in candidates {
        let original = archive
            .observed_entry(entry)
            .ok_or("the XML candidate has no original physical identity")?;
        let mut raw = Vec::new();
        archive
            .open_entry(entry)
            .map_err(|_| "cannot open the physical XML candidate")?
            .take(ENTRY_LIMIT + 1)
            .read_to_end(&mut raw)
            .map_err(|_| "cannot read the physical XML candidate")?;
        if let Some(expected) = inspect_candidate(original, &raw)? {
            return Ok(expected);
        }
    }
    Err("no CryXml candidate in the bounded physical directory sample".into())
}

fn inspect_candidate(
    original: ObservedEntry<'_>,
    raw: &[u8],
) -> Result<Option<ExpectedXml>, Box<dyn Error>> {
    let Ok(document) = cryxml::parse(raw) else {
        return Ok(None);
    };
    let Some(original_name) = unique_root_location(raw, document.root().name().as_bytes())? else {
        return Ok(None);
    };
    let encrypted = match original.stored_encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let mut rendered = Vec::new();
    document
        .write_xml(&mut rendered)
        .map_err(|_| "the direct CryXml conversion failed")?;
    Ok(Some(ExpectedXml {
        entry_index: original.entry_index(),
        encrypted,
        original_name,
        rendered,
    }))
}

fn unique_root_location(raw: &[u8], name: &[u8]) -> Result<Option<SourceLocation>, Box<dyn Error>> {
    let Some((&first, rest)) = name.split_first() else {
        return Ok(None);
    };
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !rest
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.".contains(byte))
    {
        return Ok(None);
    }
    let mut occurrences = raw
        .windows(name.len() + 1)
        .enumerate()
        .filter(|(_, bytes)| bytes.strip_suffix(&[0]) == Some(name));
    let Some((start, _)) = occurrences.next() else {
        return Ok(None);
    };
    if occurrences.next().is_some() {
        return Ok(None);
    }
    Ok(Some(SourceLocation::archive_entry_decoded_bytes(
        u64::try_from(start)?..u64::try_from(start + name.len())?,
        "stringBytes",
    )))
}

fn wait_for_document(browser: &mut Browser) -> Result<&XmlDocument, Box<dyn Error>> {
    let deadline = Instant::now() + SETTLE_LIMIT;
    while browser.preview().is_some_and(|preview| {
        preview.is_loading()
            || preview
                .xml()
                .is_some_and(|xml| matches!(xml.status(), XmlStatus::Converting))
    }) {
        if Instant::now() >= deadline {
            return Err("the selected CryXml preview did not settle".into());
        }
        browser.poll();
        std::thread::sleep(Duration::from_millis(2));
    }
    browser
        .preview()
        .and_then(Preview::xml)
        .and_then(XmlPreview::text)
        .ok_or_else(|| "the selected CryXml preview did not convert".into())
}

fn check_original_name(document: &XmlDocument, expected: &ExpectedXml) -> TestResult {
    let proof = document
        .provenance()
        .ok_or("converted XML proof")?
        .map_err(ToString::to_string)?;
    let name = proof
        .spans()
        .iter()
        .find(|span| span.kind() == XmlRenderKind::Field(XmlFieldKind::ElementName))
        .ok_or("the retained original root name has no positioned field")?;
    assert_eq!(
        name.origin().contains_encrypted_source(),
        expected.encrypted,
        "the direct stored Entry declaration determines the marker"
    );
    assert!(
        name.origin().source_set_complete(),
        "complete root name proof"
    );
    let used = proof
        .inventory()
        .uses()
        .iter()
        .find(|used| {
            name.origin().source_uses_refs().contains(used.id())
                && used.location() == &expected.original_name
        })
        .ok_or("the retained root name has no matching original decoded byte range")?;
    assert_eq!(
        used.role(),
        SourceUseRole::Conversion,
        "the root name is converted"
    );
    assert_eq!(
        used.scope(),
        InputScope::Payload,
        "the bytes are in the decoded payload"
    );
    let physical = proof
        .inventory()
        .sources()
        .iter()
        .find(|source| source.id() == used.source())
        .ok_or("the original physical root name source is absent")?;
    let ordinal = match physical.kind() {
        PhysicalSourceKind::ArchiveEntry(entry) => u64::try_from(entry.entry_index())?,
        PhysicalSourceKind::ArchiveEntryProof(entry) => entry.entry_index().get(),
        _ => return Err("the original name is not bound to a physical Archive Entry".into()),
    };
    assert_eq!(
        ordinal,
        u64::from(expected.entry_index),
        "the actual selected physical Entry"
    );
    Ok(())
}
