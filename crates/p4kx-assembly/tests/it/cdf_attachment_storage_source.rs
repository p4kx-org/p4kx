// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io::Read as _;

use p4kx_assembly::{ArchiveModels, ModelProvider};
use p4kx_extraction::Archive;
use p4kx_extraction::model::Lod;

use crate::assembly_support;
use crate::complete_cdf_source::TestResult;

pub(crate) const MATERIALS: [Option<&str>; 6] = [
    Some("Objects/Test/red.mtl"),
    Some("Objects/Test/blue.mtl"),
    Some("Objects/Test/red.mtl"),
    None,
    None,
    Some(""),
];

pub(crate) fn with_archive(check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let definition = definition()?;
    let fixture = assembly_support::fixture(p4kx_testkit::bone_thruster_entries(&definition))?;
    let archive = Archive::open(&fixture.path)?;
    read_original(&archive, &definition)?;
    original_model(&archive)?;
    check(&archive)
}

fn definition() -> Result<String, Box<dyn std::error::Error>> {
    let mut attachments = String::new();
    for (index, material) in MATERIALS.iter().enumerate() {
        let material = material.map_or_else(String::new, |value| format!(" Material=\"{value}\""));
        write!(
            attachments,
            r#"<Attachment Type="CA_BONE" AName="Variant_{index}" BoneName="tip" Binding="Objects\Test\housing.cgf" RelPosition="{index},0,0" RelRotation="1,0,0,0"{material}/>"#
        )?;
    }
    Ok(p4kx_testkit::bone_thruster_definition(&attachments))
}

fn read_original(archive: &Archive, definition: &str) -> TestResult {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == "Data\\Objects\\Test\\thruster.cdf")
        .ok_or("original variant CDF Entry")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    assert_eq!(
        bytes,
        definition.as_bytes(),
        "all original variant CDF bytes before mapping"
    );
    assert_variants(&bytes)
}

fn original_model(archive: &Archive) -> TestResult {
    let model = ArchiveModels::new(archive).model("Objects/Test/housing.cgf", 0)?;
    assert_eq!(
        model.entry, "Data\\Objects\\Test\\housing.cgf",
        "actual original readable binding selection"
    );
    assert_eq!(
        model.model.material_library.as_deref(),
        Some("Objects/Test/paint"),
        "original undecorated material before overrides"
    );
    let count = model.model.lods.iter().map(Lod::vertex_count).max();
    assert_eq!(
        count,
        Some(24),
        "genuine original source geometry before CDF mapping"
    );
    Ok(())
}

fn assert_variants(bytes: &[u8]) -> TestResult {
    let xml = roxmltree::Document::parse(str::from_utf8(bytes)?)?;
    let all: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("Attachment"))
        .collect();
    assert_eq!(
        all.len(),
        7,
        "first source skin plus six original bone occurrences"
    );
    let skin = all.first().ok_or("original selected skin")?;
    assert_eq!(
        skin.attribute("Type"),
        Some("CA_SKIN"),
        "source selection before bones"
    );
    assert_eq!(
        skin.attribute("Binding"),
        Some(r"Objects\Test\nozzle.skin"),
        "original selected skin binding"
    );
    let bones: Vec<_> = all
        .iter()
        .filter(|node| node.attribute("Type") == Some("CA_BONE"))
        .collect();
    assert_eq!(
        bones.len(),
        MATERIALS.len(),
        "every original bone is independently declared"
    );
    for (node, (index, material)) in bones.iter().zip(MATERIALS.iter().enumerate()) {
        assert_bone(node, index, *material);
    }
    Ok(())
}

fn assert_bone(node: &roxmltree::Node<'_, '_>, index: usize, material: Option<&str>) {
    assert_eq!(
        node.attribute("AName"),
        Some(format!("Variant_{index}").as_str()),
        "original ordered variant identity"
    );
    assert_eq!(
        node.attribute("Material"),
        material,
        "absent, empty and exact material override before mapping"
    );
    assert_eq!(
        node.attribute("Binding"),
        Some(r"Objects\Test\housing.cgf"),
        "same original readable binding"
    );
    assert_eq!(
        node.attribute("BoneName"),
        Some("tip"),
        "genuine original bone"
    );
    assert_eq!(
        node.attribute("RelPosition"),
        Some(format!("{index},0,0").as_str()),
        "distinct original instance offset"
    );
    assert_eq!(
        node.attribute("RelRotation"),
        Some("1,0,0,0"),
        "original valid rotation"
    );
}
