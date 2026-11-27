// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::model_info::{
    self, ModelBoneOrigins, ModelChunkOrigins, ModelMeshSummaryOrigins, ModelNodeOrigins,
    ModelOverviewMeshOrigins,
};
use p4kx_extraction::{
    CancelToken, FieldOrigin, MaterialSummaryOrigins, ModelSource, SourceInventory, SourceUses,
};

use crate::ModelInfo;
use crate::model_info::listed_chunks;

/// Original inputs of the model Info tab's fields and mesh summary.
#[derive(Debug)]
pub struct ModelInfoProvenance {
    node_count: FieldOrigin,
    bone_count: FieldOrigin,
    material_library: MaterialSummaryOrigins,
    mesh_summary: ModelMeshSummaryOrigins,
    chunk_total: FieldOrigin,
    unread: Vec<ModelChunkOrigins>,
    inventory: SourceInventory,
}

impl ModelInfoProvenance {
    pub(super) fn capture(
        source: &ModelSource,
        info: &ModelInfo,
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        let uses = SourceUses::new();
        let node_count = ModelNodeOrigins::capture(source, &uses)?.collection_origin();
        let bone_count = ModelBoneOrigins::capture(source, &uses)?.collection_origin();
        let material_library = source
            .material_summary_origins(&uses)
            .map_err(io::Error::other)?;
        let mesh_summary = ModelOverviewMeshOrigins::new(source, &uses)
            .capture_summary(info.listed_material_ids(), cancel)?;
        let chunk_total = model_info::model_chunk_count_origin(source, &uses)?;
        let unread = listed_chunks(source.model())
            .map(|(ordinal, _)| model_info::capture_chunk_origins(source, ordinal, &uses))
            .collect::<io::Result<_>>()?;
        Ok(Self {
            node_count,
            bone_count,
            material_library,
            mesh_summary,
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

    /// Returns the origins of the displayed material library summary.
    #[must_use]
    pub const fn material_library(&self) -> &MaterialSummaryOrigins {
        &self.material_library
    }

    /// Returns origins of the displayed LOD counts and listed material IDs.
    #[must_use]
    pub const fn mesh_summary(&self) -> &ModelMeshSummaryOrigins {
        &self.mesh_summary
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
