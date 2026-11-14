// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{Identity, MaterialProvider, Status};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry};
use serde_json::Value;

use super::material_support::{self as support, TestResult};

const MATERIAL: &[u8] = br#"<Material MtlFlags="-1" RootOnly="keep"><PublicParams Root="001.50"/><SubMaterials><Material Name="Hull"><PublicParams Case="NaN" case="Infinity"/><MatLayers><Layer Path="missing.mtl" Submtl="Hull"/></MatLayers><Textures><Texture Map="TexSlot1" File="$RenderToTexture" Used="1"><TexMod TileU="2.50"/></Texture></Textures><Unknown Keep="yes">text</Unknown></Material><UnknownSlot Name="retained"/></SubMaterials></Material>"#;

#[test]
fn unused_and_duplicate_named_libraries_keep_physical_identity_and_exact_original_bytes()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let binary = TestElement::new("Material")
        .attribute("Root", "binary")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    let container = TestContainer::new().entry(TestEntry::stored("unused.mtl", binary.as_slice()));
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("same.mtl", MATERIAL),
            TestEntry::stored("same.mtl", b"<Material Name='Other'/>"),
            TestEntry::stored("nested.pak", container.build()),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    assert_eq!(
        records.len(),
        3,
        "standalone sources do not require geometry or a unique path"
    );
    let physical = verify_sources(&records, &output, &index, &binary)?;
    assert_eq!(
        physical,
        [(0, 0), (0, 1), (1, 0)],
        "duplicate names and nested original indices remain distinct"
    );
    verify_raw_fields(records.first().ok_or("root material")?)?;
    Ok(())
}

fn verify_sources(
    records: &[p4kx_media::MediaRecord],
    output: &std::path::Path,
    index: &p4kx_media::Index,
    binary: &[u8],
) -> Result<Vec<(usize, usize)>, Box<dyn std::error::Error>> {
    let mut physical = Vec::new();
    for record in records {
        assert_eq!(
            record.status,
            Status::Complete,
            "supported raw declarations are not incomplete merely because a dependency was not looked up"
        );
        let Identity::Entry { containers, entry } = &record.source.identity else {
            return Err("material entry identity".into());
        };
        physical.push((containers.len(), entry.entry_index));
        let raw = record
            .source
            .raw
            .as_ref()
            .ok_or("retained original material bytes")?;
        let bytes = fs::read(output.join(&index.artifact_base).join(&raw.path))?;
        let expected = match (containers.len(), entry.entry_index) {
            (0, 0) => MATERIAL,
            (0, 1) => b"<Material Name='Other'/>",
            (1, 0) => binary,
            _ => return Err("unexpected physical source".into()),
        };
        assert_eq!(
            bytes, expected,
            "source retention is exact for both plain and binary XML"
        );
    }
    Ok(physical)
}

fn verify_raw_fields(record: &p4kx_media::MediaRecord) -> TestResult {
    let payload = record.data.get("payload").ok_or("payload")?;
    let root = payload.pointer("/authored/root").ok_or("authored root")?;
    let root_attrs = root
        .pointer("/syntax/attributes")
        .and_then(Value::as_array)
        .ok_or("root attributes")?;
    assert_eq!(
        root_attrs
            .first()
            .and_then(|attribute| attribute.get("value"))
            .and_then(Value::as_str),
        Some("-1"),
        "root flag lexeme is separate from child flags"
    );
    let first = payload
        .pointer("/materials/0")
        .ok_or("first original material")?;
    assert_eq!(
        first
            .pointer("/interpreted_surface/flags")
            .and_then(Value::as_u64),
        Some(0),
        "root flags are not inherited into children"
    );
    let params = first
        .pointer("/authored/public_params")
        .and_then(Value::as_array)
        .ok_or("PublicParams")?;
    assert_eq!(params.len(), 2, "case-distinct parameters remain ordered");
    assert_eq!(
        params
            .first()
            .and_then(|param| param.get("value"))
            .and_then(Value::as_str),
        Some("NaN"),
        "nonfinite source spelling is not replaced by a JSON float"
    );
    assert_eq!(
        first
            .pointer("/authored/textures/0/children/0/attributes/0/value")
            .and_then(Value::as_str),
        Some("2.50"),
        "non-default texture modifier survives"
    );
    assert_eq!(
        payload
            .pointer("/authored/entries/1/disposition")
            .and_then(Value::as_str),
        Some("engine-default"),
        "unknown slot retains explicit renderer disposition"
    );
    verify_references(payload)
}

fn verify_references(payload: &Value) -> TestResult {
    let references = payload
        .get("references")
        .and_then(Value::as_array)
        .ok_or("references")?;
    assert_eq!(
        references.len(),
        2,
        "one layer declaration and one placeholder are preserved"
    );
    assert_eq!(
        references
            .first()
            .and_then(|reference| reference.pointer("/reference/resolution/status"))
            .and_then(Value::as_str),
        Some("missing"),
        "complete owning-source inventory proves the absent library",
    );
    assert_eq!(
        references
            .get(1)
            .and_then(|reference| reference.pointer("/reference/resolution/status"))
            .and_then(Value::as_str),
        Some("unavailable"),
        "engine placeholder binding does not follow file lookup",
    );
    assert_eq!(
        references
            .first()
            .and_then(|reference| reference.get("submaterial"))
            .and_then(Value::as_str),
        Some("Hull"),
        "Submtl remains an original name rather than an inferred integer"
    );
    Ok(())
}

#[test]
fn shared_source_dto_roundtrip_preserves_namespace_and_original_tokens_without_media_types()
-> TestResult {
    let xml = br#"<m:Material xmlns:m="urn:m" Case="A&amp;B" case="two"><m:Unknown>before<Leaf/>after</m:Unknown></m:Material>"#;
    let library = p4kx_extraction::model::MaterialLibrary::parse("synthetic/shared", xml)?;
    let source = p4kx_extraction::MaterialLibrarySourceInfo::of(&library);
    let value = serde_json::to_value(&source)?;
    let owned: p4kx_extraction::MaterialLibrarySourceInfo<'static> = serde_json::from_value(value)?;
    assert_eq!(
        owned, source,
        "all shared fields survive owned JSON deserialization"
    );
    let root = owned.root.as_ref().ok_or("root")?;
    assert_eq!(
        root.syntax.qualified_name, "m:Material",
        "QName is not replaced by a local name"
    );
    assert_eq!(
        root.syntax
            .attributes
            .get(1)
            .and_then(|attribute| attribute.lexeme.as_deref()),
        Some("A&amp;B"),
        "authored attribute token remains independent of decoded text"
    );
    Ok(())
}
