// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_gltf::{self as gltf, GlbOptions, GlbSummary, GltfError, MeshVertexMap, TextureLoader};
use p4kx_model::ModelFile;

use crate::model_source::{ArchiveDefaultMaterial, ModelSource};
use crate::output_file::write_error;
use crate::{
    AnimationSet, CancelToken, ConversionError, FieldOrigin, FieldPath, FileReceipt, InputScope,
    ModelArchiveSource, OriginOutputAddress, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, write_origin_companion,
};

/// The written standalone GLB summary and its geometry-origin companion.
#[derive(Debug)]
#[non_exhaustive]
pub struct GlbGeometryPublication {
    /// The ordinary GLB writer's summary.
    pub summary: GlbSummary,
    /// The canonical sibling companion path.
    pub receipt: PathBuf,
}

impl ModelSource {
    /// Writes the standalone GLB and a digest-bound receipt for observed index accessor counts.
    ///
    /// # Errors
    /// Returns the ordinary GLB write errors and any companion publication error.
    pub fn write_glb_with_geometry_receipt(
        &self,
        animations: &AnimationSet,
        options: &GlbOptions,
        out: &Path,
        textures: Option<&dyn TextureLoader>,
    ) -> Result<GlbGeometryPublication, ConversionError> {
        self.refuse_glb_input(out, animations)?;
        let protection = self.protection.context()?;
        let receipt = FileReceipt::for_staged_replacement(out, protection, CancelToken::new())
            .map_err(|error| write_error(out, error))?;
        self.refuse_glb_input(receipt.path(), animations)?;
        let summary = receipt
            .publish(
                |mut writer| {
                    let mut origins = GeometryOrigins::new(self);
                    let fallback = self
                        .default_material()
                        .and_then(ArchiveDefaultMaterial::material);
                    let summary = gltf::write_glb_observed(
                        self.model(),
                        &animations.animations,
                        options,
                        textures,
                        fallback,
                        &mut writer,
                        &mut |mesh| origins.observe(mesh),
                    )
                    .map_err(io::Error::other)?;
                    Ok((summary, origins))
                },
                |writer, binding, (_, origins)| {
                    let aggregate = origins.uses.origin(&origins.contributions, false);
                    let inventory = origins.uses.freeze().map_err(io::Error::other)?;
                    write_origin_companion(
                        writer,
                        binding,
                        &inventory,
                        std::iter::once((&OriginOutputAddress::OpaquePayload, &aggregate))
                            .chain(origins.fields.iter().map(|(field, origin)| (field, origin))),
                    )
                },
            )
            .map_err(|error| match error.downcast::<GltfError>() {
                Ok(error) => self.glb_write_error(out, error),
                Err(error) => write_error(out, error),
            })?
            .0;
        Ok(GlbGeometryPublication {
            summary,
            receipt: receipt.path().to_path_buf(),
        })
    }
}

struct GeometryOrigins<'model> {
    model: &'model ModelSource,
    uses: SourceUses,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    contributions: Vec<SourceUse>,
    scopes: Vec<BoundGeometry>,
}

struct BoundGeometry {
    file: ModelFile,
    scope: SourceScope,
    complete: bool,
}

impl<'model> GeometryOrigins<'model> {
    fn new(model: &'model ModelSource) -> Self {
        Self {
            model,
            uses: SourceUses::with_compact_use_refs(),
            fields: Vec::new(),
            contributions: Vec::new(),
            scopes: Vec::new(),
        }
    }

    fn observe(&mut self, mesh: MeshVertexMap<'_>) -> io::Result<()> {
        let file = mesh.origin.map(|origin| origin.file);
        for primitive in mesh.primitives() {
            let Some(accessor) = primitive.index_accessor else {
                continue;
            };
            let path = FieldPath::parse(&format!("/accessors/{accessor}/count"))
                .map_err(io::Error::other)?;
            let origin = match (file, primitive.original) {
                (Some(file), Some(original)) => {
                    self.index_count(file, original.index_selection_range().end)?
                }
                _ => self.uses.origin(&[], false),
            };
            self.fields
                .push((OriginOutputAddress::GlbJsonField { path }, origin));
        }
        Ok(())
    }

    fn index_count(&mut self, file: ModelFile, end: u64) -> io::Result<FieldOrigin> {
        let Some(bound) = self.scope(file)? else {
            return Ok(self.uses.origin(&[], false));
        };
        let start = end - size_of::<u32>() as u64;
        let source = bound
            .scope
            .at(
                SourceLocation::Subresource {
                    namespace: "model-byte-range".into(),
                    indices: vec![start, end],
                    field: "indexCount".into(),
                },
                SourceUseRole::Payload,
            )
            .map_err(io::Error::other)?;
        let complete = bound.complete;
        let origin = self.uses.origin(std::slice::from_ref(&source), complete);
        self.contributions.push(source);
        Ok(origin)
    }

    fn scope(&mut self, file: ModelFile) -> io::Result<Option<&BoundGeometry>> {
        if let Some(index) = self.scopes.iter().position(|bound| bound.file == file) {
            return Ok(self.scopes.get(index));
        }
        let Some(entry) = self
            .model
            .archive_sources()
            .iter()
            .find(|source| source.file() == file)
            .and_then(ModelArchiveSource::origin)
        else {
            return Ok(None);
        };
        self.scopes.push(BoundGeometry {
            file,
            scope: self
                .uses
                .observe(SourceInput::Archive(entry), InputScope::Payload)
                .map_err(io::Error::other)?,
            complete: entry.ancestors().all(|ancestor| ancestor.is_some()),
        });
        Ok(self.scopes.last())
    }
}
