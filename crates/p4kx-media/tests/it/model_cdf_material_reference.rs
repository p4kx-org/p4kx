// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

const MATERIAL_REFERENCE: &str = "/payload/material_reference";
const TEXT: &str = r#"<CharacterDefinition>
  <AttachmentList>
    <Attachment Type="CA_SKIN" Binding="Models/Hull.skin" Material="surface.mtl"/>
    <Attachment Type="CA_BONE" Binding="Models/Hull.skin" Material="root_only.mtl"/>
    <Attachment Type="CA_BONE" Binding="Models/Missing.cgf" Material="surface.mtl"/>
    <Attachment Type="CA_BONE" Binding="Models/Duplicate.cga" Material="surface.mtl"/>
    <Attachment Type="CA_BONE" Binding="Models/wrong.xml" Material="surface.mtl"/>
    <Attachment Type="CA_FACE" Material="surface.mtl"/>
    <Attachment Type="CA_BONE" Binding="Models/Missing.cgf" Material="\Shared\Paint.MTL"/>
    <Attachment Type="CA_BONE" Binding="Models/Hull.skin" Material="./invalid.mtl"/>
    <Attachment Type="CA_BONE" Binding="Models/Hull.skin" Material=""/>
    <Attachment Type="CA_BONE" Binding="Models/Hull.skin"/>
  </AttachmentList>
</CharacterDefinition>"#;

#[test]
fn cdf_material_references_use_bound_model_identity_even_when_both_targets_are_filtered()
-> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = archive(temp.path())?;
    let output = temp.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            kinds: vec![MediaKind::Model],
            include: Some(Filter::parse("*.cdf")?),
            ..ExportOptions::default()
        },
    )?)?;
    assert_eq!(
        index.source_entries.written, 2,
        "only the two CDF Entries are read"
    );
    let rows = support::records(&output, &index, "model.jsonl")?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "exact closed CDF material reference schema"
        );
    }
    assert_target(attachment(&rows, 0)?.1, "surface.mtl", 6);
    assert_target(attachment(&rows, 1)?.1, "root_only.mtl", 9);
    assert_target(attachment(&rows, 6)?.1, r"\Shared\Paint.MTL", 10);
    assert_unavailable(&rows)?;
    let (_, no_material) = attachment(&rows, 9)?;
    assert_eq!(
        no_material.data.pointer(MATERIAL_REFERENCE),
        None,
        "absence of an authored attribute has no dependency edge"
    );
    assert_origins(&rows, &output.join(&index.artifact_base))?;
    Ok(())
}

