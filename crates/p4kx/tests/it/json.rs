// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The `--json` output of every `p4kx` command, as snapshots.

use std::error::Error;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, SplitDds, TestElement, TestEntry,
    TestTextureFormat, write_build_manifest,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const TEXTURE: &str = "Data\\Textures\\hull_ddna.dds";

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            root: tempfile::tempdir()?,
        })
    }

    fn dir(&self) -> &Path {
        self.root.path()
    }

    fn channel(&self, name: &str, with_build: bool) -> Result<PathBuf, Box<dyn Error>> {
        let folder = self.dir().join("library").join(name);
        std::fs::create_dir_all(&folder)?;
        let archive = folder.join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data\\readme.txt", b"hello".to_vec()))
            .entry(TestEntry::deflated("Data\\Libs\\a.xml", b"<a/>".to_vec()))
            .entry(TestEntry::zstd("Data\\Libs\\secret.xml", vec![b'x'; 100]).encrypted())
            .entry(
                TestEntry::deflated("Data\\broken.xml", b"<broken/>".to_vec())
                    .with_payload(vec![0xFF; 8]),
            )
            .write_to(&archive)?;
        if with_build {
            write_build_manifest(
                &folder,
                "sc-alpha-4.11.0",
                "4.11.42.1234",
                "12999999",
                "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0",
            )?;
        }
        Ok(archive)
    }

    fn texture_archive(&self) -> Result<PathBuf, Box<dyn Error>> {
        let archive = self.dir().join("Data.p4k");
        SplitDds::new(TestTextureFormat::BC7_SRGB, 64, 32)
            .split(4)
            .entries(TEXTURE)
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&archive)?;
        Ok(archive)
    }

    fn p4kx(&self) -> Result<Command, Box<dyn Error>> {
        let mut command = p4kx_testkit::p4kx()?;
        command
            .env("P4KX_CONFIG_DIR", self.dir().join("config"))
            .env("P4KX_LAUNCHER_DIR", self.dir().join("rsilauncher"))
            .env("P4KX_DEFAULT_LIBRARY", self.dir().join("library"))
            .arg("--json");
        Ok(command)
    }

    fn json(&self, command: &mut Command, code: i32) -> Result<String, Box<dyn Error>> {
        let output = command.output()?;
        assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
        let stdout = String::from_utf8(output.stdout)?;
        assert_eq!(stdout.lines().count(), 1, "one line of JSON: {stdout}");
        let mut value: Value = serde_json::from_str(&stdout)?;
        redact(&mut value, &self.dir().display().to_string());
        Ok(serde_json::to_string_pretty(&value)?)
    }
}

fn checked_entry_snapshot(text: &str) -> Result<String, Box<dyn Error>> {
    let mut value = serde_json::from_str(text)?;
    crate::entry_listing_origins::remove_checked_origins_for_snapshot(&mut value)?;
    Ok(serde_json::to_string_pretty(&value)?)
}

fn redact(value: &mut Value, dir: &str) {
    match value {
        Value::String(text) => {
            if let Some(rest) = text.strip_prefix(dir) {
                *text = format!("[DIR]{}", rest.replace('\\', "/"));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact(item, dir)),
        Value::Object(map) => map.values_mut().for_each(|item| redact(item, dir)),
        _ => {}
    }
}

#[test]
fn list() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;

    let json = fixture.json(fixture.p4kx()?.arg("list").arg(&archive), 0)?;
    let json = checked_entry_snapshot(&json)?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn search() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .arg("search")
            .arg(&archive)
            .arg("*libs/*.XML"),
        0,
    )?;

    let json = checked_entry_snapshot(&json)?;
    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn search_without_a_match() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;

    let json = fixture.json(
        fixture.p4kx()?.arg("search").arg(&archive).arg("missing"),
        0,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn json_may_follow_the_subcommand() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;
    let output = p4kx_testkit::p4kx()?
        .arg("info")
        .arg(&archive)
        .arg("--json")
        .output()?;

    assert!(output.status.success(), "info succeeds: {output:?}");
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["entry_count"], 4, "JSON after the subcommand");
    Ok(())
}

