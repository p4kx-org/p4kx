// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    BONE_THRUSTER, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity,
    TestEntry, entity_schema,
};
use serde_json::json;

use super::super::{TestResult, source_uses};
use super::{Observation, entries_with_definition, observed, observed_entries, text_bytes};

const HOUSING: &str = "Objects/Test/housing.cgf";
const PAINT: &str = "Objects/Test/paint.mtl";

fn attachment(name: &str, kind: &str, binding: &str, material: &str) -> TestElement {
    TestElement::new("Attachment")
        .attribute("AName", name)
        .attribute("Type", kind)
        .attribute("Binding", binding)
        .attribute("Material", material)
}

fn with_preferred_skin(ignored: &str) -> TestElement {
    TestElement::new("CharacterDefinition")
        .child(TestElement::new("Model").attribute("File", ignored))
        .child(
            TestElement::new("AttachmentList")
                .child(attachment("Invalid", "CA_SKIN", "", ignored))
                .child(attachment("First", "CA_FACE", ignored, ignored))
                .child(attachment(
                    "Selected",
                    "ca_skin",
                    "Objects/Test/nozzle.skin",
                    PAINT,
                ))
                .child(attachment("LaterSkin", "CA_SKIN", ignored, ignored)),
        )
}

fn with_entity_override(definition: Vec<u8>) -> Vec<TestEntry> {
    let entity = TestEntity::new(0x1, BONE_THRUSTER, "entities/test_thruster.xml")
        .geometry("Objects/Test/thruster.cdf", Some(PAINT));
    let database = entity_schema(DataForgeBuilder::new()).record(entity.record());
    let mut entries = entries_with_definition(definition);
    entries.retain(|entry| !entry.path().ends_with("Game2.dcb"));
    entries.push(TestEntry::zstd("Data\\Game2.dcb", database.build()));
    entries
}

#[test]
fn preferred_skin_selectors_exclude_losing_bindings_and_overridden_material() -> TestResult {
    let dir = tempfile::tempdir()?;
    let name = "preferred_skin_selectors_exclude_losing_bindings_and_overridden_material";
    let original = text_bytes(&with_preferred_skin("Objects/Test/unused-a"));
    let altered = text_bytes(&with_preferred_skin("Objects/Test/unused-b"));
    let Some(baseline) = observed(dir.path(), original.clone(), name)? else {
        return Ok(());
    };
    let Some(changed_losers) = observed(dir.path(), altered.clone(), name)? else {
        return Ok(());
    };
    let Some(overridden) =
        observed_entries(dir.path(), with_entity_override(original.clone()), name)?
    else {
        return Ok(());
    };
    let selected = ["Binding", "Material", "Type"];
    assert_text_sites(&original, &baseline, &selected)?;
    assert_text_sites(&altered, &changed_losers, &selected)?;
    assert_text_sites(&original, &overridden, &["Binding", "Type"])?;
    assert_eq!(
        baseline.image, changed_losers.image,
        "losing attributes supply no pixels"
    );
    assert_eq!(
        baseline.image, overridden.image,
        "the same library wins by another route"
    );
    for run in [baseline, changed_losers, overridden] {
        assert!(
            !source_uses(&run.rows, &run.report, "nozzle.skinm").is_empty(),
            "the preferred skin supplies the mesh"
        );
        assert!(
            !source_uses(&run.rows, &run.report, "paint.mtl").is_empty(),
            "the physical library supplies surfaces in either route"
        );
    }
    Ok(())
}

