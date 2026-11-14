// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_extraction::{Build, Entry, EntryMetadataInfo};
use serde::Serialize;

/// Writes `value` as one line of compact JSON to stdout.
pub(crate) fn print(value: &impl Serialize) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    serde_json::to_writer(&mut out, value)?;
    writeln!(out)?;
    out.flush()
}

/// Returns a path as JSON text.
pub(crate) fn path(path: &Path) -> String {
    path.display().to_string()
}

/// The JSON shape of a Build.
#[derive(Serialize)]
pub(crate) struct BuildJson<'a> {
    branch: &'a str,
    version: &'a str,
    change_number: &'a str,
    build_id: &'a str,
    public_patch: Option<&'a str>,
}

impl<'a> From<&'a Build> for BuildJson<'a> {
    fn from(build: &'a Build) -> Self {
        Self {
            branch: build.branch(),
            version: build.version(),
            change_number: build.change_number(),
            build_id: build.build_id(),
            public_patch: build.public_patch(),
        }
    }
}

/// The JSON shape of an Entry.
#[derive(Serialize)]
pub(crate) struct EntryJson<'a> {
    path: &'a str,
    compression: String,
    encrypted: bool,
    compressed_size: u64,
    size: u64,
    metadata: EntryMetadataInfo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parts: Vec<&'a str>,
}

impl<'a> EntryJson<'a> {
    pub(crate) const fn metadata(&self) -> &EntryMetadataInfo {
        &self.metadata
    }

    /// Makes the Entry the row of a split texture: the sizes are those of the whole group, and
    /// `parts` are the paths of its other Entries.
    pub(crate) fn grouped(mut self, compressed_size: u64, size: u64, parts: Vec<&'a str>) -> Self {
        self.compressed_size = compressed_size;
        self.size = size;
        self.parts = parts;
        self
    }
}

impl<'a> From<&'a Entry> for EntryJson<'a> {
    fn from(entry: &'a Entry) -> Self {
        Self {
            path: entry.path(),
            compression: entry.compression().to_string(),
            encrypted: entry.is_encrypted(),
            compressed_size: entry.compressed_size(),
            size: entry.uncompressed_size(),
            metadata: EntryMetadataInfo::of(entry),
            parts: Vec::new(),
        }
    }
}
