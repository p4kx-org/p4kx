// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use p4kx_testkit::{
    BONE_THRUSTER, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, TestMtl,
    TestMtlMaterial, bone_thruster_entries, test_key,
};
use serde_json::{Value, json};

use super::super::hologram_for;
use super::{TestResult, receipt, skip, source_uses, write_archive};

#[path = "ui_hologram_cdf_main.rs"]
mod main_model;

const DEFINITION: &str = "Data\\Objects\\Test\\thruster.cdf";
const FIELDS: [(&str, &str, &str); 6] = [
    ("Type", "Type", "selector"),
    ("BoneName", "BoneName", "selector"),
    ("Binding", "Binding", "selector"),
    ("Material", "Material", "selector"),
    ("RelPosition", "relposition", "conversion"),
    ("RelRotation", "RelRotation", "conversion"),
];

fn declaration(bone: &str, position: &str, material: &str) -> TestElement {
    TestElement::new("Attachment")
        .attribute("Type", "CA_BONE")
        .attribute("AName", "Gehäuse")
        .attribute("BoneName", bone)
        .attribute("bonename", "losingAttribute")
        .attribute("Binding", "Objects/Test/housing.cgf")
        .attribute("Material", material)
        .attribute("relposition", position)
        .attribute("RelRotation", "1,0,0,0")
}

fn document(bone: &str, position: &str, material: &str) -> TestElement {
    let list = TestElement::new("AttachmentList")
        .child(
            TestElement::new("Attachment")
                .attribute("Type", "CA_SKIN")
                .attribute("AName", "Nozzle")
                .attribute("Binding", "Objects/Test/nozzle.skin"),
        )
        .child(TestElement::new("Other"))
        .child(
            TestElement::new("Attachment")
                .attribute("Type", "CA_BONE")
                .attribute("BoneName", "unbound"),
        )
        .child(
            TestElement::new("Attachment")
                .attribute("Type", "CA_BONE")
                .attribute("BoneName", "tip")
                .attribute("Binding", "Objects/Test/unavailable.cgf"),
        )
        .child(
            TestElement::new("Attachment")
                .attribute("Type", "CA_BONE")
                .attribute("BoneName", "tip")
                .attribute("Binding", "Objects/Test/housing.cgf")
                .attribute("RelRotation", "0,0,0,0"),
        )
        .child(declaration(bone, position, material));
    TestElement::new("CharacterDefinition").child(list)
}

fn text_bytes(document: &TestElement) -> Vec<u8> {
    format!("\u{feff}{}", document.to_xml()).into_bytes()
}

struct Observation {
    image: Vec<u8>,
    rows: Vec<Value>,
    report: Value,
}

fn entries_with_definition(definition: Vec<u8>) -> Vec<TestEntry> {
    let mut entries = bone_thruster_entries("");
    entries.retain(|entry| entry.path() != DEFINITION);
    entries.push(TestEntry::stored(DEFINITION, definition).encrypted());
    let hidden = || TestMtlMaterial::new("hidden").shader("NoDraw");
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\hidden.mtl",
        TestMtl::multi(&[hidden(), hidden(), hidden()]).cryxml(),
    ));
    entries
}

fn observed(
    dir: &Path,
    definition: Vec<u8>,
    test: &str,
) -> Result<Option<Observation>, Box<dyn std::error::Error>> {
    observed_entries(dir, entries_with_definition(definition), test)
}

