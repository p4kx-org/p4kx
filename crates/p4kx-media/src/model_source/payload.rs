// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::{ByteOrder as ChunkByteOrder, Chunk, Container};
use schemars::JsonSchema;
use serde::Serialize;

use super::lod_distances::LodDistanceWords;
use crate::{ByteOrder, OffsetBasis, SourceRange};

pub(super) const ENTRY: &str = "model-entry";
pub(super) const CHUNK: &str = "model-chunk";

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum ModelSourceFamily {
    Cgf,
    Cga,
    Chr,
    Skin,
    Cgfm,
    Cgam,
    Chrm,
    Skinm,
}

impl ModelSourceFamily {
    pub(super) fn of(path: &str) -> Option<Self> {
        let (_, extension) = path.rsplit_once('.')?;
        [
            ("cgf", Self::Cgf),
            ("cga", Self::Cga),
            ("chr", Self::Chr),
            ("skin", Self::Skin),
            ("cgfm", Self::Cgfm),
            ("cgam", Self::Cgam),
            ("chrm", Self::Chrm),
            ("skinm", Self::Skinm),
        ]
        .into_iter()
        .find_map(|(name, family)| extension.eq_ignore_ascii_case(name).then_some(family))
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ModelChunkContainer {
    #[serde(rename = "#ivo")]
    Ivo,
    Crch,
}

impl From<Container> for ModelChunkContainer {
    fn from(container: Container) -> Self {
        match container {
            Container::Ivo => Self::Ivo,
            Container::CrCh => Self::Crch,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ModelChunkExtent {
    DeclaredCrchSize,
    IvoNextBoundaryIncludingPadding,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ModelChunkProjection {
    KnownOpaque,
    UnknownType,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelEntryRecord {
    pub(super) family: ModelSourceFamily,
    pub(super) container: Option<ModelChunkContainer>,
    pub(super) chunk_count: Option<usize>,
    pub(super) table: Option<SourceRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelChunkRecord {
    pub(super) type_id: u32,
    pub(super) type_name: Option<&'static str>,
    pub(super) version: u32,
    pub(super) chunk_id: Option<u32>,
    pub(super) byte_order: ByteOrder,
    pub(super) extent: ModelChunkExtent,
    pub(super) range: SourceRange,
    pub(super) projection: ModelChunkProjection,
}

impl ModelChunkRecord {
    pub(super) fn new(chunk: &Chunk) -> Self {
        let kind = chunk.chunk_type();
        Self {
            type_id: kind.id(),
            type_name: kind.name(),
            version: chunk.version(),
            chunk_id: chunk.id(),
            byte_order: match chunk.byte_order() {
                ChunkByteOrder::Little => ByteOrder::Little,
                ChunkByteOrder::Big => ByteOrder::Big,
            },
            extent: match kind.container() {
                Container::Ivo => ModelChunkExtent::IvoNextBoundaryIncludingPadding,
                Container::CrCh => ModelChunkExtent::DeclaredCrchSize,
            },
            range: SourceRange::new(
                OffsetBasis::Entry,
                chunk.offset(),
                chunk.offset() + chunk.size(),
            ),
            projection: if kind.is_known() {
                ModelChunkProjection::KnownOpaque
            } else {
                ModelChunkProjection::UnknownType
            },
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum ModelSourcePayload {
    Entry(ModelEntryRecord),
    Chunk(ModelChunkRecord),
    LodDistanceWords(LodDistanceWords),
}
