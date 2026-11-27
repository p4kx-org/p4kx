// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

use p4kx_extraction::{
    Archive, ConflictPolicy, DataForgeEntry, OriginDigestWriter, RecordExportOptions, RecordXml,
    TagTree, export_record_files,
};
use serde_json::{Value, json};

use super::{TestResult, export, nested_database, write_archive, write_archive_of};

#[test]
fn records_bind_attribute_names_and_exact_escaped_xml_values() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = write_archive(directory.path())?;
    let out = directory.path().join("out");
    let output = out.join("libs/foundry/records/ships/avenger.xml");
    let companion = output.with_file_name("avenger.xml.p4kx-origin.jsonl");
    let run = export(&archive, &out, &["--filter", "avenger", "--json"], 0)?;
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(
        report.pointer("/origin_receipts/0"),
        Some(&json!(
            "[DIR]/out/libs/foundry/records/ships/avenger.xml.p4kx-origin.jsonl"
        )),
        "the normal CLI reports the bound companion",
    );
    let bytes = fs::read(&output)?;
    let rows = read_receipt(&companion)?;
    check_binding(&rows, &bytes)?;
    let property = field(&rows, &bytes, "attributeName", b"name")?;
    let value = field(
        &rows,
        &bytes,
        "attributeValue",
        b"Avenger &amp; &lt;Titan&gt;",
    )?;
    assert_complete(property)?;
    assert_complete(value)?;
    assert_ne!(
        property.get("sourceUsesRefs"),
        value.get("sourceUsesRefs"),
        "the name and payload have distinct contributors"
    );
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&json!("end")),
        "receipt is complete"
    );
    Ok(())
}

#[test]
fn array_members_and_reference_targets_retain_distinct_original_coordinates() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = write_archive_of(directory.path(), &nested_database())?;
    let out = directory.path().join("out");
    let output = out.join("libs/foundry/records/vehicles/cart.xml");
    let companion = output.with_file_name("cart.xml.p4kx-origin.jsonl");
    export(&archive, &out, &["--filter", "cart"], 0)?;
    let bytes = fs::read(&output)?;
    let rows = read_receipt(&companion)?;
    let first = field(&rows, &bytes, "attributeValue", b"16")?;
    let second = field(&rows, &bytes, "attributeValue", b"17")?;
    let target = field(
        &rows,
        &bytes,
        "attributeValue",
        b"libs/foundry/records/makers/woodworks",
    )?;
    for value in [first, second, target] {
        assert_complete(value)?;
    }
    assert_ne!(
        first.get("sourceUsesRefs"),
        second.get("sourceUsesRefs"),
        "array occurrences use different original slots"
    );
    assert!(
        has_use(&rows, target, |source| source
            .pointer("/location/recordIndex")
            == Some(&json!(1))),
        "the target path cites the actual target Record"
    );
    Ok(())
}

#[test]
fn keeps_existing_proof_and_retires_it_for_an_unobserved_equal_byte_replacement() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = write_archive(directory.path())?;
    let out = directory.path().join("out");
    let output = out.join("libs/foundry/records/ships/avenger.xml");
    let companion = output.with_file_name("avenger.xml.p4kx-origin.jsonl");
    export(&archive, &out, &["--filter", "avenger"], 0)?;
    let proof = fs::read(&companion)?;
    let payload = fs::read(&output)?;
    for policy in ["skip", "if-changed"] {
        export(
            &archive,
            &out,
            &["--filter", "avenger", "--conflict", policy],
            0,
        )?;
        assert_eq!(
            fs::read(&companion)?,
            proof,
            "{policy} retains the bound proof"
        );
    }
    let source = Archive::open_with_key(&archive, Some(p4kx_testkit::test_key()))?;
    let input = DataForgeEntry::read(&source)?;
    let forge = input.open()?;
    let records: Vec<_> = forge
        .records()
        .filter(|record| record.name().ok() == Some("Ship.Avenger"))
        .collect();
    let tags = TagTree::default();
    let report = export_record_files::<RecordXml>(
        &forge,
        &records,
        &out,
        &RecordExportOptions {
            policy: ConflictPolicy::Overwrite,
            tags: Some(&tags),
            ..RecordExportOptions::default()
        },
    );
    assert_eq!(report.written(), 1, "ordinary XML replacement");
    assert_eq!(
        fs::read(&output)?,
        payload,
        "the tag resolver has no annotations for this synthetic Record"
    );
    assert!(
        !companion.exists(),
        "the replacement cannot inherit the retired proof even when the bytes match"
    );
    Ok(())
}

