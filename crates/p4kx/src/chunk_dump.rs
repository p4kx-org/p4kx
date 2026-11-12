// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ChunkDumpMode, ChunkDumpPayload, ChunkJson,
    ChunkTableEvidence, ConversionError, FieldOrigins, FieldPath, PreparedChunk, SourceInventory,
    SourceUse, SourceUseRole,
};
use serde::Serialize;

use super::{UNKNOWN, open};
use crate::cli_chunks::ChunkFileSource;
use crate::cryxml_origins::{EmbeddedXml, XmlReceipt};
use crate::file_receipt::FileReceipt;
use crate::json;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DumpJson<'a> {
    file: &'a str,
    chunk: ChunkJson,
    out: String,
    origin_receipt: String,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn run(
    input: &ChunkFileSource,
    index: usize,
    out: &Path,
    mode: ChunkDumpMode,
    as_json: bool,
) -> Result<()> {
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&input.path, cancel.flag());
    let source = open(input, true)?;
    let dump = source.prepare_dump(index, out, mode)?;
    let guard = protection?;
    let write_error = |source| ConversionError::Write {
        path: out.to_path_buf(),
        source,
    };
    let receipt = FileReceipt::new_with_cancel(out, Some(&guard), Some(&guard), cancel)
        .map_err(write_error)?;
    let evidence = ChunkTableEvidence::new(&source)?;
    let selectors = evidence.selected_payload(index, mode)?;
    let fields = as_json
        .then(|| report_fields(&evidence, index))
        .transpose()?;
    receipt
        .publish(
            |writer| render(&dump, writer, &evidence, index, selectors),
            |writer, binding, description| description.write(writer, binding),
        )
        .map_err(write_error)?;
    if let Some(field_origins) = fields {
        let name = receipt
            .path()
            .file_name()
            .context("missing origin companion basename")?;
        json::print(&DumpJson {
            file: source.name(),
            chunk: ChunkJson::of(index, &dump.chunk),
            out: json::path(out),
            origin_receipt: json::path(Path::new(name)),
            field_origins,
            provenance: evidence.uses().freeze()?,
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote chunk {index} ({}, {} bytes) of {} to {}",
            dump.chunk.chunk_type().name().unwrap_or(UNKNOWN),
            dump.chunk.size(),
            source.name(),
            out.display()
        )?;
    }
    Ok(())
}

fn render(
    dump: &PreparedChunk<'_>,
    writer: &mut dyn Write,
    evidence: &ChunkTableEvidence<'_>,
    index: usize,
    mut selectors: Vec<SourceUse>,
) -> io::Result<XmlReceipt> {
    let base = dump.chunk.offset();
    match &dump.payload {
        ChunkDumpPayload::Bytes(bytes) => {
            selectors.push(
                evidence
                    .site(
                        "cryEngineChunkPayload",
                        "bytes",
                        vec![index as u64, base, base + bytes.len() as u64],
                        SourceUseRole::Payload,
                    )
                    .map_err(io::Error::other)?,
            );
            XmlReceipt::opaque(writer, bytes, evidence.uses(), evidence.origin(&selectors))
        }
        ChunkDumpPayload::CryXml(document) => XmlReceipt::document(
            document,
            writer,
            (evidence.uses(), evidence.scope()),
            evidence.is_complete(),
            Some(&EmbeddedXml {
                base,
                selectors: &selectors,
            }),
        ),
        _ => Err(io::Error::other("unknown prepared chunk representation")),
    }
}

fn report_fields(evidence: &ChunkTableEvidence<'_>, index: usize) -> Result<FieldOrigins> {
    let mut fields = FieldOrigins::new();
    fields.insert(FieldPath::parse("/file")?, evidence.path()?)?;
    for pointer in ["/out", "/originReceipt"] {
        fields.insert(
            FieldPath::parse(pointer)?,
            evidence.uses().origin(&[], true),
        )?;
    }
    evidence
        .rows()?
        .write_row(&mut fields, &FieldPath::parse("/chunk")?, index)?;
    Ok(fields)
}
