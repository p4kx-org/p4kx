// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, HircLayout, OpenOptions, Soundbank};
use p4kx_media::{ExportOutcome, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, TestNodeBase, test_key};
use serde_json::{Value, json};

use super::super::audio_bank_state_provider::{
    check_retained, options, row, source_span, verify_layout,
};
use super::super::support;
use super::super::support::audio_provider::{Origins, Owner, TestResult};

const PATH: &str = "container-node.bnk";

pub(crate) fn bank(version: u32, container: TestHircObject) -> Vec<u8> {
    TestBank::new(5)
        .version(version)
        .object(container)
        .object(TestHircObject::event(1, &[]))
        .bytes()
}

pub(crate) fn source(
    type_byte: u8,
    id: u32,
    version: u32,
    continuation: &[u8],
) -> TestResult<Vec<u8>> {
    let mut body = plain_node()?;
    body.extend(continuation);
    Ok(bank(version, TestHircObject::raw(type_byte, id, body)))
}

pub(crate) fn plain_node() -> TestResult<Vec<u8>> {
    const PLAIN_NODE_BYTES: usize = 31;
    let original = bank(
        154,
        TestHircObject::actor_mixer(1, &[], TestNodeBase::Plain),
    );
    let parsed = Soundbank::parse(&original)?;
    let object = parsed.objects().first().ok_or("original synthetic node")?;
    let original_body = parsed.object_body(object);
    let start = size_of::<u32>();
    Ok(original_body
        .get(start..start + PLAIN_NODE_BYTES)
        .ok_or("original synthetic NodeBase")?
        .to_vec())
}

pub(crate) fn layout(record: &MediaRecord, expected: &str, status: Status) {
    assert_eq!(
        record.status, status,
        "separate original coverage for {}",
        record.subresource.namespace
    );
    assert_eq!(
        record.data.pointer("/payload/layout"),
        Some(&json!(expected)),
        "original layout for {}",
        record.subresource.namespace
    );
}

pub(crate) fn export(
    source: &[u8],
    encrypted: bool,
    expected: HircLayout,
    inspect: impl FnOnce(&[MediaRecord], &Evidence<'_>) -> TestResult,
) -> TestResult {
    verify_layout(source, expected)?;
    export_observed(
        source,
        encrypted,
        expected == HircLayout::Malformed,
        inspect,
    )
}

pub(crate) fn export_raw_bank(
    source: &[u8],
    encrypted: bool,
    inspect: impl FnOnce(&[MediaRecord], &Evidence<'_>) -> TestResult,
) -> TestResult {
    export_observed(source, encrypted, false, inspect)
}

fn export_observed(
    source: &[u8],
    encrypted: bool,
    diagnostic: bool,
    inspect: impl FnOnce(&[MediaRecord], &Evidence<'_>) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let entry = TestEntry::zstd(PATH, source);
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
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
    let rows = support::records(&root, &index, "audio.jsonl")?;
    let base = root.join(&index.artifact_base);
    check_retained(&base, row(&rows, "audio-bank-object", &[1, 0])?.1, source)?;
    assert_eq!(
        row(&rows, "audio-bank-object", &[1, 1])?
            .1
            .data
            .pointer("/payload/object_id"),
        Some(&json!(1)),
        "an incomplete node prefix stops before the next physical object",
    );
    let evidence = Evidence {
        source,
        origins: Origins::read(&base)?,
        encrypted,
    };
    inspect(&rows, &evidence)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in &rows {
        schema
            .validate(&serde_json::to_value(record)?)
            .map_err(|cause| cause.to_string())?;
    }
    Ok(())
}

pub(crate) struct Evidence<'a> {
    source: &'a [u8],
    origins: Origins,
    encrypted: bool,
}

impl Evidence<'_> {
    pub(crate) fn word(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u32,
    ) -> TestResult {
        self.scalar(row, field, label, value, &value.to_le_bytes())
    }

    pub(crate) fn byte(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u8,
    ) -> TestResult {
        self.scalar(row, field, label, u32::from(value), &[value])
    }

    pub(crate) fn halfword(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u16,
    ) -> TestResult {
        self.scalar(row, field, label, u32::from(value), &value.to_le_bytes())
    }

    pub(crate) fn conversion(
        &self,
        (position, record): (usize, &MediaRecord),
        field: &str,
        original: &str,
        label: &str,
    ) -> TestResult {
        let span = record
            .data
            .pointer(&format!("/payload/{original}/range"))
            .ok_or("original conversion span")?;
        self.receipt(
            position,
            &format!("/data/payload/{field}"),
            "conversion",
            label,
            span,
        )
    }

    pub(crate) fn index(
        &self,
        row_position: usize,
        index: u8,
        selector: &MediaRecord,
        field: &str,
        label: &str,
    ) -> TestResult {
        let span = selector
            .data
            .pointer(&format!("/payload/{field}/range"))
            .ok_or("original index selector span")?;
        self.receipt(
            row_position,
            &format!("/subresource/indices/{index}"),
            "selector",
            label,
            span,
        )
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
            "{field} retains its original value"
        );
        let range = format!("{field}/range");
        assert_eq!(
            source_span(self.source, &record.data, &range)?,
            bytes,
            "{field} selects its exact original bytes"
        );
        let span = record.data.pointer(&range).ok_or("original scalar span")?;
        self.receipt(
            position,
            &format!("/data{field}/value"),
            "payload",
            label,
            span,
        )
    }

    fn receipt(
        &self,
        position: usize,
        field: &str,
        role: &str,
        label: &str,
        span: &Value,
    ) -> TestResult {
        let indices = json!([span.get("start"), span.get("end_exclusive")]);
        let receipt = self
            .origins
            .field("records", position, field, self.encrypted)?;
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
                .any(|used| used.get("role") == Some(&json!(role))
                    && used.pointer("/location/field") == Some(&json!(label))
                    && used.pointer("/location/indices") == Some(&indices)),
            "{field} receipt identifies the same physical container bank and scalar",
        );
        Ok(())
    }
}
