// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_model::{ModelBoneSource, ModelBoneSources, ModelFile, PrimarySkeletonSelection};

use crate::model_geometry_scope::ModelGeometryScope;
use crate::source_provenance::ChunkHeaderField;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, ModelArchiveSource, ModelSource, SourceEvidenceError,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

/// Writes primary skeleton selection and bone evidence into a caller's inventory.
pub struct ModelBoneOrigins<'a> {
    uses: &'a SourceUses,
    scope: Option<ModelGeometryScope>,
    skeleton: Option<&'a ModelBoneSources>,
    selection: Vec<SourceUse>,
    complete: bool,
    count: usize,
}

impl<'a> ModelBoneOrigins<'a> {
    /// Borrows the primary skeleton coordinates captured by the model read.
    ///
    /// # Errors
    /// Returns a source evidence error.
    pub fn capture(source: &'a ModelSource, uses: &'a SourceUses) -> io::Result<Self> {
        let scope = ModelGeometryScope::capture(uses, source, ModelFile::Model)?;
        let primary = scope.as_ref().and_then(|_| {
            source
                .archive_sources()
                .iter()
                .find(|source| source.file() == ModelFile::Model)
        });
        let skeleton = primary.and_then(ModelArchiveSource::bone_sources);
        let survey = primary.and_then(ModelArchiveSource::primary_skeleton);
        let (selection, complete) = match &scope {
            Some(scope) => primary_skeleton_selection_uses(scope.scope(), survey, skeleton)
                .map_err(io::Error::other)?,
            None => (Vec::new(), false),
        };
        Ok(Self {
            uses,
            scope,
            skeleton,
            selection,
            complete,
            count: source.model().bones.len(),
        })
    }

    /// Returns the original selection evidence for the primary bone collection or count.
    #[must_use]
    pub fn collection_origin(&self) -> FieldOrigin {
        self.origin_when(&self.selection, self.complete)
    }

    /// Writes the origins for one primary bone at the caller's output row path.
    ///
    /// # Errors
    /// Returns an error for an invalid bone ordinal, path or source inventory.
    pub fn write_bone(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        ordinal: usize,
    ) -> io::Result<()> {
        if ordinal >= self.count {
            return Err(io::Error::other(
                "model bone evidence position is unavailable",
            ));
        }
        put(fields, path, "index", self.collection_origin())?;
        let original = self.skeleton.and_then(|skeleton| {
            Some(BoneFields {
                chunk: skeleton.chunk(),
                ordinal,
                fields: skeleton.bone(ordinal)?,
            })
        });
        for (name, role, bytes) in [
            (
                "name",
                SourceUseRole::Conversion,
                original.as_ref().map(|bone| bone.fields.name()),
            ),
            (
                "parent",
                SourceUseRole::Conversion,
                original.as_ref().map(|bone| bone.fields.parent()),
            ),
            (
                "controller_id",
                SourceUseRole::Payload,
                original.as_ref().map(|bone| bone.fields.controller_id()),
            ),
            (
                "node",
                SourceUseRole::Conversion,
                original.as_ref().and_then(|bone| bone.fields.node()),
            ),
        ] {
            let origin = self.value_origin(original.as_ref(), bytes, name, role)?;
            put(fields, path, name, origin)?;
        }
        self.write_pose(fields, path, original.as_ref(), Pose::Relative)?;
        self.write_pose(fields, path, original.as_ref(), Pose::World)
    }

    fn origin_when(&self, actual: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(
            actual,
            complete && self.scope.as_ref().is_some_and(|scope| scope.complete),
        )
    }