#[test]
fn a_foreign_companion_preserves_the_committed_xml_and_reports_the_failure() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = write_archive(directory.path())?;
    let out = directory.path().join("out");
    let output = out.join("libs/foundry/records/ships/avenger.xml");
    let companion = output.with_file_name("avenger.xml.p4kx-origin.jsonl");
    fs::create_dir_all(companion.parent().ok_or("companion parent")?)?;
    fs::write(&companion, b"foreign")?;
    let run = export(&archive, &out, &["--filter", "avenger", "--json"], 1)?;
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(report.get("exported"), Some(&json!(1)), "XML was committed");
    assert!(
        !report
            .get("origin_failures")
            .and_then(Value::as_array)
            .ok_or("origin failure")?
            .is_empty(),
        "companion failure is reported"
    );
    assert!(
        fs::read_to_string(&output)?.contains("Avenger &amp; &lt;Titan&gt;"),
        "the valid XML is still usable"
    );
    assert_eq!(
        fs::read(&companion)?,
        b"foreign",
        "foreign bytes remain unchanged"
    );
    Ok(())
}

fn read_receipt(path: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    let input = fs::read_to_string(path)?;
    Ok(input
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

fn check_binding(rows: &[Value], bytes: &[u8]) -> TestResult {
    let mut measured = OriginDigestWriter::new(Vec::new());
    measured.write_all(bytes)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let header = rows.first().ok_or("origin header")?;
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "committed artifact length"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "committed artifact digest"
    );
    Ok(())
}

fn field<'a>(
    rows: &'a [Value],
    bytes: &[u8],
    kind: &str,
    expected: &[u8],
) -> Result<&'a Value, Box<dyn Error>> {
    rows.iter()
        .find(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some("xmlBytes")
                && row.pointer("/address/field").and_then(Value::as_str) == Some(kind)
                && range_bytes(row, bytes) == Some(expected)
        })
        .ok_or_else(|| format!("missing observed XML {kind}").into())
}

fn range_bytes<'a>(row: &Value, bytes: &'a [u8]) -> Option<&'a [u8]> {
    let number = |pointer| row.pointer(pointer)?.as_str()?.parse::<usize>().ok();
    bytes.get(number("/address/start")?..number("/address/endExclusive")?)
}

fn assert_complete(field: &Value) -> TestResult {
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the field cites its encrypted DataForge Entry"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(true)),
        "the used original values are complete"
    );
    assert!(
        !field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source refs")?
            .is_empty(),
        "the field retains actual uses"
    );
    Ok(())
}

fn has_use(rows: &[Value], field: &Value, predicate: impl Fn(&Value) -> bool) -> bool {
    let Some(ids) = field.get("sourceUsesRefs").and_then(Value::as_array) else {
        return false;
    };
    rows.iter()
        .filter_map(|row| row.get("sourceUse"))
        .any(|source| source.get("id").is_some_and(|id| ids.contains(id)) && predicate(source))
}

