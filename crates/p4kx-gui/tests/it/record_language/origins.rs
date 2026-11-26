// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, FieldOrigin, SourceLocation, SourceUseRef, SourceUseRole};
use p4kx_gui::{
    Browser, EntryIndex, KeyText, LineValue, RecordPreview, RecordTreeStatus, RecordView,
    TextLanguage,
};
use p4kx_testkit::{GlobalIniBuilder, TestEntry, test_key};

use super::fixtures::{TRUCK_PATH, TestResult, archive_with, database};
use crate::waiting::wait_polling;

fn selected_browser(
    index: EntryIndex,
    language: Option<TextLanguage>,
) -> Result<Browser, Box<dyn Error>> {
    let mut browser = Browser::new(index);
    browser.load_records(|| {});
    wait_polling(&mut browser, "Record tree", poll, |browser| {
        browser
            .records()
            .is_some_and(|records| matches!(records.status(), RecordTreeStatus::Ready(_)))
    })?;
    let node = browser
        .records()
        .and_then(|records| records.find(TRUCK_PATH))
        .ok_or("the second physical Record")?;
    browser.set_record_language(language);
    browser.select_record_node(node, || {});
    browser.load_record_preview(|| {});
    Ok(browser)
}

fn poll(browser: &mut Browser) {
    browser.poll();
}

fn ready(browser: &mut Browser) -> Result<&RecordView, Box<dyn Error>> {
    wait_polling(browser, "selected Record preview", poll, |browser| {
        browser
            .record_preview()
            .and_then(RecordPreview::view)
            .is_some()
    })?;
    browser
        .record_preview()
        .and_then(RecordPreview::view)
        .ok_or_else(|| "the Record view is missing".into())
}

#[test]
fn record_key_and_translation_bind_independent_original_sources() -> TestResult {
    let english = GlobalIniBuilder::new()
        .text("vehicle_Truck", "Truck")
        .text("vehicle_TruckDesc", "Hauls.\\n<&")
        .build();
    let english = TestEntry::zstd(TextLanguage::English.global_ini_path(), english).encrypted();
    let (_folder, path) = archive_with(&[], vec![english])?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let mut browser = selected_browser(EntryIndex::build(archive), Some(TextLanguage::English))?;
    let view = ready(&mut browser)?;
    let proof = view
        .provenance()
        .ok_or("Record evidence")?
        .map_err(ToString::to_string)?;
    assert!(
        proof.inventory().uses().iter().any(|used| {
            proof.header().guid.source_uses_refs().contains(used.id())
                && matches!(used.location(), SourceLocation::DataForge { record_index: 1, field } if field == "guid")
        }),
        "physical second Record identity"
    );
    assert_localized_rows(view)?;
    assert_xml_origins(view)
}

fn assert_localized_rows(view: &RecordView) -> TestResult {
    let proof = view
        .provenance()
        .ok_or("Record evidence")?
        .map_err(ToString::to_string)?;
    let line = |name: &str| {
        view.lines()
            .iter()
            .zip(proof.lines())
            .find(|(line, _)| line.name() == name)
            .ok_or("missing visible field")
    };
    assert_eq!(
        view.lines().len(),
        proof.lines().len(),
        "one exact occurrence per visible line"
    );
    let (row, key) = line("name")?;
    assert!(
        matches!(row.value(), LineValue::Key { text: KeyText::Text { text, .. }, .. } if &**text == "Truck"),
        "the translation was shown"
    );
    assert_markers(
        &key.value,
        key.annotation.as_ref().ok_or("translated value")?,
    );
    let (_, nested) = line("[0]")?;
    let translation = nested
        .annotation
        .as_ref()
        .ok_or("translated nested value")?;
    assert_markers(&nested.value, translation);
    let (_, missing) = line("[1]")?;
    let missing = missing
        .annotation
        .as_ref()
        .ok_or("missing decision evidence")?;
    assert_eq!(
        missing.contains_encrypted_source(),
        Some(true),
        "the encrypted file was actually consulted"
    );
    let (_, placeholder) = line("unset")?;
    assert!(
        placeholder.annotation.is_none(),
        "the placeholder does not consume a language value"
    );
    assert_locale_site(view, nested.value.source_uses_refs(), translation)
}

