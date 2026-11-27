// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Conversion of `CryXml` Entries to XML during an Extraction.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{
    Archive, CancelToken, ConflictPolicy, ExtractOptions, ExtractionError, ExtractionReport,
    PathLimit, Progress, check_path_length, extract,
};
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

#[path = "conversion_receipt_collision.rs"]
mod receipt_collision;

type TestResult = Result<(), Box<dyn Error>>;

const SHIP_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<Ship name=\"Gladius\">\n  <Part class=\"wing\"/>\n</Ship>\n";

const TEXT_XML: &[u8] = b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\r\n<Text a='1' />\r\n";

fn ship(signature: CryXmlSignature, byte_order: CryXmlByteOrder) -> Vec<u8> {
    TestElement::new("Ship")
        .attribute("name", "Gladius")
        .child(TestElement::new("Part").attribute("class", "wing"))
        .to_cryxml(signature, byte_order)
}

fn little() -> Vec<u8> {
    ship(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: Archive,
    target: PathBuf,
}

fn fixture(entries: Vec<TestEntry>) -> Result<Fixture, Box<dyn Error>> {
    fixture_for_target(|_| Ok(entries))
}

fn fixture_for_target(
    entries: impl FnOnce(&Path) -> Result<Vec<TestEntry>, Box<dyn Error>>,
) -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let target = dir.path().join("out");
    entries(&target)?
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    fs::create_dir(&target)?;
    Ok(Fixture {
        archive: Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
        target,
        _dir: dir,
    })
}

fn run(fixture: &Fixture, convert: bool, policy: ConflictPolicy) -> ExtractionReport {
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        policy,
        convert,
        ..ExtractOptions::default()
    };
    extract(&fixture.archive, &entries, &fixture.target, &options)
}

fn read(target: &Path, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(target.join(path))?)
}

#[test]
fn conversion_writes_cryxml_entries_of_every_signature_and_byte_order_as_xml() -> TestResult {
    let fixture = fixture(vec![
        TestEntry::stored("Data\\b_little.xml", little()),
        TestEntry::zstd(
            "Data\\b_big.xml",
            ship(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big),
        )
        .encrypted(),
        TestEntry::deflated(
            "Data\\plain.mtl",
            ship(CryXmlSignature::CryXml, CryXmlByteOrder::Little),
        ),
        TestEntry::stored(
            "Data\\sdk.chrparams",
            ship(CryXmlSignature::Cry3Sdk, CryXmlByteOrder::Big),
        ),
    ])?;

    let report = run(&fixture, true, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 4, "every Entry is written");
    assert_eq!(report.converted(), Some(4), "every Entry is converted");
    for path in [
        "Data/b_little.xml",
        "Data/b_big.xml",
        "Data/plain.mtl",
        "Data/sdk.chrparams",
    ] {
        assert_eq!(
            String::from_utf8(read(&fixture.target, path)?)?,
            SHIP_XML,
            "{path} holds the XML"
        );
    }
    Ok(())
}

#[test]
fn without_conversion_cryxml_entries_are_written_unchanged() -> TestResult {
    let fixture = fixture(vec![TestEntry::stored("Data\\ship.xml", little())])?;

    let report = run(&fixture, false, ConflictPolicy::Skip);

    assert_eq!(report.extracted(), 1, "written");
    assert_eq!(report.converted(), None, "Conversion is off");
    assert_eq!(
        read(&fixture.target, "Data/ship.xml")?,
        little(),
        "raw bytes"
    );
    Ok(())
}

#[test]
fn conversion_leaves_text_xml_and_other_entries_byte_identical() -> TestResult {
    let binary: Vec<u8> = (0..=255).cycle().take(3 << 20).collect();
    let fixture = fixture(vec![
        TestEntry::deflated("Data\\text.xml", TEXT_XML.to_vec()),
        TestEntry::zstd("Data\\big.bin", binary.clone()),
        TestEntry::stored("Data\\tiny.txt", b"Cry".to_vec()),
        TestEntry::stored("Data\\empty.xml", Vec::new()),
    ])?;

    let report = run(&fixture, true, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 4, "every Entry is written");
    assert_eq!(report.converted(), Some(0), "nothing is CryXml");
    assert_eq!(
        read(&fixture.target, "Data/text.xml")?,
        TEXT_XML,
        "text XML"
    );
    assert_eq!(
        read(&fixture.target, "Data/big.bin")?,
        binary,
        "binary data"
    );
    assert_eq!(
        read(&fixture.target, "Data/tiny.txt")?,
        b"Cry",
        "short Entry"
    );
    assert!(
        read(&fixture.target, "Data/empty.xml")?.is_empty(),
        "empty Entry"
    );
    Ok(())
}

