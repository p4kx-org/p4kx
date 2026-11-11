// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod fixture;
mod receipts;

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry, TestMovie, TestSignature, external_image_body};
use serde_json::{Value, json};

use self::fixture::{Fixture, fixture};
pub(super) use self::receipts::assert_all_fields;
use self::receipts::{assert_tag, origin, scoped, uses};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const MOVIE: &str = "Data/UI/probe.gfx";
const FONT: u16 = 8;

fn run(input: &Path, entry: Option<&str>, expected: i32) -> TestResult<Value> {
    let mut command = p4kx_testkit::p4kx()?;
    command.args(["--json", "ui", "assets"]).arg(input);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(expected), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn archive_assets_bind_the_selected_movie_and_winning_physical_occurrences() -> TestResult {
    let fixture = fixture(TestSignature::Gfx)?;
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(MOVIE, &fixture.bytes[..]).encrypted())
        .entry(TestEntry::stored(
            MOVIE,
            TestMovie::new(TestSignature::Gfx).bytes(),
        ))
        .entry(TestEntry::stored("Data/UI/never-read.dds", b"external").encrypted())
        .write_to(&archive)?;
    let report = run(&archive, Some("data\\ui\\PROBE.gfx"), 1)?;
    assert_eq!(report["movie"], MOVIE, "the actual selected path");
    assert_eq!(
        origin(&report, "/movie")?["containsEncryptedSource"],
        false,
        "directory bytes"
    );
    assert_eq!(
        origin(&report, "/fonts/0/name")?["containsEncryptedSource"],
        true,
        "payload bytes"
    );
    let sources = report["provenance"]["sources"]
        .as_array()
        .ok_or("physical inventory")?;
    assert_eq!(
        sources.len(),
        1,
        "the duplicate and authored targets were untouched"
    );
    assert_eq!(
        sources[0]["entryIndex"], 0,
        "first selected physical occurrence"
    );

    assert_winning_font_records(&report, &fixture)?;
    assert_authored_links(&report, &fixture)
}

fn assert_winning_font_records(report: &Value, fixture: &Fixture) -> TestResult {
    for (path, value) in [
        ("/fonts/0/name", "Latest"),
        ("/fonts/1/name", "Duplicate"),
        ("/fonts/0/full_name", "Latest full"),
        ("/fonts/0/export_name", "winner"),
    ] {
        assert_eq!(
            report.pointer(path).and_then(Value::as_str),
            Some(value),
            "{path}"
        );
    }
    assert_eq!(
        report.pointer("/fonts/1/full_name"),
        Some(&Value::Null),
        "duplicate received no override"
    );
    for (path, role, code, ordinal) in [
        ("/fonts/0/name", "payload", 13, 1),
        ("/fonts/1/name", "payload", 75, 1),
        ("/fonts/0/full_name", "payload", 88, 1),
        ("/fonts/0/glyphs", "conversion", 75, 0),
        ("/exporter/swf_name", "payload", 1000, 1),
        ("/bitmaps/0/file", "payload", 1009, 0),
    ] {
        assert_tag(report, path, role, code, &fixture.offset(code, ordinal)?)?;
    }
    Ok(())
}

fn assert_authored_links(report: &Value, fixture: &Fixture) -> TestResult {
    let export = uses(report, "/fonts/0/export_name")?;
    let item = scoped(&export, "payload", "gfxTagItem")?;
    let offset = fixture.offset(56, 0)?;
    assert_eq!(
        item.pointer("/location/indices/1").and_then(Value::as_str),
        Some(offset.as_str()),
        "first export tag"
    );
    for path in [
        "/fonts/0/export_name",
        "/imports/0/assets/1/name",
        "/symbol_classes/1/name",
    ] {
        let uses = uses(report, path)?;
        let item = scoped(&uses, "payload", "gfxTagItem")?;
        assert_eq!(
            item.pointer("/location/indices/5").and_then(Value::as_str),
            Some("1"),
            "{path} authored ordinal"
        );
    }
    Ok(())
}

#[test]
fn compressed_assets_distinguish_original_consumed_bytes_from_logical_tag_locations() -> TestResult
{
    let folder = tempfile::tempdir()?;
    for (signature, start) in [(TestSignature::Cfx, 8_u64), (TestSignature::Zws, 12)] {
        let mut fixture = fixture(signature)?;
        let original_len = fixture.bytes.len() as u64;
        fixture.bytes.extend([0xcc; 32]);
        let file = folder.path().join("movie.gfx");
        std::fs::write(&file, &fixture.bytes)?;
        let report = run(&file, None, 1)?;
        let font = origin(&report, "/fonts/0/name")?;
        assert!(
            font["containsEncryptedSource"].is_null(),
            "standalone original lineage unknown"
        );
        assert_eq!(
            font["sourceSetComplete"], true,
            "the actual standalone input was observed"
        );
        let inputs = uses(&report, "/fonts/0/name")?;
        let tag = scoped(&inputs, "payload", "gfxDecodedTag")?;
        assert_eq!(
            tag["location"]["indices"][1],
            fixture.offset(13, 1)?,
            "logical offset"
        );
        let stream = inputs
            .iter()
            .find(|row| {
                row.pointer("/location/field").and_then(Value::as_str) == Some("compressedPayload")
            })
            .ok_or("actual compressed input")?;
        assert_eq!(
            stream["location"]["namespace"], "gfxMovieInput",
            "original bytes"
        );
        let coordinates = stream["location"]["indices"]
            .as_array()
            .ok_or("compressed byte range")?;
        let offset: u64 = coordinates[0].as_str().ok_or("offset")?.parse()?;
        let len: u64 = coordinates[1].as_str().ok_or("length")?.parse()?;
        assert_eq!(
            offset, start,
            "decompressor input begins at its actual cursor"
        );
        assert!(
            len > 0 && offset + len <= original_len,
            "trailing data did not contribute"
        );
        let signature_uses = uses(&report, "/signature")?;
        assert_eq!(
            signature_uses.len(),
            1,
            "the raw signature uses no decoded bytes"
        );
    }
    Ok(())
}

