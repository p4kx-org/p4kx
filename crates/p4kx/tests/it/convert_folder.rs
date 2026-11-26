// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx convert` over a folder written by `p4kx extract` without `--convert`.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, SplitDds, TestCodec, TestElement, TestEntry,
    TestTextureFormat, TestWem, check_dds_origin_pair, p4kx, test_signal,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture {
    dir: tempfile::TempDir,
    archive: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let ship = TestElement::new("Ship")
        .attribute("name", "Gladius")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let sound = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(480, 1)).bytes();
    let texture = SplitDds::new(TestTextureFormat::LegacyDxt5, 32, 16).split(3);
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Ships\\gladius.xml", ship))
        .entry(TestEntry::stored("Data\\Sounds\\5.wem", sound))
        .entry(TestEntry::stored("Data\\readme.bin", b"plain".to_vec()));
    for entry in texture.entries("Data\\Textures\\rock_diff.dds") {
        builder = builder.entry(entry);
    }
    builder.write_to(&archive)?;
    Ok(Fixture { dir, archive })
}

impl Fixture {
    fn extract(&self, name: &str, convert: bool) -> Result<PathBuf, Box<dyn Error>> {
        let out = self.dir.path().join(name);
        let mut command = p4kx()?;
        command
            .arg("extract")
            .arg(&self.archive)
            .args(["--filter", "Data", "--out"])
            .arg(&out);
        if convert {
            command.arg("--convert");
        }
        command.assert().code(0);
        Ok(out)
    }
}

fn tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut found = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let name = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.insert(name, fs::read(&path)?);
            }
        }
    }
    Ok(found)
}

#[test]
fn convert_writes_what_extract_with_convert_writes() -> TestResult {
    const DDS: &str = "Data/Textures/rock_diff.dds";
    const RECEIPT: &str = "Data/Textures/rock_diff.dds.p4kx-origin.jsonl";

    let fixture = fixture()?;
    let expected = fixture.extract("converted", true)?;
    let raw = fixture.extract("raw", false)?;
    let out = fixture.dir.path().join("from-folder");

    let output = p4kx()?
        .args(["--json", "convert"])
        .arg(&raw)
        .arg("--out")
        .arg(&out)
        .arg("--copy-other")
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let json: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["failed"], serde_json::json!([]), "no failure");
    let merged = json["merged_texture_artifacts"]
        .as_array()
        .ok_or("DDS summary")?;
    let [artifact] = merged.as_slice() else {
        return Err("the folder conversion must publish one merged DDS receipt".into());
    };
    assert_eq!(artifact["path"], DDS, "the written DDS");
    assert_eq!(
        artifact["origin_receipt"], RECEIPT,
        "the portable loose receipt"
    );
    let mut loose = tree(&out)?;
    let mut archive = tree(&expected)?;
    let archive_receipt = archive.remove(RECEIPT).ok_or("Archive DDS receipt")?;
    let loose_receipt = loose.remove(RECEIPT).ok_or("loose DDS receipt")?;
    check_dds_origin_pair(
        loose.get(DDS).ok_or("the written DDS")?,
        &archive_receipt,
        &loose_receipt,
    )?;
    check_receipt_pair(
        &mut archive,
        &mut loose,
        "Data/Sounds/5.wav",
        &json!({
            "type": "wavBytes",
            "start": "44",
            "endExclusive": "1004",
            "field": "pcmSamples",
        }),
    )?;
    check_receipt_pair(
        &mut archive,
        &mut loose,
        "Data/readme.bin",
        &json!({"type": "opaquePayload"}),
    )?;
    check_converted_xml(&json, &mut archive, &mut loose)?;
    assert_eq!(loose, archive, "the same payload files");
    Ok(())
}

fn check_converted_xml(
    report: &Value,
    archive: &mut BTreeMap<String, Vec<u8>>,
    loose: &mut BTreeMap<String, Vec<u8>>,
) -> TestResult {
    const XML: &str = "Data/Ships/gladius.xml";
    let artifacts = report["cryxml_artifacts"]
        .as_array()
        .ok_or("converted CryXml summary")?;
    let [artifact] = artifacts.as_slice() else {
        return Err("expected one converted XML artifact".into());
    };
    assert_eq!(artifact["path"], XML, "the converted XML");
    assert_eq!(
        artifact["origin_receipt"],
        format!("{XML}.p4kx-origin.jsonl"),
        "the committed CryXml companion"
    );
    let xml = std::str::from_utf8(loose.get(XML).ok_or("written XML")?)?;
    let start = xml.find("<Ship").ok_or("the root element name")? + 1;
    check_receipt_pair(
        archive,
        loose,
        XML,
        &json!({
            "type": "xmlBytes",
            "start": start.to_string(),
            "endExclusive": (start + "Ship".len()).to_string(),
            "field": "elementName",
        }),
    )
}

