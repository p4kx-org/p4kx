// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Export as GLB windows, looked up in the Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelExportText {
    /// The action in the Export menu and the preview that opens the setup.
    Action,
    /// The title of the setup window.
    SetupTitle,
    /// The model that is exported; takes `name`.
    Model,
    /// The heading of the level of detail choice.
    LodHeading,
    /// One level of detail; takes `level`, `triangles` and `number`.
    Lod,
    /// The heading of the material choice.
    MaterialsHeading,
    /// Materials as plain colours.
    MaterialsColours,
    /// Materials with their textures.
    MaterialsTextures,
    /// The heading of the texture size choice.
    TextureMipHeading,
    /// One texture size; takes `level` and `divisor`.
    TextureMip,
    /// The heading of the animation choice.
    AnimationsHeading,
    /// The switch of the animations; takes `count` and `number`.
    Animations,
    /// The switch of the animations when the model has more files next to it than are counted.
    AnimationsUnknown,
    /// The warning under that switch: the animations are read in full.
    AnimationsLarge,
    /// The switch of the vertex colours.
    VertexColours,
    /// The heading of the file choice.
    FileHeading,
    /// The action that opens the file dialog.
    ChooseFile,
    /// The hint of the file field while no file is chosen.
    FileNone,
    /// The name of the GLB file type in the file dialog.
    FileFilter,
    /// The action that starts the export.
    Start,
    /// The title of the progress window.
    RunningTitle,
    /// Shown while the model is read; takes `name`.
    Reading,
    /// The progress while the file is written; takes `done`, `total` and `percent`.
    Progress,
    /// The title of the summary of a finished export.
    SummaryTitle,
    /// The title of the summary of a cancelled export.
    SummaryCancelledTitle,
    /// The note that the export was cancelled and wrote no file.
    SummaryCancelled,
    /// The file that was written; takes `path`.
    SummaryFile,
    /// What the file holds; takes `nodes`, `meshes`, `materials`, `vertices`, `triangles` and
    /// `size`.
    SummaryCounts,
    /// The skeleton in the file; takes `bones` and `skinned`.
    SummaryBones,
    /// The material library does not exist; takes `library`.
    SummaryLibraryMissing,
    /// The material library cannot be read; takes `library` and `reason`.
    SummaryLibraryUnreadable,
    /// The mapped material library does not exist; takes `library` and `target`.
    SummaryLibraryMissingMapped,
    /// The mapped library cannot be read; takes `library`, `target` and `reason`.
    SummaryLibraryUnreadableMapped,
    /// Material IDs without a sub-material; takes `number`.
    SummaryDefaultMaterials,
    /// Library entries that are no material; takes `number`.
    SummaryUnnamedMaterials,
    /// Layer files of the material library that are missing or unreadable; takes `number`.
    SummaryMissingLayers,
    /// Submeshes left out because their material draws nothing; takes `number`.
    SummaryHidden,
    /// The heading of the chunks that were not exported; takes `number`.
    SummaryChunks,
    /// One chunk of an unknown type; takes `index`, `type_id` and `file`.
    SummaryChunkUnknown,
    /// One chunk the model does not read; takes `index`, `type`, `type_id` and `file`.
    SummaryChunkSkipped,
    /// The heading of the textures the file lacks; takes `number`.
    SummaryTextures,
    /// One texture the file lacks; takes `material`, `role` (`diffuse`, `normal` or `specular`),
    /// `file` and `reason`.
    SummaryTexture,
    /// The animations in the file; takes `written` and `channels`.
    SummaryAnimations,
    /// The heading of the animations that were not written; takes `number`.
    SummaryAnimationsUnread,
    /// One animation of a database that was not written; takes `name`, `source` and `reason`.
    SummaryAnimationUnread,
    /// One animation file that was not written; takes `source` and `reason`.
    SummaryAnimationFileUnread,
    /// One animation that was read but has no track for a bone of the model; takes `name`,
    /// `source` and `tracks` with the formatted `number` of tracks left out.
    SummaryAnimationUnmatched,
    /// The title of the window shown when the export failed.
    FailedTitle,
    /// The model could not be exported; takes `name` and `reason`.
    Failed,
    /// The DataForge has no Record of the Entity; takes `guid`. The reason of [`Self::Failed`].
    NoRecord,
    /// The export stopped without a result; takes `name`.
    Stopped,
    /// The model is encrypted and the Archive has no key; takes `name`.
    KeyRequired,
    /// The Archive key does not decrypt the model; takes `name`.
    KeyRejected,
    /// The Archive changed on disk; takes `name`.
    ArchiveChanged,
}

