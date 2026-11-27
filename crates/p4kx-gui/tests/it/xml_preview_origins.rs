// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_extraction::cryxml::{XmlFieldKind, XmlRenderKind};
use p4kx_gui::{Browser, EntryIndex, Preview, XmlDocument, XmlPreview, XmlStatus};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry,
    test_key,
};

use crate::preview_physical_entry;
use crate::waiting::wait_polling;
use crate::xml_previewing::TestResult;

fn version(label: &str) -> Vec<u8> {
    TestElement::new("selected")
        .attribute("label", label)
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

#[test]
fn the_xml_preview_keeps_the_selected_duplicate_and_encrypted_container() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (mut browser, position) = select_duplicate(folder.path().join("Data.p4k"))?;
    wait_polling(
        &mut browser,
        "the selected converted XML",
        |browser| {
            browser.poll();
        },
        |browser| {
            browser
                .preview()
                .and_then(Preview::xml)
                .is_some_and(|xml| !matches!(xml.status(), XmlStatus::Converting))
        },
    )?;
    let document = browser
        .preview()
        .and_then(Preview::xml)
        .and_then(XmlPreview::text)
        .ok_or("the converted XML")?;
    check_selected(document, position)
}

fn select_duplicate(
    path: std::path::PathBuf,
) -> Result<(Browser, u32), Box<dyn std::error::Error>> {
    let inner = TestContainer::new()
        .entry(TestEntry::stored("duplicate.xml", version("other")))
        .entry(TestEntry::stored("duplicate.xml", version("chosen")));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("outer.socpak", inner.build()).encrypted())
        .write_to(&path)?;
    let root = Archive::open_with_key(path, Some(test_key()))?;
    let [outer] = root.entries() else {
        return Err("one encrypted outer source".into());
    };
    let inner = root.open_container(outer)?;
    let [_, original] = inner.entries() else {
        return Err("two physical duplicate leaves".into());
    };
    let position = inner
        .observed_entry(original)
        .ok_or("original leaf")?
        .entry_index();
    let index = EntryIndex::build(inner);
    let id = preview_physical_entry::browser_id(&index, position)
        .ok_or("the chosen physical leaf has a tree id")?;
    let mut browser = Browser::new(index);
    browser.select_entry(id, || {});
    Ok((browser, position))
}

fn check_selected(document: &XmlDocument, position: u32) -> TestResult {
    assert!(
        std::str::from_utf8(document.bytes())?.contains("label=\"chosen\""),
        "the physical occurrence was preserved"
    );
    let proof = document
        .provenance()
        .ok_or("positioned XML proof")?
        .map_err(ToString::to_string)?;
    assert_eq!(
        proof.contains_encrypted_source(),
        Some(true),
        "the containing Entry is encrypted"
    );
    assert_eq!(
        proof.inventory().source_count(),
        2,
        "the selected leaf and its one ancestor"
    );
    let attribute = proof
        .spans()
        .iter()
        .find(|span| span.kind() == XmlRenderKind::Field(XmlFieldKind::AttributeValue))
        .ok_or("the selected original attribute value")?;
    assert_eq!(
        attribute.origin().contains_encrypted_source(),
        Some(true),
        "immediate marker"
    );
    assert!(
        attribute.origin().source_set_complete(),
        "the value's original contributors are known"
    );
    let used = proof
        .inventory()
        .uses()
        .iter()
        .find(|used| attribute.origin().source_uses_refs().contains(used.id()))
        .ok_or("the original attribute source use")?;
    let source = proof
        .inventory()
        .sources()
        .iter()
        .find(|source| source.id() == used.source())
        .ok_or("the selected physical source")?;
    let source = serde_json::to_value(source)?;
    assert_eq!(
        source.get("entryIndex").and_then(serde_json::Value::as_u64),
        Some(u64::from(position)),
        "the proof follows the physical selected leaf"
    );
    Ok(())
}
