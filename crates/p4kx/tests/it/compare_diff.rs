// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx compare --diff` end to end over a pair of synthetic Archives.

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const OLD: &str = "LIVE/Data.p4k";
const NEW: &str = "PTU/Data.p4k";

fn write(dir: &Path, archive: &str, builder: &ArchiveBuilder) -> TestResult {
    let path = dir.join(archive);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    builder.write_to(path)?;
    Ok(())
}

fn vehicle(damage: &str) -> TestElement {
    TestElement::new("Vehicle")
        .attribute("name", "AEGS_Gladius")
        .child(
            TestElement::new("Parts")
                .child(TestElement::new("Part").attribute("name", "Hull"))
                .child(TestElement::new("Part").attribute("damageMax", damage)),
        )
        .child(TestElement::new("Description").content("Light fighter"))
}

const OLD_CONFIG: &str = "r_Width=1920\nr_Height=1080\nr_Fullscreen=1\nsys_spec=3\n\
    g_language=english\ncl_fov=90\nhud_color=amber\nmouse_sensitivity=0.5\n";

fn pair() -> Result<tempfile::TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        OLD,
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("Data\\user.cfg", OLD_CONFIG))
            .entry(TestEntry::stored(
                "Data\\Libs\\text.xml",
                "<Items>\n  <Item name=\"a\" mass=\"1\"/>\n  <Item name=\"b\"/>\n</Items>\n",
            ))
            .entry(TestEntry::deflated(
                "Data\\Libs\\vehicle.xml",
                vehicle("2500").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
            ))
            .entry(TestEntry::stored("Data\\removed.txt", "first\nlast"))
            .entry(TestEntry::stored(
                "Data\\icon.dds",
                b"DDS \0\x01\x02".to_vec(),
            ))
            .entry(TestEntry::stored("Data\\same.cfg", "a=1\n"))
            .entry(TestEntry::stored("Data\\same.dds", b"DDS \0".to_vec()))
            .entry(TestEntry::stored(
                "Data\\reencoded.xml",
                vehicle("1").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
            )),
    )?;
    write(
        dir.path(),
        NEW,
        &ArchiveBuilder::v2()
            .entry(TestEntry::zstd(
                "Data\\user.cfg",
                OLD_CONFIG
                    .replace("sys_spec=3\n", "sys_spec=4\n")
                    .replace("hud_color=amber\n", ""),
            ))
            .entry(TestEntry::stored(
                "Data\\Libs\\text.xml",
                "<Items>\n  <Item name=\"a\" mass=\"2\"/>\n  <Item name=\"b\"/>\n  <Item name=\"c\"/>\n</Items>\n",
            ))
            .entry(TestEntry::stored(
                "Data\\Libs\\vehicle.xml",
                vehicle("3000").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
            ))
            .entry(TestEntry::stored("Data\\added.ini", "[section]\nkey=value\n"))
            .entry(TestEntry::stored("Data\\icon.dds", b"DDS \0\x03\x04".to_vec()))
            .entry(TestEntry::stored("Data\\same.cfg", "a=1\n"))
            .entry(TestEntry::stored("Data\\same.dds", b"DDS \0".to_vec()))
            .entry(TestEntry::stored(
                "Data\\reencoded.xml",
                vehicle("1").to_cryxml(CryXmlSignature::CryXml, CryXmlByteOrder::Big),
            )),
    )?;
    Ok(dir)
}

fn diff(dir: &Path, entry: &str, json: bool, code: i32) -> Result<String, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command.current_dir(dir);
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["compare", OLD, NEW, "--diff", entry])
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn compatible_json(dir: &Path, entry: &str) -> Result<String, Box<dyn Error>> {
    let stdout = diff(dir, entry, true, 1)?;
    let mut report: Value = serde_json::from_str(&stdout)?;
    let fields = report.as_object_mut().ok_or("line diff report object")?;
    let origins = fields
        .remove("fieldOrigins")
        .ok_or("line diff per-field origins")?;
    let provenance = fields
        .remove("provenance")
        .ok_or("line diff source inventory")?;
    for key in [
        "/path", "/change", "/differs", "/content", "/old", "/new", "/hunks",
    ] {
        assert!(origins.get(key).is_some(), "missing {key} origin");
    }
    assert!(
        provenance
            .get("uses")
            .and_then(Value::as_array)
            .is_some_and(|rows| !rows.is_empty()),
        "physical origin uses"
    );
    let (prefix, _) = stdout
        .split_once(",\"fieldOrigins\":")
        .ok_or("additive origin envelope")?;
    let original = format!("{prefix}}}\n");
    assert_eq!(
        serde_json::from_str::<Value>(&original)?,
        report,
        "the frozen payload includes every original output field"
    );
    Ok(original)
}