impl ModelExportText {
    /// Every text, for checks of the Fluent files.
    pub(crate) const ALL: [Self; 54] = [
        Self::Action,
        Self::SetupTitle,
        Self::Model,
        Self::LodHeading,
        Self::Lod,
        Self::MaterialsHeading,
        Self::MaterialsColours,
        Self::MaterialsTextures,
        Self::TextureMipHeading,
        Self::TextureMip,
        Self::AnimationsHeading,
        Self::Animations,
        Self::AnimationsUnknown,
        Self::AnimationsLarge,
        Self::VertexColours,
        Self::FileHeading,
        Self::ChooseFile,
        Self::FileNone,
        Self::FileFilter,
        Self::Start,
        Self::RunningTitle,
        Self::Reading,
        Self::Progress,
        Self::SummaryTitle,
        Self::SummaryCancelledTitle,
        Self::SummaryCancelled,
        Self::SummaryFile,
        Self::SummaryCounts,
        Self::SummaryBones,
        Self::SummaryLibraryMissing,
        Self::SummaryLibraryUnreadable,
        Self::SummaryLibraryMissingMapped,
        Self::SummaryLibraryUnreadableMapped,
        Self::SummaryDefaultMaterials,
        Self::SummaryUnnamedMaterials,
        Self::SummaryMissingLayers,
        Self::SummaryHidden,
        Self::SummaryChunks,
        Self::SummaryChunkUnknown,
        Self::SummaryChunkSkipped,
        Self::SummaryTextures,
        Self::SummaryTexture,
        Self::SummaryAnimations,
        Self::SummaryAnimationsUnread,
        Self::SummaryAnimationUnread,
        Self::SummaryAnimationFileUnread,
        Self::SummaryAnimationUnmatched,
        Self::FailedTitle,
        Self::Failed,
        Self::NoRecord,
        Self::Stopped,
        Self::KeyRequired,
        Self::KeyRejected,
        Self::ArchiveChanged,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Action => "model-export-action",
            Self::SetupTitle => "model-export-title",
            Self::Model => "model-export-model",
            Self::LodHeading => "model-export-lod-heading",
            Self::Lod => "model-export-lod",
            Self::MaterialsHeading => "model-export-materials-heading",
            Self::MaterialsColours => "model-export-materials-colours",
            Self::MaterialsTextures => "model-export-materials-textures",
            Self::TextureMipHeading => "model-export-texture-mip-heading",
            Self::TextureMip => "model-export-texture-mip",
            Self::AnimationsHeading => "model-export-animations-heading",
            Self::Animations => "model-export-animations",
            Self::AnimationsUnknown => "model-export-animations-unknown",
            Self::AnimationsLarge => "model-export-animations-large",
            Self::VertexColours => "model-export-vertex-colours",
            Self::FileHeading => "model-export-file-heading",
            Self::ChooseFile => "model-export-choose-file",
            Self::FileNone => "model-export-file-none",
            Self::FileFilter => "model-export-file-filter",
            Self::Start => "model-export-start",
            Self::RunningTitle => "model-export-running-title",
            Self::Reading => "model-export-reading",
            Self::Progress => "model-export-progress",
            Self::SummaryTitle => "model-export-summary-title",
            Self::SummaryCancelledTitle => "model-export-summary-cancelled-title",
            Self::SummaryCancelled => "model-export-summary-cancelled",
            Self::SummaryFile => "model-export-summary-file",
            Self::SummaryCounts => "model-export-summary-counts",
            Self::SummaryBones => "model-export-summary-bones",
            Self::SummaryLibraryMissing => "model-export-summary-library-missing",
            Self::SummaryLibraryUnreadable => "model-export-summary-library-unreadable",
            Self::SummaryLibraryMissingMapped => "model-export-summary-library-missing-mapped",
            Self::SummaryLibraryUnreadableMapped => {
                "model-export-summary-library-unreadable-mapped"
            }
            Self::SummaryDefaultMaterials => "model-export-summary-default-materials",
            Self::SummaryUnnamedMaterials => "model-export-summary-unnamed-materials",
            Self::SummaryMissingLayers => "model-export-summary-missing-layers",
            Self::SummaryHidden => "model-export-summary-hidden",
            Self::SummaryChunks => "model-export-summary-chunks",
            Self::SummaryChunkUnknown => "model-export-summary-chunk-unknown",
            Self::SummaryChunkSkipped => "model-export-summary-chunk-skipped",
            Self::SummaryTextures => "model-export-summary-textures",
            Self::SummaryTexture => "model-export-summary-texture",
            Self::SummaryAnimations => "model-export-summary-animations",
            Self::SummaryAnimationsUnread => "model-export-summary-animations-unread",
            Self::SummaryAnimationUnread => "model-export-summary-animation-unread",
            Self::SummaryAnimationFileUnread => "model-export-summary-animation-file-unread",
            Self::SummaryAnimationUnmatched => "model-export-summary-animation-unmatched",
            Self::FailedTitle => "model-export-failed-title",
            Self::Failed => "model-export-failed",
            Self::NoRecord => "model-export-no-record",
            Self::Stopped => "model-export-stopped",
            Self::KeyRequired => "model-export-key-required",
            Self::KeyRejected => "model-export-key-rejected",
            Self::ArchiveChanged => "model-export-archive-changed",
        }
    }
}

impl From<ModelExportText> for super::Text {
    fn from(text: ModelExportText) -> Self {
        Self::ModelExport(text)
    }
}
