// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx chunks list|dump` end to end, for chunk file Entries of an Archive and files on disk.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestElement, TestEntry,
};
use predicates::prelude::predicate;

#[path = "chunks_origin.rs"]
mod origin;

type TestResult = Result<(), Box<dyn Error>>;

const MODEL_ENTRY: &str = "Data\\Objects\\Test\\cube.cgf";

fn ivo_model() -> Vec<u8> {
    TestChunkFile::ivo()
        .chunk(0xBE5E_493E, 0x900, [0; 16])
        .chunk(0x8335_3333, 0x900, *b"cube_material\0\0\0")
        .chunk(0x9293_B9D8, 0x901, [9; 40])
        .chunk(0x00C0_FFEE, 0x900, [1; 8])
        .bytes()
}

fn crch_model() -> Vec<u8> {
    TestChunkFile::crch()
        .chunk(0x100B, 0x824, *b"node")
        .big_endian_chunk(0x1016, 0x800, [0, 0, 0, 3, 1, 2, 3])
        .chunk(0x4321, 0x001, [5; 3])
        .bytes()
}

fn write_archive(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL_ENTRY, ivo_model()))
        .write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

#[test]
fn list_of_an_ivo_entry_prints_the_chunk_table() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx()?
        .args(["chunks", "list"])
        .arg(&archive)
        .arg("data/objects/test/CUBE.cgf")
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    insta::assert_snapshot!(String::from_utf8(output.stdout)?);
    Ok(())
}

#[test]
fn list_of_a_crch_file_shows_ids_and_byte_order_per_chunk() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("model.cgf");
    fs::write(&file, crch_model())?;

    let output = p4kx()?
        .current_dir(dir.path())
        .args(["chunks", "list", "model.cgf"])
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    insta::assert_snapshot!(String::from_utf8(output.stdout)?);
    Ok(())
}

#[test]
fn list_json_gives_every_chunk_as_an_object() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx()?
        .args(["--json", "chunks", "list"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let mut actual: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let report = actual.as_object_mut().ok_or("chunk report object")?;
    assert!(report.remove("fieldOrigins").is_some(), "per-field origins");
    assert!(report.remove("provenance").is_some(), "source inventory");
    let previous =
        include_str!("snapshots/it__chunks__list_json_gives_every_chunk_as_an_object.snap")
            .lines()
            .find(|line| line.starts_with('{'))
            .ok_or("previous chunk-list JSON")?;
    let previous: serde_json::Value = serde_json::from_str(previous)?;
    assert_eq!(actual, previous, "the existing fields retain their values");
    Ok(())
}

#[test]
fn dump_of_an_entry_writes_the_chunk_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("mesh.bin");

    p4kx()?
        .args(["chunks", "dump"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .args(["--chunk", "2", "--out"])
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("wrote chunk 2 (Mesh, 40 bytes)"));

    assert_eq!(fs::read(&out)?, [9; 40], "the chunk's data");
    Ok(())
}

#[test]
fn dump_of_a_file_with_json_describes_the_chunk() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("model.cgf");
    fs::write(&file, crch_model())?;
    let out = dir.path().join("stream.bin");

    let output = p4kx()?
        .args(["--json", "chunks", "dump"])
        .arg(&file)
        .args(["--chunk", "1", "--out"])
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["chunk"]["type"], "DataStream", "type");
    assert_eq!(json["chunk"]["byte_order"], "big", "byte order");
    assert_eq!(json["chunk"]["id"], 2, "chunk ID");
    assert_eq!(json["chunk"]["size"], 7, "size");
    assert_eq!(json["out"], out.display().to_string(), "target");
    assert_eq!(fs::read(&out)?, [0, 0, 0, 3, 1, 2, 3], "bytes unchanged");
    Ok(())
}

#[test]
fn dump_of_a_chunk_the_file_does_not_have_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("missing.bin");

    p4kx()?
        .args(["chunks", "dump"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .args(["--chunk", "4", "--out"])
        .arg(&out)
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "has no chunk 4; its 4 chunks are numbered from 0",
        ));

    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn dump_onto_the_chunk_file_itself_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("model.cgf");
    fs::write(&file, crch_model())?;

    p4kx()?
        .args(["chunks", "dump"])
        .arg(&file)
        .args(["--chunk", "0", "--out"])
        .arg(&file)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("is one of the inputs"));

    assert_eq!(fs::read(&file)?, crch_model(), "the input is unchanged");
    Ok(())
}

