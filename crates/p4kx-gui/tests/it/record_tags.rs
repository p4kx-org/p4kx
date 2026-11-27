// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tag paths in the Record export and the Record preview, over a synthetic DataForge whose Gear
//! Record refers to the tag Weapon.Gun.

use crate::exporting;

use exporting::{TestResult, ended, files_below, opened, select_node, wait_until};
use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole};
use p4kx_gui::{
    Browser, LineValue, Model, RecordExportStatus, RecordFieldProvenance, RecordForm,
    RecordPreviewStatus, RecordView, TargetKind, form_annotates_tags,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

const DATABASE: u128 = 0x0d;
const WEAPON: u128 = 0xa0;
const GUN: u128 = 0xa1;
const GEAR: u128 = 0xc0;
const TAG_PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";
pub(crate) const GEAR_NODE: &str = "libs/gear/kit";

fn tag(guid: u128, name: &str, children: &[u128]) -> TestRecord {
    let children = children.iter().map(|g| TestValue::Reference(*g)).collect();
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), TAG_PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(u32::MAX))
        .value("children", TestValue::Array(children))
}

pub(crate) fn forge() -> DataForgeBuilder {
    let tag_reference = || TestType::ReferenceTo("Tag".to_owned());
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", tag_reference()),
        )
        .structure(TestStruct::new("TagDatabase").array("tags", tag_reference()))
        .structure(TestStruct::new("Gear").field("mount", TestType::Reference))
        .record(
            TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", TAG_PATH)
                .value("tags", TestValue::Array(vec![TestValue::Reference(WEAPON)])),
        )
        .record(tag(WEAPON, "Weapon", &[GUN]))
        .record(tag(GUN, "Gun", &[]))
        .record(
            TestRecord::new(GEAR, "Gear", "Gear.Kit", "libs/gear/kit.xml")
                .value("mount", TestValue::Reference(GUN)),
        )
}

fn exported(form: RecordForm, tags: bool) -> Result<String, Box<dyn std::error::Error>> {
    let (_dir, mut model, out) = opened(&forge())?;
    select_node(&mut model, GEAR_NODE)?;
    model.begin_record_export();
    let setup = model.record_export_setup_mut().ok_or("no setup")?;
    setup.set_form(form);
    setup.set_tags(tags);
    setup.set_kind(TargetKind::ExistingFolder);
    setup.set_folder(out.clone());
    assert!(model.submit_record_export(|| {}), "started");
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };
    assert_eq!(summary.written(), 1, "the Gear Record");
    let files = files_below(&out)?;
    let [file, companion] = files.as_slice() else {
        return Err(format!("expected one payload and one companion: {files:?}").into());
    };
    assert_eq!(
        summary.origin_receipts(),
        [out.join(companion)],
        "the companion is exposed"
    );
    if tags && form_annotates_tags(form) {
        verify_exported_tag_proof(&out.join(companion), form)?;
    }
    Ok(std::fs::read_to_string(out.join(file))?)
}

fn verify_exported_tag_proof(companion: &std::path::Path, form: RecordForm) -> TestResult {
    let proof = std::fs::read_to_string(companion)?;
    let rows: Vec<serde_json::Value> = proof
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let tag = rows
        .iter()
        .find(|row| {
            row.pointer("/address/path")
                .and_then(serde_json::Value::as_str)
                == Some("/mount/tag")
        })
        .ok_or("exported tag field")?;
    let line = tag
        .pointer("/address/line")
        .and_then(serde_json::Value::as_str);
    assert_eq!(
        line,
        (form == RecordForm::Ndjson).then_some("0"),
        "the actual tag field address for {form}"
    );
    let sources = tag
        .get("sourceUsesRefs")
        .and_then(serde_json::Value::as_array)
        .ok_or("tag sources")?;
    for (field, owner) in [("children", "1"), ("tags", "0")] {
        assert!(
            rows.iter()
                .filter_map(|row| row.get("sourceUse"))
                .any(|source| {
                    source.get("id").is_some_and(|id| sources.contains(id))
                        && source.get("role").and_then(serde_json::Value::as_str)
                            == Some("selector")
                        && source
                            .pointer("/location/namespace")
                            .and_then(serde_json::Value::as_str)
                            == Some("dataforgeTagRelations")
                        && source
                            .pointer("/location/field")
                            .and_then(serde_json::Value::as_str)
                            == Some(field)
                        && source
                            .pointer("/location/indices/0")
                            .and_then(serde_json::Value::as_str)
                            == Some(owner)
                }),
            "{form}: original {field} selector is bound to the emitted tag"
        );
    }
    Ok(())
}