fn check_receipt_pair(
    archive: &mut BTreeMap<String, Vec<u8>>,
    loose: &mut BTreeMap<String, Vec<u8>>,
    artifact: &str,
    address: &Value,
) -> TestResult {
    let companion = format!("{artifact}.p4kx-origin.jsonl");
    let leaf = Path::new(artifact).file_name().ok_or("artifact leaf")?;
    for (files, encryption) in [(archive, json!(false)), (loose, Value::Null)] {
        let bytes = files.get(artifact).ok_or("written artifact")?;
        let mut measured = OriginDigestWriter::new(io::sink());
        measured.write_all(bytes)?;
        let digest = measured.finish()?;
        let receipt = files.remove(&companion).ok_or("origin companion")?;
        let rows: Vec<Value> = std::str::from_utf8(&receipt)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        let header = rows.first().ok_or("receipt header")?;
        assert_eq!(
            header["type"], "header",
            "the companion starts with its binding"
        );
        assert_eq!(
            header["binding"],
            json!({
                "type": "file",
                "path": {"encoding": "utf8", "value": leaf.to_str().ok_or("UTF-8 leaf")?},
                "byteLength": digest.byte_length().to_string(),
                "sha256": digest.sha256(),
            }),
            "the companion binds the installed artifact",
        );
        let field = rows
            .iter()
            .find(|row| row.get("address") == Some(address))
            .ok_or("selected receipt address")?;
        assert_eq!(
            field["sourceSetComplete"], true,
            "the payload source is complete"
        );
        assert_eq!(
            field["containsEncryptedSource"], encryption,
            "the actual input determines the encryption state"
        );
        assert_eq!(
            rows.last().ok_or("receipt end")?["type"],
            "end",
            "the companion has a terminal row"
        );
    }
    Ok(())
}

#[test]
fn files_without_a_conversion_are_left_out_and_counted() -> TestResult {
    let fixture = fixture()?;
    let raw = fixture.extract("raw", false)?;
    let out = fixture.dir.path().join("out");

    let output = p4kx()?
        .args(["--json", "convert"])
        .arg(&raw)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let json: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["not_converted"], 1, "readme.bin");
    assert!(!out.join("Data/readme.bin").exists(), "not copied");
    Ok(())
}

#[test]
fn an_output_that_overlaps_the_input_is_invalid_input() -> TestResult {
    let fixture = fixture()?;
    let raw = fixture.extract("raw", false)?;
    let before = tree(&raw)?;

    p4kx()?
        .arg("convert")
        .arg(&raw)
        .arg("--out")
        .arg(raw.join("inside"))
        .assert()
        .code(2);

    assert_eq!(tree(&raw)?, before, "the input is untouched");
    assert!(!raw.join("inside").exists(), "nothing was created");
    Ok(())
}

#[test]
fn a_filter_that_matches_no_file_is_invalid_input() -> TestResult {
    let fixture = fixture()?;
    let raw = fixture.extract("raw", false)?;

    p4kx()?
        .arg("convert")
        .arg(&raw)
        .args(["--filter", "no-such-file"])
        .arg("--out")
        .arg(fixture.dir.path().join("out"))
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn an_input_that_is_no_folder_is_invalid_input() -> TestResult {
    let fixture = fixture()?;

    p4kx()?
        .arg("convert")
        .arg(&fixture.archive)
        .arg("--out")
        .arg(fixture.dir.path().join("out"))
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn a_run_cancelled_before_the_folder_is_read_writes_nothing_and_ends_with_code_1() -> TestResult {
    let fixture = fixture()?;
    let raw = fixture.extract("raw", false)?;
    let out = fixture.dir.path().join("out");

    p4kx()?
        .arg("convert")
        .arg(&raw)
        .arg("--out")
        .arg(&out)
        .args(["--cancel-after", "0"])
        .assert()
        .code(1);

    assert!(!out.exists(), "nothing was created");
    Ok(())
}
