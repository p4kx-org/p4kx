// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract --convert` over a synthetic Archive with split DDS textures.

use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use predicates::prelude::{PredicateBooleanExt, predicate};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const PANEL: &str = "Data\\Textures\\panel_diff.dds";
const BROKEN: &str = "Data\\Textures\\broken_ddna.dds";

fn panel() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 8).split(2)
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: PathBuf,
    out: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let mut broken = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .split(1)
        .entries(BROKEN);
    broken.truncate(1);
    panel()
        .entries(PANEL)
        .into_iter()
        .chain(broken)
        .chain([TestEntry::stored("Data\\readme.txt", b"hello".to_vec())])
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
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

fn files(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push(relative);
            }
        }
    }
    found.sort();
    Ok(found)
}

fn png_size(path: &Path) -> Result<(u32, u32), Box<dyn Error>> {
    let reader = png::Decoder::new(std::io::BufReader::new(File::open(path)?)).read_info()?;
    let info = reader.info();
    Ok((info.width, info.height))
}

fn merged(dds: &SplitDds) -> Vec<u8> {
    let mut merged = dds
        .header_bytes()
        .get(..dds.header_len())
        .unwrap_or_default()
        .to_vec();
    for level in 0..5 {
        merged.extend_from_slice(&dds.mip(level));
    }
    merged
}

#[test]
fn convert_merges_textures_and_exports_png_once_per_texture() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .arg("--convert")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "extracted 5 (3 converted, 1 not converted), skipped 0, failed 0, cancelled 0",
        ))
        .stderr(predicate::str::contains(format!(
            "not converted {BROKEN}: cannot convert the texture: {BROKEN}.1 is missing"
        )));

    assert_eq!(
        files(&fixture.out)?,
        [
            "Data/Textures/broken_ddna.dds",
            "Data/Textures/panel_diff.dds",
            "Data/Textures/panel_diff.png",
            "Data/readme.txt",
        ],
        "one DDS and one PNG for the texture, the raw header of the broken one"
    );
    assert_eq!(
        fs::read(fixture.out.join("Data/Textures/panel_diff.dds"))?,
        merged(&panel()),
        "merged DDS"
    );
    assert_eq!(
        png_size(&fixture.out.join("Data/Textures/panel_diff.png"))?,
        (16, 8),
        "the largest mip"
    );
    Ok(())
}

#[test]
fn textures_and_mip_choose_the_outputs() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .args(["--convert", "--textures", "png,raw", "--mip", "1"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "extracted 5 (4 converted), skipped 0, failed 0, cancelled 0",
        ));

    assert_eq!(
        files(&fixture.out)?,
        [
            "Data/Textures/broken_ddna.dds",
            "Data/Textures/broken_ddna.png",
            "Data/Textures/panel_diff.dds",
            "Data/Textures/panel_diff.dds.1",
            "Data/Textures/panel_diff.dds.2",
            "Data/Textures/panel_diff.png",
            "Data/readme.txt",
        ],
        "the raw Entries and the PNGs; mip 1 of the broken texture needs no part"
    );
    assert_eq!(
        fs::read(fixture.out.join("Data/Textures/panel_diff.dds"))?,
        panel().header_bytes(),
        "the raw header"
    );
    assert_eq!(
        png_size(&fixture.out.join("Data/Textures/panel_diff.png"))?,
        (8, 4),
        "mip 1"
    );
    Ok(())
}

#[test]
fn textures_need_convert() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .args(["--textures", "merge"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--convert"));
    Ok(())
}

#[test]
fn the_json_report_lists_textures_that_were_not_converted() -> TestResult {
    let fixture = fixture()?;

    let output = extract(&fixture)?
        .args(["--convert", "--json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let report: Value = serde_json::from_slice(&output)?;
    assert_eq!(
        report["converted"], 3,
        "the Entries of the converted texture"
    );
    assert_eq!(
        report["unconverted"][0]["path"], BROKEN,
        "the broken texture"
    );
    assert_eq!(
        report["unconverted"][0]["error"],
        format!("cannot convert the texture: {BROKEN}.1 is missing"),
        "and why"
    );
    Ok(())
}

#[test]
fn without_convert_textures_are_written_raw() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .assert()
        .code(0)
        .stdout(predicate::str::contains("converted").not());

    assert_eq!(files(&fixture.out)?.len(), 5, "the raw Entries");
    Ok(())
}

#[test]
fn convert_merges_an_alpha_tail_into_the_png_and_writes_cubemap_faces() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let tailed = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .split(1)
        .alpha(TestTextureFormat::R8);
    let cube = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .cubemap()
        .split(1);
    tailed
        .entries("Data\\glass.dds")
        .into_iter()
        .chain(cube.entries("Data\\sky.dds"))
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive)?;
    let out = dir.path().join("out");

    p4kx_testkit::p4kx()?
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "Data\\", "--convert", "--out"])
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("extracted 6 (6 converted)"));

    let mut expected = vec![
        "Data/glass.dds".to_owned(),
        "Data/glass.png".to_owned(),
        "Data/sky.dds".to_owned(),
    ];
    expected.extend(
        ["posx", "negx", "posy", "negy", "posz", "negz"]
            .iter()
            .map(|face| format!("Data/sky_{face}.png")),
    );
    expected.sort();
    assert_eq!(files(&out)?, expected, "no tail Entries, six cubemap faces");
    let alpha = png_alpha(&out.join("Data/glass.png"))?;
    let tail = tailed.alpha_tail().ok_or("a tail")?;
    assert_eq!(alpha, tail.mip(0), "the PNG's alpha is the tail");
    Ok(())
}

