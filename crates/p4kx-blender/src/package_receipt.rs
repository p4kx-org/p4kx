// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_extraction::{OriginArtifactDigest, OriginDigestWriter};
use serde::Serialize;

use crate::{BuildInfo, MANIFEST_FILE, Manifest, PackageError, PackageResult, PackageWriteContext};

pub(crate) const RECEIPT_FILE: &str = "receipt.json";
const RECEIPT_SCHEMA: u8 = 1;

#[derive(Serialize)]
struct Receipt<'a> {
    schema: u8,
    generator: Generator<'a>,
    archive_build: Option<&'a BuildInfo>,
    files: Vec<PackageFile>,
}

#[derive(Serialize)]
struct Generator<'a> {
    name: &'a str,
    version: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_revision: Option<&'static str>,
}

#[derive(Serialize)]
pub(crate) struct PackageFile {
    pub(crate) path: String,
    sha256: String,
    #[serde(skip)]
    bytes: u64,
}

impl PackageFile {
    pub(crate) fn new(path: String, digest: &OriginArtifactDigest) -> Self {
        Self {
            path,
            sha256: digest.sha256().to_owned(),
            bytes: digest.byte_length(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PackageTotals {
    pub(crate) files: usize,
    pub(crate) bytes: u64,
}

pub(crate) fn write_metadata(
    staging: &Path,
    manifest: &Manifest,
    mut files: Vec<PackageFile>,
    context: &PackageWriteContext<'_>,
) -> PackageResult<PackageTotals> {
    let manifest_digest = write_json(&staging.join(MANIFEST_FILE), manifest, context, true)?;
    files.push(PackageFile::new(MANIFEST_FILE.to_owned(), &manifest_digest));
    files.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    let receipt_path = staging.join(RECEIPT_FILE);
    let expected = std::iter::once(MANIFEST_FILE)
        .chain(manifest.meshes.iter().map(|mesh| mesh.file.as_str()))
        .chain(
            manifest
                .textures
                .iter()
                .map(|texture| texture.file.as_str()),
        )
        .chain(manifest.authored_soc.iter().map(|soc| soc.file.as_str()))
        .chain(
            manifest
                .collision_helpers
                .iter()
                .map(|helper| helper.file.as_str()),
        );
    let mut expected: Vec<_> = expected.collect();
    expected.sort_unstable();
    if expected
        .into_iter()
        .ne(files.iter().map(|file| file.path.as_str()))
    {
        return Err(PackageError::write(
            &receipt_path,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "written package artifacts differ from the manifest",
            ),
        ));
    }
    let files_len = files.len() + 1;
    let payload_bytes = files.iter().map(|file| file.bytes).sum::<u64>();
    let receipt = Receipt {
        schema: RECEIPT_SCHEMA,
        generator: Generator {
            name: &manifest.generator.name,
            version: &manifest.generator.version,
            source_revision: source_revision(option_env!("P4KX_SOURCE_REVISION")),
        },
        archive_build: manifest.source.build.as_ref(),
        files,
    };
    let digest = write_json(&receipt_path, &receipt, context, true)?;
    Ok(PackageTotals {
        files: files_len,
        bytes: payload_bytes + digest.byte_length(),
    })
}

fn source_revision(candidate: Option<&'static str>) -> Option<&'static str> {
    candidate.filter(|value| {
        matches!(value.len(), 40 | 64)
            && value
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

pub(crate) fn write_json(
    path: &Path,
    data: &impl Serialize,
    context: &PackageWriteContext<'_>,
    pretty: bool,
) -> PackageResult<OriginArtifactDigest> {
    let (file, ownership) = context.create_file(path)?.into_parts();
    let mut sink = BufWriter::new(file);
    let mut writer = OriginDigestWriter::new(&mut sink);
    let result = if pretty {
        serde_json::to_writer_pretty(&mut writer, data)
    } else {
        serde_json::to_writer(&mut writer, data)
    };
    result.map_err(|error| PackageError::write(path, error.into()))?;
    writer
        .write_all(b"\n")
        .map_err(|source| PackageError::write(path, source))?;
    let digest = writer
        .finish()
        .map_err(|source| PackageError::write(path, source))?;
    ownership.ensure_writable(path, context.cancel_flag())?;
    sink.get_ref()
        .sync_all()
        .map_err(|source| PackageError::write(path, source))?;
    Ok(digest)
}

#[cfg(test)]
#[path = "package_receipt_tests.rs"]
mod tests;
