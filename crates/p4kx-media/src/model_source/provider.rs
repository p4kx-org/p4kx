// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ChunkDirectory, ChunkDirectoryError, ChunkError, Container};

use super::companions;
use super::lod_distances;
use super::origins::{self, ModelOrigins};
use super::payload::{
    self, ModelChunkRecord, ModelEntryRecord, ModelSourceFamily, ModelSourcePayload,
};
use super::physics;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::required_reader::RequiredReader;
use crate::{
    Capability, FieldGroup, MediaError, MediaIssue, MediaKind, MediaWriter, OffsetBasis,
    Projection, Provider, Severity, SourceContext, SourceRange, Status, Subresource, error,
};

pub(crate) struct ModelSourceProvider;

impl Provider for ModelSourceProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Model,
            "p4kx.model.source",
            "1.3.0",
            "Selected original model and geometry Entry bytes with positioned chunk table records.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "chunk-directory",
                "Original table order, raw byte ranges, IDs, versions and physical source identities.",
            ),
            FieldGroup::new(
                "collision-shapes",
                "Original StatObjPhysics table slots and validated source-frame geometry.",
            ),
            FieldGroup::new(
                "companion-candidates",
                "Same-Archive geometry and LOD sibling identity and selection without target payload reads.",
            ),
            FieldGroup::new(
                "lod-distance-words",
                "Positioned original words of measured LodDistances chunks.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "model-semantics",
                "Render model decoding, material dependencies and entity-frame collision transforms are outside this projection.",
            ),
        ];
        capability
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let Some(family) = ModelSourceFamily::of(context.entry.path()) else {
            return Ok(Projection::Skipped);
        };
        let input = context.origin_input(writer)?;
        let origins = ModelOrigins {
            input: &input,
            entry: context.entry.index(),
        };
        let length = context.source.size;
        let budget = &context.directory.input_state()?.budget;
        let limits = writer.limits();
        let checkpoint = writer.checkpoint;
        let name = context.entry.path();
        let mut reader =
            RequiredReader::open(context.payload, name, length, budget, checkpoint, limits)?;
        let parsed = ChunkDirectory::read(
            &mut reader,
            length,
            |rows| {
                budget
                    .metadata(0, rows, limits, checkpoint)
                    .map_err(io::Error::other)
            },
            || checkpoint.check().map_err(io::Error::other),
        );
        match parsed {
            Ok(file) => {
                entry(family, &file, &origins, writer)?;
                companions::emit(context, &file, &origins, writer)?;
                for index in 0..file.chunks().len() {
                    writer.checkpoint()?;
                    chunk(&file, index, &origins, writer)?;
                    physics::emit(&file, index, &mut reader, &origins, writer)?;
                    lod_distances::emit(&file, index, &mut reader, &origins, writer)?;
                }
            }
            Err(ChunkDirectoryError::Format(ChunkError::Allocation)) => {
                return Err(MediaError::Contract(
                    "cannot allocate admitted model chunk directory".into(),
                ));
            }
            Err(ChunkDirectoryError::Format(error)) => {
                failed(family, length.min(16), &error, None, &origins, writer)?;
            }
            Err(ChunkDirectoryError::InvalidRow { source, range }) => {
                failed(family, 16, &source, Some(range), &origins, writer)?;
            }
            Err(ChunkDirectoryError::Input(cause)) => {
                return Err(error::io(Path::new(name), cause));
            }
            Err(_) => {
                return Err(MediaError::Contract(
                    "unsupported model chunk directory failure".into(),
                ));
            }
        }
        Ok(Projection::Written)
    }
}

fn entry(
    family: ModelSourceFamily,
    file: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    put(
        &mut prepared,
        "family",
        origins.directory("modelExtension", writer)?,
        writer,
    )?;
    let signature = origins.payload(
        0..4,
        "modelContainerSignature",
        SourceUseRole::Selector,
        writer,
    )?;
    put(&mut prepared, "container", signature, writer)?;
    let count = origins.payload(8..12, "modelChunkCount", SourceUseRole::Payload, writer)?;
    put(&mut prepared, "chunk_count", count, writer)?;
    let table = origins.payload(
        8..16,
        "modelChunkTableAddress",
        SourceUseRole::Selector,
        writer,
    )?;
    put(&mut prepared, "table", table, writer)?;
    put(&mut prepared, "table/basis", origins.independent(), writer)?;
    prepared.write(
        writer,
        &Subresource::new(payload::ENTRY, Vec::new()),
        Status::Complete,
        &ModelSourcePayload::Entry(entry_data(family, Some(file))),
        &[],
    )
}

