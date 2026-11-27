// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_media::{MediaRecord, Severity, Status};
use serde_json::{Value, json};

use super::BankTestResult;

pub(super) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(super) fn read(directory: &Path) -> BankTestResult<Self> {
        Ok(Self {
            fields: rows(directory, "field-origins.jsonl")?,
            uses: rows(directory, "origin-uses.jsonl")?,
            sources: rows(directory, "origin-sources.jsonl")?,
        })
    }

    pub(super) fn media(
        &self,
        record: usize,
        owner: usize,
        start: usize,
        prefetch: bool,
    ) -> BankTestResult {
        let class = self.use_rows(record, owner, "/data/payload/data/class")?;
        assert!(
            has_header(&class, start, "signature"),
            "class references its actual selected physical embedded signature"
        );
        assert_eq!(
            has_header(&class, start, "form"),
            prefetch,
            "only the RIFF class consults its form"
        );
        assert!(
            !has_header(&class, start, "riffSize"),
            "class decision skips the RIFF size word"
        );

        let length = self.use_rows(record, owner, "/data/payload/data/declared_wem_length")?;
        assert!(
            has_header(&length, start, "signature"),
            "length consulted the actual signature"
        );
        assert_eq!(
            has_header(&length, start, "riffSize"),
            prefetch,
            "only RIFF length consults its size word"
        );
        Ok(())
    }

    pub(super) fn check_duplicate_media(
        &self,
        record: usize,
        owner: usize,
        row: &MediaRecord,
        fixture: &super::fixture::BankFixture<'_>,
        position: usize,
    ) -> BankTestResult {
        let duplicate = row
            .issues
            .iter()
            .filter(|issue| issue.code == "audio.bank.duplicate-media")
            .collect::<Vec<_>>();
        assert_eq!(
            duplicate.len(),
            usize::from(position > 0),
            "only repeated physical DIDX IDs have a duplicate notice"
        );
        if let [issue] = duplicate.as_slice() {
            assert_eq!(issue.severity, Severity::Info, "all bytes remain available");
            assert_eq!(
                row.status,
                Status::Complete,
                "duplicate data is fully retained"
            );
            let first = didx_body_start(fixture)?;
            self.duplicate_media(record, owner, first, first + 3 * size_of::<u32>())?;
        }
        Ok(())
    }

    pub(super) fn duplicate_media(
        &self,
        record: usize,
        owner: usize,
        first: usize,
        repeated: usize,
    ) -> BankTestResult {
        let uses = self.use_rows(record, owner, "/issues/0/detail")?;
        for position in [first, repeated] {
            let expected = json!({
                "type": "subresource",
                "namespace": "bnkDidx",
                "indices": [position.to_string()],
                "field": "mediaId"
            });
            assert!(
                uses.iter()
                    .any(|row| row.get("location") == Some(&expected)),
                "duplicate notice refers to both original media ID words"
            );
        }
        Ok(())
    }

    fn use_rows(&self, record: usize, owner: usize, path: &str) -> BankTestResult<Vec<&Value>> {
        let position = record.to_string();
        let receipt = self
            .fields
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.get("field").and_then(Value::as_str) == Some(path)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("audio")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(position.as_str())
            })
            .ok_or("actual per-field Audio origin")?;
        assert_eq!(
            receipt.get("sourceSetComplete"),
            Some(&json!(true)),
            "complete chosen source inputs"
        );
        assert_eq!(
            receipt.get("containsEncryptedSource"),
            Some(&json!(owner == 1)),
            "actual physical owner supplies the immediate encryption marker"
        );
        let references = receipt
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source-use references")?;
        references
            .iter()
            .map(|reference| self.lookup_owner(reference, owner))
            .collect()
    }

    fn lookup_owner(&self, reference: &Value, owner: usize) -> BankTestResult<&Value> {
        let used = self
            .uses
            .iter()
            .find(|row| row.get("id") == Some(reference))
            .ok_or("actual source use")?;
        let source = used.get("source").ok_or("source-use owner")?;
        let physical = self
            .sources
            .iter()
            .find(|row| row.get("id") == Some(source))
            .ok_or("physical source")?;
        assert_eq!(
            physical.get("entryIndex"),
            Some(&json!(owner)),
            "no cross-bank rebinding of duplicate short IDs"
        );
        Ok(used)
    }
}

