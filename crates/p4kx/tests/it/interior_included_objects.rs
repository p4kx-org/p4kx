// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use p4kx_testkit::{
    ArchiveBuilder, SOC_IDENTITY, TestChunkFile, TestContainer, TestEntry, TestModel,
    TestObjectContainer, TestSoc,
};
use serde_json::{Value, json};

#[path = "interior_included_objects_package.rs"]
mod package;

type Check<T = ()> = Result<T, Box<dyn Error>>;

const CONTAINER: &str = "Data/placed.socpak";
const MODEL_SELECTOR: &str = "objects/test/positioned.cgf";
const FIRST_LAYOUT: u32 = 8;
const SECOND_LAYOUT: u32 = 15;
const INCLUDED_OBJECTS: u32 = 0x0010;
const FIRST_ORDINAL: u32 = 0;
const SECOND_ORDINAL: u32 = 2;
const NAME_BYTES: usize = 256;
const MATERIAL_START: usize = 8 + NAME_BYTES + 4;
const TRANSLATION: f64 = 1.0 + f64::EPSILON;
const PREFIX_VALUE: f64 = -6.999_999_999_999_999;
const PREFIX_NAN: u64 = 0x7ff8_0000_0000_001e;

#[test]
fn the_authored_package_and_standalone_source_retain_identical_binary_placements() -> Check {
    let dir = tempfile::tempdir()?;
    let fixture = Fixture::write(dir.path())?;
    let standalone = dir.path().join("source.json");
    fixture.run("source", &standalone, &[])?;
    let plain = dir.path().join("plain");
    fixture.run("package", &plain, &[])?;
    let package = dir.path().join("authored");
    fixture.run("package", &package, &["--authored-soc"])?;

    let source = read_json(&standalone)?;
    let packaged = package::verify(&plain, &package)?;
    assert_eq!(
        source, packaged,
        "the CLI command and package publish the same source document"
    );
    verify_chronology(&source)?;
    verify_chunks(&source, &fixture)
}

struct Fixture {
    path: PathBuf,
    first: Vec<u8>,
    second: Vec<u8>,
}

