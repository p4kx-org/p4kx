// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{ModelArchiveSource, SourceUse};
use p4kx_gltf::MeshVertexMap;
use p4kx_model::Model;

use crate::PackageWriteContext;
use crate::mesh_vertex_map::BoundMesh;
use crate::package_scene_provenance::SceneFields;
use crate::triangle_evidence::{self, TriangleEvidence as EmittedCounts};

pub(super) struct TriangleEvidence {
    evidence: SceneFields,
    contributions: Vec<SourceUse>,
    counts: EmittedCounts,
}

impl TriangleEvidence {
    pub(super) fn new() -> Self {
        Self {
            evidence: SceneFields::new(),
            contributions: Vec::new(),
            counts: EmittedCounts::new(),
        }
    }

    pub(super) fn observe(
        &mut self,
        mesh: &MeshVertexMap<'_>,
        model: &Model,
        sources: &[ModelArchiveSource],
        context: &PackageWriteContext<'_>,
    ) -> io::Result<()> {
        let bound = BoundMesh::open(&self.evidence.sources, model, sources, mesh.origin)?;
        self.counts.observe(mesh, context, |operand| {
            if let Some(bound) = &bound {
                self.contributions.push(bound.index_count(operand.word)?);
            }
            Ok(())
        })
    }

    pub(super) fn finish(mut self, written: usize) -> io::Result<SceneFields> {
        self.counts.verify(written)?;
        triangle_evidence::append_field(
            0,
            &mut self.evidence.fields,
            &self.evidence.sources,
            &self.contributions,
        )?;
        Ok(self.evidence)
    }
}
