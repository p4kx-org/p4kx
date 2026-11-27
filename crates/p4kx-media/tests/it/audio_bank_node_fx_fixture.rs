// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOutcome, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::json;

use super::super::audio_bank_state_provider::{check_retained, options, row, source_span};
use super::super::support;
use super::super::support::audio_provider::{Origins, Owner, TestResult};

const PATH: &str = "node-routing.bnk";
pub(super) const EFFECT_ID: u32 = 0xB707_2041;
pub(super) const REFLECTIONS: u32 = 0xC408_A003;
pub(super) const NODE_FX: [u8; 23] = [
    0xA1, 2, 0xE0, 4, 0x41, 0x20, 0x07, 0xB7, 0x87, 4, 0, 0, 0, 0, 0x30, 0x82, 1, 0, 0x41, 0x20,
    0x07, 0xB7, 0xFE,
];
pub(super) const NO_FX: [u8; 4] = [0, 0, 0, 0];
pub(super) const AUX_WORDS: [u32; 4] = [0, 0x9001_22EF, 0x9001_22EF, 0xFFFF_FFFF];
const BASE_MIDDLE: [u8; 12] = [0; 12];
const BASE_SUFFIX: [u8; 10] = [0, 1, 0, 0, 3, 0x0C, 0, 0, 0, 0];

pub(super) fn bank(
    sound: bool,
    version: u32,
    effects: &[u8],
    aux: &[u8],
    finished: bool,
) -> Vec<u8> {
    let mut body = Vec::new();
    if sound {
        body.extend(0x0004_0001_u32.to_le_bytes());
        body.push(2);
        body.extend([1_u32, 2, 0].into_iter().flat_map(u32::to_le_bytes));
        body.push(0);
    }
    body.extend(effects);
    if !aux.is_empty() {
        body.extend(BASE_MIDDLE);
        body.extend(aux);
    }
    if finished {
        body.extend(BASE_SUFFIX);
        if !sound {
            body.extend(0_u32.to_le_bytes());
        }
    }
    TestBank::new(5)
        .version(version)
        .object(TestHircObject::raw(if sound { 2 } else { 7 }, 123, body))
        .object(TestHircObject::event(1, &[]))
        .bytes()
}

pub(super) fn aux(user: bool) -> Vec<u8> {
    let mut bytes = vec![if user { 0xE8 } else { 0xE0 }];
    if user {
        bytes.extend(AUX_WORDS.into_iter().flat_map(u32::to_le_bytes));
    }
    bytes.extend(if user { REFLECTIONS } else { 0 }.to_le_bytes());
    bytes
}

fn archive(folder: &Path, source: &[u8], encrypted: bool) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    let entry = TestEntry::zstd(PATH, source);
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

pub(super) fn export(
    source: &[u8],
    encrypted: bool,
    diagnostic: bool,
    check: impl FnOnce(&[MediaRecord], &Evidence<'_>) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(folder.path(), source, encrypted)?;
    let output = folder.path().join("out");
    let mut export_options = options();
    export_options.diagnostic = diagnostic;
    let (index, root) = match p4kx_media::export(&archive, &output, &export_options)? {
        ExportOutcome::Published { index, .. } if !diagnostic => (index, output),
        ExportOutcome::Incomplete {
            index,
            diagnostic: report,
        } if diagnostic => (
            index,
            report
                .parent()
                .ok_or("diagnostic generation root")?
                .to_path_buf(),
        ),
        _ => return Err("unexpected publication outcome".into()),
    };
    let records = support::records(&root, &index, "audio.jsonl")?;
    let base = root.join(&index.artifact_base);
    check_retained(
        &base,
        row(&records, "audio-bank-object", &[1, 0])?.1,
        source,
    )?;
    assert_eq!(
        row(&records, "audio-bank-object", &[1, 1])?
            .1
            .data
            .pointer("/payload/object_id"),
        Some(&json!(1)),
        "the adjacent Event retains its original physical object"
    );
    let evidence = Evidence {
        source,
        origins: Origins::read(&base)?,
        encrypted,
    };
    check(&records, &evidence)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in &records {
        schema
            .validate(&serde_json::to_value(record)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) struct Evidence<'a> {
    source: &'a [u8],
    origins: Origins,
    encrypted: bool,
}

impl Evidence<'_> {
    pub(super) fn byte(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u8,
    ) -> TestResult {
        self.scalar(row, field, label, u32::from(value), &[value])
    }

    pub(super) fn word(
        &self,
        row: (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: u32,
    ) -> TestResult {
        self.scalar(row, field, label, value, &value.to_le_bytes())
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
            "the typed {field} retains the original scalar value"
        );
        assert_eq!(
            source_span(self.source, &record.data, &format!("{field}/range"))?,
            bytes,
            "the {field} range selects the original bytes"
        );
        let span = record
            .data
            .pointer(&format!("{field}/range"))
            .ok_or("original scalar span")?;
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
            uses.iter().any(|used| {
                used.get("role") == Some(&json!("payload"))
                    && used.pointer("/location/field") == Some(&json!(label))
                    && used.pointer("/location/indices") == Some(&indices)
            }),
            "the {field} receipt selects the exact owning-bank bytes"
        );
        Ok(())
    }
}