impl Fixture {
    fn write(dir: &Path) -> Check<Self> {
        let mut transform = SOC_IDENTITY;
        let [first_row, ..] = &mut transform;
        let [.., x] = first_row;
        *x = TRANSLATION;
        let mut first = TestSoc::new()
            .layout(FIRST_LAYOUT)
            .model(MODEL_SELECTOR)
            .object(0, transform)
            .skipped_object()
            .chunk();
        insert_material(&mut first)?;
        set_prefix(&mut first, 2)?;
        let mut second = TestSoc::new()
            .layout(SECOND_LAYOUT)
            .model(MODEL_SELECTOR)
            .object_with(0, transform, u8::MAX)
            .object_with(0, [[f64::NAN; 4]; 3], u8::MAX)
            .object_of_kind(7)
            .chunk();
        set_prefix(&mut second, 1)?;
        let bytes = TestChunkFile::crch()
            .chunk(INCLUDED_OBJECTS, 1, first.clone())
            .chunk(0x0004, 3, b"<Entities/>".to_vec())
            .chunk(INCLUDED_OBJECTS, 1, second.clone())
            .bytes();
        let descriptor = TestObjectContainer::new("placed")
            .stream("placed.soc")
            .stream("placed.soc");
        let container = TestContainer::new()
            .entry(TestEntry::stored("placed.xml", descriptor.xml()))
            .entry(TestEntry::deflated("placed.soc", bytes));
        let model = TestModel::cube();
        let path = dir.join("sample.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::deflated(CONTAINER, container.build()).encrypted())
            .entry(TestEntry::stored(
                "Data/objects/test/positioned.cgf",
                model.cgf(),
            ))
            .entry(TestEntry::stored(
                "Data/objects/test/positioned.cgfm",
                model.cgfm(),
            ))
            .write_to(&path)?;
        Ok(Self {
            path,
            first,
            second,
        })
    }

    fn run(&self, subcommand: &str, out: &Path, extra: &[&str]) -> Check {
        let output = p4kx_testkit::p4kx()?
            .args([
                "--key",
                &p4kx_testkit::test_key().to_hex(),
                "interior",
                subcommand,
            ])
            .arg(&self.path)
            .arg(CONTAINER)
            .arg("--out")
            .arg(out)
            .args(extra)
            .output()?;
        assert!(
            output.status.success(),
            "{subcommand}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}

fn insert_material(first: &mut Vec<u8>) -> Check {
    let mut material = [0; NAME_BYTES];
    let name = b"test/material.mtl";
    material
        .get_mut(..name.len())
        .ok_or("material path buffer")?
        .copy_from_slice(name);
    first
        .get_mut(MATERIAL_START - 4..MATERIAL_START - 2)
        .ok_or("material count field")?
        .copy_from_slice(&1_u16.to_le_bytes());
    first.splice(MATERIAL_START..MATERIAL_START, material);
    Ok(())
}

fn set_prefix(chunk: &mut [u8], paths: usize) -> Check {
    let prefix = 8 + paths * NAME_BYTES + 4 + 32 + size_of::<u32>();
    let middle = prefix + size_of::<u64>();
    let end = middle + size_of::<u64>();
    chunk
        .get_mut(prefix..middle)
        .ok_or("first prefix lane")?
        .copy_from_slice(&PREFIX_VALUE.to_le_bytes());
    chunk
        .get_mut(middle..end)
        .ok_or("second prefix lane")?
        .copy_from_slice(&PREFIX_NAN.to_le_bytes());
    Ok(())
}

fn verify_chronology(source: &Value) -> Check {
    let streams = source
        .pointer("/occurrences/0/streams")
        .and_then(Value::as_array)
        .ok_or("two authored stream declarations")?;
    assert_eq!(
        streams.len(),
        2,
        "repeat stream declarations both remain addressable"
    );
    for stream in streams {
        assert_eq!(
            stream.get("includedObjects"),
            Some(&json!([0, 1])),
            "each occurrence addresses the same two physical tables"
        );
    }
    Ok(())
}

fn verify_chunks(source: &Value, fixture: &Fixture) -> Check {
    let chunks = source
        .get("includedObjects")
        .and_then(Value::as_array)
        .ok_or("physical source table")?;
    let [first, second] = chunks.as_slice() else {
        return Err("two physical rows, regardless of repeat stream selectors".into());
    };
    assert_eq!(
        first.get("source"),
        second.get("source"),
        "same original SOC member"
    );
    verify_chunk(first, FIRST_ORDINAL, FIRST_LAYOUT, &fixture.first)?;
    verify_chunk(second, SECOND_ORDINAL, SECOND_LAYOUT, &fixture.second)?;
    let material_path = first
        .pointer("/tables/value/materials/0/path")
        .ok_or("original material declaration")?;
    assert_eq!(
        material_path.get("value").and_then(Value::as_str),
        Some("test/material.mtl"),
        "material paths export without assuming a rendering override"
    );
    verify_origin(source, first, material_path)?;
    verify_rows(source, first, 0, "kind16", 1)?;
    verify_rows(source, second, 255, "unmeasuredKind", 2)?;
    let nonfinite = second
        .pointer("/tables/value/rows/1/data/value/transform")
        .ok_or("nonfinite placement matrix state")?;
    assert_eq!(
        nonfinite.get("state"),
        Some(&json!("present")),
        "the authored matrix was readable"
    );
    assert_eq!(
        nonfinite.pointer("/value/0/0"),
        Some(&Value::Null),
        "nonfinite cell remains distinct without an invalid JSON number"
    );
    verify_origin(source, second, nonfinite)
}

fn verify_rows(
    source: &Value,
    chunk: &Value,
    selector: u8,
    tail_kind: &str,
    tail_ordinal: u32,
) -> Check {
    let tables = chunk
        .pointer("/tables/value")
        .ok_or("positioned binary tables")?;
    let placement = tables
        .pointer("/rows/0/data/value")
        .ok_or("typed shape of known row")?;
    let tail = tables
        .pointer(&format!("/rows/{tail_ordinal}/data/value"))
        .ok_or("positioned tail")?;
    assert_eq!(
        placement.get("type"),
        Some(&json!("placement")),
        "known placement kind"
    );
    assert_eq!(
        tail.get("type"),
        Some(&json!(tail_kind)),
        "original tail remains distinct from a placement"
    );
    assert_eq!(
        placement.pointer("/modelIndex/value"),
        Some(&json!(0)),
        "original geometry selector"
    );
    assert_eq!(
        placement.pointer("/paletteSelector/value"),
        Some(&json!(selector)),
        "original palette byte"
    );
    assert_eq!(
        placement.pointer("/transform/value/0/3"),
        Some(&json!(TRANSLATION)),
        "original binary64 precision survives the JSON projection"
    );
    verify_prefix(source, chunk, placement)?;
    for cell in [
        "/models/0/path",
        "/rows/0/data/value/modelIndex",
        "/rows/0/data/value/paletteSelector",
        "/rows/0/data/value/transform",
    ] {
        let value = tables
            .pointer(cell)
            .ok_or_else(|| format!("sourced field {cell}"))?;
        verify_origin(source, chunk, value)?;
    }
    Ok(())
}

fn verify_prefix(source: &Value, chunk: &Value, placement: &Value) -> Check {
    let prefix = placement
        .get("prefixF64")
        .ok_or("original f64 prefix lanes")?;
    assert_eq!(
        prefix.pointer("/value/0/0"),
        Some(&json!(PREFIX_VALUE)),
        "prefix binary64 precision"
    );
    assert_eq!(
        prefix.pointer("/value/0/1"),
        Some(&Value::Null),
        "uncommon nonfinite prefix lane uses null while original words survive"
    );
    verify_origin(source, chunk, prefix)
}

fn verify_chunk(chunk: &Value, ordinal: u32, layout: u32, original: &[u8]) -> Check {
    use base64::Engine as _;

    assert_eq!(
        chunk.get("ordinal"),
        Some(&json!(ordinal)),
        "physical chunk order retains the intervening XML chunk"
    );
    assert_eq!(
        chunk.pointer("/tables/value/layout/value"),
        Some(&json!(layout)),
        "the layout word comes from the chunk"
    );
    let raw = chunk
        .pointer("/raw/value")
        .and_then(Value::as_str)
        .ok_or("raw original bytes")?;
    assert_eq!(
        STANDARD.decode(raw)?,
        original,
        "every physical payload byte survives"
    );
    assert_eq!(
        chunk.pointer("/raw/containsEncryptedSource"),
        Some(&json!(true)),
        "encrypted container contributes payload bytes"
    );
    Ok(())
}

fn verify_origin(source: &Value, chunk: &Value, value: &Value) -> Check {
    assert_eq!(
        value.get("containsEncryptedSource"),
        Some(&json!(true)),
        "inherited source encryption reaches the leaf value"
    );
    assert_eq!(
        value.get("sourceSetComplete"),
        Some(&json!(true)),
        "original leaf evidence is complete"
    );
    let refs = value
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field source references")?;
    let uses = source
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("physical source use arena")?;
    assert!(
        refs.iter().any(|id| {
            uses.iter().any(|site| {
                site.get("id") == Some(id)
                    && site.get("source") == chunk.get("source")
                    && site.get("effectiveEncryption").and_then(Value::as_str) == Some("encrypted")
            })
        }),
        "the value links to an encrypted use of the actual SOC member"
    );
    Ok(())
}

fn read_json(path: &Path) -> Check<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
