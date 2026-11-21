// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::PathBuf;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{
    ArchiveProvider, ExportOptions, ExportOutcome, Index, MediaKind, ProviderRegistry,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(super) const MANIFEST: &str = "build_manifest.id";
pub(super) const FIELDS: [(&str, &str); 4] = [
    ("branch", "Data.Branch"),
    ("version", "Data.Version"),
    ("change_number", "Data.RequestedP4ChangeNum"),
    ("build_id", "Data.BuildId"),
];

#[derive(Clone, Copy)]
pub(super) enum Input<'a> {
    Missing,
    Unreadable,
    File(&'a [u8]),
}

pub(super) struct Channel {
    pub(super) folder: tempfile::TempDir,
    pub(super) archive: Archive,
}

impl Channel {
    pub(super) fn new(input: Input<'_>, twin: bool) -> TestResult<Self> {
        let folder = tempfile::tempdir()?;
        let manifest = folder.path().join(MANIFEST);
        match input {
            Input::Missing => {}
            Input::Unreadable => fs::create_dir(&manifest)?,
            Input::File(bytes) => fs::write(&manifest, bytes)?,
        }
        let entry = if twin {
            let Input::File(bytes) = input else {
                return Err("twin needs actual manifest bytes".into());
            };
            TestEntry::zstd(MANIFEST, bytes).encrypted()
        } else {
            TestEntry::stored("source.bin", b"synthetic payload")
        };
        let path = folder.path().join("Data.p4k");
        ArchiveBuilder::new().entry(entry).write_to(&path)?;
        let archive =
            Archive::open_with(path, OpenOptions::new().key(Some(p4kx_testkit::test_key())))?;
        Ok(Self { folder, archive })
    }

    pub(super) fn export(
        &self,
        name: &str,
        options: &ExportOptions,
        registry: &ProviderRegistry,
    ) -> TestResult<Run> {
        let output = self.folder.path().join(name);
        let ExportOutcome::Published { index, .. } =
            p4kx_media::export_with(&self.archive, &output, options, registry)?
        else {
            return Err("expected actual complete Archive metadata generation".into());
        };
        let base = output.join(&index.artifact_base);
        let records = if index
            .kinds
            .iter()
            .any(|kind| kind.provider.kind == MediaKind::Archive)
        {
            rows(base.join("archive.jsonl"))?
        } else {
            Vec::new()
        };
        Ok(Run {
            records,
            fields: rows(base.join("field-origins.jsonl"))?,
            uses: rows(base.join("origin-uses.jsonl"))?,
            sources: rows(base.join("origin-sources.jsonl"))?,
            output,
            base,
            index: *index,
        })
    }
}

pub(super) struct Run {
    pub(super) output: PathBuf,
    pub(super) base: PathBuf,
    pub(super) index: Index,
    pub(super) records: Vec<Value>,
    pub(super) fields: Vec<Value>,
    pub(super) uses: Vec<Value>,
    pub(super) sources: Vec<Value>,
}

impl Run {
    pub(super) fn root(&self) -> TestResult<(usize, &Value)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, record)| {
                record
                    .pointer("/subresource/namespace")
                    .and_then(Value::as_str)
                    == Some("archive-root")
            })
            .ok_or_else(|| "actual Archive root row".into())
    }

    pub(super) fn field(&self, stream: &str, pointer: &str) -> TestResult<&Value> {
        let ordinal = if stream == "records" {
            self.root()?.0.to_string()
        } else {
            "0".to_owned()
        };
        let field = self.fields.iter().find(|row| {
            row.get("type").and_then(Value::as_str) == Some("field")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some(stream)
                && (stream != "records"
                    || row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("archive"))
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                && row.get("field").and_then(Value::as_str) == Some(pointer)
        });
        assert!(
            field.is_some(),
            "actual {stream} output needs its immediate field origin at {pointer}"
        );
        field.ok_or_else(|| "asserted immediate origin".into())
    }

    pub(super) fn contributions<'a>(&'a self, field: &Value) -> TestResult<Vec<&'a Value>> {
        references(field)?
            .iter()
            .map(|reference| {
                self.uses
                    .iter()
                    .find(|used| used.get("id") == Some(reference))
                    .ok_or_else(|| "actual shared use".into())
            })
            .collect()
    }

    pub(super) fn physical(&self, used: &Value) -> TestResult<&Value> {
        self.sources
            .iter()
            .find(|source| source.get("id") == used.get("source"))
            .ok_or_else(|| "actual shared physical source".into())
    }
}

pub(super) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Archive],
        diagnostic: true,
        ..ExportOptions::default()
    }
}

pub(super) fn registry() -> Result<ProviderRegistry, p4kx_media::MediaError> {
    ProviderRegistry::try_new(vec![Box::new(ArchiveProvider)])
}

pub(super) fn manifest(value: &str) -> TestResult<Vec<u8>> {
    Ok(serde_json::to_vec(
        &json!({"Data":{"Branch":value,"Version":value,"RequestedP4ChangeNum":value,"BuildId":value}}),
    )?)
}

pub(super) fn rows(path: PathBuf) -> TestResult<Vec<Value>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

pub(super) fn references(field: &Value) -> TestResult<&Vec<Value>> {
    field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or_else(|| "source use reference array".into())
}