fn archive(folder: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let cryxml = TestElement::new("CharacterDefinition")
        .child(
            TestElement::new("AttachmentList").child(
                TestElement::new("Attachment")
                    .attribute("Binding", "Models/Hull.skin")
                    .attribute("Material", "surface.mtl"),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let path = folder.join("Data.p4k");
    let mut blob = ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Cdf/model.cdf", TEXT.as_bytes()).encrypted())
        .entry(TestEntry::stored("synthetic-cryxml.cdf", cryxml));
    for path in [
        "Data/Models/Hull.skin",
        "Data/Models/Duplicate.cga",
        "data/models/duplicate.CGA",
        "Data/Models/wrong.xml",
        "Data/Models/surface.mtl",
        "Data/Cdf/surface.mtl",
        "Data/surface.mtl",
        "Data/root_only.mtl",
        "Data/Shared/Paint.mtl",
    ] {
        blob = blob.entry(TestEntry::stored(path, b"unread target").with_crc32c(1));
    }
    blob.write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn material_value(row: &MediaRecord, pointer: &str) -> Option<Value> {
    row.data
        .pointer(&format!("{MATERIAL_REFERENCE}/{pointer}"))
        .cloned()
}

fn assert_target(row: &MediaRecord, authored: &str, entry_index: usize) {
    assert_eq!(
        row.data.pointer("/payload/material"),
        Some(&json!(authored)),
        "existing authored spelling"
    );
    for (path, expected) in [
        ("original/value", json!(authored)),
        ("resolution/status", json!("not-selected")),
        ("resolution/reason", json!("filter")),
        ("resolution/target/kind", json!("material")),
        (
            "resolution/target/source/entry/entry_index",
            json!(entry_index),
        ),
    ] {
        assert_eq!(
            material_value(row, path),
            Some(expected),
            "binding-derived {path}"
        );
    }
}

fn assert_unavailable(rows: &[MediaRecord]) -> TestResult {
    for (child, binding_status, reason) in [
        (2, Some("missing"), "incomplete-inventory"),
        (3, Some("ambiguous"), "incomplete-inventory"),
        (4, Some("not-selected"), "incomplete-inventory"),
        (5, None, "incomplete-inventory"),
        (7, Some("not-selected"), "invalid-identifier"),
        (8, Some("not-selected"), "invalid-identifier"),
    ] {
        let (_, row) = attachment(rows, child)?;
        assert_eq!(
            row.data
                .pointer("/payload/binding/resolution/status")
                .and_then(Value::as_str),
            binding_status,
            "original binding classification survives"
        );
        assert_eq!(
            material_value(row, "resolution/status"),
            Some(json!("unavailable")),
            "unproved sibling scope never falls through to the root or CDF folder"
        );
        assert_eq!(
            material_value(row, "resolution/reason/type"),
            Some(json!(reason)),
            "reason distinguishes unknown scope from invalid authored material name"
        );
        assert_eq!(
            material_value(row, "observed_candidates"),
            Some(json!(0)),
            "no material candidate is invented from an unavailable scope"
        );
    }
    Ok(())
}

fn assert_origins(rows: &[MediaRecord], base: &Path) -> TestResult {
    let (ordinal, _) = attachment(rows, 0)?;
    let pointer = "/data/payload/material_reference/original/value";
    let uses = field_uses(base, ordinal, pointer, 0, true)?;
    let material = r#"Material="surface.mtl""#;
    let start = TEXT.find(material).ok_or("authored Material attribute")?;
    let literal = json!({
        "type":"subresource", "namespace":"cdf-xml-attribute", "field":"Material",
        "indices":[start.to_string(), (start + material.len()).to_string()],
    });
    assert_eq!(
        uses.first().and_then(|usage| usage.get("location")),
        Some(&literal),
        "encrypted material spelling is scoped only to its original attribute"
    );
    let selector = "/data/payload/material_reference/resolution/status";
    let sources = source_fields(base, ordinal, selector)?;
    for field in ["Material", "Binding", "reference-candidate"] {
        assert!(
            sources
                .iter()
                .any(|usage| usage.pointer("/location/field") == Some(&json!(field))),
            "resolution provenance includes {field} without payload reads"
        );
    }
    assert_binary_origin(rows, base, pointer)
}

fn assert_binary_origin(rows: &[MediaRecord], base: &Path, pointer: &str) -> TestResult {
    let (ordinal, row) = rows
        .iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == "model-character-attachment"
                && row.subresource.indices == [0, 0]
                && entry_is(row, 1)
        })
        .ok_or("binary CDF attachment")?;
    assert_target(row, "surface.mtl", 6);
    let uses = field_uses(base, ordinal, pointer, 1, false)?;
    assert!(
        uses.iter().any(|usage| usage.get("location")
            == Some(&json!({
                "type":"subresource", "namespace":"cdf-cryxml-attribute", "field":"Material",
                "indices":["2", "1"],
            }))),
        "CryXml original Material table coordinate"
    );
    Ok(())
}

fn source_fields(
    base: &Path,
    ordinal: usize,
    field: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let ordinal = ordinal.to_string();
    let fields = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let fields = fields
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let source = fields
        .iter()
        .find(|entry| {
            entry.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                && entry.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("model")
                && entry.get("field").and_then(Value::as_str) == Some(field)
        })
        .ok_or("actual material resolution provenance")?;
    assert_eq!(
        source.get("containsEncryptedSource"),
        Some(&json!(true)),
        "resolution depends on an encrypted selector"
    );
    assert_eq!(
        source.get("sourceSetComplete"),
        Some(&json!(true)),
        "actual selector and directory evidence are complete"
    );
    let refs = source
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use refs")?;
    let uses = fs::read_to_string(base.join("origin-uses.jsonl"))?;
    let uses = uses
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(uses
        .into_iter()
        .filter(|usage| usage.get("id").is_some_and(|id| refs.contains(id)))
        .collect())
}

fn attachment(
    rows: &[MediaRecord],
    child: u64,
) -> Result<(usize, &MediaRecord), Box<dyn std::error::Error>> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == "model-character-attachment"
                && row.subresource.indices == [0, child]
                && entry_is(row, 0)
        })
        .ok_or_else(|| format!("missing attachment {child}").into())
}

const fn entry_is(row: &MediaRecord, index: usize) -> bool {
    matches!(&row.source.identity, p4kx_media::Identity::Entry { entry, .. } if entry.entry_index == index)
}
