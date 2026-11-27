// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb tags` and `p4kx dcb export --tags` over a synthetic DataForge: the tag tree as a
//! table and as JSON, flat and nested, one tag with everything below it, invalid input, and the
//! tag paths in exported Records.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const DATABASE: u128 = 0x0d;
const WEAPON: u128 = 0xa0;
const GUN: u128 = 0xa1;
const MISSILE: u128 = 0xa2;
const ARMOUR: u128 = 0xb0;
const ITEM: u128 = 0x100;
const PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";

fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().map(|g| TestValue::Reference(*g)).collect())
}

fn tag(guid: u128, name: &str, legacy: u32, children: &[u128]) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(legacy))
        .value("children", references(children))
}

fn text(guid: u128) -> String {
    let hex = format!("{guid:032x}");
    let part = |from: usize, to: usize| hex.get(from..to).unwrap_or_default().to_owned();
    format!(
        "{}-{}-{}-{}-{}",
        part(0, 8),
        part(8, 12),
        part(12, 16),
        part(16, 20),
        part(20, 32)
    )
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("Item").array("requiredTags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(
            TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", PATH)
                .value("tags", references(&[WEAPON, ARMOUR])),
        )
        .record(tag(WEAPON, "Weapon", 7, &[GUN, MISSILE]))
        .record(tag(GUN, "Gun", u32::MAX, &[]))
        .record(tag(MISSILE, "Missile", 9, &[]))
        .record(tag(ARMOUR, "Armour", 4, &[]))
        .record(
            TestRecord::new(ITEM, "Item", "Item.Sample", "libs/foundry/records/item.xml")
                .value("requiredTags", references(&[GUN, MISSILE])),
        )
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

struct Run {
    stdout: String,
    stderr: String,
}

fn run(command: &str, archive: &Path, args: &[&str], code: i32) -> Result<Run, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", command])
        .arg(archive)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
    })
}

fn json(command: &str, archive: &Path, args: &[&str]) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(
        &run(command, archive, args, 0)?.stdout,
    )?)
}

fn archive() -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    Ok((dir, archive))
}