#[test]
fn a_text_entry_prints_a_unified_diff() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(diff(dir.path(), "data/USER.cfg", false, 1)?);
    Ok(())
}

#[test]
fn a_text_xml_entry_prints_a_unified_diff() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(diff(dir.path(), "Data\\Libs\\text.xml", false, 1)?);
    Ok(())
}

#[test]
fn a_cryxml_entry_is_converted_before_diffing() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(diff(dir.path(), "Data\\Libs\\vehicle.xml", false, 1)?);
    Ok(())
}

#[test]
fn an_added_entry_is_diffed_against_an_empty_side() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(diff(dir.path(), "Data\\added.ini", false, 1)?);
    Ok(())
}

#[test]
fn a_removed_entry_is_diffed_against_an_empty_side() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(diff(dir.path(), "Data\\removed.txt", false, 1)?);
    Ok(())
}

#[test]
fn a_binary_entry_says_it_is_binary_and_changed() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(diff(dir.path(), "Data\\icon.dds", false, 1)?);
    Ok(())
}

#[test]
fn an_unchanged_entry_prints_nothing_and_exits_with_code_0() -> TestResult {
    let dir = pair()?;

    assert_eq!(
        diff(dir.path(), "Data\\same.cfg", false, 0)?,
        "",
        "no output"
    );
    Ok(())
}

#[test]
fn an_unchanged_binary_entry_says_so_and_exits_with_code_0() -> TestResult {
    let dir = pair()?;

    assert_eq!(
        diff(dir.path(), "Data\\same.dds", false, 0)?,
        "Data\\same.dds is binary; it is unchanged\n",
        "the note"
    );
    Ok(())
}

#[test]
fn a_cryxml_entry_with_the_same_xml_says_so_and_exits_with_code_1() -> TestResult {
    let dir = pair()?;

    assert_eq!(
        diff(dir.path(), "Data\\reencoded.xml", false, 1)?,
        "Data\\reencoded.xml changed, but its text is the same line by line\n",
        "the note"
    );
    Ok(())
}

#[test]
fn json_of_a_removed_entry_has_no_new_side() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(compatible_json(dir.path(), "Data\\removed.txt")?);
    Ok(())
}

#[test]
fn json_gives_structured_hunks() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(compatible_json(dir.path(), "Data\\Libs\\text.xml")?);
    Ok(())
}

#[test]
fn json_of_a_binary_entry_has_no_hunks() -> TestResult {
    let dir = pair()?;

    insta::assert_snapshot!(compatible_json(dir.path(), "Data\\icon.dds")?);
    Ok(())
}

#[test]
fn json_of_an_added_cryxml_entry_names_its_encoding() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        OLD,
        &ArchiveBuilder::new().entry(TestEntry::stored("other.txt", "x")),
    )?;
    write(
        dir.path(),
        NEW,
        &ArchiveBuilder::v2().entry(TestEntry::stored(
            "v.xml",
            TestElement::new("a")
                .attribute("b", "1")
                .to_cryxml(CryXmlSignature::Cry3Sdk, CryXmlByteOrder::Big),
        )),
    )?;

    insta::assert_snapshot!(compatible_json(dir.path(), "v.xml")?);
    Ok(())
}

#[test]
fn an_entry_in_neither_archive_exits_with_code_2() -> TestResult {
    let dir = pair()?;

    diff(dir.path(), "Data\\missing.txt", false, 2)?;
    Ok(())
}

#[test]
fn corrupt_cryxml_exits_with_code_3() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bytes = vehicle("1").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let cut = bytes.get(..bytes.len() / 2).ok_or("short")?.to_vec();
    write(
        dir.path(),
        OLD,
        &ArchiveBuilder::new().entry(TestEntry::stored("v.xml", bytes)),
    )?;
    write(
        dir.path(),
        NEW,
        &ArchiveBuilder::v2().entry(TestEntry::stored("v.xml", cut)),
    )?;

    diff(dir.path(), "v.xml", false, 3)?;
    Ok(())
}

#[test]
fn diff_cannot_be_combined_with_a_filter() -> TestResult {
    let dir = pair()?;

    p4kx_testkit::p4kx()?
        .current_dir(dir.path())
        .args(["compare", OLD, NEW, "--diff", "a", "--filter", "b"])
        .assert()
        .code(2);
    Ok(())
}
