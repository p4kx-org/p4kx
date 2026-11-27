// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

const STREAM: &str = "model.jsonl";
const TEXT: &str = r#"<CharacterDefinition>
  <Model File="Data/rig.chr"/>
  <AttachmentList>
    <Attachment AName="collider" Type="CA_BONE" Binding="Part.CGF" Material="part.mtl"
      BoneName="mount" RelPosition="1,-2.5,3" RelRotation="1,0,0,0"/>
    <Future/>
    <Attachment Type="CA_BONE" Binding="duplicate.cga" BoneName="duplicate"/>
    <Attachment Type="CA_BONE" Binding="missing.cga" RelRotation="0,0,0,0"/>
    <Attachment Type="CA_FACE" Binding="" RelPosition="invalid"/>
    <Attachment AName="no-file" Type="CA_PROX"/>
  </AttachmentList>
  <AttachmentList><Attachment Binding="../unsafe.cgf"/></AttachmentList>
</CharacterDefinition>"#;

#[test]
fn character_links_keep_source_ordinals_placement_and_unselected_targets() -> TestResult {
    let temp = tempfile::tempdir()?;
    let bytes = text_archive(temp.path())?;
    let archive = Archive::open_with(bytes, OpenOptions::new().key(Some(test_key())))?;
    let output = temp.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options("*.cdf")?)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    let base = output.join(&index.artifact_base);
    assert_eq!(index.source_entries.written, 1, "target bytes stay unread");
    assert_eq!(rows.len(), 8, "document, skeleton and six declarations");
    let document = find(&rows, "model-character-definition", &[])?;
    assert_payload(document, "encoding", &json!("text-xml"));
    assert_payload(document, "disposition", &json!("character-definition"));
    let skeleton = find(&rows, "model-character-skeleton", &[0])?;
    assert_reference(skeleton, "file", "Data/rig.chr", "not-selected");
    let attachment = find(&rows, "model-character-attachment", &[1, 0])?;
    assert_attachment(attachment);
    check_text_origins(&rows, attachment, &base)?;
    assert_different_reference_states(&rows)?;
    for row in &rows {
        let raw = row.source.raw.as_ref().ok_or("source artifact")?;
        assert_eq!(
            fs::read(base.join(&raw.path))?,
            TEXT.as_bytes(),
            "every row refers to its original authenticated CDF input"
        );
        assert_eq!(
            row.source.raw, document.source.raw,
            "raw data is retained once for the source"
        );
    }
    validate_schema(&rows)?;
    Ok(())
}

fn text_archive(dir: &Path) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let file = dir.join("Data.p4k");
    let unread = |name| TestEntry::stored(name, b"unread").with_crc32c(1);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("synthetic.cdf", TEXT.as_bytes()).encrypted())
        .entry(unread("Data/rig.chr"))
        .entry(unread("Data/Part.CGF"))
        .entry(unread("Data/Duplicate.cga"))
        .entry(unread("data/duplicate.CGA"))
        .write_to(&file)?;
    Ok(file)
}

fn assert_attachment(row: &MediaRecord) {
    assert_reference(row, "binding", "Part.CGF", "not-selected");
    for (field, value) in [
        ("name", json!("collider")),
        ("material", json!("part.mtl")),
        ("bone_name", json!("mount")),
        (
            "relative_position",
            json!({"authored": "1,-2.5,3", "value": [1.0, -2.5, 3.0]}),
        ),
        (
            "relative_rotation_wxyz",
            json!({"authored": "1,0,0,0", "value": [1.0, 0.0, 0.0, 0.0]}),
        ),
    ] {
        assert_payload(row, field, &value);
    }
}

fn assert_different_reference_states(rows: &[MediaRecord]) -> TestResult {
    let get = |list, child| find(rows, "model-character-attachment", &[list, child]);
    let duplicate = get(1, 2)?;
    assert_reference(duplicate, "binding", "duplicate.cga", "ambiguous");
    assert_payload(duplicate, "binding/observed_candidates", &json!(2));
    assert_payload(
        duplicate,
        "relative_position",
        &json!({"authored": null, "value": [0.0, 0.0, 0.0]}),
    );
    let missing = get(1, 3)?;
    assert_reference(missing, "binding", "missing.cga", "missing");
    assert_payload(
        missing,
        "relative_rotation_wxyz",
        &json!({"authored": "0,0,0,0", "value": null}),
    );
    let empty = get(1, 4)?;
    assert_reference(empty, "binding", "", "unavailable");
    assert_payload(
        empty,
        "relative_position",
        &json!({"authored": "invalid", "value": null}),
    );
    let absent = get(1, 5)?;
    assert_payload(absent, "binding", &Value::Null);
    let invalid = get(2, 0)?;
    assert_reference(invalid, "binding", "../unsafe.cgf", "unavailable");
    Ok(())
}

