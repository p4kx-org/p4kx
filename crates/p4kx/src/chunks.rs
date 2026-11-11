// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_extraction::{ChunkJson, ChunkSource};
use serde::Serialize;

use crate::cli_chunks::{ChunkFileSource, ChunksAction, ChunksArgs};
use crate::json;

#[path = "chunk_list_origins.rs"]
mod origin;

use origin::ChunkListOrigins;

const UNKNOWN: &str = "unknown";
const NONE: &str = "-";

#[derive(Serialize)]
struct ListJson<'a> {
    file: &'a str,
    container: &'static str,
    version: u32,
    chunks: Vec<ChunkJson>,
    #[serde(flatten)]
    origins: ChunkListOrigins,
}

#[derive(Serialize)]
struct DumpJson<'a> {
    file: &'a str,
    chunk: ChunkJson,
    out: String,
}

/// Runs `p4kx chunks list` or `p4kx chunks dump`.
pub(crate) fn run(args: &ChunksArgs, as_json: bool) -> Result<()> {
    match &args.action {
        ChunksAction::List(source) => list(&open(source, as_json)?, as_json),
        ChunksAction::Dump {
            source,
            chunk,
            out,
            text,
        } => dump(&open(source, false)?, *chunk, out, *text, as_json),
    }
}

fn open(source: &ChunkFileSource, observe: bool) -> Result<ChunkSource> {
    Ok(match &source.entry {
        Some(entry) => ChunkSource::from_archive(&crate::key::open(&source.path)?, entry)?,
        None if observe => ChunkSource::from_file_observed(&source.path)?,
        None => ChunkSource::from_file(&source.path)?,
    })
}

fn list(source: &ChunkSource, as_json: bool) -> Result<()> {
    let file = source.file();
    let container = file.container();
    if as_json {
        json::print(&ListJson {
            file: source.name(),
            container: container.name(),
            version: container.version(),
            chunks: file
                .chunks()
                .iter()
                .enumerate()
                .map(|(index, chunk)| ChunkJson::of(index, chunk))
                .collect(),
            origins: ChunkListOrigins::capture(source)?,
        })?;
        return Ok(());
    }
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(out, "Chunk file:  {}", source.name())?;
    writeln!(out, "Container:   {container} {:#x}", container.version())?;
    writeln!(out, "Chunks:      {}", file.chunks().len())?;
    let unknown = file
        .chunks()
        .iter()
        .filter(|chunk| !chunk.chunk_type().is_known())
        .count();
    if unknown > 0 {
        writeln!(out, "Unknown:     {unknown}")?;
    }
    if !file.chunks().is_empty() {
        writeln!(out)?;
        writeln!(
            out,
            "{:>5}  {:<24}  {:>10}  {:>7}  {:>10}  {:<6}  {:>10}  {:>10}",
            "Index", "Type", "Type ID", "Version", "ID", "Order", "Offset", "Size"
        )?;
    }
    for (index, chunk) in file.chunks().iter().enumerate() {
        let chunk_type = chunk.chunk_type();
        let id = chunk
            .id()
            .map_or_else(|| NONE.to_owned(), |id| id.to_string());
        writeln!(
            out,
            "{index:>5}  {:<24}  {:>10}  {:>#7x}  {id:>10}  {:<6}  {:>10}  {:>10}",
            chunk_type.name().unwrap_or(UNKNOWN),
            chunk_type.hex_id(),
            chunk.version(),
            chunk.byte_order(),
            chunk.offset(),
            chunk.size()
        )?;
    }
    out.flush()?;
    Ok(())
}

fn dump(source: &ChunkSource, index: usize, out: &Path, text: bool, as_json: bool) -> Result<()> {
    let chunk = if text {
        source.write_chunk_text_to_file(index, out)?
    } else {
        source.write_chunk_to_file(index, out)?
    };
    if as_json {
        json::print(&DumpJson {
            file: source.name(),
            chunk: ChunkJson::of(index, &chunk),
            out: json::path(out),
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote chunk {index} ({}, {} bytes) of {} to {}",
            chunk.chunk_type().name().unwrap_or(UNKNOWN),
            chunk.size(),
            source.name(),
            out.display()
        )?;
    }
    Ok(())
}
