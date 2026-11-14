// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::gfx::{Movie, RgbaImage};
use p4kx_extraction::{OriginDigestWriter, Texture};
use p4kx_testkit::{
    ArchiveBuilder, SplitDds, TestEntry, TestFill, TestMovie, TestShape, TestShapeRecord,
    TestSignature, TestTextureFormat, external_image_body, gray_jpeg, jpeg_body, sub_image_body,
};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const MOVIE: &str = "Data/UI/panel.gfx";
const TEXTURE: &str = "Data/UI/atlas.dds";
const COMPANION: &str = ".p4kx-origin.jsonl";

fn extract(input: &Path, entry: Option<&str>, out: &Path, exit: i32) -> TestResult<Value> {
    let mut command = p4kx_testkit::p4kx()?;
    command.args(["--json", "ui", "extract"]).arg(input);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let output = command.arg("--out").arg(out).output()?;
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn rows(out: &Path) -> TestResult<Vec<Value>> {
    Ok(fs::read_to_string(out.join(COMPANION))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

fn artifact<'a>(rows: &'a [Value], file: &str) -> TestResult<(String, &'a Value)> {
    rows.iter()
        .filter(|row| row["type"] == "artifact")
        .enumerate()
        .find(|(_, row)| row.pointer("/path/value").and_then(Value::as_str) == Some(file))
        .map(|(ordinal, row)| (ordinal.to_string(), row))
        .ok_or_else(|| format!("missing artifact binding for {file}").into())
}

fn fields<'a>(rows: &'a [Value], ordinal: &'a str) -> impl Iterator<Item = &'a Value> {
    rows.iter()
        .filter(move |row| row["type"] == "artifactField" && row["artifact"] == ordinal)
}

fn contributors<'a>(report: &'a Value, field: &Value) -> TestResult<Vec<(&'a Value, &'a Value)>> {
    let refs = field["sourceUsesRefs"].as_array().ok_or("use references")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    let sources = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("sources")?;
    refs.iter()
        .map(|key| {
            let usage = uses
                .iter()
                .find(|item| &item["id"] == key)
                .ok_or("unresolved use")?;
            let input = sources
                .iter()
                .find(|item| item["id"] == usage["source"])
                .ok_or("unresolved source")?;
            Ok((usage, input))
        })
        .collect()
}

fn bitmap_contributors<'a>(
    report: &'a Value,
    rows: &[Value],
    file: &str,
) -> TestResult<Vec<(&'a Value, &'a Value)>> {
    let (id, _) = artifact(rows, file)?;
    let field = fields(rows, &id).next().ok_or("PNG origin")?;
    assert_eq!(
        field.pointer("/address/type").and_then(Value::as_str),
        Some("opaquePayload"),
        "the receipt binds the whole PNG payload"
    );
    contributors(report, field)
}

fn movie(signature: TestSignature) -> Vec<u8> {
    let shape = TestShape::new(17, 3, [0, 80, 0, 80])
        .fill(TestFill::Solid([19, 23, 41, 255]))
        .record(TestShapeRecord::Move {
            to: (0, 0),
            fill0: 1,
            fill1: 0,
            line: 0,
        })
        .record(TestShapeRecord::Line(80, 0))
        .record(TestShapeRecord::Line(0, 80));
    let duplicate = TestShape::new(17, 3, [0, 160, 0, 160]);
    TestMovie::new(signature)
        .tag(
            1009,
            &external_image_body(2, 2, 19, (8, 4), ("atlas", "atlas.tif")),
        )
        .tag(1008, &sub_image_body(3, 2, [1, 1, 3, 3]))
        .tag(shape.code(), &shape.body())
        .tag(duplicate.code(), &duplicate.body())
        .bytes()
}

fn dds() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4)
        .split(2)
        .alpha(TestTextureFormat::R8)
}

