// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, HircLayout, OpenOptions};
use p4kx_media::{ExportOutcome, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::{Value, json};

use super::super::audio_bank_state_provider::{
    check_retained, options, row, source_span, verify_layout,
};
use super::super::support;
use super::super::support::audio_provider::{Origins, Owner, TestResult};

const PATH: &str = "positioning.bnk";
const NODE_PREFIX: [u8; 15] = [0; 15];
const AUX: [u8; 5] = [0; 5];
const STATE_AND_RTPC: [u8; 4] = [0; 4];
pub(super) const ADVANCED: [u8; 6] = [0xF1, 0xD5, 0x34, 0x12, 0xE4, 0xBF];

#[derive(Clone, Copy)]
pub(super) enum Node {
    Sound,
    ActorMixer,
}

pub(super) fn positioning() -> Vec<u8> {
    let mut bytes = vec![0xE7, 0xB9, 0xFD];
    bytes.extend((-123_i32).to_le_bytes());
    bytes.extend(2_u32.to_le_bytes());
    bytes.extend(
        [0x7FC1_0023_u32, 0x8000_0000, 0x3FC0_0000]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes.extend((-5_i32).to_le_bytes());
    bytes.extend(
        [0xFF80_0000_u32, 0, 0x4000_0000, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes.extend([1_u32, 1, 1].into_iter().flat_map(u32::to_le_bytes));
    bytes.extend(
        [0x3F80_0000_u32, 0x3F00_0000, 0x7F80_0000]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes
}

#[derive(Clone, Copy)]
pub(super) enum Body<'a> {
    Absent,
    Complete(&'a [u8]),
    PositioningPrefix(&'a [u8]),
    AdvancedPrefix(&'a [u8]),
}

pub(super) fn bank(node: Node, version: u32, body: Body<'_>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let code = match node {
        Node::Sound => {
            bytes.extend(0x0004_0001_u32.to_le_bytes());
            bytes.push(2);
            bytes.extend([1_u32, 2, 0].into_iter().flat_map(u32::to_le_bytes));
            bytes.push(0);
            2
        }
        Node::ActorMixer => 7,
    };
    if !matches!(body, Body::Absent) {
        bytes.extend(NODE_PREFIX);
    }
    match body {
        Body::Absent => {}
        Body::Complete(positioning) => {
            bytes.extend(positioning);
            bytes.extend(AUX);
            bytes.extend(ADVANCED);
            bytes.extend(STATE_AND_RTPC);
            if matches!(node, Node::ActorMixer) {
                bytes.extend(0_u32.to_le_bytes());
            }
        }
        Body::PositioningPrefix(prefix) => bytes.extend(prefix),
        Body::AdvancedPrefix(prefix) => {
            bytes.push(0);
            bytes.extend(AUX);
            bytes.extend(prefix);
        }
    }
    TestBank::new(5)
        .version(version)
        .object(TestHircObject::raw(code, 123, bytes))
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
        row(&records, "audio-bank-object", &[1, 1])?
            .1
            .data
            .pointer("/payload/object_id"),
        Some(&json!(1)),
        "a partial position table cannot read the adjacent physical Event",
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
    pub(super) fn scalar(
        &self,
        (position, record): (usize, &MediaRecord),
        field: &str,
        label: &str,
        value: i64,
        bytes: &[u8],
    ) -> TestResult {
        let field = format!("/payload/{field}");
        assert_eq!(
            record.data.pointer(&format!("{field}/value")),
            Some(&json!(value)),
            "{field} preserves the authored value",
        );
        assert_eq!(
            source_span(self.source, &record.data, &format!("{field}/range"))?,
            bytes,
            "{field} has the exact original scalar bytes",
        );
        let span = record
            .data
            .pointer(&format!("{field}/range"))
            .ok_or("scalar range")?;
        self.receipt(
            position,
            &format!("/data{field}/value"),
            "payload",
            label,
            span,
        )
    }

    pub(super) fn conversion(
        &self,
        (position, record): (usize, &MediaRecord),
        field: &str,
        original: &str,
        label: &str,
    ) -> TestResult {
        let span = record
            .data
            .pointer(&format!("/payload/{original}/range"))
            .ok_or("conversion source range")?;
        self.receipt(
            position,
            &format!("/data/payload/{field}"),
            "conversion",
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
            "{field} receipt names the exact owning physical bank bytes",
        );
        Ok(())
    }
}
