// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_shader::Limits as ShaderLimits;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::Value;

use crate::checkpoint::Checkpoint;
use crate::origin_read_policy::OriginReadPolicy;
use crate::origin_test_support as origins;
use crate::origin_verify::OriginAvailability;
use crate::{ExportOptions, ExportOutcome, Index, ProviderRegistry, ShaderProvider, origin_verify};

pub(super) type TestResult = Result<(), Box<dyn Error>>;
pub(super) const SOURCE: &[u8] = b"Property { Name(%MissingClose);";
const TINY_LIMIT: usize = 1;
const UNSUPPORTED_DEPTH: usize = 129;

#[derive(Debug, Clone, Copy)]
pub(super) enum Scene {
    Plain,
    EncryptedLeaf,
    EncryptedOuter,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Failure {
    Syntax,
    SourceLimit,
    TokenLimit,
    ConfigurationDepth,
    ParsedDepth,
    SourceAndConfigurationDepth,
}

pub(super) struct Fixture {
    _folder: tempfile::TempDir,
    archive: Archive,
    output: PathBuf,
    root: PathBuf,
    pub(super) index: Index,
    pub(super) record: Value,
    fields: Vec<Value>,
    inventory: Vec<Value>,
    sources: Vec<Value>,
    options: ExportOptions,
}

impl Fixture {
    pub(super) fn new(scene: Scene, failure: Failure) -> Result<Self, Box<dyn Error>> {
        let folder = tempfile::tempdir()?;
        let archive = open_archive(folder.path(), scene)?;
        let limits = failure.limits();
        let registry =
            ProviderRegistry::try_new(vec![Box::new(ShaderProvider::new_with_limits(limits))])?;
        let options = ExportOptions {
            diagnostic: true,
            ..ExportOptions::default()
        };
        let output = folder.path().join("media");
        let ExportOutcome::Incomplete { index, diagnostic } =
            crate::export_with(&archive, &output, &options, &registry)?
        else {
            return Err(
                "the genuine malformed shader retains an incomplete diagnostic generation".into(),
            );
        };
        let root = diagnostic
            .parent()
            .ok_or("actual diagnostic index directory")?
            .join(&index.artifact_base);
        let records = origins::rows(&root, &index, "shader.jsonl")?;
        assert_eq!(
            records.len(),
            1,
            "the actual shader producer emits one failure occurrence"
        );
        let record = records.into_iter().next().ok_or("actual failure payload")?;
        let fields = origins::rows(&root, &index, "field-origins.jsonl")?;
        let inventory = origins::rows(&root, &index, "origin-uses.jsonl")?;
        let sources = origins::rows(&root, &index, "origin-sources.jsonl")?;
        Ok(Self {
            _folder: folder,
            archive,
            output,
            root,
            index: *index,
            record,
            fields,
            inventory,
            sources,
            options,
        })
    }