fn check_text_origins(rows: &[MediaRecord], attachment: &MediaRecord, base: &Path) -> TestResult {
    let index = rows
        .iter()
        .position(|row| std::ptr::eq(row, attachment))
        .ok_or("source occurrence")?;
    for (attribute, field, spelling) in [
        ("Binding", "binding/original/value", r#"Binding="Part.CGF""#),
        ("BoneName", "bone_name", r#"BoneName="mount""#),
        (
            "RelPosition",
            "relative_position/authored",
            r#"RelPosition="1,-2.5,3""#,
        ),
    ] {
        let start = TEXT.find(spelling).ok_or("literal attribute")?;
        let uses = field_uses(base, index, &format!("/data/payload/{field}"), 0, true)?;
        assert!(uses.iter().any(|origin| origin.get("location") == Some(&json!({
            "type": "subresource", "namespace": "cdf-xml-attribute",
            "indices": [start.to_string(), (start + spelling.len()).to_string()], "field": attribute,
        }))), "exact {attribute} original bytes");
    }
    Ok(())
}

#[test]
fn character_cryxml_keeps_original_attribute_table_and_other_roots() -> TestResult {
    let temp = tempfile::tempdir()?;
    let bytes = TestElement::new("CharacterDefinition")
        .child(
            TestElement::new("AttachmentList").child(
                TestElement::new("Attachment")
                    .attribute("Type", "CA_BONE")
                    .attribute("BoneName", "mount")
                    .attribute("Binding", "geometry.cgf"),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let archive = support::archive_with(
        temp.path(),
        [
            TestEntry::stored("synthetic.CDF", bytes),
            TestEntry::stored("other.cdf", b"<Other><Model File='ignored.chr'/></Other>"),
        ],
    )?;
    let output = temp.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options("*")?)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    assert_eq!(
        rows.len(),
        3,
        "other XML root emits an opaque definition row only"
    );
    let binary = find(&rows, "model-character-attachment", &[0, 0])?;
    assert_reference(binary, "binding", "geometry.cgf", "missing");
    let ordinal = rows
        .iter()
        .position(|row| std::ptr::eq(row, binary))
        .ok_or("binary occurrence")?;
    let uses = field_uses(
        &output.join(&index.artifact_base),
        ordinal,
        "/data/payload/bone_name",
        0,
        false,
    )?;
    assert!(uses.iter().any(|origin| origin.get("location") == Some(&json!({
        "type": "subresource", "namespace": "cdf-cryxml-attribute", "indices": ["2", "1"], "field": "BoneName",
    }))), "CryXml node and attribute table ordinals");
    let other = rows
        .iter()
        .find(|row| row.data.pointer("/payload/root_element") == Some(&json!("Other")))
        .ok_or("other root")?;
    assert_payload(other, "disposition", &json!("other-root"));
    assert_eq!(
        other.status,
        Status::Complete,
        "valid alternate roots retain opaque source data"
    );
    validate_schema(&rows)?;
    Ok(())
}

#[test]
fn malformed_definition_is_retained_only_in_diagnostic_generation() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive =
        support::archive_with(temp.path(), [TestEntry::stored("invalid.cdf", b"<Broken")])?;
    let output = temp.path().join("out");
    let outcome = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            diagnostic: true,
            ..options("*")?
        },
    )?;
    let ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("malformed CDF needs a diagnosable incomplete generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let records = fs::read_to_string(base.join(STREAM))?;
    let row: MediaRecord = serde_json::from_str(records.lines().next().ok_or("diagnostic row")?)?;
    assert_eq!(
        row.status,
        Status::Partial,
        "malformed CDF has an explicit partial status"
    );
    assert_payload(&row, "disposition", &json!("malformed"));
    let raw = row
        .source
        .raw
        .as_ref()
        .ok_or("failed parser retains raw bytes")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        b"<Broken",
        "diagnostic output preserves source bytes"
    );
    validate_schema(&[row])?;
    Ok(())
}

fn options(include: &str) -> Result<ExportOptions, Box<dyn std::error::Error>> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Model],
        include: Some(Filter::parse(include)?),
        ..ExportOptions::default()
    })
}

fn find<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> Result<&'a MediaRecord, Box<dyn std::error::Error>> {
    rows.iter()
        .find(|row| row.subresource.namespace == namespace && row.subresource.indices == indices)
        .ok_or_else(|| format!("absent {namespace} occurrence {indices:?}").into())
}

fn assert_reference(row: &MediaRecord, field: &str, original: &str, status: &str) {
    assert_payload(row, &format!("{field}/original/value"), &json!(original));
    assert_payload(row, &format!("{field}/resolution/status"), &json!(status));
}

fn assert_payload(row: &MediaRecord, field: &str, value: &Value) {
    assert_eq!(
        row.data.pointer(&format!("/payload/{field}")),
        Some(value),
        "original or derived CDF field {field}",
    );
}
fn validate_schema(rows: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        let value = serde_json::to_value(row)?;
        assert!(schema.is_valid(&value), "closed CDF record schema");
        let mut unknown = value;
        let payload = unknown
            .pointer_mut("/data/payload")
            .and_then(Value::as_object_mut)
            .ok_or("closed CDF payload object")?;
        payload.insert("unknown_field".to_owned(), Value::Bool(true));
        assert!(!schema.is_valid(&unknown), "closed CDF payload");
    }
    Ok(())
}
