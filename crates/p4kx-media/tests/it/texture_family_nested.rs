// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, SplitDds, TestContainer, TestEntry, TestTextureFormat, test_key,
};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

#[test]
fn nested_directory_and_member_origins_keep_the_actual_encrypted_container() -> TestResult {
    const HEADER: &str = "Data/nested.dds";
    const CONTAINER: &str = "nested.socpak";
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4).split(1);
    let container = TestContainer::new()
        .entry(TestEntry::stored(HEADER, dds.header_bytes()))
        .entry(TestEntry::stored("Data/nested.dds.1", dds.part(1)));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(CONTAINER, container.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = root.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        include: Some(Filter::parse("*nested.dds")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    let base = output.join(&index.artifact_base);
    let fields: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    let uses: Vec<Value> = read_rows(base.join("origin-uses.jsonl"))?;
    let sources: Vec<Value> = read_rows(base.join("origin-sources.jsonl"))?;
    let (member_ordinal, member) = row(&rows, "texture.dds-member", &[1])?;
    let Identity::Entry { containers, .. } = &member.source.identity else {
        return Err("member row is owned by its physical header".into());
    };
    let [container] = containers.as_slice() else {
        return Err("one exact container ancestry".into());
    };
    assert_eq!(
        container.path, CONTAINER,
        "selected physical containing Entry"
    );
    let declared = field(&fields, member_ordinal, "/data/payload/declared_bytes")?;
    check_parent(declared, &uses, &sources, CONTAINER)?;
    let (family_ordinal, _) = row(&rows, "texture.dds-family", &[])?;
    let inventory = field(&fields, family_ordinal, "/data/payload/directory/status")?;
    assert_eq!(
        inventory.get("containsEncryptedSource"),
        Some(&json!(true)),
        "opened child directory is part of the encrypted container"
    );
    Ok(())
}

fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing nested DDS row {namespace} {indices:?}").into())
}

fn field<'a>(rows: &'a [Value], ordinal: usize, path: &str) -> TestResult<&'a Value> {
    let ordinal = ordinal.to_string();
    rows.iter()
        .find(|row| {
            row.get("field").and_then(Value::as_str) == Some(path)
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("texture")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
        })
        .ok_or_else(|| format!("missing nested DDS field origin {ordinal} {path}").into())
}

fn check_parent(
    field: &Value,
    uses: &[Value],
    sources: &[Value],
    expected_path: &str,
) -> TestResult {
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(true)),
        "nested directory bytes inherit container encryption"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(true)),
        "source use has an original physical owner"
    );
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field use refs")?;
    let [reference] = refs.as_slice() else {
        return Err("one actual declared size use".into());
    };
    let matched = uses
        .iter()
        .find(|row| row.get("id") == Some(reference))
        .ok_or("declared size use")?;
    let source_id = matched.get("source").ok_or("source identifier")?;
    let source = sources
        .iter()
        .find(|row| row.get("id") == Some(source_id))
        .ok_or("member original source")?;
    let ancestors = source
        .get("ancestors")
        .and_then(Value::as_array)
        .ok_or("member ancestry")?;
    let [container_id] = ancestors.as_slice() else {
        return Err("one actual source ancestor".into());
    };
    let container = sources
        .iter()
        .find(|row| row.get("id") == Some(container_id))
        .ok_or("original container source")?;
    assert_eq!(
        container.pointer("/path/value"),
        Some(&json!(expected_path)),
        "exact physical container ancestor"
    );
    assert_eq!(
        container.get("storedEncryption"),
        Some(&json!("encrypted")),
        "ancestor's stored encryption"
    );
    Ok(())
}
