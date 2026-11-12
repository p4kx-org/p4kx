// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::{Chunk, Container, KnownChunkType};
use p4kx_extraction::{
    ChunkEvidenceError, ChunkJson, ChunkSource, ChunkTableEvidence, FieldOrigins, FieldPath,
    SourceInventory,
};

use crate::archives::Opened;
use crate::budget::{self, MAX_RESPONSE_LEN};
use crate::error::ToolError;
use crate::model_shapes::{ChunkShape, ChunksResult};
use crate::models::converted;
use crate::page::Page;

type ChunkRow<'a> = (usize, &'a Chunk);

pub(crate) fn list(
    opened: &Opened,
    path: &str,
    filter: Option<&str>,
    page: Page,
) -> Result<ChunksResult, ToolError> {
    let source = ChunkSource::from_index(&opened.index(), path)
        .map_err(|error| converted(path, "a chunk file", error))?;
    let file = source.file();
    let wanted = filter
        .map(|text| type_id(text, file.container()))
        .transpose()?;
    let mut listing = Listing {
        archive: opened.archive().path().display().to_string(),
        source: &source,
        total: 0,
        unknown: 0,
        filtered: wanted.is_some(),
        page,
    };
    let mut candidates = Vec::with_capacity(page.limit().min(file.chunks().len()));
    for (index, chunk) in file.chunks().iter().enumerate() {
        let kind = chunk.chunk_type();
        listing.unknown += usize::from(!kind.is_known());
        if wanted.is_none_or(|id| kind.id() == id) {
            if page.contains(listing.total) {
                candidates.push((index, chunk));
            }
            listing.total += 1;
        }
    }
    page.check(listing.total)?;
    listing.fit(&candidates)
}

struct Listing<'a> {
    archive: String,
    source: &'a ChunkSource,
    total: usize,
    unknown: usize,
    filtered: bool,
    page: Page,
}

impl Listing<'_> {
    fn fit(&self, candidates: &[ChunkRow<'_>]) -> Result<ChunksResult, ToolError> {
        let full = self.response(candidates)?;
        if budget::json_len(&full) <= MAX_RESPONSE_LEN {
            return Ok(full);
        }
        drop(full);
        let mut fitting = 0;
        let mut oversized = candidates.len();
        let mut best = None;
        while oversized.saturating_sub(fitting) > 1 {
            let size = fitting + (oversized - fitting) / 2;
            let (prefix, _) = candidates.split_at(size);
            let response = self.response(prefix)?;
            if budget::json_len(&response) <= MAX_RESPONSE_LEN {
                fitting = size;
                best = Some(response);
            } else {
                oversized = size;
            }
        }
        best.ok_or_else(|| ToolError::TooLargeItem {
            what: candidates.first().map_or_else(
                || "result without its list".to_owned(),
                |(index, _)| format!("chunk {index}"),
            ),
            limit: MAX_RESPONSE_LEN,
            hint: "ask for it by a narrower filter",
        })
    }

    fn response(&self, selected: &[ChunkRow<'_>]) -> Result<ChunksResult, ToolError> {
        let container = self.source.file().container();
        let next_cursor = self.page.next_cursor_after(selected.len(), self.total)?;
        let (field_origins, provenance) = self.origins(selected, next_cursor.is_some())?;
        Ok(ChunksResult {
            archive: self.archive.clone(),
            path: self.source.name().to_owned(),
            container: container.name(),
            container_version: container.version(),
            chunk_count: self.source.file().chunks().len(),
            unknown_count: self.unknown,
            total: self.total,
            chunks: selected
                .iter()
                .map(|&(index, chunk)| ChunkShape {
                    chunk: ChunkJson::of(index, chunk),
                    type_id_hex: chunk.chunk_type().hex_id(),
                })
                .collect(),
            next_cursor,
            field_origins,
            provenance,
        })
    }

    fn origins(
        &self,
        selected: &[ChunkRow<'_>],
        cursor: bool,
    ) -> Result<(FieldOrigins, SourceInventory), ChunkEvidenceError> {
        let evidence = ChunkTableEvidence::new(self.source)?;
        let rows = evidence.rows()?;
        let types = rows.type_selection()?;
        let selection = if self.filtered {
            types.clone()
        } else {
            rows.index().clone()
        };
        let mut fields = FieldOrigins::new();
        for (path, origin) in [
            (
                "/structuredContent/archive",
                evidence.uses().origin(&[], true),
            ),
            ("/structuredContent/path", evidence.path()?),
            ("/structuredContent/container", evidence.container()?),
            ("/structuredContent/container_version", evidence.version()?),
            ("/structuredContent/chunk_count", rows.count()),
            ("/structuredContent/unknown_count", types),
            ("/structuredContent/total", selection.clone()),
            ("/structuredContent/chunks", selection.clone()),
        ] {
            fields.insert(FieldPath::parse(path)?, origin)?;
        }
        if cursor {
            fields.insert(
                FieldPath::parse("/structuredContent/next_cursor")?,
                selection,
            )?;
        }
        let mut path = FieldPath::parse("/structuredContent/chunks")?;
        for (on_page, (original, _)) in selected.iter().enumerate() {
            path.push_index(on_page)?;
            rows.write_row(&mut fields, &path, *original)?;
            path.push_field("type_id_hex")?;
            fields.insert(path.clone(), rows.hexadecimal_type(*original)?)?;
            path.pop();
            path.pop();
        }
        Ok((fields, evidence.uses().freeze()?))
    }
}

fn type_id(text: &str, container: Container) -> Result<u32, ToolError> {
    let text = text.trim();
    let bad = || ToolError::BadChunkType(text.to_owned());
    if let Some(digits) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return u32::from_str_radix(digits, 16).map_err(|_| bad());
    }
    KnownChunkType::table(container)
        .iter()
        .find(|known| known.name.eq_ignore_ascii_case(text))
        .map(|known| known.id)
        .ok_or_else(bad)
}