    pub(super) fn field(&self, pointer: &str) -> Result<&Value, Box<dyn Error>> {
        self.fields
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("records")
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("shader")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some("0")
                    && row.get("field").and_then(Value::as_str) == Some(pointer)
            })
            .ok_or_else(|| format!("actual shader occurrence field {pointer}").into())
    }

    pub(super) fn uses(&self, field: &Value) -> Result<Vec<&Value>, Box<dyn Error>> {
        field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("immediate contributing references")?
            .iter()
            .map(|reference| {
                self.inventory
                    .iter()
                    .find(|row| row.get("id") == Some(reference))
                    .ok_or_else(|| "an actual contributing source use is retained".into())
            })
            .collect()
    }

    pub(super) fn domain(&self, pointer: &str, scope: &str, encrypted: bool) -> TestResult {
        let field = self.field(pointer)?;
        assert_eq!(
            origins::marker(field),
            Some(encrypted),
            "{pointer}: exact consumed byte domain"
        );
        assert_eq!(
            origins::complete(field),
            Some(true),
            "{pointer}: the actual input is fully mapped"
        );
        let references = self.uses(field)?;
        assert_eq!(
            references.len(),
            1,
            "{pointer}: one actual owning source use"
        );
        let used = references.first().ok_or("actual mapped use")?;
        let physical = self
            .sources
            .iter()
            .find(|source| {
                source.pointer("/path/value").and_then(Value::as_str) == Some("source.ext")
            })
            .ok_or("original observed shader Entry")?;
        assert_eq!(
            used.get("source"),
            physical.get("id"),
            "the actual leaf owns its field"
        );
        assert_eq!(used.get("scope").and_then(Value::as_str), Some(scope));
        assert_eq!(used.get("role").and_then(Value::as_str), Some("conversion"));
        assert_eq!(
            used.pointer("/location/type").and_then(Value::as_str),
            Some("subresource")
        );
        assert_eq!(
            used.pointer("/location/namespace").and_then(Value::as_str),
            Some("shader-definition-failure")
        );
        let location = pointer
            .strip_prefix("/data/payload")
            .ok_or("field in the shader failure payload")?;
        assert_eq!(
            used.pointer("/location/field").and_then(Value::as_str),
            Some(location)
        );
        Ok(())
    }

    pub(super) fn independent(&self, pointer: &str) -> TestResult {
        let field = self.field(pointer)?;
        assert_eq!(
            origins::marker(field),
            Some(false),
            "{pointer}: literal configuration or control"
        );
        assert_eq!(origins::complete(field), Some(true));
        assert_eq!(
            self.uses(field)?.len(),
            0,
            "a literal has no invented input contributor"
        );
        Ok(())
    }

    pub(super) fn metadata_only(&self) -> TestResult {
        for field in &self.fields {
            let Some(pointer) = field.get("field").and_then(Value::as_str) else {
                continue;
            };
            if !pointer.starts_with("/data")
                || field.pointer("/scope/stream/type").and_then(Value::as_str) != Some("records")
                || field.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("shader")
            {
                continue;
            }
            for contribution in self.uses(field)? {
                assert_eq!(
                    contribution.get("scope").and_then(Value::as_str),
                    Some("archiveDirectoryMetadata"),
                    "pre-read failure datum {pointer} never borrows an unused leaf payload"
                );
            }
        }
        Ok(())
    }

    pub(super) fn preserved(&self) -> TestResult {
        assert_eq!(
            self.record.get("status").and_then(Value::as_str),
            Some("failed")
        );
        assert_eq!(
            self.record
                .pointer("/data/record_type")
                .and_then(Value::as_str),
            Some("definition-failure")
        );
        assert_eq!(
            self.record.pointer("/data/payload/counts"),
            Some(&Value::Null)
        );
        let raw = self
            .record
            .pointer("/source/raw/path")
            .and_then(Value::as_str)
            .ok_or("exact retained source bytes")?;
        assert_eq!(fs::read(self.root.join(raw))?, SOURCE);
        let summary = self
            .index
            .kinds
            .first()
            .ok_or("actual shader counting unit")?;
        assert_eq!(summary.counts.records.enumerated, 1);
        assert_eq!(summary.counts.records.failed, 1);
        assert!(
            !self.index.complete,
            "origin transport does not heal a failed semantic projection"
        );
        assert!(
            !self.output.join("media-index.json").exists(),
            "diagnostics cannot replace a complete export"
        );
        let watch = ArchiveWatch::new(&self.archive);
        let protection =
            ArchiveWriteProtection::new(self.archive.path(), self.options.cancel.flag())?;
        let checkpoint = Checkpoint {
            cancel: &self.options.cancel,
            watch: &watch,
            protection: &protection,
        };
        let receipts = self
            .index
            .field_origins
            .as_ref()
            .ok_or("actual current origin receipts")?;
        assert_eq!(
            origin_verify::generation(
                &self.root,
                &self.index,
                checkpoint,
                OriginReadPolicy::from_receipts(receipts),
            )?,
            OriginAvailability::Current,
            "the actual reader verifies every diagnostic origin occurrence"
        );
        Ok(())
    }
}

impl Failure {
    fn limits(self) -> ShaderLimits {
        match self {
            Self::Syntax => ShaderLimits::default(),
            Self::SourceLimit => ShaderLimits {
                source_bytes: TINY_LIMIT,
                ..ShaderLimits::default()
            },
            Self::TokenLimit => ShaderLimits {
                token_bytes: TINY_LIMIT,
                ..ShaderLimits::default()
            },
            Self::ConfigurationDepth => ShaderLimits {
                depth: UNSUPPORTED_DEPTH,
                ..ShaderLimits::default()
            },
            Self::ParsedDepth => ShaderLimits {
                depth: TINY_LIMIT,
                ..ShaderLimits::default()
            },
            Self::SourceAndConfigurationDepth => ShaderLimits {
                source_bytes: TINY_LIMIT,
                depth: UNSUPPORTED_DEPTH,
                ..ShaderLimits::default()
            },
        }
    }
}

fn open_archive(folder: &Path, scene: Scene) -> Result<Archive, Box<dyn Error>> {
    let archive_path = folder.join("Data.p4k");
    let leaf = TestEntry::stored("source.ext", SOURCE);
    let entry = match scene {
        Scene::Plain => leaf,
        Scene::EncryptedLeaf => TestEntry::zstd("source.ext", SOURCE).encrypted(),
        Scene::EncryptedOuter => {
            let nested = TestContainer::new().entry(leaf).build();
            TestEntry::zstd("outer.socpak", nested).encrypted()
        }
    };
    ArchiveBuilder::new().entry(entry).write_to(&archive_path)?;
    Ok(Archive::open_with_key(
        &archive_path,
        Some(p4kx_testkit::test_key()),
    )?)
}