fn assert_markers(key: &FieldOrigin, translation: &FieldOrigin) {
    assert_eq!(
        key.contains_encrypted_source(),
        Some(false),
        "the original authored key came from plain DataForge"
    );
    assert_eq!(
        translation.contains_encrypted_source(),
        Some(true),
        "the visible text came from encrypted localisation"
    );
    assert!(
        key.source_set_complete() && translation.source_set_complete(),
        "each field has its own complete origin"
    );
}

fn assert_locale_site(
    view: &RecordView,
    key_uses: &[SourceUseRef],
    translation: &FieldOrigin,
) -> TestResult {
    let proof = view
        .provenance()
        .ok_or("Record evidence")?
        .map_err(ToString::to_string)?;
    assert!(
        proof.inventory().uses().iter().any(|used| {
            key_uses.contains(used.id())
                && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. } if namespace == "dataforgeDecodedBytes" && field == "arrayHeader")
        }),
        "the selected array ordinal retains the original header"
    );
    assert!(
        proof.inventory().uses().iter().any(|used| {
            translation.source_uses_refs().contains(used.id())
                && used.role() == SourceUseRole::Conversion
                && matches!(used.location(), SourceLocation::LocalizationLookup { language, .. } if *language == TextLanguage::English)
        }),
        "the actual English lookup supplied the resolved text"
    );
    Ok(())
}

#[test]
fn a_standalone_dataforge_keeps_exact_fields_with_unknown_original_encryption() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Game2.dcb");
    std::fs::write(&path, database().build())?;
    let mut browser = selected_browser(EntryIndex::build_dataforge_file(path)?, None)?;
    let view = ready(&mut browser)?;
    let proof = view
        .provenance()
        .ok_or("Record evidence")?
        .map_err(ToString::to_string)?;
    assert_eq!(
        proof.header().guid.contains_encrypted_source(),
        None,
        "original P4K encryption is unknown"
    );
    let first_field = proof.lines().first().ok_or("first stored field")?;
    assert_eq!(
        first_field.value.contains_encrypted_source(),
        None,
        "field origin is explicitly unknown"
    );
    assert!(
        first_field.value.source_set_complete(),
        "the standalone file's original field position is known"
    );
    assert!(
        !first_field.value.source_uses_refs().is_empty(),
        "the field retains original storage proof"
    );
    Ok(())
}

fn assert_xml_origins(view: &RecordView) -> TestResult {
    let record = view
        .provenance()
        .ok_or("Record evidence")?
        .map_err(ToString::to_string)?;
    let fields = record
        .xml_fields()
        .ok_or("Record XML evidence")?
        .map_err(ToString::to_string)?;
    for (fragment, marker) in [
        ("@vehicle_TruckDesc", false),
        ("Hauls.&#10;&lt;&amp;", true),
        ("true", true),
        ("", false),
    ] {
        let origin = fields
            .iter()
            .find_map(|field| {
                let range = usize::try_from(field.bytes.start).ok()?
                    ..usize::try_from(field.bytes.end).ok()?;
                (view.xml().bytes().get(range)? == fragment.as_bytes()).then_some(&field.origin)
            })
            .ok_or("expected exact emitted XML fragment")?;
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(marker),
            "marker for {fragment:?}"
        );
        assert!(
            origin.source_set_complete(),
            "complete proof for {fragment:?}"
        );
    }
    assert!(!fields.is_empty(), "Record XML fields were retained");
    let first = fields.first().ok_or("first emitted field")?;
    assert!(
        first.bytes.start > 0,
        "the generated declaration has no original field"
    );
    Ok(())
}
