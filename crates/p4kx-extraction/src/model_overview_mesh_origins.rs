// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;
use std::ops::Range;

use p4kx_chunks::Container;
use p4kx_model::{CrchMeshSource, Lod, Mesh, ModelChunk, ModelFile};

use super::submesh_origins::{
    covers_mesh_indices, original_count, original_material, source_chunk,
};
use crate::model_geometry_scope::ModelGeometryScope;
use crate::source_provenance::ChunkHeaderField;
use crate::{
    FieldOrigins, FieldPath, ModelSource, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

/// Writes retained geometry inputs for LOD counts and the distinct material IDs.
pub struct ModelOverviewMeshOrigins<'a> {
    source: &'a ModelSource,
    uses: &'a SourceUses,
}

impl<'a> ModelOverviewMeshOrigins<'a> {
    /// Binds the summary to the model read and its output inventory.
    #[must_use]
    pub const fn new(source: &'a ModelSource, uses: &'a SourceUses) -> Self {
        Self { source, uses }
    }

    /// Writes exact original word runs at the caller's summary paths.
    ///
    /// # Errors
    /// Returns an invalid path, duplicate field, or source evidence error.
    pub fn write(
        &self,
        root: &FieldPath,
        material_ids: &[u16],
        fields: &mut FieldOrigins,
    ) -> io::Result<()> {
        let mut materials = BTreeMap::<u16, Vec<SourceUse>>::new();
        let mut path = root.clone();
        path.push_field("lods").map_err(io::Error::other)?;
        for (index, lod) in self.source.model().lods.iter().enumerate() {
            path.push_index(index).map_err(io::Error::other)?;
            let inputs = self.lod_words(lod, &mut materials)?;
            for (name, words) in [
                ("triangles", inputs.triangles),
                ("vertices", inputs.vertices),
            ] {
                path.push_field(name).map_err(io::Error::other)?;
                fields
                    .insert(path.clone(), self.uses.origin(&words, false))
                    .map_err(io::Error::other)?;
                path.pop();
            }
            path.pop();
        }
        if let Some(selector) = self.generated_material_selector()? {
            materials.entry(0).or_default().push(selector);
        }
        path.pop();
        path.push_field("material_ids").map_err(io::Error::other)?;
        for (index, material) in material_ids.iter().enumerate() {
            path.push_index(index).map_err(io::Error::other)?;
            let uses = materials.get(material).map_or(&[][..], Vec::as_slice);
            fields
                .insert(path.clone(), self.uses.origin(uses, false))
                .map_err(io::Error::other)?;
            path.pop();
        }
        let aggregate: Vec<_> = materials.into_values().flatten().collect();
        fields
            .insert(path, self.uses.origin(&aggregate, false))
            .map_err(io::Error::other)
    }

    fn lod_words(
        &self,
        lod: &Lod,
        materials: &mut BTreeMap<u16, Vec<SourceUse>>,
    ) -> io::Result<LodCountUses> {
        let mut counts = LodCountUses::default();
        let model = self.source.model();
        for mesh in &lod.meshes {
            if let Some(physics) = mesh.crch_physics_source(&model.chunks) {
                if let Some(scope) =
                    ModelGeometryScope::capture(self.uses, self.source, ModelFile::Model)?
                {
                    counts.append_crch(&scope, physics)?;
                }
                continue;
            }
            let Some(chunk) = source_chunk(self.source, mesh) else {
                continue;
            };
            let words = MeshWords::capture(mesh, chunk);
            let Some(scope) = ModelGeometryScope::capture(self.uses, self.source, chunk.file)?
            else {
                continue;
            };
            if covers_mesh_indices(mesh, words.observed_counts) {
                append_uses(
                    &mut counts.triangles,
                    &scope,
                    chunk,
                    "indexCount",
                    words.counts,
                )?;
            }
            for (material, runs) in words.materials {
                append_uses(
                    materials.entry(material).or_default(),
                    &scope,
                    chunk,
                    "materialId",
                    runs,
                )?;
            }
        }
        Ok(counts)
    }

