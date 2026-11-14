// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::{Compression, GzBuilder};
use p4kx_extraction::material_source_info::MaterialSourceEvidence;
use p4kx_extraction::{
    EntryOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope, MaterialLibrarySourceInfo,
    OriginDigestWriter, PhysicalSource, SourceInput, SourceUseDescription, SourceUses,
};
use p4kx_model::MaterialSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::package_receipt::PackageFile;
use crate::{PackageError, PackageResult, PackageWriteContext};

pub(crate) const DIRECTORY: &str = "material-sources";
const FORMAT: &str = "p4kx-blender-material-source";
const VERSION: u8 = 1;

type Sink = GzEncoder<OriginDigestWriter<BufWriter<std::fs::File>>>;

/// Package-local reference to the retained declarations of one material-library row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialSourceReference {
    /// The sidecar path derived from the zero-based position of its library row.
    #[schemars(regex(pattern = "^material-sources/s[0-9]{4,}\\.jsonl\\.gz$"))]
    pub file: String,
    /// The material-source sidecar's schema version.
    #[schemars(extend("const" = VERSION))]
    pub schema_version: u8,
}

impl MaterialSourceReference {
    pub(crate) fn new(ordinal: usize) -> Self {
        Self {
            file: file_name(ordinal),
            schema_version: VERSION,
        }
    }

    pub(crate) fn is_valid(&self, ordinal: usize) -> bool {
        self.schema_version == VERSION && self.file == file_name(ordinal)
    }
}

fn file_name(ordinal: usize) -> String {
    format!("{DIRECTORY}/s{ordinal:04}.jsonl.gz")
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Row<'a> {
    Header {
        #[schemars(extend("const" = FORMAT))]
        format: &'static str,
        #[schemars(extend("const" = VERSION))]
        version: u8,
        library: usize,
    },
    Library {
        authored: &'a MaterialLibrarySourceInfo<'a>,
    },
    Source {
        source: &'a PhysicalSource,
    },
    SourceUse {
        #[serde(rename = "sourceUse")]
        source_use: &'a SourceUseDescription,
    },
    Field {
        path: &'a FieldPath,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
    },
    End {
        sources: usize,
        uses: usize,
        fields: usize,
    },
}

/// Returns the schema of one streamed authored material-library row.
#[must_use]
pub fn material_source_sidecar_schema() -> serde_json::Value {
    schemars::schema_for!(Row<'static>).to_value()
}

pub(crate) fn write(
    staging: &Path,
    ordinal: usize,
    owner: &MaterialSource,
    entry: Option<&EntryOrigin>,
    context: &PackageWriteContext<'_>,
) -> PackageResult<PackageFile> {
    let file_name = file_name(ordinal);
    let path = staging.join(&file_name);
    let (file, ownership) = context.create_file(&path)?.into_parts();
    let writer = OriginDigestWriter::new(BufWriter::new(file));
    let mut sink = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(writer, Compression::default());
    let write_error = |source| PackageError::write(&path, source);
    row(
        &mut sink,
        &Row::Header {
            format: FORMAT,
            version: VERSION,
            library: ordinal,
        },
    )
    .map_err(write_error)?;
    let authored = MaterialLibrarySourceInfo::of_source(owner);
    row(
        &mut sink,
        &Row::Library {
            authored: &authored,
        },
    )
    .map_err(write_error)?;
    write_origins(&mut sink, owner, entry, context, &path)?;
    context.check()?;
    ownership.ensure_writable(&path, context.cancel_flag())?;
    let writer = sink.finish().map_err(write_error)?;
    let (digest, file) = writer.finish_with_sink().map_err(write_error)?;
    file.get_ref().sync_all().map_err(write_error)?;
    Ok(PackageFile::new(file_name, &digest))
}

fn write_origins(
    sink: &mut Sink,
    owner: &MaterialSource,
    entry: Option<&EntryOrigin>,
    context: &PackageWriteContext<'_>,
    path: &Path,
) -> PackageResult<()> {
    let uses = SourceUses::with_compact_use_refs();
    let scope = entry
        .map(|entry| uses.observe(SourceInput::Archive(entry), InputScope::Payload))
        .transpose()
        .map_err(evidence_error)?;
    let complete = entry.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
    let selection = uses.shared_origin(&[], complete);
    let mut fields = FieldOrigins::new();
    let output = FieldPath::parse("/authored").map_err(evidence_error)?;
    MaterialSourceEvidence::new(&uses, scope.as_ref(), &selection)
        .project(owner, &output, &mut fields)
        .map_err(evidence_error)?;
    let inventory = uses.freeze().map_err(evidence_error)?;
    let write_error = |source| PackageError::write(path, source);
    for source in inventory.sources() {
        context.check()?;
        row(sink, &Row::Source { source }).map_err(write_error)?;
    }
    for source_use in inventory.uses() {
        context.check()?;
        row(sink, &Row::SourceUse { source_use }).map_err(write_error)?;
    }
    let mut count = 0;
    for (path, origin) in fields.iter() {
        context.check()?;
        row(sink, &Row::Field { path, origin }).map_err(write_error)?;
        count += 1;
    }
    row(
        sink,
        &Row::End {
            sources: inventory.sources().len(),
            uses: inventory.uses().len(),
            fields: count,
        },
    )
    .map_err(write_error)
}

fn row(sink: &mut Sink, row: &Row<'_>) -> io::Result<()> {
    serde_json::to_writer(&mut *sink, row)?;
    sink.write_all(b"\n")
}

fn evidence_error(error: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::MaterialSourceEvidence(Box::new(error))
}