fn observed_entries(
    dir: &Path,
    entries: Vec<TestEntry>,
    test: &str,
) -> Result<Option<Observation>, Box<dyn std::error::Error>> {
    let input = tempfile::tempdir_in(dir)?;
    let archive = write_archive(input.path(), entries)?;
    let output = input.path().join("ship.png");
    let run = hologram_for(
        &archive,
        &output,
        BONE_THRUSTER,
        &["--size", "96", "--key", &test_key().to_hex()],
        true,
    )?;
    if skip(test, &run) {
        return Ok(None);
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(
        report.pointer("/artifactOrigin/containsEncryptedSource"),
        Some(&json!(true)),
        "the encrypted CDF contributes to the artifact"
    );
    Ok(Some(Observation {
        image: fs::read(&output)?,
        rows: receipt(&output, &report)?,
        report,
    }))
}

impl Observation {
    fn sites(&self, namespace: &str) -> BTreeMap<&str, (&Value, &Value)> {
        let mut observed = BTreeMap::new();
        let sites = source_uses(&self.rows, &self.report, "thruster.cdf")
            .into_iter()
            .filter_map(|site| {
                let location = site.get("location")?;
                if location.get("namespace")?.as_str()? != namespace {
                    return None;
                }
                Some((
                    location.get("field")?.as_str()?,
                    (location.get("indices")?, site.get("role")?),
                ))
            });
        for (field, site) in sites {
            assert!(
                observed.insert(field, site).is_none(),
                "only one winning {field}"
            );
        }
        observed
    }

    fn pose_sites(&self) -> Vec<&Value> {
        source_uses(&self.rows, &self.report, "nozzle.skin")
            .into_iter()
            .filter(|site| {
                site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("modelBonePlacementBytes")
            })
            .collect()
    }
}

#[test]
fn the_same_selected_cdf_attributes_use_their_original_format_coordinates() -> TestResult {
    let dir = tempfile::tempdir()?;
    let test = "the_same_selected_cdf_attributes_use_their_original_format_coordinates";
    let input = document("tip", "1,0,0", "Objects/Test/paint.mtl");
    let xml = text_bytes(&input);
    let Some(text_run) = observed(dir.path(), xml.clone(), test)? else {
        return Ok(());
    };
    let cryxml = input.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let Some(binary_run) = observed(dir.path(), cryxml, test)? else {
        return Ok(());
    };
    assert_eq!(
        text_run.image, binary_run.image,
        "the same admitted model and pose"
    );
    assert_text_sites(&xml, &text_run)?;
    assert_binary_sites(&binary_run)?;
    assert_eq!(
        text_run.pose_sites().len(),
        5,
        "the selected bone and its ancestor"
    );
    Ok(())
}

fn assert_text_sites(text: &[u8], run: &Observation) -> TestResult {
    let source = str::from_utf8(text)?;
    let sites = run.sites("cdfAttachmentTextBytes");
    assert_eq!(
        sites.len(),
        FIELDS.len(),
        "no skipped, losing or display fields"
    );
    let selected = source
        .rfind("<Attachment")
        .ok_or("the selected attachment")?;
    let selected = source.get(selected..).ok_or("the selected tag")?;
    for (field, spelling, role) in FIELDS {
        let needle = format!("{spelling}=\"");
        let local = selected.find(&needle).ok_or("the winning attribute")?;
        let source_base = source.len() - selected.len();
        let start = source_base + local;
        let value_start = local + needle.len();
        let value = selected.get(value_start..).ok_or("the selected value")?;
        let end = source_base + value_start + value.find('"').ok_or("the closing quote")? + 1;
        let (indices, actual_role) = sites.get(field).ok_or("the reported text site")?;
        assert_eq!(
            **indices,
            json!([start.to_string(), end.to_string()]),
            "original text bytes for {field}"
        );
        assert_eq!(**actual_role, role, "role for {field}");
    }
    Ok(())
}

fn assert_binary_sites(run: &Observation) -> TestResult {
    let sites = run.sites("cdfAttachmentCryXmlAttribute");
    assert_eq!(
        sites.len(),
        FIELDS.len(),
        "no skipped, losing or display fields"
    );
    let selected_node = "7";
    let preceding_attributes = 3 + 2 + 3 + 4;
    for ((field, _, role), physical_index) in FIELDS.into_iter().zip([0, 2, 4, 5, 6, 7]) {
        let (indices, actual_role) = sites.get(field).ok_or("the reported CryXml site")?;
        assert_eq!(
            **indices,
            json!([
                selected_node,
                (preceding_attributes + physical_index).to_string()
            ]),
            "original global attribute index for {field}"
        );
        assert_eq!(**actual_role, role, "role for {field}");
    }
    Ok(())
}

#[test]
fn missing_bone_and_material_do_not_claim_pose_or_material_payloads() -> TestResult {
    let dir = tempfile::tempdir()?;
    let test = "missing_bone_and_material_do_not_claim_pose_or_material_payloads";
    let input = |position| {
        text_bytes(&document(
            "missingBone",
            position,
            "Objects/Test/missing.mtl",
        ))
    };
    let Some(baseline) = observed(dir.path(), input("1,0,0"), test)? else {
        return Ok(());
    };
    let Some(moved) = observed(dir.path(), input("4,0,0"), test)? else {
        return Ok(());
    };
    assert_ne!(
        baseline.image, moved.image,
        "the authored offset still places the model"
    );
    for run in [baseline, moved] {
        let sites = run.sites("cdfAttachmentTextBytes");
        assert_eq!(sites.len(), 5, "only the placement route is observed");
        assert!(
            sites.contains_key("BoneName"),
            "the missing bone chose the fallback"
        );
        assert!(
            sites.contains_key("RelPosition"),
            "the actual offset remains observed"
        );
        assert!(
            !sites.contains_key("Material"),
            "no chosen library uses this override"
        );
        assert!(
            run.pose_sites().is_empty(),
            "no skeleton bone supplied the transform"
        );
    }
    Ok(())
}

#[test]
fn material_suppression_observes_selection_without_the_unused_pose() -> TestResult {
    let dir = tempfile::tempdir()?;
    let test = "material_suppression_observes_selection_without_the_unused_pose";
    let input = |position| text_bytes(&document("tip", position, "Objects/Test/hidden.mtl"));
    let Some(baseline) = observed(dir.path(), input("1,0,0"), test)? else {
        return Ok(());
    };
    let Some(moved) = observed(dir.path(), input("4,0,0"), test)? else {
        return Ok(());
    };
    assert_eq!(
        baseline.image, moved.image,
        "the suppressed model uses no placement"
    );
    for run in [baseline, moved] {
        let sites = run.sites("cdfAttachmentTextBytes");
        assert_eq!(
            sites.keys().copied().collect::<Vec<_>>(),
            ["Binding", "Material", "Type"],
            "the source route selects the model and its suppression"
        );
        assert!(
            run.pose_sites().is_empty(),
            "the suppressed model consumes no pose"
        );
        let material = source_uses(&run.rows, &run.report, "hidden.mtl");
        assert!(
            material.iter().any(
                |site| site.pointer("/location/field").and_then(Value::as_str)
                    == Some("hiddenMaterial")
            ),
            "the observed library hides the attachment"
        );
    }
    Ok(())
}
