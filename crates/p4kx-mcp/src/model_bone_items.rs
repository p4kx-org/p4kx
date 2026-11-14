// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{ModelBoneSource, ModelBoneSources, PrimarySkeletonSelection};
use p4kx_extraction::{
    BoneInfo, FieldOrigin, FieldOrigins, FieldPath, ModelArchiveSource, SourceEvidenceError,
    SourceLocation, SourceUse, SourceUseRole,
};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(crate) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let bones = &request.source.model().bones;
    let primary = request.primary();
    let values = request
        .page
        .slice(bones)
        .iter()
        .enumerate()
        .map(|(position, bone)| {
            serde_json::to_value(BoneInfo::of_bone(request.page.offset() + position, bone))
                .map_err(ToolError::from)
        });
    request.publish(bones.len(), values, |range, cursor| {
        field_origins(primary, range, cursor)
    })
}

fn field_origins(
    primary: Option<&ModelArchiveSource>,
    range: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let entry = EntryPageSources::from_origin(primary.and_then(ModelArchiveSource::origin))?;
    let skeleton = primary.and_then(ModelArchiveSource::bone_sources);
    let survey = primary.and_then(ModelArchiveSource::primary_skeleton);
    let mut fields = Fields::new(&entry, survey, skeleton)?;
    let root = FieldPath::parse("/structuredContent")?;
    fields.put(&root, "archive", entry.local())?;
    fields.put(&root, "model", entry.path.clone())?;
    fields.put(&root, "section", entry.local())?;
    let count = fields.common_origin();
    fields.put(&root, "total", count.clone())?;
    fields.put(&root, "items", count.clone())?;
    if cursor.is_some() {
        fields.put(&root, "next_cursor", count)?;
    }
    let mut path = root;
    path.push_field("items")?;
    for (position, ordinal) in range.enumerate() {
        path.push_index(position)?;
        fields.bone(&path, ordinal, skeleton)?;
        path.pop();
    }
    Ok(ModelItemOrigins {
        field_origins: fields.result,
        provenance: entry.uses.freeze()?,
    })
}

struct Fields<'a> {
    entry: &'a EntryPageSources,
    selection: Vec<SourceUse>,
    complete: bool,
    result: FieldOrigins,
}

impl<'a> Fields<'a> {
    fn new(
        entry: &'a EntryPageSources,
        survey: Option<&PrimarySkeletonSelection>,
        skeleton: Option<&ModelBoneSources>,
    ) -> Result<Self, SourceEvidenceError> {
        let (selection, complete) = primary_selection(entry, survey, skeleton)?;
        Ok(Self {
            entry,
            selection,
            complete,
            result: FieldOrigins::new(),
        })
    }

    fn common_origin(&self) -> FieldOrigin {
        self.entry.origin_when(&self.selection, self.complete)
    }

    fn put(
        &mut self,
        parent: &FieldPath,
        name: &str,
        value: FieldOrigin,
    ) -> Result<(), EvidenceError> {
        model_item_page::put(&mut self.result, parent, name, value)
    }

    fn bone(
        &mut self,
        path: &FieldPath,
        ordinal: usize,
        skeleton: Option<&ModelBoneSources>,
    ) -> Result<(), EvidenceError> {
        self.put(path, "index", self.common_origin())?;
        let original = skeleton.and_then(|skeleton| {
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
            self.put(path, name, origin)?;
        }
        self.pose(path, original.as_ref(), Pose::Relative)?;
        self.pose(path, original.as_ref(), Pose::World)
    }

    fn value_origin(
        &self,
        bone: Option<&BoneFields<'_>>,
        bytes: Option<Range<u64>>,
        name: &str,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let Some(bone) = bone else {
            return Ok(self.entry.origin_when(&self.selection, false));
        };
        let Some(bytes) = bytes else {
            return Ok(self.common_origin());
        };
        let mut uses = self.selection.clone();
        uses.push(site(
            self.entry,
            "cryEngineSkeleton",
            name,
            vec![
                bone.chunk as u64,
                bone.ordinal as u64,
                bytes.start,
                bytes.end,
            ],
            role,
        )?);
        Ok(self.entry.origin_when(&uses, self.complete))
    }

    fn pose(
        &mut self,
        path: &FieldPath,
        bone: Option<&BoneFields<'_>>,
        pose: Pose,
    ) -> Result<(), EvidenceError> {
        let base = bone.map(|bone| pose.range(bone.fields));
        let name = pose.name();
        let complete = self.value_origin(bone, base.clone(), name, SourceUseRole::Payload)?;
        self.put(path, name, complete)?;
        let mut path = path.clone();
        path.push_field(name)?;
        for (vector, first, count) in [("rotation", 0, 4_usize), ("translation", 4, 3)] {
            let width = size_of::<f32>() as u64;
            let field = format!("{name}.{vector}");
            let span = base.as_ref().map(|base| {
                base.start + first * width..base.start + (first + count as u64) * width
            });
            let origin = self.value_origin(bone, span.clone(), &field, SourceUseRole::Payload)?;
            self.put(&path, vector, origin)?;
            path.push_field(vector)?;
            for index in 0..count {
                let component = span.as_ref().map(|span| {
                    span.start + index as u64 * width..span.start + (index as u64 + 1) * width
                });
                path.push_index(index)?;
                self.result.insert(
                    path.clone(),
                    self.value_origin(bone, component, &field, SourceUseRole::Payload)?,
                )?;
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

pub(crate) fn primary_selection(
    entry: &EntryPageSources,
    survey: Option<&PrimarySkeletonSelection>,
    skeleton: Option<&ModelBoneSources>,
) -> Result<(Vec<SourceUse>, bool), SourceEvidenceError> {
    let (mut selection, mut complete) = selection_uses(entry, survey)?;
    if let Some(skeleton) = skeleton {
        if let Some(row) = skeleton.table_row() {
            for (name, span) in [("type", &row.kind), ("offset", &row.offset)] {
                selection.push(site(
                    entry,
                    "cryEngineChunkTable",
                    name,
                    vec![skeleton.chunk() as u64, span.start, span.end],
                    SourceUseRole::Selector,
                )?);
            }
        } else {
            complete = false;
        }
        let count = skeleton.count_range();
        selection.push(site(
            entry,
            "cryEngineSkeleton",
            "boneCount",
            vec![skeleton.chunk() as u64, count.start, count.end],
            SourceUseRole::Payload,
        )?);
    }
    Ok((selection, complete))
}

fn selection_uses(
    entry: &EntryPageSources,
    survey: Option<&PrimarySkeletonSelection>,
) -> Result<(Vec<SourceUse>, bool), SourceEvidenceError> {
    let mut selection = Vec::new();
    let complete = match survey {
        Some(PrimarySkeletonSelection::ContainerOnly) => {
            selection.push(site(
                entry,
                "cryEngineChunkHeader",
                "signature",
                vec![0, 4],
                SourceUseRole::Selector,
            )?);
            true
        }
        Some(PrimarySkeletonSelection::ChunkTable { prefix }) => {
            for (field, start, end) in [
                ("signature", 0, 4),
                ("rowCount", 8, 12),
                ("tableOffset", 12, 16),
            ] {
                selection.push(site(
                    entry,
                    "cryEngineChunkHeader",
                    field,
                    vec![start, end],
                    SourceUseRole::Selector,
                )?);
            }
            if let Some(prefix) = prefix {
                let first = prefix.first_word();
                selection.push(site(
                    entry,
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
    entry: &EntryPageSources,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    entry.subresource(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            field: field.into(),
            indices,
        },
        role,
    )
}