#[test]
fn repeated_media_and_missing_data_keep_separate_source_receipts() -> BankTestResult {
    use p4kx_testkit::TestEntry;

    use super::{AUDIO_STREAM, DATA_SECTION, audio_options, fixture, require_schema, support};

    let folder = tempfile::tempdir()?;
    let mut bank = fixture::fixture(70, 101, b"PLUG sample", false)?;
    let data_start = bank
        .sections
        .get(DATA_SECTION)
        .ok_or("DATA section")?
        .header
        .start;
    bank.bytes.truncate(data_start);
    let archive = support::archive_with(
        folder.path(),
        [TestEntry::stored("without-data.bnk", bank.bytes.as_slice())],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &audio_options())?)?;
    let records = support::records(&output, &index, AUDIO_STREAM)?;
    require_schema(&records)?;
    let didx = records
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "audio-bank-media")
        .collect::<Vec<_>>();
    let [(first_index, first), (repeated_index, repeated)] = didx.as_slice() else {
        return Err("two original DIDX declarations".into());
    };
    assert_eq!(issue_codes(first), ["audio.bank.media-without-data"]);
    assert_eq!(
        issue_codes(repeated),
        [
            "audio.bank.duplicate-media",
            "audio.bank.media-without-data"
        ]
    );

    let origins = Origins::read(&output.join(&index.artifact_base))?;
    let first_word = didx_body_start(&bank)?;
    origins.duplicate_media(
        *repeated_index,
        0,
        first_word,
        first_word + 3 * size_of::<u32>(),
    )?;
    verify_missing_data_origin(&origins, *first_index, "/issues/0/detail")?;
    verify_missing_data_origin(&origins, *repeated_index, "/issues/1/detail")
}

fn issue_codes(row: &MediaRecord) -> Vec<&str> {
    assert_eq!(
        row.status,
        Status::Complete,
        "original DIDX row remains complete"
    );
    row.issues
        .iter()
        .map(|issue| {
            assert_eq!(
                issue.severity,
                Severity::Info,
                "a readable DIDX declaration is informative"
            );
            issue.code.as_str()
        })
        .collect()
}

fn verify_missing_data_origin(origins: &Origins, record: usize, path: &str) -> BankTestResult {
    let uses = origins.use_rows(record, 0, path)?;
    assert!(
        uses.iter()
            .any(|row| row.pointer("/location/field") == Some(&json!("dataSectionOccurrences"))),
        "missing DATA has its original directory coverage"
    );
    assert!(
        uses.iter()
            .all(|row| row.pointer("/location/field") != Some(&json!("mediaId"))),
        "missing DATA does not rely on the duplicate ID words"
    );
    Ok(())
}

fn didx_body_start(fixture: &super::fixture::BankFixture<'_>) -> BankTestResult<usize> {
    fixture
        .sections
        .iter()
        .find(|section| &section.tag == b"DIDX")
        .map(|section| section.body.start)
        .ok_or_else(|| "original DIDX body".into())
}

fn has_header(uses: &[&Value], start: usize, field: &str) -> bool {
    let expected = json!({
        "type": "subresource",
        "namespace": "bnkMediaHeader",
        "indices": [start.to_string()],
        "field": field
    });
    uses.iter()
        .any(|row| row.get("location") == Some(&expected))
}

fn rows(directory: &Path, file: &str) -> BankTestResult<Vec<Value>> {
    fs::read_to_string(directory.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