fn soc_file() -> Vec<u8> {
    let entities = TestElement::new("Entities")
        .child(TestElement::new("Entity").attribute("Name", "door"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let utf16_xml: Vec<u8> = std::iter::once(0xFEFF_u16)
        .chain("<Entities/>".encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    TestChunkFile::crch()
        .chunk(0x0002, 5, [0; 8])
        .chunk(0x0004, 3, entities.clone())
        .chunk(0x0011, 1, *b"{\"a\":1}")
        .chunk(0x0013, 1, *b"objects/a.cgf\nobjects/b.cgf\n")
        .chunk(0x000E, 1, [0; 8])
        .chunk(0x0011, 1, [0xFF, 0xFE])
        .chunk(0x0014, 3, entities)
        .chunk(0x0004, 1, utf16_xml)
        .chunk(0x0004, 1, *b"<Entities/>")
        .bytes()
}

#[test]
fn list_of_a_soc_file_names_its_chunk_types() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("room.soc");
    fs::write(&file, soc_file())?;

    let output = p4kx()?.args(["chunks", "list"]).arg(&file).output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let text = String::from_utf8(output.stdout)?;
    for name in ["Soc0002", "CryXmlB", "Json", "ModelPaths", "AreaShape"] {
        assert!(text.contains(name), "{name} in {text}");
    }
    assert!(!text.contains("Unknown:"), "no unknown type: {text}");
    Ok(())
}

#[test]
fn dump_text_writes_cryxml_as_xml_and_json_as_stored() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("room.soc");
    fs::write(&file, soc_file())?;
    let xml = dir.path().join("entities.xml");
    let json = dir.path().join("chunk.json");
    let paths = dir.path().join("paths.txt");
    let second = dir.path().join("second.xml");
    let text_xml = dir.path().join("text.xml");

    for (index, out) in [
        ("1", &xml),
        ("2", &json),
        ("3", &paths),
        ("6", &second),
        ("8", &text_xml),
    ] {
        p4kx()?
            .args(["chunks", "dump"])
            .arg(&file)
            .args(["--text", "--chunk", index, "--out"])
            .arg(out)
            .assert()
            .code(0);
    }

    let xml = fs::read_to_string(&xml)?;
    assert!(
        xml.contains("<Entity Name=\"door\""),
        "the entities as XML: {xml}"
    );
    assert!(
        fs::read_to_string(&second)?.contains("<Entity Name=\"door\""),
        "the 0x0014 chunk as XML"
    );
    assert_eq!(
        fs::read_to_string(&text_xml)?,
        "<Entities/>",
        "text XML as stored"
    );
    assert_eq!(fs::read_to_string(&json)?, "{\"a\":1}", "JSON as stored");
    assert_eq!(
        fs::read_to_string(&paths)?,
        "objects/a.cgf\nobjects/b.cgf\n",
        "paths as stored"
    );
    Ok(())
}

#[test]
fn dump_text_of_a_chunk_without_a_text_form_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("room.soc");
    fs::write(&file, soc_file())?;
    let out = dir.path().join("area.txt");

    p4kx()?
        .args(["chunks", "dump"])
        .arg(&file)
        .args(["--text", "--chunk", "4", "--out"])
        .arg(&out)
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "AreaShape, which has no text form",
        ));

    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn dump_text_of_corrupt_text_exits_with_code_3() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("room.soc");
    fs::write(&file, soc_file())?;
    let out = dir.path().join("bad.json");

    for index in ["5", "7"] {
        p4kx()?
            .args(["chunks", "dump"])
            .arg(&file)
            .args(["--text", "--chunk", index, "--out"])
            .arg(&out)
            .assert()
            .code(3);
        assert!(!out.exists(), "nothing is written for chunk {index}");
    }
    Ok(())
}

#[test]
fn a_file_that_is_no_chunk_file_exits_with_code_3() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("not_a_model.cgf");
    fs::write(&file, b"<Material/>\n")?;

    p4kx()?
        .args(["chunks", "list"])
        .arg(&file)
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "not a chunk file: the signature is \"<Mat\"",
        ));
    Ok(())
}

#[test]
fn a_truncated_chunk_file_exits_with_code_3() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("truncated.cgf");
    let model = crch_model();
    fs::write(&file, model.get(..model.len() - 2).unwrap_or_default())?;

    p4kx()?
        .args(["chunks", "list"])
        .arg(&file)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("chunk 2 at offset"));
    Ok(())
}

#[test]
fn a_missing_file_or_entry_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .args(["chunks", "list"])
        .arg(dir.path().join("missing.cgf"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("does not exist"));
    p4kx()?
        .args(["chunks", "list"])
        .arg(&archive)
        .arg("Data\\Objects\\Test\\sphere.cgf")
        .assert()
        .code(2);
    Ok(())
}
