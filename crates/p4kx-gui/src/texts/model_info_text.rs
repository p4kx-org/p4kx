// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Info tab of the model preview, looked up in the Fluent file of the current
/// language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelInfoText {
    /// The preview tab that lists what a model file holds.
    Tab,
    /// The key of the file name of the geometry companion.
    Geometry,
    /// The geometry is in the model file itself.
    GeometryInFile,
    /// The key of the number of nodes.
    Nodes,
    /// The key of the number of meshes of the most detailed level.
    Meshes,
    /// The key of the number of levels of detail.
    Lods,
    /// The key of the material library.
    Materials,
    /// The key of the material IDs the submeshes use.
    MaterialIds,
    /// The model has no submesh with a material ID.
    MaterialIdsNone,
    /// The key of the number of bones.
    Bones,
    /// The key of the number of animations.
    Animations,
    /// Shown while the animations of the model are not read, because they are too many or cannot
    /// be.
    AnimationsUnread,
    /// Shown while the animations of the model are being read.
    AnimationsReading,
    /// The key of the chunks.
    Chunks,
    /// The counts of the chunks; takes `total`, `read`, `skipped` and `unknown`.
    ChunksValue,
    /// The model names no material library.
    MaterialsUnnamed,
    /// The library was read; takes `library` and `count` with the formatted `number` of
    /// sub-materials.
    MaterialsLoaded,
    /// The library is one material for every ID; takes `library`.
    MaterialsSingle,
    /// The library does not exist; takes `library`.
    MaterialsMissing,
    /// The library cannot be read; takes `library` and `reason`.
    MaterialsUnreadable,
    /// The heading of the levels of detail.
    LodsHeading,
    /// The column of the level.
    ColumnLod,
    /// The column of the vertices.
    ColumnVertices,
    /// The column of the triangles.
    ColumnTriangles,
    /// The column of the meshes.
    ColumnMeshes,
    /// The heading of the chunks the model did not read; takes `number`.
    ChunksHeading,
    /// The column of the file holding a chunk.
    ColumnFile,
    /// The column of the chunk type.
    ColumnType,
    /// The column of the chunk type ID.
    ColumnTypeId,
    /// The column of how the model used the chunk.
    ColumnUsage,
    /// The chunk type is known but not read.
    UsageSkipped,
    /// The chunk type is unknown.
    UsageUnknown,
    /// The chunk is in the model file.
    FileModel,
    /// The chunk is in the geometry companion.
    FileGeometry,
    /// The chunk is in the companion of a level of detail; takes `level`.
    FileLod,
    /// The chunk is in the model file of a level of detail; takes `level`.
    FileLodModel,
    /// The type of a chunk has no name.
    TypeUnknown,
}

impl ModelInfoText {
    /// Every text of the Info tab.
    pub(crate) const ALL: [Self; 37] = [
        Self::Tab,
        Self::Geometry,
        Self::GeometryInFile,
        Self::Nodes,
        Self::Meshes,
        Self::Lods,
        Self::Materials,
        Self::MaterialIds,
        Self::MaterialIdsNone,
        Self::Bones,
        Self::Animations,
        Self::AnimationsUnread,
        Self::AnimationsReading,
        Self::Chunks,
        Self::ChunksValue,
        Self::MaterialsUnnamed,
        Self::MaterialsLoaded,
        Self::MaterialsSingle,
        Self::MaterialsMissing,
        Self::MaterialsUnreadable,
        Self::LodsHeading,
        Self::ColumnLod,
        Self::ColumnVertices,
        Self::ColumnTriangles,
        Self::ColumnMeshes,
        Self::ChunksHeading,
        Self::ColumnFile,
        Self::ColumnType,
        Self::ColumnTypeId,
        Self::ColumnUsage,
        Self::UsageSkipped,
        Self::UsageUnknown,
        Self::FileModel,
        Self::FileGeometry,
        Self::FileLod,
        Self::FileLodModel,
        Self::TypeUnknown,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tab => "preview-tab-model-info",
            Self::Geometry => "model-info-geometry",
            Self::GeometryInFile => "model-info-geometry-in-file",
            Self::Nodes => "model-info-nodes",
            Self::Meshes => "model-info-meshes",
            Self::Lods => "model-info-lods",
            Self::Materials => "model-info-materials",
            Self::MaterialIds => "model-info-material-ids",
            Self::MaterialIdsNone => "model-info-material-ids-none",
            Self::Bones => "model-info-bones",
            Self::Animations => "model-info-animations",
            Self::AnimationsUnread => "model-info-animations-unread",
            Self::AnimationsReading => "model-info-animations-reading",
            Self::Chunks => "model-info-chunks",
            Self::ChunksValue => "model-info-chunks-value",
            Self::MaterialsUnnamed => "model-info-materials-unnamed",
            Self::MaterialsLoaded => "model-info-materials-loaded",
            Self::MaterialsSingle => "model-info-materials-single",
            Self::MaterialsMissing => "model-info-materials-missing",
            Self::MaterialsUnreadable => "model-info-materials-unreadable",
            Self::LodsHeading => "model-info-lods-heading",
            Self::ColumnLod => "model-info-column-lod",
            Self::ColumnVertices => "model-info-column-vertices",
            Self::ColumnTriangles => "model-info-column-triangles",
            Self::ColumnMeshes => "model-info-column-meshes",
            Self::ChunksHeading => "model-info-chunks-heading",
            Self::ColumnFile => "model-info-column-file",
            Self::ColumnType => "model-info-column-type",
            Self::ColumnTypeId => "model-info-column-type-id",
            Self::ColumnUsage => "model-info-column-usage",
            Self::UsageSkipped => "model-info-usage-skipped",
            Self::UsageUnknown => "model-info-usage-unknown",
            Self::FileModel => "model-info-file-model",
            Self::FileGeometry => "model-info-file-geometry",
            Self::FileLod => "model-info-file-lod",
            Self::FileLodModel => "model-info-file-lod-model",
            Self::TypeUnknown => "model-info-type-unknown",
        }
    }
}

impl From<ModelInfoText> for super::Text {
    fn from(text: ModelInfoText) -> Self {
        Self::ModelInfo(text)
    }
}