#[test]
fn an_entry_that_fails_to_convert_is_reported_and_the_rest_continues() -> TestResult {
    let mut corrupt = little();
    corrupt.truncate(40);
    let fixture = fixture(vec![
        TestEntry::stored("Data\\broken.xml", corrupt),
        TestEntry::stored("Data\\ship.xml", little()),
    ])?;
    fs::create_dir(fixture.target.join("Data"))?;
    fs::write(fixture.target.join("Data/broken.xml"), "old")?;

    let report = run(&fixture, true, ConflictPolicy::Overwrite);

    assert_eq!(report.extracted(), 1, "the good Entry is written");
    assert_eq!(report.converted(), Some(1), "and converted");
    let [failure] = report.failures() else {
        return Err(format!("one failure expected: {report:?}").into());
    };
    assert_eq!(failure.path(), "Data\\broken.xml", "the corrupt Entry");
    assert!(
        failure.error().to_string().contains("CryXml"),
        "the reason names the conversion: {}",
        failure.error()
    );
    assert_eq!(
        read(&fixture.target, "Data/broken.xml")?,
        b"old",
        "the existing file is kept"
    );
    let mut outputs = fs::read_dir(fixture.target.join("Data"))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    outputs.sort_unstable();
    assert_eq!(
        outputs,
        ["broken.xml", "ship.xml", "ship.xml.p4kx-origin.jsonl"].map(std::ffi::OsString::from),
        "only the successful file receives a companion and no partial output is left"
    );
    Ok(())
}

#[test]
fn converted_cryxml_survives_an_overlong_companion_path() -> TestResult {
    const OUTPUT_UNITS: usize = 250;
    const ORIGIN_SUFFIX: &str = ".p4kx-origin.jsonl";

    let fixture = fixture_for_target(|target| {
        let template = std::path::absolute(target)?.join("Data").join(".xml");
        let fixed_units = template.to_string_lossy().encode_utf16().count() + 1;
        let stem_units = OUTPUT_UNITS
            .checked_sub(fixed_units)
            .filter(|size| *size != 0)
            .ok_or("the fixture target must leave room for the XML filename")?;
        let entry = format!("Data\\{}.xml", "s".repeat(stem_units));
        Ok(vec![TestEntry::stored(entry, little())])
    })?;
    let entry = fixture.archive.entries().first().ok_or("fixture Entry")?;
    let relative = entry.path().replace('\\', "/");
    let target = std::path::absolute(&fixture.target)?;
    let output = target.join(&relative);
    check_path_length(&output)?;
    let receipt = target.join(format!("{relative}{ORIGIN_SUFFIX}"));
    let limit = check_path_length(&receipt).expect_err("the fixture companion exceeds the limit");

    let report = run(&fixture, true, ConflictPolicy::Overwrite);

    assert_eq!(fs::read(output)?, SHIP_XML.as_bytes(), "complete XML");
    assert_eq!(
        (report.extracted(), report.converted()),
        (1, Some(1)),
        "the converted payload was committed"
    );
    assert!(!receipt.exists(), "the overlong companion was not written");
    let [artifact] = report.cryxml_artifacts() else {
        return Err("one converted artifact expected".into());
    };
    assert!(
        artifact.origin_receipt.is_none(),
        "no receipt was published"
    );
    let [failure] = report.failures() else {
        return Err("one companion path failure expected".into());
    };
    assert_eq!(failure.path(), entry.path(), "the original Entry");
    assert!(
        matches!(
            failure.error(),
            ExtractionError::Write { path, source }
                if path == &receipt && source.get_ref().and_then(|cause| cause.downcast_ref::<PathLimit>()) == Some(&limit)
        ),
        "the only failure identifies the exact companion path and its unit limit: {failure:?}"
    );
    Ok(())
}

fn prefilled(existing: &[u8]) -> Result<Fixture, Box<dyn Error>> {
    let fixture = fixture(vec![
        TestEntry::stored("Data\\ship.xml", little()),
        TestEntry::stored("Data\\text.xml", TEXT_XML.to_vec()),
    ])?;
    fs::create_dir(fixture.target.join("Data"))?;
    fs::write(fixture.target.join("Data/ship.xml"), existing)?;
    fs::write(fixture.target.join("Data/text.xml"), TEXT_XML)?;
    Ok(fixture)
}