#[test]
fn the_tags_option_writes_the_tag_path_of_a_tag_reference() -> TestResult {
    let with = exported(RecordForm::Json, true)?;
    let without = exported(RecordForm::Json, false)?;

    let tag = |text: &str| -> Result<Option<String>, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        Ok(value["mount"]["tag"].as_str().map(str::to_owned))
    };
    assert_eq!(tag(&with)?.as_deref(), Some("Weapon.Gun"), "with tags");
    assert_eq!(tag(&without)?, None, "without tags the GUID alone");
    Ok(())
}

#[test]
fn the_tags_option_applies_to_ndjson_and_not_to_the_flat_forms() -> TestResult {
    let ndjson = exported(RecordForm::Ndjson, true)?;
    let flat = exported(RecordForm::FlatNdjson, true)?;

    assert!(
        ndjson.contains("\"tag\":\"Weapon.Gun\""),
        "NDJSON: {ndjson}"
    );
    assert!(!flat.contains("Weapon.Gun"), "flat: {flat}");
    assert!(form_annotates_tags(RecordForm::Xml), "XML");
    assert!(form_annotates_tags(RecordForm::Json), "JSON");
    assert!(form_annotates_tags(RecordForm::Ndjson), "NDJSON");
    assert!(!form_annotates_tags(RecordForm::FlatCsv), "flat CSV");
    assert!(!form_annotates_tags(RecordForm::FlatNdjson), "flat NDJSON");
    Ok(())
}

fn previewed(model: &mut Model) -> Result<&RecordView, Box<dyn std::error::Error>> {
    model
        .browser_mut()
        .ok_or("no Archive")?
        .load_record_preview(|| {});
    wait_until(model, |model| {
        model
            .browser()
            .and_then(Browser::record_preview)
            .is_some_and(|preview| !matches!(preview.status(), RecordPreviewStatus::Loading))
    })?;
    match model
        .browser()
        .and_then(Browser::record_preview)
        .ok_or("no Record preview")?
        .status()
    {
        RecordPreviewStatus::Ready(view) => Ok(view),
        other => Err(format!("not read: {other:?}").into()),
    }
}

#[test]
fn the_preview_shows_the_tag_path_of_a_tag_reference() -> TestResult {
    let (_dir, mut model, _) = opened(&forge())?;
    select_node(&mut model, GEAR_NODE)?;

    let view = previewed(&mut model)?;

    let link = view
        .lines()
        .iter()
        .find_map(|line| match line.value() {
            LineValue::Reference(link) if line.name() == "mount" => Some(link),
            _ => None,
        })
        .ok_or("no mount line")?;
    assert_eq!(link.tag(), Some("Weapon.Gun"));
    assert_eq!(link.type_name(), "Tag");
    assert_tag_route(view)
}

fn assert_tag_route(view: &RecordView) -> TestResult {
    let proof = view
        .provenance()
        .ok_or("Record source evidence")?
        .map_err(ToString::to_string)?;
    let mounted = view
        .lines()
        .iter()
        .zip(proof.lines())
        .find_map(|(line, origins)| (line.name() == "mount").then_some(origins))
        .ok_or("mount line origin")?;
    assert_reference_members(proof.inventory(), mounted)?;
    let path = mounted
        .annotation
        .as_ref()
        .ok_or("actual displayed tag path")?;
    assert_eq!(
        path.contains_encrypted_source(),
        Some(false),
        "the bound Tag Records are plain"
    );
    assert!(
        path.source_set_complete(),
        "both Tag names and selected original edges are available"
    );
    for (expected_field, expected_indices) in [("tags", [0, 0]), ("children", [1, 0])] {
        assert!(proof.inventory().uses().iter().any(|used| {
            path.source_uses_refs().contains(used.id())
                && used.role() == SourceUseRole::Selector
                && matches!(used.location(), SourceLocation::Subresource { namespace, field, indices }
                    if namespace == "dataforgeTagRelations" && field == expected_field && indices == &expected_indices)
        }), "actual Tag path selector {expected_field}");
    }
    for expected in [1, 2] {
        assert!(
            proof.inventory().uses().iter().any(|used| {
                path.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::DataForge { record_index, field }
                    if *record_index == expected && field == "tagName")
            }),
            "original name of Tag Record {expected}"
        );
    }
    Ok(())
}

fn assert_reference_members(
    inventory: &SourceInventory,
    mounted: &RecordFieldProvenance,
) -> TestResult {
    let proof = mounted
        .reference
        .as_ref()
        .ok_or("reference member origins")?;
    assert!(
        inventory.uses().iter().any(|used| {
            proof.guid.source_uses_refs().contains(used.id())
                && used.role() == SourceUseRole::Payload
                && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
                if namespace == "dataforgeDecodedBytes" && field == "slot")
        }),
        "the raw caller GUID supplies the reference identity"
    );
    assert!(
        inventory.uses().iter().any(|used| {
            proof.path.source_uses_refs().contains(used.id())
                && matches!(used.location(), SourceLocation::DataForge { record_index: 2, field }
                if field == "fileName")
        }),
        "the chosen physical target supplies its displayed filename"
    );
    Ok(())
}