#[test]
fn tag_and_language_fields_cite_their_exact_xml_ranges_in_both_xml_forms() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    tag_archive(&archive)?;
    for (mode, file) in [("single", "fixture/kit.xml"), ("combined", "Game2.xml")] {
        let out = directory.path().join(mode);
        let mut args = vec!["--filter", "kit", "--tags", "--lang", "en", "--json"];
        if mode == "combined" {
            args.push("--combined");
        }
        let run = export(&archive, &out, &args, 0)?;
        let report: Value = serde_json::from_str(&run.stdout)?;
        assert_eq!(
            report
                .get("origin_receipts")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1),
            "the sole written XML file has one origin companion"
        );
        let output = out.join(file);
        let bytes = fs::read(&output)?;
        let name = format!(
            "{}.p4kx-origin.jsonl",
            output.file_name().ok_or("XML name")?.to_string_lossy()
        );
        let rows = read_receipt(&output.with_file_name(name))?;
        check_binding(&rows, &bytes)?;
        assert_xml_annotation_origins(&rows, &bytes)?;
    }
    Ok(())
}

fn assert_xml_annotation_origins(rows: &[Value], bytes: &[u8]) -> TestResult {
    let tag = field(
        rows,
        bytes,
        "attributeValue",
        b"Weapon&amp;kit.Gun&lt;test&gt;",
    )?;
    assert_complete(tag)?;
    for (field_name, owner) in [("children", "1"), ("tags", "0")] {
        assert!(
            has_use(rows, tag, |source| {
                source.get("role").and_then(Value::as_str) == Some("selector")
                    && source
                        .pointer("/location/namespace")
                        .and_then(Value::as_str)
                        == Some("dataforgeTagRelations")
                    && source.pointer("/location/field").and_then(Value::as_str) == Some(field_name)
                    && source.pointer("/location/indices") == Some(&json!([owner, "1"]))
            }),
            "tag cites the selected {field_name} array occurrence"
        );
    }
    assert!(
        has_use(rows, tag, |source| {
            source.get("role").and_then(Value::as_str) == Some("selector")
                && source
                    .pointer("/location/namespace")
                    .and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
                && source.pointer("/location/field").and_then(Value::as_str) == Some("arrayElement")
        }),
        "tag cites the original reference slot"
    );
    let text = field(rows, bytes, "attributeValue", b"An &lt;item&gt;")?;
    assert_complete(text)?;
    assert!(
        has_use(rows, text, |source| {
            source.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
                && source.pointer("/location/language").and_then(Value::as_str) == Some("en")
        }),
        "translation cites the language lookup"
    );
    Ok(())
}

fn tag_archive(file: &Path) -> TestResult {
    use p4kx_testkit::{
        ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
        TestType, TestValue,
    };

    let reference = || TestType::ReferenceTo("Tag".to_owned());
    let tag = |guid, name: &str, children| {
        TestRecord::new(guid, "Tag", format!("Tag.{name}"), "fixture/tags.xml")
            .value("tagName", TestValue::text(name))
            .value("children", TestValue::Array(children))
    };
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("TagDatabase").array("tags", reference()))
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", reference()),
        )
        .structure(
            TestStruct::new("Item")
                .array("mounts", reference())
                .field("label", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "TagDatabase", "TagDatabase.Roots", "fixture/roots.xml").value(
                "tags",
                TestValue::Array(vec![TestValue::Null, TestValue::Reference(2)]),
            ),
        )
        .record(tag(
            2,
            "Weapon&kit",
            vec![TestValue::Null, TestValue::Reference(3)],
        ))
        .record(tag(3, "Gun<test>", Vec::new()))
        .record(
            TestRecord::new(4, "Item", "Item.Kit", "fixture/kit.xml")
                .value(
                    "mounts",
                    TestValue::Array(vec![TestValue::Null, TestValue::Reference(3)]),
                )
                .value("label", TestValue::text("@fixture_Kit")),
        )
        .build();
    let ini = GlobalIniBuilder::new()
        .text("fixture_Kit", "An <item>")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            ini,
        ))
        .write_to(file)?;
    Ok(())
}
