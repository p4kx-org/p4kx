// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::{
    Exclusion, ExtractionError, FolderSource, OriginDigestWriter, convert_folder, extract,
};
use p4kx_testkit::{
    CryXmlByteOrder, CryXmlSignature, SplitDds, TestElement, TestEntry, TestTextureFormat,
};
use serde_json::{Value, json};

use super::{TestResult, converting, database, files, fixture, run};

const ENTRY: &str = r"Data\Game2.dcb";
const XML: &str = "Data/libs/foundry/records/items/n1.xml";
const RECEIPT: &str = "Data/libs/foundry/records/items/n1.xml.p4kx-origin.jsonl";

#[test]
fn record_receipt_uses_the_exact_selected_entry_among_duplicate_paths() -> TestResult {
    let payload = database(2);
    let fixture = fixture(vec![
        TestEntry::stored(ENTRY, payload.clone()),
        TestEntry::zstd(ENTRY, payload).encrypted(),
    ])?;
    let selected = fixture
        .archive
        .entries()
        .get(1)
        .ok_or("second physical Entry")?;
    let report = extract(
        &fixture.archive,
        &[selected],
        &fixture.target,
        &converting(),
    );
    assert!(report.is_complete(), "complete conversion: {report:?}");
    let records = report.records().ok_or("Record report")?;
    assert_eq!(
        records.origin_receipts().len(),
        2,
        "each committed Record has evidence"
    );
    let rows = receipt(&fixture.target)?;
    let source = original(&rows)?;
    assert_eq!(
        source.pointer("/entryIndex"),
        Some(&json!(selected.index())),
        "the observed selected Entry supplies the payload"
    );
    let label = label_origin(&fixture.target, &rows)?;
    assert_eq!(
        label.get("containsEncryptedSource"),
        Some(&json!(true)),
        "selected Entry is encrypted"
    );
    assert_eq!(
        label.get("sourceSetComplete"),
        Some(&json!(true)),
        "readable exact label"
    );
    Ok(())
}

#[test]
fn folder_record_receipt_retains_the_loose_reader_and_unknown_archive_encryption() -> TestResult {
    let root = tempfile::tempdir()?;
    let input = root.path().join("input");
    fs::create_dir_all(input.join("Data"))?;
    fs::write(input.join("Data/Game2.dcb"), database(2))?;
    let source = FolderSource::scan(&input, None)?;
    let selection = source.select(None, &Exclusion::default());
    let target = root.path().join("out");
    let report = convert_folder(&source, &selection, false, &target, &converting())?;
    assert!(report.is_complete(), "complete conversion: {report:?}");
    let rows = receipt(&target)?;
    assert_eq!(
        original(&rows)?.get("kind"),
        Some(&json!("file")),
        "the open loose file"
    );
    let label = label_origin(&target, &rows)?;
    assert_eq!(
        label.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the original Archive is unknown"
    );
    assert_eq!(
        label.get("sourceSetComplete"),
        Some(&json!(true)),
        "readable exact label"
    );
    Ok(())
}

#[test]
fn a_foreign_record_companion_keeps_both_payloads_and_reports_its_failure() -> TestResult {
    let fixture = fixture(vec![
        TestEntry::stored(ENTRY, database(2)),
        TestEntry::stored(RECEIPT.replace('/', "\\"), b"native data".to_vec()),
    ])?;
    let report = run(&fixture, &converting());
    assert!(
        !report.is_complete(),
        "the companion failure prevents a complete report"
    );
    let records = report.records().ok_or("Record report")?;
    assert_eq!(records.written(), 2, "both XML payloads were written");
    assert!(records.failures().is_empty(), "no Record payload failed");
    let companion = fixture.target.join(RECEIPT);
    assert_eq!(
        fs::read(&companion)?,
        b"native data",
        "native file retained"
    );
    assert!(
        fs::read_to_string(fixture.target.join(XML))?.contains("label=\"fine\""),
        "Record XML retained"
    );
    assert!(
        matches!(records.origin_failures(), [ExtractionError::Write { path, .. }] if path == &companion),
        "only the actual companion failure is reported: {records:?}"
    );
    Ok(())
}

fn receipt(target: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let rows: Vec<Value> = fs::read_to_string(target.join(RECEIPT))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(&fs::read(target.join(XML))?)?;
    let digest = digest.finish()?;
    let header = rows.first().ok_or("receipt header")?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(digest.sha256())),
        "exact Record XML bytes"
    );
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&json!("end")),
        "complete companion"
    );
    Ok(rows)
}

fn original(rows: &[Value]) -> Result<&Value, Box<dyn std::error::Error>> {
    let mut sources = rows.iter().filter_map(|row| row.get("source"));
    let source = sources.next().ok_or("original DataForge input")?;
    assert!(
        sources.next().is_none(),
        "one physical input supplies this Record"
    );
    Ok(source)
}

fn label_origin<'a>(
    target: &Path,
    rows: &'a [Value],
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let xml = fs::read(target.join(XML))?;
    for row in rows {
        if row.pointer("/address/field") != Some(&json!("attributeValue")) {
            continue;
        }
        let start = offset(row, "/address/start")?;
        let end = offset(row, "/address/endExclusive")?;
        if xml.get(start..end) == Some(b"fine") {
            return Ok(row);
        }
    }
    Err("no XML-span proof for the selected Record's label".into())
}

fn offset(row: &Value, path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(row
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or("field byte offset")?
        .parse()?)
}

#[test]
fn cryxml_textures_and_the_dataforge_convert_in_one_run() -> TestResult {
    let cryxml = TestElement::new("Ship")
        .attribute("name", "Gladius")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let mut entries = SplitDds::new(TestTextureFormat::LegacyDxt5, 32, 16)
        .split(3)
        .entries("Data\\Textures\\rock_diff.dds");
    entries.push(TestEntry::stored("Data\\ship.xml", cryxml));
    entries.push(TestEntry::stored("Data\\Game2.dcb", database(2)));
    let fixture = fixture(entries)?;

    let report = run(&fixture, &converting());

    assert!(report.is_complete(), "complete: {report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
            "Data/libs/foundry/records/items/n0.xml",
            "Data/libs/foundry/records/items/n0.xml.p4kx-origin.jsonl",
            "Data/libs/foundry/records/items/n1.xml",
            "Data/libs/foundry/records/items/n1.xml.p4kx-origin.jsonl",
            "Data/ship.xml",
            "Data/ship.xml.p4kx-origin.jsonl",
        ],
        "texture, Record and CryXml outputs with their expected companions"
    );
    assert!(
        fs::read(fixture.target.join("Data/ship.xml"))?.starts_with(b"<?xml"),
        "CryXml as XML"
    );
    assert_eq!(
        report
            .records()
            .map(p4kx_extraction::RecordExportReport::written),
        Some(2),
        "both Records"
    );
    Ok(())
}