fn png_alpha(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut reader = png::Decoder::new(std::io::BufReader::new(File::open(path)?)).read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    reader.next_frame(&mut pixels)?;
    Ok(pixels.iter().skip(3).step_by(4).copied().collect())
}

#[test]
fn converted_array_images_bind_their_own_colour_and_alpha_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let format = TestTextureFormat::Dx10Pixel {
        dxgi_format: 28,
        pixel_bytes: 4,
    };
    let dds = SplitDds::new(format, 8, 8)
        .array(2)
        .split(2)
        .alpha(TestTextureFormat::R8);
    dds.entries("Data/glass.dds")
        .into_iter()
        .map(|entry| match entry.path() {
            "Data/glass.dds.1" | "Data/glass.dds.2a" => entry.encrypted(),
            _ => entry,
        })
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive)?;
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "extract"])
        .arg(&archive)
        .args(["--filter", "Data/", "--convert", "--out"])
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let textures = report["texture_artifacts"]
        .as_array()
        .ok_or("a receipt for every converted texture")?;
    let [texture] = textures.as_slice() else {
        return Err("one converted texture".into());
    };
    let images = texture["images"].as_array().ok_or("converted images")?;
    assert_eq!(images.len(), 2, "a receipt for each array layer");
    let first = check_image_receipt(&out, texture, &images[0], "Data/glass_layer0.png")?;
    let second = check_image_receipt(&out, texture, &images[1], "Data/glass_layer1.png")?;
    assert_eq!(first, [0, 0], "first layer offsets in color and alpha");
    assert_eq!(second, [256, 64], "second layer offsets in color and alpha");
    Ok(())
}

fn check_image_receipt(
    out: &Path,
    texture: &Value,
    image: &Value,
    path: &str,
) -> Result<Vec<u64>, Box<dyn Error>> {
    let bytes = fs::read(out.join(path))?;
    let mut digest = p4kx_extraction::OriginDigestWriter::new(std::io::sink());
    std::io::Write::write_all(&mut digest, &bytes)?;
    let digest = digest.finish()?;
    assert_eq!(image["path"], path, "target-relative receipt path");
    assert_eq!(
        image["digest"],
        serde_json::to_value(digest)?,
        "digest binds the written bytes"
    );
    assert_eq!(image["sourceSetComplete"], true, "contributors are known");
    assert_eq!(
        image["containsEncryptedSource"], true,
        "encrypted selected alpha contributes"
    );

    let provenance = &texture["provenance"];
    let refs = image["sourceUsesRefs"].as_array().ok_or("source uses")?;
    let uses = provenance["uses"].as_array().ok_or("uses inventory")?;
    let sources = provenance["sources"]
        .as_array()
        .ok_or("sources inventory")?;
    let mut offsets = Vec::new();
    for reference in refs {
        let usage = uses
            .iter()
            .find(|entry| &entry["id"] == reference)
            .ok_or("use")?;
        if usage.pointer("/location/namespace").and_then(Value::as_str) != Some("ddsMipPayload") {
            continue;
        }
        let source = sources
            .iter()
            .find(|entry| entry["id"] == usage["source"])
            .ok_or("source")?;
        let source_path = source
            .pointer("/path/value")
            .and_then(Value::as_str)
            .ok_or("source path")?;
        assert!(
            matches!(source_path, "Data/glass.dds.2" | "Data/glass.dds.2a"),
            "{source_path}"
        );
        offsets.push(
            usage
                .pointer("/location/indices/1")
                .and_then(Value::as_str)
                .ok_or("byte offset")?
                .parse::<u64>()?,
        );
    }
    assert_eq!(offsets.len(), 2, "one color and one alpha byte span");
    offsets.sort_unstable_by(|left, right| right.cmp(left));
    Ok(offsets)
}
