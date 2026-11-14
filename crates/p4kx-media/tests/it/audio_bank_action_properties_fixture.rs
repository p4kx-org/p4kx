// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, HircLayout, OpenOptions};
use p4kx_media::{ExportOutcome, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::json;

use super::super::audio_bank_state_provider::{
    check_retained, options, row, source_span, verify_layout,
};
use super::super::support;
use super::super::support::audio_provider::{Origins, Owner, TestResult};

const PATH: &str = "action-properties.bnk";
pub(super) const PLAY: u16 = 0x0403;
pub(super) const STOP: u16 = 0x0102;
pub(super) const TARGET: u32 = 0xC4F1_20A2;
pub(super) const BANK_TYPE: u32 = 0xF0AE_0193;
pub(super) const TARGET_FLAGS: u8 = 0x82;
pub(super) const FADE_FLAGS: u8 = 0xFD;
pub(super) const PROPERTY_IDS: [u8; 3] = [0xF1, 0x03, 0xF1];
pub(super) const PROPERTY_BITS: [u32; 3] = [0x7FC1_1234, 0, 0x8000_0000];
pub(super) const RANGE_IDS: [u8; 2] = [0xFA, 0x07];
pub(super) const RANGE_BITS: [u32; 4] = [0xFF80_0000, 0, 0x8000_0000, 0x7F80_0000];

pub(super) fn bundle(ids: &[u8], words: &[u32]) -> TestResult<Vec<u8>> {
    let mut bytes = vec![u8::try_from(ids.len())?];
    bytes.extend(ids);
    bytes.extend(words.iter().copied().flat_map(u32::to_le_bytes));
    Ok(bytes)
}

pub(super) fn complete() -> TestResult<Vec<u8>> {
    let mut bytes = vec![TARGET_FLAGS];
    bytes.extend(bundle(&PROPERTY_IDS, &PROPERTY_BITS)?);
    bytes.extend(bundle(&RANGE_IDS, &RANGE_BITS)?);
    bytes.push(FADE_FLAGS);
    bytes.extend([0, BANK_TYPE].into_iter().flat_map(u32::to_le_bytes));
    Ok(bytes)
}

pub(super) fn bank(version: u32, code: u16, continuation: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(code.to_le_bytes());
    body.extend(TARGET.to_le_bytes());
    body.extend(continuation);
    original_bank(version, body)
}

pub(super) fn original_bank(version: u32, body: Vec<u8>) -> Vec<u8> {
    TestBank::new(5)
        .version(version)
        .object(TestHircObject::raw(3, 123, body))
        .object(TestHircObject::raw(8, TARGET, []))
        .object(TestHircObject::event(1, &[]))
        .bytes()
}

pub(super) fn export(
    source: &[u8],
    encrypted: bool,
    expected: HircLayout,
    inspect: impl FnOnce(&[MediaRecord], &Evidence<'_>) -> TestResult,
) -> TestResult {
    verify_layout(source, expected)?;
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let entry = TestEntry::zstd(PATH, source);
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let diagnostic = expected == HircLayout::Malformed;
    let mut export_options = options();
    export_options.diagnostic = diagnostic;
    let output = folder.path().join("out");
    let (index, root) = match p4kx_media::export(&archive, &output, &export_options)? {
        ExportOutcome::Published { index, .. } if !diagnostic => (index, output),
        ExportOutcome::Incomplete {
            index,
            diagnostic: report,
        } if diagnostic => (
            index,
            report
                .parent()
                .ok_or("diagnostic generation")?
                .to_path_buf(),
        ),
        _ => return Err("unexpected publication disposition".into()),
    };
    let records = support::records(&root, &index, "audio.jsonl")?;
    let base = root.join(&index.artifact_base);
    check_retained(
        &base,
        row(&records, "audio-bank-object", &[1, 0])?.1,
        source,
    )?;
    assert_eq!(
        row(&records, "audio-bank-object", &[1, 2])?
            .1
            .data
            .pointer("/payload/object_id"),
        Some(&json!(1)),
        "a short property list cannot consume the following physical objects",
    );
    let evidence = Evidence {
        source,
        origins: Origins::read(&base)?,
        encrypted,
    };
    inspect(&records, &evidence)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in &records {
        schema
            .validate(&serde_json::to_value(record)?)
            .map_err(|cause| cause.to_string())?;
    }
    Ok(())
}

pub(super) struct Evidence<'a> {
    source: &'a [u8],
    origins: Origins,
    encrypted: bool,
}

impl Evidence<'_> {
    pub(super) fn word(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u32,
    ) -> TestResult {
        self.scalar(row, field, label, value, &value.to_le_bytes())
    }

    pub(super) fn byte(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u8,
    ) -> TestResult {
        self.scalar(row, field, label, u32::from(value), &[value])
    }

    fn scalar(
        &self,
        (position, record): (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u32,
        bytes: &[u8],
    ) -> TestResult {
        let field = format!("/payload/{field}");
        assert_eq!(
            record.data.pointer(&format!("{field}/value")),
            Some(&json!(value)),
            "{field} preserves the original scalar",
        );
        let range = format!("{field}/range");
        assert_eq!(
            source_span(self.source, &record.data, &range)?,
            bytes,
            "{field} selects its exact original bytes"
        );
        let span = record.data.pointer(&range).ok_or("original word span")?;
        let indices = json!([span.get("start"), span.get("end_exclusive")]);
        let receipt = self.origins.field(
            "records",
            position,
            &format!("/data{field}/value"),
            self.encrypted,
        )?;
        let uses = self.origins.require_owner(
            receipt,
            Owner {
                path: PATH,
                entry: 0,
                parent: None,
                stored: if self.encrypted {
                    "encrypted"
                } else {
                    "unencrypted"
                },
                metadata: false,
                payload: self.encrypted,
            },
        )?;
        assert!(
            uses.iter()
                .any(|used| used.get("role") == Some(&json!("payload"))
                    && used.pointer("/location/namespace")
                        == Some(&json!("bnkHircActionProperties"))
                    && used.pointer("/location/field") == Some(&json!(label))
                    && used.pointer("/location/indices") == Some(&indices)),
            "{field} receipt identifies its actual owning bank and physical span",
        );
        Ok(())
    }
}
