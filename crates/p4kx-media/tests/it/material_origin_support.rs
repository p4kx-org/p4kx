// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{
    ExportOptions, ExportOutcome, Index, MaterialProvider, MediaKind, ProviderRegistry,
    ShaderProvider,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(super) const MTL: &str = "Data/source.mtl";
pub(super) const SHADER: &str = "Data/demo.ext";
pub(super) const DEFINITION: &[u8] = b"Property { Name = %A\nMask = 0x1\n}\n";

pub(super) struct Fixture {
    pub(super) folder: tempfile::TempDir,
    pub(super) archive: Archive,
    pub(super) output: PathBuf,
    pub(super) base: PathBuf,
    pub(super) index: Index,
    pub(super) options: ExportOptions,
    pub(super) records: Vec<Value>,
    origins: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

pub(super) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Material, MediaKind::Shader],
        diagnostic: true,
        ..ExportOptions::default()
    }
}

pub(super) fn registry(
    limits: MaterialSourceLimits,
) -> Result<ProviderRegistry, p4kx_media::MediaError> {
    ProviderRegistry::try_new(vec![
        Box::new(MaterialProvider::new_with_limits(limits)),
        Box::new(ShaderProvider::default()),
    ])
}

impl Fixture {
    pub(super) fn new(entries: impl IntoIterator<Item = TestEntry>) -> TestResult<Self> {
        Self::with(entries, options(), MaterialSourceLimits::default())
    }

    pub(super) fn with(
        entries: impl IntoIterator<Item = TestEntry>,
        options: ExportOptions,
        limits: MaterialSourceLimits,
    ) -> TestResult<Self> {
        let folder = tempfile::tempdir()?;
        let archive = open_archive(folder.path(), entries)?;
        let output = folder.path().join("media");
        let (index, base) = export_at(&archive, &output, &options, limits)?;
        let records = rows(base.join("material.jsonl"))?;
        let origins = rows(base.join("field-origins.jsonl"))?;
        let uses = rows(base.join("origin-uses.jsonl"))?;
        let sources = rows(base.join("origin-sources.jsonl"))?;
        Ok(Self {
            folder,
            archive,
            output,
            base,
            index,
            options,
            records,
            origins,
            uses,
            sources,
        })
    }

    pub(super) fn record(&self, path: &str) -> TestResult<(usize, &Value)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, row)| {
                row.pointer("/source/identity/entry/path")
                    .and_then(Value::as_str)
                    == Some(path)
            })
            .ok_or_else(|| "original material row".into())
    }

    pub(super) fn field(&self, path: &str, pointer: &str) -> TestResult<&Value> {
        let ordinal = self.record(path)?.0.to_string();
        self.origins
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("records")
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("material")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some(pointer)
            })
            .ok_or_else(|| format!("original Material field {path}:{pointer}").into())
    }

    pub(super) fn marker(&self, path: &str, pointer: &str, expected: Option<bool>) -> TestResult {
        let field = self.field(path, pointer)?;
        let expected = expected.map_or(Value::Null, Value::Bool);
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&expected),
            "{path}:{pointer}: immediate marker is explicitly present and follows actual inputs"
        );
        Ok(())
    }

    pub(super) fn independent(&self, path: &str, pointer: &str) -> TestResult {
        self.marker(path, pointer, Some(false))?;
        let field = self.field(path, pointer)?;
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&Value::Bool(true)),
            "{pointer}: literal contributor inventory is complete"
        );
        assert!(
            references(field)?.is_empty(),
            "{pointer}: literal has no physical input"
        );
        Ok(())
    }

    pub(super) fn contributions(&self, path: &str, pointer: &str) -> TestResult<Vec<&Value>> {
        references(self.field(path, pointer)?)?
            .iter()
            .map(|reference| {
                self.uses
                    .iter()
                    .find(|row| row.get("id") == Some(reference))
                    .ok_or_else(|| "original source use".into())
            })
            .collect()
    }

    pub(super) fn input_path(&self, used: &Value) -> Option<&str> {
        let source = self
            .sources
            .iter()
            .find(|row| row.get("id") == used.get("source"))?;
        source.pointer("/path/value").and_then(Value::as_str)
    }

    pub(super) fn has_use(
        &self,
        path: &str,
        pointer: &str,
        input: &str,
        scope: &str,
        role: &str,
    ) -> TestResult<bool> {
        Ok(self.contributions(path, pointer)?.iter().any(|used| {
            self.input_path(used) == Some(input)
                && used.get("scope").and_then(Value::as_str) == Some(scope)
                && used.get("role").and_then(Value::as_str) == Some(role)
        }))
    }

    pub(super) fn has_root_inventory(&self, path: &str, pointer: &str) -> TestResult<bool> {
        Ok(self.contributions(path, pointer)?.iter().any(|used| {
            let source = self
                .sources
                .iter()
                .find(|row| row.get("id") == used.get("source"));
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.get("scope").and_then(Value::as_str) == Some("payload")
                && used.get("effectiveEncryption").and_then(Value::as_str) == Some("unknown")
                && source
                    .and_then(|source| source.get("kind"))
                    .and_then(Value::as_str)
                    == Some("archiveFile")
        }))
    }

    pub(super) fn no_payload(&self, path: &str, pointer: &str, input: &str) -> TestResult {
        assert!(
            !self.contributions(path, pointer)?.iter().any(|used| {
                self.input_path(used) == Some(input)
                    && used.get("scope").and_then(Value::as_str) == Some("payload")
            }),
            "{pointer}: an untouched or unused target payload cannot contribute"
        );
        Ok(())
    }
}

fn open_archive(
    folder: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(p4kx_testkit::test_key())),
    )?)
}

fn export_at(
    archive: &Archive,
    output: &Path,
    options: &ExportOptions,
    limits: MaterialSourceLimits,
) -> TestResult<(Index, PathBuf)> {
    match p4kx_media::export_with(archive, output, options, &registry(limits)?)? {
        ExportOutcome::Published { index, .. } => {
            let base = output.join(&index.artifact_base);
            Ok((*index, base))
        }
        ExportOutcome::Incomplete { index, diagnostic } => Ok((
            *index,
            diagnostic
                .parent()
                .ok_or("diagnostic parent")?
                .to_path_buf(),
        )),
        _ => Err("unexpected initial material export".into()),
    }
}

pub(super) fn references(field: &Value) -> TestResult<&Vec<Value>> {
    field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or_else(|| "field source-use array".into())
}

pub(super) fn rows(path: PathBuf) -> TestResult<Vec<Value>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

#[derive(Clone, Copy)]
pub(super) enum Storage {
    Plain,
    Encrypted,
    Unknown,
}

pub(super) fn material(path: &str, source: &[u8], storage: Storage) -> TestEntry {
    match storage {
        Storage::Plain => TestEntry::stored(path, source),
        Storage::Encrypted => TestEntry::zstd(path, source).encrypted(),
        Storage::Unknown => TestEntry::zstd(path, source)
            .encrypted()
            .with_encryption_field(7),
    }
}

pub(super) fn shader(storage: Storage) -> TestEntry {
    material(SHADER, DEFINITION, storage)
}
