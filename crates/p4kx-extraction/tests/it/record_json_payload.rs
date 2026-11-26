// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;

use p4kx_extraction::{
    Checksum, ConflictPolicy, DataForgeEntry, FilePublication, FileReceipt, JsonLayout, RecordJson,
    RecordJsonReceipt, TreeBuilder,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{TestResult, archive, child};

const FIELD_POINTER: &str = "/value~1~0";

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Fixture").field("value/~", TestType::String))
        .record(
            TestRecord::new(1, "Fixture", "Fixture.One", "fixture/one.xml")
                .value("value/~", TestValue::text("example\ntext")),
        )
        .build()
}

fn container(bytes: &[u8], encrypted: bool) -> ArchiveBuilder {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes.to_vec()))
        .build();
    let entry = TestEntry::stored("Data\\fixture.socpak", leaf);
    ArchiveBuilder::new().entry(if encrypted { entry.encrypted() } else { entry })
}

fn rendered(entry: &DataForgeEntry, layout: JsonLayout) -> TestResult<RecordJsonReceipt> {
    let forge = entry.open()?;
    let record = forge.records().next().ok_or("Fixture Record")?;
    let record = TreeBuilder::new().build_positioned(&record)?;
    let mut reference = Vec::new();
    let mut writer = RecordJson::new(layout);
    writer.write(record.tree(), &mut reference)?;
    let receipt = RecordJsonReceipt::render(&mut writer, entry, &forge, &record)?;
    assert_eq!(
        receipt.bytes(),
        reference,
        "observed output equals canonical legacy bytes"
    );
    Ok(receipt)
}

#[test]
fn record_json_payload_companion_follows_the_committed_physical_source() -> TestResult {
    let bytes = database();
    let (directory, plain) = archive(&container(&bytes, false))?;
    let (_other, cipher) = archive(&container(&bytes, true))?;
    let plain = child(&plain)?;
    let cipher = child(&cipher)?;
    let plain = DataForgeEntry::read(&plain)?;
    let cipher = DataForgeEntry::read(&cipher)?;
    for (name, layout) in [
        ("pretty.json", JsonLayout::Pretty),
        ("compact.json", JsonLayout::Compact),
    ] {
        let path = directory.path().join(name);
        let receipt = FileReceipt::new(&path, None, None)?;
        let unencrypted = rendered(&plain, layout)?;
        let encrypted = rendered(&cipher, layout)?;

        unencrypted.publish(&receipt, ConflictPolicy::Overwrite)?;
        let first = fs::read(receipt.path())?;
        assert_receipt(&fs::read(&path)?, &first, false)?;
        assert!(
            matches!(
                encrypted.publish(&receipt, ConflictPolicy::IfChanged)?,
                FilePublication::Kept {
                    verified_content: true,
                    ..
                }
            ),
            "equal payload retains the original artifact"
        );
        assert_eq!(
            fs::read(receipt.path())?,
            first,
            "keeping does not reassign source"
        );
        encrypted.publish(&receipt, ConflictPolicy::Overwrite)?;
        let output = fs::read(&path)?;
        assert_eq!(
            output,
            unencrypted.bytes(),
            "replacement publishes equal JSON bytes"
        );
        assert_receipt(&output, &fs::read(receipt.path())?, true)?;
    }
    Ok(())
}

fn assert_receipt(output: &[u8], journal: &[u8], encrypted: bool) -> TestResult {
    let journal = std::str::from_utf8(journal)?;
    let rows = journal
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let header = rows.first().ok_or("bound header")?;
    let size = output.len().to_string();
    let digest = Checksum::Sha256(Sha256::digest(output).into()).hex();
    assert_eq!(
        header
            .pointer("/binding/byteLength")
            .and_then(Value::as_str),
        Some(size.as_str()),
        "exact size"
    );
    assert_eq!(
        header.pointer("/binding/sha256").and_then(Value::as_str),
        Some(digest.as_str()),
        "exact digest"
    );
    let body: Value = serde_json::from_slice(output)?;
    let field = rows
        .iter()
        .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(FIELD_POINTER))
        .ok_or("escaped original field pointer")?;
    assert_eq!(
        body.pointer(FIELD_POINTER).and_then(Value::as_str),
        Some("example\ntext"),
        "the pointer addresses actual output"
    );
    assert_eq!(
        field
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "effective Container encryption"
    );
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "complete original inputs"
    );
    assert!(
        field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|refs| !refs.is_empty()),
        "actual uses retained"
    );
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "companion completed"
    );
    Ok(())
}

#[test]
fn an_equal_but_unrelated_input_cannot_attest_a_record_json_payload() -> TestResult {
    let bytes = database();
    let (_directory, root) = archive(&container(&bytes, true))?;
    let root = child(&root)?;
    let entry = DataForgeEntry::read(&root)?;
    let other = DataForgeEntry::read(&root)?;
    let forge = entry.open()?;
    let record = forge.records().next().ok_or("Fixture Record")?;
    let record = TreeBuilder::new().build_positioned(&record)?;
    let rejected = RecordJsonReceipt::render(&mut RecordJson::default(), &other, &forge, &record);
    assert!(
        matches!(rejected, Err(error) if error.kind() == io::ErrorKind::InvalidInput),
        "equal bytes are not the observed input"
    );
    Ok(())
}