#[test]
fn info_with_a_build() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("PTU", true)?;

    let json = fixture.json(fixture.p4kx()?.arg("info").arg(&archive), 0)?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn info_without_a_build() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;

    let json = fixture.json(fixture.p4kx()?.arg("info").arg(&archive), 0)?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn channels() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.channel("LIVE", true)?;
    fixture.channel("PTU", false)?;

    let json = fixture.json(fixture.p4kx()?.arg("channels"), 0)?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn channels_add_and_remove() -> TestResult {
    let fixture = Fixture::new()?;
    let folder = fixture.dir().join("Games").join("StarCitizen");
    std::fs::create_dir_all(&folder)?;
    let archive = fixture.channel("EPTU", false)?;
    let mut steps = Vec::new();

    for (args, code) in [
        (vec!["add", &*folder.to_string_lossy()], 0),
        (vec!["add", &*folder.to_string_lossy()], 0),
        (vec!["add", &*archive.to_string_lossy()], 0),
        (vec!["remove", &*folder.to_string_lossy()], 0),
    ] {
        steps.push(fixture.json(fixture.p4kx()?.arg("channels").args(args), code)?);
    }

    insta::assert_snapshot!(steps.join("\n"));
    Ok(())
}

#[test]
fn extract() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;
    let out = fixture.dir().join("out");
    std::fs::create_dir_all(out.join("Data"))?;
    std::fs::write(out.join("Data").join("readme.txt"), "mine")?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .arg("extract")
            .arg(&archive)
            .args(["Data\\readme.txt", "Data/Libs/a.xml", "Data\\broken.xml"])
            .arg("--out")
            .arg(&out),
        1,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn extract_with_a_filter() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.channel("LIVE", false)?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .arg("extract")
            .arg(&archive)
            .args(["--filter", "*.xml", "--out"])
            .arg(fixture.dir().join("out")),
        1,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn extract_with_conversion() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.dir().join("Data.p4k");
    let vehicle = TestElement::new("Vehicle")
        .attribute("name", "AEGS_Gladius")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\vehicle.xml", vehicle.clone()))
        .entry(TestEntry::stored("Data\\text.xml", b"<a/>".to_vec()))
        .entry(TestEntry::stored(
            "Data\\broken.xml",
            vehicle.get(..20).unwrap_or_default().to_vec(),
        ))
        .write_to(&archive)?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .arg("extract")
            .arg(&archive)
            .args(["--filter", "*.xml", "--convert", "--out"])
            .arg(fixture.dir().join("out")),
        1,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn cryxml() -> TestResult {
    let fixture = Fixture::new()?;
    let input = fixture.dir().join("vehicle.xml");
    std::fs::write(
        &input,
        TestElement::new("Vehicle")
            .attribute("name", "AEGS_Gladius")
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    )?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .arg("cryxml")
            .arg(&input)
            .arg("--out")
            .arg(fixture.dir().join("converted.xml")),
        0,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn texture_info() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.texture_archive()?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .args(["texture", "info"])
            .arg(&archive)
            .arg(TEXTURE),
        0,
    )?;

    let mut value: Value = serde_json::from_str(&json)?;
    let object = value.as_object_mut().ok_or("texture info JSON object")?;
    let fields = object
        .remove("field_origins")
        .ok_or("texture field origins")?;
    let source = object
        .remove("provenance")
        .ok_or("texture source inventory")?;
    assert!(
        fields.get("/source_metadata/raw/legacy/width").is_some(),
        "authored width has exact evidence"
    );
    assert!(
        source
            .get("uses")
            .and_then(Value::as_array)
            .is_some_and(|uses| !uses.is_empty()),
        "field uses resolve in the shared inventory"
    );
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn texture_merge() -> TestResult {
    let fixture = Fixture::new()?;
    let archive = fixture.texture_archive()?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .args(["texture", "merge"])
            .arg(&archive)
            .arg(TEXTURE)
            .arg("--out")
            .arg(fixture.dir().join("hull_ddna.dds")),
        0,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn texture_png() -> TestResult {
    let fixture = Fixture::new()?;
    let header = SplitDds::new(TestTextureFormat::LegacyDxt1, 64, 32)
        .split(4)
        .write_files(fixture.dir(), "hull_diff.dds")?;

    let json = fixture.json(
        fixture
            .p4kx()?
            .args(["texture", "png"])
            .arg(&header)
            .arg("--out")
            .arg(fixture.dir().join("hull_diff.png"))
            .args(["--mip", "1"]),
        0,
    )?;

    insta::assert_snapshot!(json);
    Ok(())
}
