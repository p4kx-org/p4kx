// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use super::{
    COMPANION, DataForgeBuilder, TestRecord, TestResult, TestStruct, TestType, TestValue, export,
    write_archive, write_archive_of,
};

fn receipt(path: &Path) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn assert_field(
    rows: &[serde_json::Value],
    xml: &str,
    escaped: &str,
    source_at: impl Fn(&serde_json::Value) -> bool,
) -> TestResult {
    let byte_start = xml.find(escaped).ok_or("escaped XML field")?;
    let byte_end = byte_start + escaped.len();
    let field = rows
        .iter()
        .find(|row| {
            row.pointer("/address/type")
                .and_then(serde_json::Value::as_str)
                == Some("xmlBytes")
                && row
                    .pointer("/address/field")
                    .and_then(serde_json::Value::as_str)
                    == Some("attributeValue")
                && row.pointer("/address/start") == Some(&serde_json::json!(byte_start.to_string()))
                && row.pointer("/address/endExclusive")
                    == Some(&serde_json::json!(byte_end.to_string()))
        })
        .ok_or("output-relative XML address")?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&serde_json::json!(true)),
        "the addressed field has an encrypted contribution"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&serde_json::json!(true)),
        "the addressed field has complete evidence"
    );
    let uses = field
        .get("sourceUsesRefs")
        .and_then(serde_json::Value::as_array)
        .ok_or("original source refs")?;
    assert!(
        rows.iter()
            .filter_map(|row| row.get("sourceUse"))
            .any(|source| {
                source.get("id").is_some_and(|id| uses.contains(id)) && source_at(source)
            }),
        "the field cites an original coordinate"
    );
    Ok(())
}

fn record_at(index: u32) -> impl Fn(&serde_json::Value) -> bool {
    move |source| source.pointer("/location/recordIndex") == Some(&serde_json::json!(index))
}

#[test]
fn combined_receipt_uses_the_final_document_byte_ranges() -> TestResult {
    use std::io::Write as _;

    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    export(&archive, &out, &[], 0)?;
    let xml = fs::read_to_string(out.join("Game2.xml"))?;
    let rows = receipt(&out.join(COMPANION))?;
    let mut measured = p4kx_extraction::OriginDigestWriter::new(Vec::new());
    measured.write_all(xml.as_bytes())?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let header = rows.first().ok_or("receipt header")?;
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "the binding covers the final XML length"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "the binding covers the final XML digest"
    );
    assert_field(&rows, &xml, "Item.Anchor", record_at(3))?;
    assert_field(&rows, &xml, "Ship.Avenger", record_at(0))?;
    assert_field(&rows, &xml, "Avenger &amp; &lt;Titan&gt;", |source| {
        source
            .pointer("/location/namespace")
            .and_then(serde_json::Value::as_str)
            == Some("dataforgeDecodedBytes")
            && source
                .pointer("/location/field")
                .and_then(serde_json::Value::as_str)
                == Some("text")
    })?;
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&serde_json::json!("end")),
        "the staged companion has a complete trailer",
    );
    Ok(())
}

#[test]
fn combined_keeps_proof_on_skip_and_retires_it_on_unobserved_replacement() -> TestResult {
    use p4kx_extraction::{Archive, ConflictPolicy, DataForgeEntry, RecordExportOptions, TagTree};
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    export(&archive, &out, &[], 0)?;
    let companion = out.join(COMPANION);
    let proof = fs::read(&companion)?;
    let bytes = fs::read(out.join("Game2.xml"))?;
    for policy in ["skip", "if-changed"] {
        export(&archive, &out, &["--conflict", policy], 0)?;
        assert_eq!(fs::read(&companion)?, proof, "{policy} keeps the companion");
    }
    let source = Archive::open_with_key(&archive, Some(p4kx_testkit::test_key()))?;
    let input = DataForgeEntry::read(&source)?;
    let forge = input.open()?;
    let records: Vec<_> = forge.records().collect();
    let tags = TagTree::default();
    let report = p4kx_extraction::export_combined(
        &forge,
        &records,
        &out.join("Game2.xml"),
        &RecordExportOptions {
            policy: ConflictPolicy::Overwrite,
            tags: Some(&tags),
            ..RecordExportOptions::default()
        },
    )?;
    assert_eq!(report.written(), 4, "ordinary combined XML replacement");
    assert_eq!(
        fs::read(out.join("Game2.xml"))?,
        bytes,
        "the synthetic tags add no values"
    );
    assert!(
        !companion.exists(),
        "an equal-byte replacement retires the old proof"
    );
    Ok(())
}

