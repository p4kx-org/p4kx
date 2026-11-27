// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, FieldOrigin, InputScope, SourceLocation, SourceUseRole};
use p4kx_gui::{
    Browser, EntryIndex, EntryPreviewProvenance, Language, PREVIEW_READ_LIMIT, Preview,
};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};

use super::{TestResult, opened, preview};
use crate::preview_physical_entry;
use crate::waiting::wait_polling;

fn marker(origin: &FieldOrigin, encrypted: bool) {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(encrypted),
        "immediate encryption"
    );
    assert!(
        origin.source_set_complete(),
        "the contributing input is known"
    );
}

fn window(proof: &EntryPreviewProvenance, origin: &FieldOrigin, len: u64, role: SourceUseRole) {
    let matches: Vec<_> = proof
        .inventory()
        .uses()
        .iter()
        .filter(|used| origin.source_uses_refs().contains(used.id()))
        .collect();
    assert_eq!(matches.len(), 1, "one retained window contributes");
    if let [used] = matches.as_slice() {
        assert_eq!(used.role(), role, "input role");
        assert_eq!(used.scope(), InputScope::Payload, "decoded payload scope");
        assert_eq!(
            used.location(),
            &SourceLocation::archive_entry_decoded_bytes(0..len, "bytes"),
            "exact retained window"
        );
    }
}

#[test]
fn text_and_hex_keep_the_selected_entry_window_and_separate_directory_metadata() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;
    for (name, encrypted) in [
        ("config.xml", false),
        ("secret.txt", true),
        ("long.txt", false),
    ] {
        preview(&mut harness, name)?;
        let selected = harness
            .state()
            .browser()
            .and_then(Browser::preview)
            .and_then(Preview::contents)
            .ok_or("the preview has retained contents")?;
        let proof = selected.provenance()?;
        marker(&proof.path, false);
        marker(&proof.size, false);
        for origin in [
            &proof.bytes,
            &proof.text,
            &proof.hex,
            &proof.shown_length,
            &proof.truncated,
        ] {
            marker(origin, encrypted);
        }
        let len = u64::try_from(selected.bytes().len())?;
        window(proof, &proof.bytes, len, SourceUseRole::Payload);
        for origin in [&proof.text, &proof.hex, &proof.shown_length] {
            window(proof, origin, len, SourceUseRole::Conversion);
        }
        assert_eq!(
            proof.truncated.source_uses_refs().len(),
            2,
            "size and retained bytes"
        );
        if name == "long.txt" {
            assert_eq!(
                selected.bytes().len(),
                PREVIEW_READ_LIMIT,
                "the ordinary preview cap"
            );
            assert!(
                selected.is_truncated(),
                "successful prefix is incomplete as content"
            );
        }
    }
    Ok(())
}

#[test]
fn an_empty_preview_still_binds_the_payload_and_its_encrypted_container() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let nested = TestContainer::new()
        .entry(TestEntry::stored("z-spare.txt", "other payload"))
        .entry(TestEntry::stored("empty.txt", Vec::new()));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("clue.socpak", nested.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let [entry] = archive.entries() else {
        return Err("the outer Entry is unique".into());
    };
    let container = archive.open_container(entry)?;
    let original = container
        .entries()
        .iter()
        .find(|entry| entry.path() == "empty.txt")
        .and_then(|entry| container.observed_entry(entry))
        .ok_or("the exact nested Entry")?;
    let physical_position = original.entry_index();
    let index = EntryIndex::build(container);
    let id = preview_physical_entry::browser_id(&index, physical_position)
        .ok_or("the nested tree id")?;
    assert_ne!(
        id, physical_position,
        "tree order differs from physical order"
    );
    assert_eq!(index.name(id), "empty.txt", "selected nested Entry");
    let mut browser = Browser::new(index);
    browser.select_entry(id, || {});
    wait_polling(
        &mut browser,
        "the empty nested preview",
        |browser| {
            browser.poll();
        },
        |browser| {
            browser
                .preview()
                .is_some_and(|preview| !preview.is_loading())
        },
    )?;
    let contents = browser
        .preview()
        .and_then(Preview::contents)
        .ok_or("empty contents")?;
    assert!(contents.bytes().is_empty(), "the retained payload is empty");
    let proof = contents.provenance()?;
    marker(&proof.bytes, true);
    marker(&proof.size, true);
    window(proof, &proof.bytes, 0, SourceUseRole::Payload);
    assert_eq!(
        proof.inventory().source_count(),
        2,
        "the physical leaf and containing Entry"
    );
    Ok(())
}
