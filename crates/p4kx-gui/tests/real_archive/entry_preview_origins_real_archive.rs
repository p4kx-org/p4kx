// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;
use std::time::{Duration, Instant};

use p4kx_extraction::{Archive, InputScope, SourceEncryption, SourceLocation, SourceUseRole};
use p4kx_gui::{
    Browser, EntryIndex, EntryPreviewProvenance, PREVIEW_READ_LIMIT, Preview, TextLanguage,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

#[path = "../support/preview_physical_entry.rs"]
mod physical_entry;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
#[ignore = "installed Archive: set P4KX_DATA_P4K and the Archive key"]
fn installed_entry_preview_keeps_the_original_localization_entry_prefix() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| {
            TextLanguage::from_global_ini_path(entry.path()) == Some(TextLanguage::English)
        })
        .ok_or("the original English localization Entry is missing")?;
    let original = archive
        .observed_entry(entry)
        .ok_or("the exact Entry is unbound")?;
    let physical_position = original.entry_index();
    let encrypted = match original.stored_encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let mut expected = Vec::new();
    let limit = u64::try_from(PREVIEW_READ_LIMIT)?;
    archive
        .open_entry(entry)?
        .take(limit)
        .read_to_end(&mut expected)?;
    assert!(
        !expected.is_empty(),
        "the original entry has a nonempty prefix"
    );

    let index = EntryIndex::build(archive);
    let id = physical_entry::browser_id(&index, physical_position)
        .ok_or("the browser has no tree id for the original Entry")?;
    let mut browser = Browser::new(index);
    browser.select_entry(id, || {});
    let deadline = Instant::now() + Duration::from_secs(45);
    while browser.preview().is_some_and(Preview::is_loading) {
        if Instant::now() >= deadline {
            return Err("the selected preview did not settle".into());
        }
        browser.poll();
        std::thread::sleep(Duration::from_millis(2));
    }
    let preview = browser
        .preview()
        .and_then(Preview::contents)
        .ok_or("the original preview failed")?;
    assert!(
        preview.bytes() == expected,
        "the independently read physical prefix"
    );
    check_origin(
        preview.provenance()?,
        physical_position,
        encrypted,
        u64::try_from(expected.len())?,
    )
}

fn check_origin(
    proof: &EntryPreviewProvenance,
    physical_position: u32,
    encrypted: Option<bool>,
    len: u64,
) -> TestResult {
    assert_eq!(
        proof.bytes.contains_encrypted_source(),
        encrypted,
        "original stored declaration"
    );
    let inventory = proof.inventory();
    let used = inventory
        .uses()
        .iter()
        .find(|used| proof.bytes.source_uses_refs().contains(used.id()))
        .ok_or("the retained bytes need an original source use")?;
    assert_eq!(
        used.scope(),
        InputScope::Payload,
        "the original decoded byte domain"
    );
    assert_eq!(
        used.role(),
        SourceUseRole::Payload,
        "the retained bytes are payload"
    );
    assert_eq!(
        used.location(),
        &SourceLocation::archive_entry_decoded_bytes(0..len, "bytes"),
        "the exact retained window"
    );
    let original_source = inventory
        .sources()
        .iter()
        .find(|source| source.id() == used.source())
        .ok_or("the physical source is missing")?;
    assert_eq!(
        serde_json::to_value(original_source)?
            .get("entryIndex")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(physical_position)),
        "the exact physical Entry position"
    );
    Ok(())
}
