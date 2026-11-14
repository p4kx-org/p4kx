// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{Chunk, ChunkDirectory, ivo};
use p4kx_extraction::model::{NmcFrames, NmcNodeTables};

use super::nmc_record::{self as wire, MaterialIndex, NmcRecord, Table};
use super::origins::{self, ModelOrigins};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::required_reader::RequiredReader;
use crate::{
    MediaError, MediaIssue, MediaWriter, OffsetBasis, Severity, SourceRange, Status, Subresource,
};

#[path = "nmc_nodes.rs"]
mod nodes;

pub(super) fn emit(
    file: &ChunkDirectory,
    index: usize,
    admitted: Option<&[u8]>,
    reader: &mut RequiredReader<'_>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let chunk = file.chunks().get(index).ok_or_else(missing)?;
    if chunk.chunk_type() != ivo::NODE_MESH_COMBO {
        return Ok(());
    }
    let row = file.table_row(index).ok_or_else(missing)?;
    let extent = origins::chunk_extent(file.table_range(), row.offset, row.size, origins, writer)?;
    let mut evidence = writer.capture_origins();
    evidence.include_original(&extent.end)?;
    for (span, field) in [(row.kind, "nmcType"), (row.version, "nmcVersion")] {
        evidence.include_original(&origins.payload(
            span,
            field,
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    let coordinate = Projection {
        index,
        chunk,
        origins,
        layout: evidence.finish(),
    };
    if !NmcFrames::supports(chunk) {
        return coordinate.write_table(None, None, writer);
    }
    let loaded;
    let bytes = if let Some(bytes) = admitted {
        bytes
    } else {
        loaded = reader.read_chunk(chunk.offset()..chunk.offset() + chunk.size())?;
        loaded.as_slice()
    };
    let table = match NmcNodeTables::read(bytes) {
        Ok(table) => table,
        Err(error) => return coordinate.write_table(None, Some(error.to_string()), writer),
    };
    coordinate.write_table(Some(&table), None, writer)?;
    for units in [
        table.frames().len() as u64,
        (table.names_range().end - table.names_range().start),
        (table.properties_range().end - table.properties_range().start),
        table.material_indices().len() as u64,
    ] {
        reader.charge_metadata_work(units)?;
    }
    for node in table.nodes() {
        writer.checkpoint()?;
        nodes::emit(&coordinate, &node, bytes, writer)?;
    }
    coordinate.materials(&table, writer)
}

struct Projection<'a, 'input> {
    index: usize,
    chunk: &'a Chunk,
    origins: &'a ModelOrigins<'input>,
    layout: OriginContributions,
}

impl Projection<'_, '_> {
    fn materials(
        &self,
        table: &NmcNodeTables<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let start = table.material_range().start;
        for (ordinal, word) in table.material_indices().enumerate() {
            writer.checkpoint()?;
            let word_start = start + ordinal as u64 * size_of::<u16>() as u64;
            let mut prepared =
                self.prepared(self.selector(4..20, "nmcMaterialLayout", writer)?, writer)?;
            self.bind(&mut prepared, "word", word_start..word_start + 2, writer)?;
            prepared.write(
                writer,
                &Subresource::new(wire::MATERIAL, vec![self.index as u64, ordinal as u64]),
                Status::Complete,
                &NmcRecord::NodeMaterialIndex(MaterialIndex { word }),
                &[],
            )?;
        }
        Ok(())
    }

    fn write_table(
        &self,
        table: Option<&NmcNodeTables<'_>>,
        error: Option<String>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.prepared(self.layout.clone(), writer)?;
        let (payload, issue) = match (table, error) {
            (Some(table), _) => self.measured(&mut prepared, table, writer)?,
            (None, Some(error)) => (
                Table::Malformed,
                Some(MediaIssue::new(
                    "model.nmc-layout",
                    "chunk",
                    Severity::Warning,
                    error,
                )),
            ),
            (None, None) => (Table::UnsupportedVersion, None),
        };
        let status = if issue.is_some() {
            Status::Partial
        } else {
            Status::Complete
        };
        let issues = if let Some(issue) = issue {
            let evidence = self.selector(0..self.chunk.size(), "nmcLayoutFailure", writer)?;
            origins::at(&mut prepared, "/status", evidence.clone(), writer)?;
            origins::at(&mut prepared, "/issues", evidence.clone(), writer)?;
            origins::at(&mut prepared, "/data/payload/disposition", evidence, writer)?;
            vec![issue]
        } else {
            let evidence = if matches!(payload, Table::Measured { .. }) {
                self.selector(4..28, "nmcTableLayout", writer)?
            } else {
                self.layout.clone()
            };
            origins::at(&mut prepared, "/data/payload/disposition", evidence, writer)?;
            Vec::new()
        };
        prepared.write(
            writer,
            &Subresource::new(wire::TABLE, vec![self.index as u64]),
            status,
            &NmcRecord::NodeTable(payload),
            &issues,
        )
    }

    fn measured(
        &self,
        prepared: &mut PreparedRecordOrigins,
        table: &NmcNodeTables<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(Table, Option<MediaIssue>), MediaError> {
        self.bind(prepared, "nodes", NmcFrames::count_range(), writer)?;
        self.bind(prepared, "declared_property_bytes", 24..28, writer)?;
        for field in ["names", "properties", "material_indices"] {
            let evidence = self.selector(4..28, "nmcTableLayout", writer)?;
            origins::at(
                prepared,
                &format!("/data/payload/{field}"),
                evidence,
                writer,
            )?;
        }
        let properties = table.properties_range();
        let truncated =
            u64::from(table.declared_property_bytes()) != properties.end - properties.start;
        let issue = truncated.then(|| {
            MediaIssue::new(
                "model.nmc-property-table",
                "chunk",
                Severity::Warning,
                "Original property table is truncated.",
            )
        });
        Ok((
            Table::Measured {
                nodes: table.frames().len(),
                names: self.range(table.names_range()),
                properties: self.range(properties),
                declared_property_bytes: table.declared_property_bytes(),
                material_indices: self.range(table.material_range()),
            },
            issue,
        ))
    }

    fn prepared(
        &self,
        evidence: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = self.origins.prepared()?;
        origins::at(&mut prepared, "/subresource/indices", evidence, writer)?;
        Ok(prepared)
    }

    fn bind(
        &self,
        prepared: &mut PreparedRecordOrigins,
        name: &str,
        span: Range<u64>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let evidence =
            self.origins
                .payload(self.absolute(span), name, SourceUseRole::Payload, writer)?;
        origins::at(prepared, &format!("/data/payload/{name}"), evidence, writer)
    }

    fn selector(
        &self,
        span: Range<u64>,
        field: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut evidence = writer.capture_origins();
        evidence.include_original(&self.layout)?;
        evidence.include_original(&self.origins.payload(
            self.absolute(span),
            field,
            SourceUseRole::Selector,
            writer,
        )?)?;
        Ok(evidence.finish())
    }

    const fn absolute(&self, span: Range<u64>) -> Range<u64> {
        self.chunk.offset() + span.start..self.chunk.offset() + span.end
    }

    const fn range(&self, span: Range<u64>) -> SourceRange {
        let absolute = self.absolute(span);
        SourceRange::new(OffsetBasis::Entry, absolute.start, absolute.end)
    }
}

fn missing() -> MediaError {
    MediaError::Contract("NMC table lost its physical chunk coordinate".into())
}
