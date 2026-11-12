// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::EntryOrigin;
use p4kx_model::{ChunkUsage, Model, ModelBoneSources, ModelFile, ModelIssue, ModelNodeSources};

/// One admitted model or companion Entry and its interpreted file role.
#[derive(Debug, Clone)]
pub struct ModelArchiveSource {
    file: ModelFile,
    origin: Option<EntryOrigin>,
    nodes: Option<Arc<ModelNodeSources>>,
    bones: Option<Arc<ModelBoneSources>>,
}

impl ModelArchiveSource {
    pub(crate) const fn new(file: ModelFile, origin: Option<EntryOrigin>) -> Self {
        Self {
            file,
            origin,
            nodes: None,
            bones: None,
        }
    }

    pub(crate) fn set_node_sources(&mut self, nodes: ModelNodeSources) {
        self.nodes = Some(Arc::new(nodes));
    }

    pub(crate) fn set_bone_sources(&mut self, bones: ModelBoneSources) {
        self.bones = Some(Arc::new(bones));
    }

    /// Returns this file's optional original skeleton coordinates.
    #[must_use]
    pub fn bone_sources(&self) -> Option<&ModelBoneSources> {
        self.bones.as_deref()
    }

    /// Returns the primary file's optional original node coordinates.
    #[must_use]
    pub fn node_sources(&self) -> Option<&ModelNodeSources> {
        self.nodes.as_deref()
    }

    /// Returns the role for which the model decoder requested these bytes.
    #[must_use]
    pub const fn file(&self) -> ModelFile {
        self.file
    }

    /// Returns the bound Entry when its physical source could be identified.
    #[must_use]
    pub const fn origin(&self) -> Option<&EntryOrigin> {
        self.origin.as_ref()
    }

    /// Returns whether this file supplied the retained model levels.
    #[must_use]
    pub fn supplies(&self, model: &Model) -> bool {
        let decoded = || {
            model
                .chunks
                .iter()
                .any(|chunk| chunk.file == self.file && chunk.usage == ChunkUsage::Read)
        };
        match self.file {
            ModelFile::Model => true,
            ModelFile::Geometry => {
                decoded()
                    && model.lods.iter().any(|lod| {
                        lod.level == 0 || model.issues.iter().any(|issue| {
                    matches!(issue, ModelIssue::LodFallback { level, .. } if *level == lod.level)
                })
                    })
            }
            ModelFile::LodGeometry(level) => {
                model.lods.iter().any(|lod| lod.level == level) && decoded()
            }
            ModelFile::LodModel(level) => model
                .lods
                .iter()
                .any(|lod| lod.level == level && lod.bones.is_some()),
            _ => false,
        }
    }
}

/// A decoded model together with its read-bound Archive sources.
#[derive(Debug)]
#[non_exhaustive]
pub struct ArchiveModelRead {
    /// The interpreted model.
    pub model: Model,
    /// The primary Entry and successfully read typed companion files.
    pub sources: Vec<ModelArchiveSource>,
}
