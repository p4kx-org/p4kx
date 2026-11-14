// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::SocScene;
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, SOC_IDENTITY, TestContainer, TestEntry, TestObjectContainer, TestSoc,
};
use serde_json::{Value, json};

type Check = Result<(), Box<dyn std::error::Error>>;

const CONTAINER: &str = "placed.socpak";

#[test]
fn a_palette_sentinel_never_selects_an_existing_palette_slot() -> Check {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    let source = palette_fixture();
    let descriptor = TestObjectContainer::new("placed").stream("placed.soc");
    let container = TestContainer::new()
        .entry(TestEntry::stored("placed.xml", descriptor.xml()))
        .entry(TestEntry::stored("placed.soc", source.bytes()));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(CONTAINER, container.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive
        .entries()
        .first()
        .ok_or("physical container Entry")?;
    let container = archive.open_container(entry)?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    let decoded = serde_json::to_value(SocScene::from_capture(&capture)?)?;
    let table = decoded
        .pointer("/includedObjects/0/tables/value")
        .ok_or("positioned table")?;
    assert!(
        table
            .pointer("/palettes/255/path/value")
            .and_then(Value::as_str)
            .is_some(),
        "original palette slot 255 remains in the path inventory"
    );
    verify_selector(table, 0, 254, &json!(254))?;
    verify_selector(table, 1, 255, &Value::Null)
}

fn palette_fixture() -> TestSoc {
    let source = TestSoc::new()
        .model("objects/test/synthetic.cga")
        .object_with(0, SOC_IDENTITY, 254)
        .object_with(0, SOC_IDENTITY, 255);
    (0..=u8::MAX).fold(source, |soc, index| {
        soc.palette(format!("synthetic/{index}"))
    })
}

fn verify_selector(table: &Value, ordinal: usize, raw: u8, expected: &Value) -> Check {
    let row = table
        .pointer(&format!("/rows/{ordinal}/data/value"))
        .ok_or("original placement")?;
    assert_eq!(
        row.pointer("/paletteSelector/value"),
        Some(&json!(raw)),
        "original unchanged selector"
    );
    assert_eq!(
        row.get("paletteSlot"),
        Some(expected),
        "no-palette sentinel is separate from addressable indices"
    );
    Ok(())
}
