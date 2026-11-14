// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::Archive;
use p4kx_media::{ExportOptions, ExportOutcome, Index, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) struct Exported {
    pub(super) _directory: tempfile::TempDir,
    pub(super) output: PathBuf,
    pub(super) index: Index,
    pub(super) materials: Vec<MediaRecord>,
    pub(super) material_rows: Vec<MediaRecord>,
    pub(super) shaders: Vec<MediaRecord>,
}

pub(super) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Shader, MediaKind::Material],
        ..ExportOptions::default()
    }
}

pub(super) fn archive(
    root: &Path,
    entries: Vec<TestEntry>,
) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok(Archive::open(path)?)
}

pub(super) fn run(
    entries: Vec<TestEntry>,
    options: &ExportOptions,
) -> Result<Exported, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let archive = archive(directory.path(), entries)?;
    let output = directory.path().join("media");
    let (index, base) = match p4kx_media::export(&archive, &output, options)? {
        ExportOutcome::Published { index, .. } => {
            let base = output.join(&index.artifact_base);
            (*index, base)
        }
        ExportOutcome::Incomplete { index, diagnostic } => (
            *index,
            diagnostic
                .parent()
                .ok_or("diagnostic parent")?
                .to_path_buf(),
        ),
        _ => return Err("unexpected media result".into()),
    };
    let material_rows = rows(&base.join("material.jsonl"))?;
    let materials = super::material_support::libraries(&material_rows)?;
    let shaders = rows(&base.join("shader.jsonl"))?;
    Ok(Exported {
        _directory: directory,
        output,
        index,
        materials,
        material_rows,
        shaders,
    })
}

fn rows(path: &Path) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

pub(super) fn material(shader: &str, flags: &str) -> TestEntry {
    TestEntry::stored(
        "Data/materials.mtl",
        format!("<Material Shader=\"{shader}\" StringGenMask=\"{flags}\"/>").into_bytes(),
    )
}

pub(super) fn container(path: &str, name: &str, shader: &[u8]) -> TestEntry {
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored(
            format!("Data/{name}.ext"),
            shader.to_vec(),
        ))
        .build();
    TestEntry::stored(path, bytes)
}

pub(super) fn links(result: &Exported) -> Result<&Vec<Value>, Box<dyn std::error::Error>> {
    result
        .materials
        .first()
        .and_then(|row| row.data.pointer("/payload/shader_references"))
        .and_then(Value::as_array)
        .ok_or_else(|| "material shader references".into())
}