#[test]
fn every_emitted_field_has_a_receipt_and_timelines_count_their_own_frames() -> TestResult {
    let folder = tempfile::tempdir()?;
    let file = folder.path().join("movie.gfx");
    std::fs::write(&file, fixture(TestSignature::Gfx)?.bytes)?;
    let report = run(&file, None, 1)?;
    assert_all_fields(&report)?;
    assert_eq!(report["frame_count"], 9, "declared frames");
    assert_eq!(report["frames"], 1, "actual main frames");
    assert_eq!(report["sprites"][0]["frames"], 2, "parent sprite");
    assert_eq!(report["sprites"][1]["frames"], 1, "child sprite");
    assert_eq!(
        report["sprites"][0]["frame_labels"][0]["frame"], 1,
        "label's own prefix"
    );
    let placed = uses(&report, "/sprites/0/placed/0")?;
    assert_eq!(placed.len(), 2, "both occurrences of the retained ID");
    assert_ne!(
        placed[0]["id"], placed[1]["id"],
        "physical placements are distinct"
    );
    let nested = uses(&report, "/sprites/1/frames")?;
    let root = uses(&report, "/frames")?;
    assert_ne!(
        nested[0]["id"], root[0]["id"],
        "nested and root surveys use different headers"
    );
    let unknown = uses(&report, "/unknown_tags/0/length")?;
    assert_eq!(
        unknown[0]["location"]["namespace"], "gfxTagHeader",
        "unknown bytes were not consumed as values"
    );
    Ok(())
}

#[test]
fn malformed_absence_is_distinct_from_a_proved_plain_absence() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let movie = |bad| {
        let movie = TestMovie::new(TestSignature::Gfx).tag(
            1009,
            &external_image_body(6, 2, 14, (4, 2), ("tex", "unused.dds")),
        );
        if bad {
            movie.tag(56, &[1, 0]).bytes()
        } else {
            movie.bytes()
        }
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored(MOVIE, movie(true)))
        .entry(TestEntry::stored("plain.gfx", movie(false)))
        .entry(
            TestEntry::stored("unknown.gfx", movie(false))
                .encrypted()
                .with_encryption_field(7),
        )
        .write_to(&archive)?;
    let bad = run(&archive, Some(MOVIE), 1)?;
    let plain = run(&archive, Some("plain.gfx"), 0)?;
    let unknown = run(&archive, Some("unknown.gfx"), 0)?;
    let field = "/bitmaps/0/export_name";
    assert!(
        bad["bitmaps"][0]["export_name"].is_null(),
        "legacy missing value"
    );
    assert_eq!(
        origin(&bad, field)?["sourceSetComplete"],
        false,
        "unreadable export table"
    );
    assert!(
        origin(&bad, field)?["containsEncryptedSource"].is_null(),
        "incomplete absence"
    );
    assert_eq!(
        origin(&plain, field)?["containsEncryptedSource"],
        false,
        "proved plain absence"
    );
    assert_eq!(
        origin(&plain, field)?["sourceSetComplete"],
        true,
        "complete survey"
    );
    assert!(
        origin(&unknown, "/signature")?["containsEncryptedSource"].is_null(),
        "unknown declaration despite successful read"
    );
    assert_eq!(
        origin(&unknown, "/signature")?["sourceSetComplete"],
        true,
        "complete input set"
    );
    Ok(())
}

#[test]
fn truncated_stream_reports_the_retained_header_and_actual_available_extent() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let base = TestMovie::new(TestSignature::Gfx).tag(1, &[]);
    let header = 8 + base.body().len() - size_of::<u16>();
    let malformed = [0x3f, 0x0e, 0x64, 0, 0, 0];
    ArchiveBuilder::new()
        .entry(TestEntry::stored(MOVIE, base.raw(&malformed).bytes()))
        .write_to(&archive)?;
    let report = run(&archive, Some(MOVIE), 1)?;
    assert_eq!(report["frames"], 1, "the accepted prefix was retained");
    assert_eq!(
        report["failures"][0]["code"], 56,
        "the partial read retained the tag code"
    );
    assert_eq!(
        origin(&report, "/frames")?["sourceSetComplete"],
        false,
        "the scan stopped on a broken tag"
    );
    let actual = uses(&report, "/failures/0/error")?;
    let prefix = scoped(&actual, "conversion", "gfxBytes")?;
    let extent = scoped(&actual, "conversion", "gfxSurvey")?;
    assert_eq!(
        prefix["location"]["indices"],
        json!([header.to_string(), "6"]),
        "retained long header"
    );
    assert_eq!(
        extent["location"]["indices"],
        json!([(header + 6).to_string(), "2"]),
        "measured remainder"
    );
    Ok(())
}