fn entry_data(family: ModelSourceFamily, file: Option<&ChunkDirectory>) -> ModelEntryRecord {
    ModelEntryRecord {
        family,
        container: file.map(|file| file.container().into()),
        chunk_count: file.map(|file| file.chunks().len()),
        table: file.map(|file| {
            let table = file.table_range();
            SourceRange::new(OffsetBasis::Entry, table.start, table.end)
        }),
    }
}

fn failed(
    family: ModelSourceFamily,
    length: u64,
    error: &ChunkError,
    row: Option<Range<u64>>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    put(
        &mut prepared,
        "family",
        origins.directory("modelExtension", writer)?,
        writer,
    )?;
    let mut failure = writer.capture_origins();
    failure.include_original(&origins.payload(
        0..length,
        "modelChunkHeaderFailure",
        SourceUseRole::Selector,
        writer,
    )?)?;
    if let Some(range) = row {
        failure.include_original(&origins.payload(
            range,
            "modelChunkRowFailure",
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    failure.include_original(&origins.directory("modelDecodedEntryLength", writer)?)?;
    let invalid = failure.finish();
    for name in ["container", "chunk_count", "table"] {
        put(&mut prepared, name, invalid.clone(), writer)?;
    }
    origins::at(&mut prepared, "/status", invalid.clone(), writer)?;
    origins::at(&mut prepared, "/issues", invalid, writer)?;
    let issue = MediaIssue::new(
        "model.chunk-table",
        "entry",
        Severity::Warning,
        error.to_string(),
    );
    prepared.write(
        writer,
        &Subresource::new(payload::ENTRY, Vec::new()),
        Status::Partial,
        &ModelSourcePayload::Entry(entry_data(family, None)),
        &[issue],
    )
}

fn chunk(
    file: &ChunkDirectory,
    index: usize,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let missing = || MediaError::Contract("model chunk lost its original table slot".into());
    let row = file.table_row(index).ok_or_else(missing)?;
    let chunk = file.chunks().get(index).ok_or_else(missing)?;
    let mut prepared = origins.prepared()?;
    let kind = origins.payload(row.kind, "modelChunkType", SourceUseRole::Payload, writer)?;
    for field in ["type_id", "type_name", "projection"] {
        put(&mut prepared, field, kind.clone(), writer)?;
    }
    origins::at(&mut prepared, "/subresource/indices", kind, writer)?;
    let version = origins.payload(
        row.version,
        "modelChunkVersion",
        SourceUseRole::Payload,
        writer,
    )?;
    put(&mut prepared, "version", version.clone(), writer)?;
    let signature = origins.payload(
        0..4,
        "modelContainerSignature",
        SourceUseRole::Selector,
        writer,
    )?;
    let byte_order = match file.container() {
        Container::Ivo => signature.clone(),
        Container::CrCh => version,
    };
    put(&mut prepared, "byte_order", byte_order, writer)?;
    let id = match row.id {
        Some(range) => origins.payload(range, "modelChunkId", SourceUseRole::Payload, writer)?,
        None => signature.clone(),
    };
    put(&mut prepared, "chunk_id", id, writer)?;
    put(&mut prepared, "extent", signature, writer)?;
    let range = origins::chunk_extent(file.table_range(), row.offset, row.size, origins, writer)?;
    put(&mut prepared, "range", range.end.clone(), writer)?;
    put(&mut prepared, "range/start", range.start, writer)?;
    put(&mut prepared, "range/end_exclusive", range.end, writer)?;
    put(&mut prepared, "range/basis", origins.independent(), writer)?;
    prepared.write(
        writer,
        &Subresource::new(payload::CHUNK, vec![index as u64]),
        Status::Complete,
        &ModelSourcePayload::Chunk(ModelChunkRecord::new(chunk)),
        &[],
    )
}

fn put(
    prepared: &mut PreparedRecordOrigins,
    field: &str,
    evidence: crate::origin_contributions::OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    origins::at(
        prepared,
        &format!("/data/payload/{field}"),
        evidence,
        writer,
    )
}