/// Returns the rows of a table, without its heading and rule, with runs of spaces made one.
fn rows(run: &Run) -> Vec<String> {
    run.stdout
        .lines()
        .skip(2)
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

#[test]
fn tags_lists_every_tag_with_its_path_in_tree_order() -> TestResult {
    let (_dir, archive) = archive()?;

    let run = run("tags", &archive, &[], 0)?;

    assert_eq!(
        rows(&run),
        [
            format!("Weapon {} 7", text(WEAPON)),
            format!("Weapon.Gun {}", text(GUN)),
            format!("Weapon.Missile {} 9", text(MISSILE)),
            format!("Armour {} 4", text(ARMOUR)),
            "4 of 4 tags".to_owned(),
        ],
        "full tag paths, GUIDs and legacy ids; 4294967295 is no legacy id"
    );
    assert!(
        run.stdout.starts_with("tag path"),
        "heading: {}",
        run.stdout
    );
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    Ok(())
}

#[test]
fn tags_tree_indents_each_tag_below_its_parent() -> TestResult {
    let (_dir, archive) = archive()?;

    let run = run("tags", &archive, &["--tree"], 0)?;

    assert!(
        run.stdout.contains("\nWeapon  "),
        "a root is not indented: {}",
        run.stdout
    );
    assert!(
        run.stdout.contains("\n  Gun  "),
        "a child is indented by two spaces: {}",
        run.stdout
    );
    assert!(run.stdout.starts_with("tag  "), "heading: {}", run.stdout);
    Ok(())
}

#[test]
fn tags_with_a_tag_lists_it_and_everything_below_it() -> TestResult {
    let (_dir, archive) = archive()?;

    let by_path = run("tags", &archive, &["weapon"], 0)?;
    let by_guid = run("tags", &archive, &[&text(MISSILE)], 0)?;

    assert_eq!(
        rows(&by_path),
        [
            format!("Weapon {} 7", text(WEAPON)),
            format!("Weapon.Gun {}", text(GUN)),
            format!("Weapon.Missile {} 9", text(MISSILE)),
            "3 of 4 tags".to_owned(),
        ],
        "a tag path is matched ignoring case"
    );
    assert_eq!(
        rows(&by_guid),
        [
            format!("Weapon.Missile {} 9", text(MISSILE)),
            "1 of 4 tags".to_owned(),
        ],
        "a GUID names one tag"
    );
    Ok(())
}

#[test]
fn tags_json_is_the_flat_list_with_parent_and_children() -> TestResult {
    let (_dir, archive) = archive()?;

    let value = json("tags", &archive, &["--json"])?;

    assert_eq!(value["tag_count"], 4, "{value}");
    assert_eq!(value["shown_count"], 4);
    assert_eq!(value["tree"], false);
    assert_eq!(
        value["issues"],
        serde_json::json!({"unclassified": 0, "unreadable": 0, "dangling": 0, "repeated": 0, "too_deep": 0, "too_long": 0, "duplicate": 0, "unlisted": 0})
    );
    let tags = value["tags"].as_array().ok_or("no tags")?;
    assert_eq!(tags.len(), 4);
    assert_eq!(tags[0]["path"], "Weapon");
    assert_eq!(tags[0]["legacy_id"], 7);
    assert_eq!(tags[0]["parent"], Value::Null, "a root has no parent");
    assert_eq!(
        tags[0]["children"],
        serde_json::json!([text(GUN), text(MISSILE)])
    );
    assert_eq!(tags[1]["name"], "Gun");
    assert_eq!(tags[1]["path"], "Weapon.Gun");
    assert_eq!(tags[1]["legacy_id"], Value::Null, "4294967295 is none");
    assert_eq!(tags[1]["parent"], text(WEAPON));
    assert_eq!(tags[1]["depth"], 1);
    Ok(())
}

#[test]
fn tags_tree_json_nests_the_children() -> TestResult {
    let (_dir, archive) = archive()?;

    let value = json("tags", &archive, &["--tree", "--format", "json"])?;

    assert_eq!(value["tree"], true, "{value}");
    let roots = value["tags"].as_array().ok_or("no tags")?;
    assert_eq!(roots.len(), 2, "two roots");
    assert_eq!(roots[0]["path"], "Weapon");
    assert_eq!(roots[0]["children"][1]["path"], "Weapon.Missile");
    assert_eq!(roots[0]["children"][1]["children"], serde_json::json!([]));
    assert_eq!(roots[1]["path"], "Armour");
    Ok(())
}

#[test]
fn tags_rejects_an_unknown_tag_and_json_with_a_table() -> TestResult {
    let (_dir, archive) = archive()?;

    let unknown = run("tags", &archive, &["Weapon.Sword"], 2)?;
    let table = run("tags", &archive, &["--json", "--format", "table"], 2)?;

    assert!(
        unknown
            .stderr
            .contains("no tag has the tag path or GUID `Weapon.Sword`"),
        "{}",
        unknown.stderr
    );
    assert!(
        table.stderr.contains("--json writes JSON"),
        "{}",
        table.stderr
    );
    Ok(())
}

#[test]
fn export_with_tags_writes_the_tag_path_next_to_each_tag_reference() -> TestResult {
    let (dir, archive) = archive()?;
    let plain = dir.path().join("plain");
    let annotated = dir.path().join("annotated");
    let out = |target: &Path| target.to_string_lossy().into_owned();

    run(
        "export",
        &archive,
        &["--filter", "item", "--out", &out(&plain)],
        0,
    )?;
    let report = json(
        "export",
        &archive,
        &[
            "--filter",
            "item",
            "--tags",
            "--out",
            &out(&annotated),
            "--json",
        ],
    )?;

    let file = "libs/foundry/records/item.xml";
    let plain_xml = fs::read_to_string(plain.join(file))?;
    let annotated_xml = fs::read_to_string(annotated.join(file))?;
    assert!(
        !plain_xml.contains(" tag="),
        "no tag path by default: {plain_xml}"
    );
    assert!(
        annotated_xml.contains("type=\"Tag\" tag=\"Weapon.Gun\"/>")
            && annotated_xml.contains("type=\"Tag\" tag=\"Weapon.Missile\"/>"),
        "{annotated_xml}"
    );
    assert_eq!(
        report["tags"], true,
        "the report says tag paths were written"
    );
    Ok(())
}

#[path = "dcb_tags_deep.rs"]
mod deep;

#[path = "dcb_tags_origins.rs"]
mod origins;
