// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::assembly_support;
use p4kx_assembly::{ArchiveModels, MAX_ASSEMBLY_VERTICES, ModelProvider};
use p4kx_extraction::Archive;
use p4kx_extraction::model::{Lod, Model};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(crate) const CDF: &str = "Objects/Test/thruster.cdf";
pub(crate) const SKIN: &str = "Data\\Objects\\Test\\nozzle.skin";
const CDF_ENTRY: &str = "Data\\Objects\\Test\\thruster.cdf";

pub(crate) fn definition(bones: u16, left_out: u16) -> String {
    use std::fmt::Write as _;

    let mut attachments = String::new();
    for index in 0..bones {
        let _infallible = write!(
            attachments,
            r#"<Attachment Type="CA_BONE" AName="Bone_{index}" BoneName="tip" Binding="Objects\Test\housing.cgf" RelPosition="{index},0,0" RelRotation="1,0,0,0"/>"#,
        );
    }
    for index in 0..left_out {
        let _infallible = write!(
            attachments,
            r#"<Attachment Type="CA_SKIN" AName="Left_{index}" Binding="Objects\Test\left_{index}.skin"/>"#,
        );
    }
    p4kx_testkit::bone_thruster_definition(&attachments)
}

pub(crate) fn with_archive(
    bones: u16,
    left_out: u16,
    check: impl FnOnce(&Archive) -> TestResult,
) -> TestResult {
    use std::io::Read as _;

    let definition = definition(bones, left_out);
    let fixture = assembly_support::fixture(p4kx_testkit::bone_thruster_entries(&definition))?;
    let archive = Archive::open(&fixture.path)?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == CDF_ENTRY)
        .ok_or("the actual original CDF Entry")?;
    assert!(
        entry.uncompressed_size() < 1 << 20,
        "the original CDF fits independent byte admission"
    );
    let mut original = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut original)?;
    assert_eq!(
        original,
        definition.as_bytes(),
        "all original CDF bytes are retained"
    );
    original_slots(&original, bones, left_out)?;
    original_models(&archive, bones)?;
    check(&archive)
}

fn original_slots(bytes: &[u8], bones: u16, left_out: u16) -> TestResult {
    let document = roxmltree::Document::parse(str::from_utf8(bytes)?)?;
    assert_eq!(
        document.root_element().tag_name().name(),
        "CharacterDefinition",
        "source root"
    );
    let slots: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("Attachment"))
        .collect();
    assert_eq!(
        slots.len(),
        1 + usize::from(bones) + usize::from(left_out),
        "all original slots exist"
    );
    let first = slots.first().ok_or("the actual first skin")?;
    assert_eq!(
        first.attribute("Type"),
        Some("CA_SKIN"),
        "original model-selection premise"
    );
    assert_eq!(
        first.attribute("Binding"),
        Some(r"Objects\Test\nozzle.skin"),
        "original first skin"
    );
    original_bones(&slots, bones);
    original_left_out(&slots, left_out);
    Ok(())
}

fn original_bones(slots: &[roxmltree::Node<'_, '_>], count: u16) {
    let actual: Vec<_> = slots
        .iter()
        .filter(|node| node.attribute("Type") == Some("CA_BONE"))
        .collect();
    assert_eq!(
        actual.len(),
        usize::from(count),
        "all original bone attachments precede mapping"
    );
    for (node, index) in actual.iter().zip(0..count) {
        assert_eq!(
            node.attribute("AName"),
            Some(format!("Bone_{index}").as_str()),
            "ordered original name"
        );
        assert_eq!(
            node.attribute("BoneName"),
            Some("tip"),
            "actual existing bone"
        );
        assert_eq!(
            node.attribute("Binding"),
            Some(r"Objects\Test\housing.cgf"),
            "actual readable binding"
        );
        assert_eq!(
            node.attribute("RelPosition"),
            Some(format!("{index},0,0").as_str()),
            "exact late offset"
        );
        assert_eq!(
            node.attribute("RelRotation"),
            Some("1,0,0,0"),
            "valid original rotation"
        );
    }
}

fn original_left_out(slots: &[roxmltree::Node<'_, '_>], count: u16) {
    let actual: Vec<_> = slots
        .iter()
        .filter(|node| {
            node.attribute("AName")
                .is_some_and(|name| name.starts_with("Left_"))
        })
        .collect();
    assert_eq!(
        actual.len(),
        usize::from(count),
        "all original unselected attachments exist"
    );
    for (node, index) in actual.iter().zip(0..count) {
        assert_eq!(
            node.attribute("Type"),
            Some("CA_SKIN"),
            "honestly unselected extra skin"
        );
        assert_eq!(
            node.attribute("Binding"),
            Some(format!(r"Objects\Test\left_{index}.skin").as_str()),
            "the actual original late binding precedes mapper assertions",
        );
    }
}

fn original_models(archive: &Archive, bones: u16) -> TestResult {
    let models = ArchiveModels::new(archive);
    let skin = models.model("Objects/Test/nozzle.skin", 0)?;
    let attachment = models.model("Objects/Test/housing.cgf", 0)?;
    assert_eq!(
        skin.entry, SKIN,
        "the actual chosen skin is readable independently of CDF"
    );
    let vertices = |model: &Model| model.lods.iter().map(Lod::vertex_count).max().unwrap_or(0);
    assert_eq!(
        vertices(&skin.model),
        24,
        "the genuine source skin has known geometry"
    );
    assert_eq!(
        vertices(&attachment.model),
        24,
        "the original attachment model is valid"
    );
    assert!(
        24 * (1 + usize::from(bones)) < MAX_ASSEMBLY_VERTICES,
        "the independent existing vertex policy cannot explain the list cutoff",
    );
    Ok(())
}
