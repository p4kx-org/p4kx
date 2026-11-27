// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::model_info::{self, ModelBoneOrigins, ModelChunkOrigins, ModelNodeOrigins};
use p4kx_extraction::{FieldOrigin, ModelSource, SourceInventory, SourceUses};

use crate::model_info::listed_chunks;

/// Original inputs of the model Info tab's counts and visible chunk notes.
#[derive(Debug)]
pub struct ModelInfoProvenance {
    node_count: FieldOrigin,
    bone_count: FieldOrigin,
    chunk_total: FieldOrigin,
    unread: Vec<ModelChunkOrigins>,
    inventory: SourceInventory,
}

impl ModelInfoProvenance {
    pub(super) fn capture(source: &ModelSource) -> io::Result<Self> {
        let uses = SourceUses::new();
        let node_count = ModelNodeOrigins::capture(source, &uses)?.collection_origin();
        let bone_count = ModelBoneOrigins::capture(source, &uses)?.collection_origin();
        let chunk_total = model_info::model_chunk_count_origin(source, &uses)?;
        let unread = listed_chunks(source.model())
            .map(|(ordinal, _)| model_info::capture_chunk_origins(source, ordinal, &uses))
            .collect::<io::Result<_>>()?;
        Ok(Self {
            node_count,
            bone_count,
            chunk_total,
            unread,
            inventory: uses.freeze().map_err(io::Error::other)?,
        })
    }

    /// Returns the origin of the displayed node count.
    #[must_use]
    pub const fn node_count(&self) -> &FieldOrigin {
        &self.node_count
    }

    /// Returns the origin of the displayed primary skeleton bone count.
    #[must_use]
    pub const fn bone_count(&self) -> &FieldOrigin {
        &self.bone_count
    }

    /// Returns the origin of the count of all admitted chunks.
    #[must_use]
    pub const fn chunk_total(&self) -> &FieldOrigin {
        &self.chunk_total
    }

    /// Returns origins in the same order as the visible unread chunk notes.
    #[must_use]
    pub fn unread_chunks(&self) -> &[ModelChunkOrigins] {
        &self.unread
    }

    /// Returns the physical sources and uses referenced by the model facts.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}
