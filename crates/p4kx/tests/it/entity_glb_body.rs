// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx entity glb` says which body the clothing of a character was chosen for and which head
//! names it.

use std::error::Error;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestModel, TestPort, entity_schema,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn archive(dir: &std::path::Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let head = TestEntity::new(0x2000, "TEST_Head", "entities/test_head.xml")
        .attach_type("Char_Head")
        .sub_type("Female")
        .geometry("Objects/Test/cube.cgf", None);
    let jacket = TestEntity::new(0x3000, "TEST_Jacket", "entities/test_jacket.xml")
        .attach_type("Char_Clothing_Torso_1")
        .geometry("Objects/Test/cube.cgf", None)
        .geometry_variant("Female", "Objects/Test/cube.cgf", None);
    let woman = TestEntity::new(0x1000, "TEST_Woman", "entities/test_woman.xml")
        .port(TestPort::new("head").default_item(0x2000))
        .port(TestPort::new("torso").default_item(0x3000));
    let database = entity_schema(DataForgeBuilder::new())
        .record(head.record())
        .record(jacket.record())
        .record(woman.record());
    let model = TestModel::cube();
    let path = dir.join("Data.p4k");
    [
        TestEntry::zstd("Data\\Game2.dcb", database.build()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgf", model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgfm", model.cgfm()),
    ]
    .into_iter()
    .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
    .write_to(&path)?;
    Ok(path)
}

#[test]
fn the_report_names_the_body_and_the_head_that_chose_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("woman.glb");

    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "glb"])
        .arg(&archive)
        .arg("TEST_Woman")
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["body"]["variant"], "female");
    assert_eq!(report["body"]["head"], "TEST_Head");
    assert_eq!(report["body"]["named_by"], "TEST_Head");
    assert_eq!(report["body"]["named_by_head"], true);
    Ok(())
}

#[test]
fn the_text_report_says_the_same() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("woman.glb");

    let output = p4kx_testkit::p4kx()?
        .args(["entity", "glb"])
        .arg(&archive)
        .arg("TEST_Woman")
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.lines()
            .any(|line| line == "body female, named by the head TEST_Head"),
        "{text}"
    );
    Ok(())
}