    fn value_origin(
        &self,
        bone: Option<&BoneFields<'_>>,
        bytes: Option<Range<u64>>,
        name: &str,
        role: SourceUseRole,
    ) -> io::Result<FieldOrigin> {
        let Some(bone) = bone else {
            return Ok(self.origin_when(&self.selection, false));
        };
        let Some(bytes) = bytes else {
            return Ok(self.collection_origin());
        };
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| io::Error::other("model bone evidence has no original Entry"))?;
        let mut uses = self.selection.clone();
        uses.push(
            site(
                scope.scope(),
                "cryEngineSkeleton",
                name,
                vec![
                    bone.chunk as u64,
                    bone.ordinal as u64,
                    bytes.start,
                    bytes.end,
                ],
                role,
            )
            .map_err(io::Error::other)?,
        );
        Ok(self.origin_when(&uses, self.complete))
    }

    fn write_pose(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        bone: Option<&BoneFields<'_>>,
        pose: Pose,
    ) -> io::Result<()> {
        let base = bone.map(|bone| pose.range(bone.fields));
        let name = pose.name();
        let origin = self.value_origin(bone, base.clone(), name, SourceUseRole::Payload)?;
        put(fields, path, name, origin)?;
        let mut path = path.clone();
        path.push_field(name).map_err(io::Error::other)?;
        for (vector, first, count) in [("rotation", 0, 4_usize), ("translation", 4, 3)] {
            let width = size_of::<f32>() as u64;
            let field = format!("{name}.{vector}");
            let span = base.as_ref().map(|base| {
                base.start + first * width..base.start + (first + count as u64) * width
            });
            let origin = self.value_origin(bone, span.clone(), &field, SourceUseRole::Payload)?;
            put(fields, &path, vector, origin)?;
            path.push_field(vector).map_err(io::Error::other)?;
            for index in 0..count {
                let component = span.as_ref().map(|span| {
                    span.start + index as u64 * width..span.start + (index as u64 + 1) * width
                });
                path.push_index(index).map_err(io::Error::other)?;
                fields
                    .insert(
                        path.clone(),
                        self.value_origin(bone, component, &field, SourceUseRole::Payload)?,
                    )
                    .map_err(io::Error::other)?;
                path.pop();
            }
            path.pop();
        }
        Ok(())
    }
}

struct BoneFields<'a> {
    chunk: usize,
    ordinal: usize,
    fields: &'a ModelBoneSource,
}

#[derive(Clone, Copy)]
enum Pose {
    Relative,
    World,
}

impl Pose {
    const fn name(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::World => "world",
        }
    }

    fn range(self, fields: &ModelBoneSource) -> Range<u64> {
        match self {
            Self::Relative => fields.relative_pose(),
            Self::World => fields.world_pose(),
        }
    }
}

/// Records the original primary-skeleton survey and selected chunk contributors.
/// The completeness flag describes the selection before combining Archive source coverage.
///
/// # Errors
/// Returns a source evidence error.
pub fn primary_skeleton_selection_uses(
    scope: &SourceScope,
    survey: Option<&PrimarySkeletonSelection>,
    skeleton: Option<&ModelBoneSources>,
) -> Result<(Vec<SourceUse>, bool), SourceEvidenceError> {
    let (mut selection, mut complete) = selection_uses(scope, survey)?;
    if let Some(skeleton) = skeleton {
        let ordinal = skeleton.chunk() as u64;
        if let Some(row) = skeleton.table_row() {
            for (name, span) in [("type", &row.kind), ("offset", &row.offset)] {
                selection.push(site(
                    scope,
                    "cryEngineChunkTable",
                    name,
                    vec![ordinal, span.start, span.end],
                    SourceUseRole::Selector,
                )?);
            }
        } else {
            complete = false;
        }
        let count = skeleton.count_range();
        selection.push(site(
            scope,
            "cryEngineSkeleton",
            "boneCount",
            vec![ordinal, count.start, count.end],
            SourceUseRole::Payload,
        )?);
    }
    Ok((selection, complete))
}

fn selection_uses(
    scope: &SourceScope,
    survey: Option<&PrimarySkeletonSelection>,
) -> Result<(Vec<SourceUse>, bool), SourceEvidenceError> {
    let mut selection = Vec::new();
    let complete = match survey {
        Some(PrimarySkeletonSelection::ContainerOnly) => {
            selection.push(ChunkHeaderField::Signature.observe(scope, SourceUseRole::Selector)?);
            true
        }
        Some(PrimarySkeletonSelection::ChunkTable { prefix }) => {
            for field in [
                ChunkHeaderField::Signature,
                ChunkHeaderField::RowCount,
                ChunkHeaderField::TableOffset,
            ] {
                selection.push(field.observe(scope, SourceUseRole::Selector)?);
            }
            if let Some(prefix) = prefix {
                let first = prefix.first_word();
                selection.push(site(
                    scope,
                    "cryEngineChunkTable",
                    "skeletonTypePrefix",
                    vec![
                        first.start,
                        first.end,
                        prefix.stride(),
                        prefix.count() as u64,
                    ],
                    SourceUseRole::Selector,
                )?);
            }
            true
        }
        _ => false,
    };
    Ok((selection, complete))
}

fn site(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            field: field.into(),
            indices,
        },
        role,
    )
}

fn put(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> io::Result<()> {
    let mut path = parent.clone();
    path.push_field(name).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
