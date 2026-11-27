// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract --convert` over a synthetic Archive with `CryXml` and text XML Entries.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use predicates::prelude::predicate;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const TEXT_XML: &[u8] = b"<?xml version=\"1.0\"?>\r\n<Loadout ship=\"Gladius\" />\r\n";

fn vehicle() -> Vec<u8> {
    TestElement::new("Vehicle")
        .attribute("name", "AEGS_Gladius")
        .child(TestElement::new("Parts").child(TestElement::new("Part").attribute("name", "Nose")))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn broken() -> Vec<u8> {
    let mut data = vehicle();
    data.truncate(30);
    data
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: PathBuf,
    out: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Vehicles\\gladius.xml", vehicle()).encrypted())
        .entry(TestEntry::deflated(
            "Data\\Loadouts\\gladius.xml",
            TEXT_XML.to_vec(),
        ))
        .entry(TestEntry::stored("Data\\Broken\\broken.xml", broken()))
        .entry(TestEntry::stored("Data\\readme.txt", b"hello".to_vec()))
        .write_to(&archive)?;
    let out = dir.path().join("out");
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn extract(fixture: &Fixture) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .arg("extract")
        .arg(&fixture.archive)
        .args(["--filter", "Data\\"])
        .arg("--out")
        .arg(&fixture.out);
    Ok(command)
}

fn read(fixture: &Fixture, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(fixture.out.join(path))?)
}

#[test]
fn convert_writes_cryxml_entries_as_xml() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .arg("--convert")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "extracted 3 (1 converted), skipped 0, failed 1, cancelled 0",
        ));

    insta::assert_snapshot!(String::from_utf8(read(
        &fixture,
        "Data/Vehicles/gladius.xml"
    )?)?);
    let receipt = converted_receipt(&fixture)?;
    let element = receipt
        .iter()
        .find(|row| row["address"]["field"] == "elementName")
        .ok_or("a converted element name")?;
    assert_eq!(element["address"]["type"], "xmlBytes", "a field address");
    assert_eq!(element["containsEncryptedSource"], true, "encrypted Entry");
    assert_eq!(element["sourceSetComplete"], true, "original contributors");
    Ok(())
}

fn converted_receipt(fixture: &Fixture) -> Result<Vec<Value>, Box<dyn Error>> {
    let bytes = read(fixture, "Data/Vehicles/gladius.xml.p4kx-origin.jsonl")?;
    Ok(std::str::from_utf8(&bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

#[test]
fn convert_keeps_text_xml_and_other_entries_byte_identical() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?.arg("--convert").assert().code(1);

    assert_eq!(
        read(&fixture, "Data/Loadouts/gladius.xml")?,
        TEXT_XML,
        "text XML"
    );
    assert_eq!(read(&fixture, "Data/readme.txt")?, b"hello", "other Entry");
    Ok(())
}

#[test]
fn an_entry_that_fails_to_convert_is_reported_with_its_reason() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .arg("--convert")
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "failed Data\\Broken\\broken.xml: cannot convert the CryXml: ",
        ));

    assert!(
        !fixture.out.join("Data/Broken/broken.xml").exists(),
        "no file for the failed Entry"
    );
    Ok(())
}

#[test]
fn without_convert_the_output_is_unchanged() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "extracted 4, skipped 0, failed 0, cancelled 0",
        ));

    assert_eq!(
        read(&fixture, "Data/Vehicles/gladius.xml")?,
        vehicle(),
        "raw"
    );
    assert_eq!(read(&fixture, "Data/Broken/broken.xml")?, broken(), "raw");
    Ok(())
}

#[test]
fn if_changed_keeps_converted_files_of_an_earlier_run() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture)?.arg("--convert").assert().code(1);
    let original_receipt = converted_receipt(&fixture)?;

    extract(&fixture)?
        .args(["--convert", "--conflict", "if-changed"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "skipped Data\\Vehicles\\gladius.xml: unchanged",
        ))
        .stdout(predicate::str::contains(
            "extracted 0 (0 converted), skipped 3, failed 1, cancelled 0",
        ));
    assert_eq!(
        converted_receipt(&fixture)?,
        original_receipt,
        "unchanged XML retains its existing bound receipt"
    );
    Ok(())
}

#[test]
fn if_changed_converts_raw_files_of_a_run_without_conversion() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture)?.assert().code(0);

    extract(&fixture)?
        .args(["--convert", "--conflict", "if-changed"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "extracted 1 (1 converted), skipped 2, failed 1, cancelled 0",
        ));

    assert_eq!(
        read(&fixture, "Data/Broken/broken.xml")?,
        broken(),
        "a failed Conversion keeps the existing file"
    );
    assert!(
        read(&fixture, "Data/Vehicles/gladius.xml")?.starts_with(b"<?xml"),
        "converted"
    );
    let receipt = converted_receipt(&fixture)?;
    assert!(
        receipt
            .iter()
            .any(|row| row["address"]["type"] == "xmlBytes"),
        "the converted XML has its own field receipt"
    );
    assert!(
        !receipt
            .iter()
            .any(|row| row["address"]["type"] == "opaquePayload"),
        "the old proof of the binary payload was retired"
    );
    Ok(())
}