#[test]
fn split_png_and_cut_receive_used_sources_while_shape_fields_keep_their_movie_origin() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let data = folder.path().join("Data.p4k");
    let out = folder.path().join("assets");
    let movie = movie(TestSignature::Cfx);
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored(MOVIE, &movie[..]))
        .entry(TestEntry::stored("Data/UI/atlas.tif", b"discarded candidate").encrypted());
    for entry in dds().entries(TEXTURE) {
        let selected = matches!(entry.path(), "Data/UI/atlas.dds.1" | "Data/UI/atlas.dds.2a");
        archive = archive.entry(if selected { entry.encrypted() } else { entry });
    }
    archive.write_to(&data)?;

    let report = extract(&data, Some(MOVIE), &out, 0)?;
    assert_eq!(report["originReceipt"], COMPANION);
    assert_eq!(
        report["fieldOrigins"]["/extracted/0/id"]["containsEncryptedSource"],
        false
    );
    assert_eq!(
        report["fieldOrigins"]["/extracted/0/file"]["containsEncryptedSource"],
        false
    );
    assert_eq!(
        report["fieldOrigins"]["/extracted/0/size"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        report["fieldOrigins"]["/extracted/1/size"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        report["fieldOrigins"]["/extracted/2/size"]["containsEncryptedSource"],
        false
    );

    let rows = rows(&out)?;
    assert_eq!(rows.last().ok_or("end")?["type"], "end");
    assert_mixed_bitmaps(&report, &rows)?;

    let inputs = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("sources")?;
    assert!(inputs.iter().all(|input| !matches!(
        input["path"]["value"].as_str(),
        Some("Data/UI/atlas.tif" | "Data/UI/atlas.dds.1")
    )));
    assert_shape(&rows, &out, movie)
}

fn assert_mixed_bitmaps(report: &Value, rows: &[Value]) -> TestResult {
    for name in ["bitmap_2.png", "bitmap_3.png"] {
        let (ordinal, _) = artifact(rows, name)?;
        let opaque = fields(rows, &ordinal).next().ok_or("PNG field")?;
        assert_eq!(
            opaque["containsEncryptedSource"], true,
            "actual alpha split is encrypted"
        );
        assert_eq!(
            opaque["sourceSetComplete"], true,
            "both texture contributors are present"
        );
        let uses = bitmap_contributors(report, rows, name)?;
        let mut mips = uses
            .iter()
            .filter_map(|(usage, input)| {
                (usage.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("ddsMipPayload"))
                .then(|| input.pointer("/path/value").and_then(Value::as_str))
                .flatten()
            })
            .collect::<Vec<_>>();
        mips.sort_unstable();
        assert_eq!(
            mips,
            ["Data/UI/atlas.dds.2", "Data/UI/atlas.dds.2a"],
            "actual selected split mips"
        );
        assert_eq!(
            uses.iter().any(
                |(usage, _)| usage.pointer("/location/field").and_then(Value::as_str)
                    == Some("cropRectangle")
            ),
            name == "bitmap_3.png",
            "the cut and base use different authored selection data"
        );
    }

    Ok(())
}

fn assert_shape(rows: &[Value], out: &Path, movie: Vec<u8>) -> TestResult {
    let (ordinal, binding) = artifact(rows, "shape_17.json")?;
    let actual = fs::read(out.join("shape_17.json"))?;
    let mut measure = OriginDigestWriter::new(io::sink());
    measure.write_all(&actual)?;
    let digest = serde_json::to_value(measure.finish()?)?;
    assert_eq!(
        binding.get("sha256"),
        digest.get("sha256"),
        "the binding measures actual shape bytes"
    );
    assert_eq!(
        binding.get("byteLength"),
        digest.get("byteLength"),
        "actual artifact length"
    );
    let movie = Movie::parse(movie)?;
    let assets = movie.assets();
    let mut expected = Vec::new();
    assets
        .shapes
        .first()
        .ok_or("source shape")?
        .write_json(&mut expected)?;
    assert_eq!(actual, expected, "the first duplicate's legacy JSON bytes");
    let typed: Value = serde_json::from_slice(&actual)?;
    let mut addressed = Vec::new();
    for field in fields(rows, &ordinal) {
        assert_eq!(
            field["containsEncryptedSource"], false,
            "shape fields use the plain movie"
        );
        assert_eq!(
            field["sourceSetComplete"], true,
            "the accepted shape has a complete source"
        );
        let pointer = field
            .pointer("/address/path")
            .and_then(Value::as_str)
            .ok_or("shape field pointer")?;
        assert!(typed.pointer(pointer).is_some(), "{pointer}");
        addressed.push(pointer);
    }
    assert!(addressed.contains(&"/bounds/x_max"), "nested shape bounds");
    assert!(
        addressed.contains(&"/records/0/type"),
        "ordered shape record kind"
    );
    Ok(())
}

#[test]
fn disk_dds_reads_keep_actual_file_names_and_legacy_png_bytes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data/UI");
    fs::create_dir_all(&source)?;
    dds().write_files(&source, "atlas.dds")?;
    let file = source.join("panel.gfx");
    fs::write(&file, movie(TestSignature::Cfx))?;
    let out = folder.path().join("assets");
    let report = extract(&file, None, &out, 0)?;
    let rows = rows(&out)?;
    let (ordinal, _) = artifact(&rows, "bitmap_2.png")?;
    let field = fields(&rows, &ordinal).next().ok_or("PNG field")?;
    assert_eq!(field["containsEncryptedSource"], Value::Null);
    assert_eq!(field["sourceSetComplete"], true);
    let mut mips = bitmap_contributors(&report, &rows, "bitmap_2.png")?
        .into_iter()
        .filter_map(|(usage, input)| {
            (usage["location"]["namespace"] == "ddsMipPayload")
                .then(|| input["name"]["value"].as_str())
                .flatten()
        })
        .collect::<Vec<_>>();
    mips.sort_unstable();
    assert_eq!(mips, ["atlas.dds.2", "atlas.dds.2a"]);
    let texture = Texture::from_file(&source.join("atlas.dds"))?;
    let expected = texture.decode_mip(0)?;
    let expected =
        RgbaImage::from_pixels(expected.width(), expected.height(), expected.into_pixels())?;
    let cut = expected.crop((1, 1), (3, 3)).ok_or("expected crop")?;
    for (name, bitmap) in [("bitmap_2.png", expected), ("bitmap_3.png", cut)] {
        let mut original = Vec::new();
        bitmap.write_png(&mut original)?;
        assert_eq!(fs::read(out.join(name))?, original, "{name}");
    }
    Ok(())
}