#[test]
fn first_attachment_and_model_file_fallback_have_distinct_exact_selectors() -> TestResult {
    let dir = tempfile::tempdir()?;
    let name = "first_attachment_and_model_file_fallback_have_distinct_exact_selectors";
    let attachment = text_bytes(
        &TestElement::new("CharacterDefinition")
            .child(TestElement::new("Model").attribute("File", "unused.chr"))
            .child(
                TestElement::new("AttachmentList")
                    .child(attachment("Invalid", "CA_SKIN", "", "unused.mtl"))
                    .child(attachment("Selected", "CA_FACE", HOUSING, PAINT))
                    .child(attachment("Later", "CA_FACE", "unused.cgf", "unused.mtl")),
            ),
    );
    let fallback = TestElement::new("CharacterDefinition")
        .child(
            TestElement::new("Model")
                .attribute("Ignored", "not a file")
                .attribute("File", HOUSING),
        )
        .child(TestElement::new("Model").attribute("File", "unused.chr"))
        .child(TestElement::new("AttachmentList").child(attachment_without_binding()))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let Some(first) = observed(dir.path(), attachment.clone(), name)? else {
        return Ok(());
    };
    let Some(file) = observed(dir.path(), fallback, name)? else {
        return Ok(());
    };
    assert_text_sites(&attachment, &first, &["Binding", "Material"])?;
    let sites = file.sites("cdfModelCryXmlAttribute");
    assert_eq!(sites.len(), 1, "only the first Model.File is used");
    let (indices, role) = sites
        .get("File")
        .ok_or("the physical Model.File selector")?;
    assert_eq!(
        **indices,
        json!(["1", "1"]),
        "original node and global attribute ordinal"
    );
    assert_eq!(**role, "selector", "Model.File chooses the rendered model");
    assert_eq!(
        first.image, file.image,
        "both winning routes select the same mesh and library"
    );
    Ok(())
}

#[test]
fn the_suppressed_main_model_uses_its_authored_material_decision() -> TestResult {
    let dir = tempfile::tempdir()?;
    let name = "the_suppressed_main_model_uses_its_authored_material_decision";
    let housing = attachment("Housing", "CA_BONE", HOUSING, PAINT).attribute("BoneName", "missing");
    let main = attachment(
        "Selected",
        "CA_SKIN",
        "Objects/Test/nozzle.skin",
        "Objects/Test/hidden.mtl",
    );
    let definition = text_bytes(
        &TestElement::new("CharacterDefinition").child(
            TestElement::new("AttachmentList")
                .child(main)
                .child(housing),
        ),
    );
    let Some(run) = observed(dir.path(), definition.clone(), name)? else {
        return Ok(());
    };
    assert_text_sites(&definition, &run, &["Binding", "Material", "Type"])?;
    let hidden = source_uses(&run.rows, &run.report, "hidden.mtl");
    assert!(
        hidden.iter().any(|site| {
            site.pointer("/location/field")
                .and_then(serde_json::Value::as_str)
                == Some("hiddenMaterial")
        }),
        "the actual library suppresses the main model"
    );
    assert!(
        source_uses(&run.rows, &run.report, "nozzle.skinm").is_empty(),
        "the suppressed mesh supplies no geometry payload"
    );
    assert!(
        !source_uses(&run.rows, &run.report, "housing.cgfm").is_empty(),
        "the attachment still supplies visible geometry"
    );
    Ok(())
}

fn attachment_without_binding() -> TestElement {
    TestElement::new("Attachment")
        .attribute("Type", "CA_SKIN")
        .attribute("Material", "unused.mtl")
}

fn assert_text_sites(source: &[u8], run: &Observation, expected: &[&str]) -> TestResult {
    let sites = run.sites("cdfModelTextBytes");
    assert_eq!(
        sites.keys().copied().collect::<Vec<_>>(),
        expected,
        "exact fields on the winning main route"
    );
    let text = str::from_utf8(source)?;
    let selected_start = text
        .find(r#"AName="Selected""#)
        .ok_or("the selected attachment marker")?;
    let selected = text
        .get(selected_start..)
        .ok_or("selected attachment bytes")?;
    for (field, (indices, role)) in sites {
        let needle = format!("{field}=\"");
        let name_start = selected.find(&needle).ok_or("original attribute name")?;
        let value_start = name_start + needle.len();
        let value = selected.get(value_start..).ok_or("original value bytes")?;
        let start = selected_start + name_start;
        let end = selected_start + value_start + value.find('"').ok_or("closing quote")? + 1;
        assert_eq!(
            *indices,
            json!([start.to_string(), end.to_string()]),
            "exact original bytes of {field}"
        );
        assert_eq!(*role, "selector", "{field} chooses a rendered input");
    }
    Ok(())
}