    fn generated_material_selector(&self) -> io::Result<Option<SourceUse>> {
        let model = self.source.model();
        let crch = model.chunks.iter().any(|chunk| {
            chunk.file == ModelFile::Model && chunk.chunk_type.container() == Container::CrCh
        });
        let has_geometry = model.lods.iter().any(|lod| !lod.meshes.is_empty());
        if !crch || !has_geometry {
            return Ok(None);
        }
        let Some(scope) = ModelGeometryScope::capture(self.uses, self.source, ModelFile::Model)?
        else {
            return Ok(None);
        };
        // CrCh physics meshes use a generated material 0. The signature selects that layout.
        ChunkHeaderField::Signature
            .observe(scope.scope(), SourceUseRole::Selector)
            .map(Some)
            .map_err(io::Error::other)
    }
}

#[derive(Default)]
struct LodCountUses {
    triangles: Vec<SourceUse>,
    vertices: Vec<SourceUse>,
}

impl LodCountUses {
    fn append_crch(
        &mut self,
        scope: &ModelGeometryScope,
        source: CrchMeshSource,
    ) -> io::Result<()> {
        let primitive = scope.word(
            source.primitive_range(),
            "primitive",
            SourceUseRole::Selector,
        )?;
        self.triangles.push(primitive.clone());
        self.triangles.push(scope.word(
            source.triangle_count_range(),
            "triangleCount",
            SourceUseRole::Conversion,
        )?);
        self.vertices.push(primitive);
        self.vertices.push(scope.word(
            source.vertex_count_range(),
            "vertexCount",
            SourceUseRole::Conversion,
        )?);
        Ok(())
    }
}

#[derive(Default)]
struct MeshWords {
    observed_counts: usize,
    counts: Vec<WordRun>,
    materials: BTreeMap<u16, Vec<WordRun>>,
}

impl MeshWords {
    fn capture(mesh: &Mesh, chunk: &ModelChunk) -> Self {
        let mut words = Self::default();
        for (index, submesh) in mesh.submeshes.iter().enumerate() {
            let Some(original) = mesh.source_submesh(index, chunk) else {
                continue;
            };
            let row = original.range();
            let stride = row.end - row.start;
            if let Some(count) = original_count(Some(original), submesh.index_count) {
                words.observed_counts += 1;
                WordRun::push(&mut words.counts, count, stride);
            }
            if let Some(material) = original_material(Some(original), submesh.material_id) {
                WordRun::push(
                    words.materials.entry(submesh.material_id).or_default(),
                    material,
                    stride,
                );
            }
        }
        words
    }
}

struct WordRun {
    first: Range<u64>,
    stride: u64,
    count: u64,
}

impl WordRun {
    fn push(runs: &mut Vec<Self>, word: Range<u64>, stride: u64) {
        if let Some(last) = runs.last_mut()
            && last.stride == stride
            && last.stride.checked_mul(last.count).is_some_and(|offset| {
                last.first.start.checked_add(offset) == Some(word.start)
                    && last.first.end.checked_add(offset) == Some(word.end)
            })
        {
            last.count += 1;
        } else {
            runs.push(Self {
                first: word,
                stride,
                count: 1,
            });
        }
    }
}

fn append_uses(
    target: &mut Vec<SourceUse>,
    scope: &ModelGeometryScope,
    chunk: &ModelChunk,
    field: &str,
    runs: Vec<WordRun>,
) -> io::Result<()> {
    for run in runs {
        let source_use = scope
            .scope()
            .at(
                SourceLocation::Subresource {
                    namespace: "model-submesh-words".into(),
                    indices: vec![
                        chunk.index as u64,
                        run.first.start,
                        run.first.end,
                        run.stride,
                        run.count,
                    ],
                    field: field.into(),
                },
                SourceUseRole::Conversion,
            )
            .map_err(io::Error::other)?;
        target.push(source_use);
    }
    Ok(())
}