#[test]
fn only_define_bits_uses_the_first_admitted_jpeg_tables() -> TestResult {
    let folder = tempfile::tempdir()?;
    let file = folder.path().join("movie.gfx");
    let raw = gray_jpeg(8, 8, 120);
    let movie = TestMovie::new(TestSignature::Gfx)
        .tag(8, &[0xff, 0xd8, 0xff, 0xd9])
        .tag(8, &[0x01, 0x02])
        .tag(6, &jpeg_body(5, &raw, None))
        .tag(21, &jpeg_body(6, &raw, None))
        .bytes();
    fs::write(&file, movie)?;
    let out = folder.path().join("assets");
    let report = extract(&file, None, &out, 0)?;
    let rows = rows(&out)?;
    let legacy = bitmap_contributors(&report, &rows, "bitmap_5.png")?;
    let tables = legacy
        .iter()
        .filter(|(usage, _)| usage["location"]["field"] == "jpegTables")
        .collect::<Vec<_>>();
    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0].0["location"]["indices"][4], "4",
        "first tables length"
    );
    let own = bitmap_contributors(&report, &rows, "bitmap_6.png")?;
    assert!(
        !own.iter()
            .any(|(usage, _)| usage["location"]["field"] == "jpegTables")
    );
    assert_eq!(
        report["fieldOrigins"]["/extracted/0/size"]["containsEncryptedSource"],
        json!(null)
    );
    Ok(())
}

#[test]
fn a_discarded_encrypted_candidate_does_not_taint_its_plain_dds_fallback() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let out = folder.path().join("assets");
    let movie = movie(TestSignature::Gfx);
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::stored(MOVIE, &movie[..]))
        .entry(TestEntry::stored("Data/UI/atlas.tif", b"discarded header").encrypted());
    for entry in dds().entries(TEXTURE) {
        builder = builder.entry(entry);
    }
    builder.write_to(&archive)?;
    let report = extract(&archive, Some(MOVIE), &out, 0)?;
    let rows = rows(&out)?;
    let (ordinal, _) = artifact(&rows, "bitmap_2.png")?;
    let field = fields(&rows, &ordinal).next().ok_or("PNG field")?;
    assert_eq!(field["containsEncryptedSource"], false);
    assert_eq!(field["sourceSetComplete"], true);
    assert_eq!(
        report["fieldOrigins"]["/extracted/0/size"]["containsEncryptedSource"],
        false
    );
    Ok(())
}
