// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use p4kx_gltf::{
    self as gltf, GlbOptions, GlbSummary, GltfError, MeshVertexMap, PrimitiveMaterialSource,
    TextureLoader,
};
use p4kx_model::ModelFile;
use thiserror::Error;

use crate::model_geometry_scope::ModelGeometryScope;
use crate::model_observed::Observed;
use crate::model_source::{ArchiveDefaultMaterial, ModelSource};
use crate::output_file::write_error;
use crate::{
    AnimationSet, CancelToken, ConversionError, FieldOrigin, FieldPath, FileReceipt,
    OriginOutputAddress, SourceUse, SourceUseRole, SourceUses, WriteProtectionError,
    write_origin_companion,
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
    /// Writes the standalone GLB and a digest-bound receipt for observed geometry fields.
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
        self.write_glb_with_geometry_receipt_observed(
            animations,
            options,
            out,
            textures,
            &CancelToken::new(),
            &|_, _| {},
        )
    }

    /// Publishes the GLB and its digest-bound receipt with cancellation and GLB write progress.
    ///
    /// # Errors
    /// Returns the GLB or receipt write error, or cancellation before the GLB commits.
    pub fn write_glb_with_geometry_receipt_observed(
        &self,
        animations: &AnimationSet,
        options: &GlbOptions,
        out: &Path,
        textures: Option<&dyn TextureLoader>,
        cancel: &CancelToken,
        progress: &dyn Fn(u64, u64),
    ) -> Result<GlbGeometryPublication, ConversionError> {
        self.refuse_glb_input(out, animations)?;
        if cancel.is_cancelled() {
            return Err(ConversionError::Cancelled);
        }
        let protection = self.protection.context()?;
        let receipt = FileReceipt::for_staged_replacement(out, protection, cancel.clone())
            .map_err(|error| self.geometry_publication_error(out, error, false, cancel))?;
        self.refuse_glb_input(receipt.path(), animations)?;
        let mut committed = false;
        let summary = receipt
            .publish_observing_commit(
                |writer| {
                    let mut writer = Observed::new(writer, cancel, progress);
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
                || committed = true,
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
            .map_err(|error| self.geometry_publication_error(out, error, committed, cancel))?
            .0;
        Ok(GlbGeometryPublication {
            summary,
            receipt: receipt.path().to_path_buf(),
        })
    }

    fn geometry_publication_error(
        &self,
        out: &Path,
        error: io::Error,
        committed: bool,
        cancel: &CancelToken,
    ) -> ConversionError {
        if committed {
            return ConversionError::committed_glb_publication(out, error);
        }
        match error.downcast::<GltfError>() {
            Ok(error) => self.glb_write_error(out, error),
            Err(error) => publication_io_error(out, error, cancel),
        }
    }
}

fn publication_io_error(out: &Path, error: io::Error, cancel: &CancelToken) -> ConversionError {
    if FileReceipt::is_precommit_cancellation(&error, cancel) {
        return ConversionError::Cancelled;
    }
    if let Some(protection) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<WriteProtectionError>())
    {
        return ConversionError::Installation(protection.clone());
    }
    write_error(out, error)
}

#[derive(Debug, Error)]
#[error("the GLB was committed, but origin publication did not finish: {0}")]
struct CommittedGlbPublication(#[source] io::Error);

impl ConversionError {
    /// Reports a standalone GLB publication failure after the file committed.
    #[must_use]
    pub fn committed_glb_publication(out: &Path, source: io::Error) -> Self {
        write_error(out, io::Error::other(CommittedGlbPublication(source)))
    }

    /// Returns whether a standalone GLB committed before publication failed.
    #[must_use]
    pub fn standalone_glb_was_committed(&self) -> bool {
        matches!(
            self,
            Self::Write { source, .. }
                if source.get_ref().is_some_and(
                    <dyn std::error::Error + Send + Sync>::is::<CommittedGlbPublication>,
                )
        )
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
    scope: ModelGeometryScope,
}

#[derive(Clone, Copy)]
enum ContributionCoverage {
    Complete,
    Partial,
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
        let library = self
            .model
            .model()
            .materials
            .library()
            .filter(|library| !library.single);
        for primitive in mesh.primitives() {
            if let Some(accessor) = primitive.index_accessor {
                let origin = match (file, primitive.original) {
                    (Some(file), Some(original)) => self.origin_for_range(
                        file,
                        original.index_count_range(),
                        "indexCount",
                        SourceUseRole::Payload,
                        ContributionCoverage::Complete,
                    )?,
                    _ => self.uses.origin(&[], false),
                };
                self.push_field(&format!("/accessors/{accessor}/count"), origin)?;
            }
            if primitive.material_index.is_some() {
                let origin = match (file, primitive.original, primitive.material_source) {
                    (Some(file), Some(original), PrimitiveMaterialSource::Library(slot))
                        if usize::from(original.material()) == slot
                            && library.is_some_and(|library| {
                                !library.is_empty_slot(original.material())
                            }) =>
                    {
                        self.origin_for_range(
                            file,
                            original.material_range(),
                            "materialId",
                            SourceUseRole::Selector,
                            ContributionCoverage::Partial,
                        )?
                    }
                    _ => self.uses.origin(&[], false),
                };
                self.push_field(
                    &format!(
                        "/meshes/{}/primitives/{}/material",
                        mesh.mesh, primitive.primitive
                    ),
                    origin,
                )?;
            }
        }
        Ok(())
    }

    fn push_field(&mut self, pointer: &str, origin: FieldOrigin) -> io::Result<()> {
        let path = FieldPath::parse(pointer).map_err(io::Error::other)?;
        self.fields
            .push((OriginOutputAddress::GlbJsonField { path }, origin));
        Ok(())
    }

    fn origin_for_range(
        &mut self,
        file: ModelFile,
        range: Range<u64>,
        field: &str,
        role: SourceUseRole,
        coverage: ContributionCoverage,
    ) -> io::Result<FieldOrigin> {
        let Some(bound) = self.scope(file)? else {
            return Ok(self.uses.origin(&[], false));
        };
        let source = bound.scope.word(range, field, role)?;
        let complete = bound.scope.complete && matches!(coverage, ContributionCoverage::Complete);
        let origin = self.uses.origin(std::slice::from_ref(&source), complete);
        self.contributions.push(source);
        Ok(origin)
    }

    fn scope(&mut self, file: ModelFile) -> io::Result<Option<&BoundGeometry>> {
        if let Some(index) = self.scopes.iter().position(|bound| bound.file == file) {
            return Ok(self.scopes.get(index));
        }
        let Some(scope) = ModelGeometryScope::capture(&self.uses, self.model, file)? else {
            return Ok(None);
        };
        self.scopes.push(BoundGeometry { file, scope });
        Ok(self.scopes.last())
    }
}