#[test]
fn skip_keeps_an_existing_file_of_a_cryxml_entry() -> TestResult {
    let fixture = prefilled(b"old")?;

    let report = run(&fixture, true, ConflictPolicy::Skip);

    assert_eq!(report.skipped(), ["Data\\ship.xml", "Data\\text.xml"]);
    assert_eq!(report.converted(), Some(0), "nothing converted");
    assert_eq!(read(&fixture.target, "Data/ship.xml")?, b"old", "kept");
    Ok(())
}

#[test]
fn overwrite_replaces_an_existing_file_with_the_converted_xml() -> TestResult {
    let fixture = prefilled(b"old")?;

    let report = run(&fixture, true, ConflictPolicy::Overwrite);

    assert_eq!(report.extracted(), 2, "both replaced");
    assert_eq!(report.converted(), Some(1), "one converted");
    assert_eq!(read(&fixture.target, "Data/ship.xml")?, SHIP_XML.as_bytes());
    Ok(())
}

#[test]
fn if_changed_keeps_an_existing_file_that_equals_the_converted_xml() -> TestResult {
    let fixture = prefilled(SHIP_XML.as_bytes())?;

    let report = run(&fixture, true, ConflictPolicy::IfChanged);

    assert_eq!(report.skipped(), ["Data\\ship.xml", "Data\\text.xml"]);
    assert_eq!(report.extracted(), 0, "nothing written");
    Ok(())
}

#[test]
fn if_changed_replaces_an_existing_raw_cryxml_file_with_the_converted_xml() -> TestResult {
    let fixture = prefilled(&little())?;

    let report = run(&fixture, true, ConflictPolicy::IfChanged);

    assert_eq!(
        report.skipped(),
        ["Data\\text.xml"],
        "text XML is unchanged"
    );
    assert_eq!(report.converted(), Some(1), "the raw file is converted");
    assert_eq!(read(&fixture.target, "Data/ship.xml")?, SHIP_XML.as_bytes());
    Ok(())
}

#[test]
fn if_changed_replaces_an_edited_converted_file() -> TestResult {
    let edited = SHIP_XML.replace("wing", "fins");
    let fixture = prefilled(edited.as_bytes())?;

    let report = run(&fixture, true, ConflictPolicy::IfChanged);

    assert_eq!(report.converted(), Some(1), "rewritten");
    assert_eq!(read(&fixture.target, "Data/ship.xml")?, SHIP_XML.as_bytes());
    Ok(())
}

#[test]
fn conversion_writes_the_same_bytes_on_every_run() -> TestResult {
    let fixture = fixture(vec![TestEntry::stored("Data\\ship.xml", little())])?;

    run(&fixture, true, ConflictPolicy::Overwrite);
    let first = read(&fixture.target, "Data/ship.xml")?;
    run(&fixture, true, ConflictPolicy::Overwrite);

    assert_eq!(read(&fixture.target, "Data/ship.xml")?, first, "identical");
    Ok(())
}

#[test]
fn conversion_reports_the_full_progress_of_every_entry() -> TestResult {
    let fixture = fixture(vec![
        TestEntry::stored("Data\\ship.xml", little()),
        TestEntry::stored("Data\\text.xml", TEXT_XML.to_vec()),
    ])?;
    let last = std::sync::Mutex::new(Progress::default());
    let on_progress = |now: Progress| {
        if let Ok(mut last) = last.lock() {
            *last = now;
        }
    };
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        convert: true,
        progress: Some(&on_progress),
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &entries, &fixture.target, &options);

    assert!(report.is_complete(), "{report:?}");
    let last = *last.lock().map_err(|_| "poisoned")?;
    assert_eq!(last.entries_done, 2, "both Entries ended");
    assert_eq!(last.bytes_done, last.bytes_total, "every byte counted once");
    Ok(())
}

#[test]
fn a_cancelled_conversion_leaves_no_file() -> TestResult {
    let fixture = fixture(vec![TestEntry::stored("Data\\ship.xml", little())])?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        convert: true,
        cancel: Some(&cancel),
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &entries, &fixture.target, &options);

    assert_eq!(report.cancelled(), ["Data\\ship.xml"], "cancelled");
    assert_eq!(fs::read_dir(&fixture.target)?.count(), 0, "nothing written");
    Ok(())
}