#[test]
fn combined_reports_foreign_companion_and_keeps_the_new_xml() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    fs::create_dir(&out)?;
    let companion = out.join(COMPANION);
    fs::write(&companion, b"foreign bytes")?;
    let stdout = export(&archive, &out, &["--json"], 1)?;
    let report: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        report.get("exported"),
        Some(&serde_json::json!(4)),
        "all records committed to XML"
    );
    assert!(
        report.get("origin_receipts").is_none(),
        "a foreign companion cannot become a receipt"
    );
    assert_eq!(
        report
            .get("origin_failures")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "the companion conflict is reported"
    );
    let xml = fs::read_to_string(out.join("Game2.xml"))?;
    assert_eq!(
        roxmltree::Document::parse(&xml)?
            .root_element()
            .children()
            .filter(roxmltree::Node::is_element)
            .count(),
        4,
        "the committed XML retains each record"
    );
    assert_eq!(
        fs::read(&companion)?,
        b"foreign bytes",
        "a foreign file is preserved"
    );
    Ok(())
}

#[test]
fn combined_receipt_keeps_physical_uses_across_a_batch_boundary() -> TestResult {
    let dir = tempfile::tempdir()?;
    let database = (0_u32..1025).fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("name", TestType::String)),
        |database, index| {
            database.record(
                TestRecord::new(
                    u128::from(index) + 1,
                    "Item",
                    format!("Item.Unit{index:04}"),
                    format!("libs/foundry/records/items/unit{index:04}.xml"),
                )
                .value("name", TestValue::text(format!("Unit & <{index:04}>"))),
            )
        },
    );
    let archive = write_archive_of(dir.path(), &database)?;
    let out = dir.path().join("out");
    export(&archive, &out, &[], 0)?;
    let xml = fs::read_to_string(out.join("Game2.xml"))?;
    let rows = receipt(&out.join(COMPANION))?;
    assert_field(&rows, &xml, "Item.Unit1023", record_at(1023))?;
    assert_field(&rows, &xml, "Item.Unit1024", record_at(1024))?;
    Ok(())
}

#[test]
fn combined_localization_keeps_exact_lookup_evidence_across_batches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    write_localized_batch_archive(&archive)?;

    let out = dir.path().join("out");
    let report: serde_json::Value =
        serde_json::from_str(&export(&archive, &out, &["--lang", "en", "--json"], 0)?)?;
    assert_eq!(
        report.get("exported").and_then(serde_json::Value::as_u64),
        Some(1025),
        "all batches commit"
    );
    assert_eq!(
        report
            .get("origin_receipts")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "one receipt covers the whole document"
    );
    let xml = fs::read_to_string(out.join("Game2.xml"))?;
    let rows = receipt(&out.join(COMPANION))?;
    let translated = "Name &amp; &lt;details&gt;";
    assert!(
        xml.contains(&format!("p4kx:text-name=\"{translated}\"")),
        "translation bytes are escaped in XML"
    );
    assert!(
        xml.contains("p4kx:missing-name=\"true\""),
        "the missing translation has a separate XML field"
    );
    let is_lookup = |source: &serde_json::Value| {
        source
            .pointer("/location/type")
            .and_then(serde_json::Value::as_str)
            == Some("localizationLookup")
            && source
                .pointer("/location/language")
                .and_then(serde_json::Value::as_str)
                == Some("en")
    };
    assert_field(&rows, &xml, translated, is_lookup)?;
    assert_field(&rows, &xml, translated, |source| {
        source
            .pointer("/location/namespace")
            .and_then(serde_json::Value::as_str)
            == Some("globalIniBytes")
    })?;
    assert_field(&rows, &xml, "true", is_lookup)?;
    Ok(())
}

fn write_localized_batch_archive(archive: &Path) -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry};

    let database = (0_u32..1025).fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("name", TestType::Locale)),
        |database, index| {
            let key = match index {
                1023 => "@fixture_Translated",
                1024 => "@fixture_Missing",
                _ => "@LOC_PLACEHOLDER",
            };
            database.record(
                TestRecord::new(
                    u128::from(index) + 1,
                    "Item",
                    format!("Item.Unit{index:04}"),
                    format!("fixture/unit{index:04}.xml"),
                )
                .value("name", TestValue::text(key)),
            )
        },
    );
    let ini = GlobalIniBuilder::new()
        .text("fixture_Translated", "Name & <details>")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::zstd("Data\\Localization\\english\\global.ini", ini).encrypted())
        .write_to(archive)?;
    Ok(())
}
