// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::checkpoint::Checkpoint;
use crate::origin_contract::OriginReceipts;
use crate::origin_read_policy::OriginReadPolicy;
use crate::origin_verify::OriginAvailability;
use crate::{ExportOptions, ExportOutcome, Index, MediaError, ProviderRegistry};
use crate::{hashing, origin_verify};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(crate) struct Fixture {
    pub(crate) folder: tempfile::TempDir,
    pub(crate) archive: Archive,
    pub(crate) output: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) index: Index,
    pub(crate) options: ExportOptions,
}

impl Fixture {
    pub(crate) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_registry(&ProviderRegistry::default())
    }

    pub(crate) fn with_registry(
        registry: &ProviderRegistry,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let source = folder.path().join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("first.bin", b"same decoded bytes"))
            .entry(TestEntry::stored("second.bin", b"same decoded bytes"))
            .write_to(&source)?;
        let archive = Archive::open(source)?;
        let output = folder.path().join("media");
        let options = ExportOptions::default();
        let index = match crate::export_with(&archive, &output, &options, registry)? {
            ExportOutcome::Published { index, .. } => *index,
            _ => return Err("fixture must publish its first complete generation".into()),
        };
        let root = output.join(&index.artifact_base);
        Ok(Self {
            folder,
            archive,
            output,
            root,
            index,
            options,
        })
    }

    pub(crate) fn verify(&self, index: &Index) -> Result<OriginAvailability, MediaError> {
        let watch = ArchiveWatch::new(&self.archive);
        let protection =
            ArchiveWriteProtection::new(self.archive.path(), self.options.cancel.flag())?;
        let checkpoint = Checkpoint {
            cancel: &self.options.cancel,
            watch: &watch,
            protection: &protection,
        };
        let policy = index.field_origins.as_ref().map_or_else(
            || OriginReadPolicy::from_receipts(self.receipts()),
            OriginReadPolicy::from_receipts,
        );
        origin_verify::generation(&self.root, index, checkpoint, policy)
    }

    pub(crate) fn receipts(&self) -> &OriginReceipts {
        self.index
            .field_origins
            .as_ref()
            .expect("current writer emits its receipt group")
    }

    pub(crate) fn rows(&self) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
        let text = fs::read_to_string(self.root.join(&self.receipts().fields.path))?;
        text.lines()
            .map(|line| serde_json::from_str(line).map_err(Into::into))
            .collect()
    }

    pub(crate) fn rewrite(
        &self,
        index: &mut Index,
        rows: &[Value],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut bytes, row)?;
            bytes.push(b'\n');
        }
        let receipts = index.field_origins.as_mut().ok_or("current group")?;
        fs::write(self.root.join(&receipts.fields.path), &bytes)?;
        receipts.fields.size = u64::try_from(bytes.len())?;
        receipts.fields.digest.hex = hashing::hex(Sha256::digest(&bytes).into());
        Ok(())
    }
}

pub(crate) fn row_type(row: &Value) -> Option<&str> {
    row.get("type").and_then(Value::as_str)
}

pub(crate) fn is_record(row: &Value) -> bool {
    row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("records")
}
