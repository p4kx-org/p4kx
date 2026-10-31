// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Export of an Entity as GLB windows, looked up in the Fluent file of the current
/// language. The texts of a failed export are those of [`ModelExportText`](crate::ModelExportText).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssemblyExportText {
    /// The title of the setup window.
    SetupTitle,
    /// The title of the setup window of an Interior container.
    SetupTitleInterior,
    /// The action that opens the export of the Interior of a socpak container.
    ActionInterior,
    /// The heading of the choice between the GLB file and the Blender package.
    FormHeading,
    /// The GLB file as the output.
    FormGlb,
    /// The Blender package as the output.
    FormPackage,
    /// What the Blender package is.
    FormPackageNote,
    /// The heading of the package folder choice.
    FolderHeading,
    /// The action that opens the folder dialog.
    ChooseFolder,
    /// The hint of the folder field while no folder is chosen.
    FolderNone,
    /// Shown when the chosen folder holds an earlier package that is replaced.
    PackageReplaces,
    /// Shown when the chosen folder is not empty and holds no package.
    PackageNotAPackage,
    /// Shown when the chosen path names no folder.
    PackageNoName,
    /// The title of the progress window of a package.
    RunningTitlePackage,
    /// The progress while the package is written; takes `name`.
    WritingPackage,
    /// The title of the summary of a written package.
    SummaryTitlePackage,
    /// What the package holds; takes `parts`, `modelled`, `nodes`, `meshes`, `textures`,
    /// `materials`, `palettes`, `files`, `vertices`, `triangles` and `size`.
    SummaryPackageCounts,
    /// The textures the package lacks; takes `count` and `number`.
    SummaryPackageTextures,
    /// The Entity that is exported; takes `name`.
    Entity,
    /// The container whose Interior is exported; takes `name`.
    Container,
    /// The heading of the choice to export the Interior with the hull.
    InteriorHeading,
    /// The switch that exports the Interior with the hull.
    Interior,
    /// What the changes to the Loadout amount to; takes `excluded` and `replaced`.
    Changes,
    /// Shown when the Loadout is the default one.
    ChangesNone,
    /// The heading of the field of the Loadout picks.
    PicksHeading,
    /// The hint of the picks field while it is empty.
    PicksHint,
    /// What the picks do and how indices count.
    PicksNote,
    /// Shown when the text of the picks field is no picks.
    PicksInvalid,
    /// The heading of the level of detail choice.
    LodHeading,
    /// One level of detail; takes `level`.
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
    /// The title of the progress window of an Interior container.
    RunningTitleInterior,
    /// Shown while the Loadout is resolved; takes `name`.
    Resolving,
    /// Shown while the models are read; takes `name`.
    Reading,
    /// Shown while the Interior is read; takes `name`, `done` and `total` model files.
    ReadingInterior,
    /// The progress while the file is written; takes `size`.
    Writing,
    /// The title of the summary of a finished export.
    SummaryTitle,
    /// The title of the summary of a finished export of an Interior container.
    SummaryTitleInterior,
    /// What the Interior of an Entity holds; takes `containers` (its object containers), `parts`,
    /// `models`, `modelled` and `lights`.
    SummaryInterior,
    /// What a container exported on its own holds; takes `containers` (opened, nested ones
    /// included), `parts`, `models`, `modelled` and `lights`.
    SummaryInteriorOpened,
    /// The title of the summary of a cancelled export.
    SummaryCancelledTitle,
    /// The note that the export was cancelled and wrote no file.
    SummaryCancelled,
    /// The file that was written; takes `path`.
    SummaryFile,
    /// A replacement written although its Entity does not fit the port; takes `entity`, `port`
    /// and `reason`.
    SummaryMismatch,
    /// What the file holds; takes `parts`, `modelled`, `nodes`, `meshes`, `materials`,
    /// `vertices`, `triangles` and `size`.
    SummaryCounts,
    /// The textures in the file; takes `images`, `textured` and `materials`.
    SummaryImages,
    /// The models read at a lower level of detail; takes `count` and `number`.
    SummaryFallbacks,
    /// Submeshes left out because their material draws nothing; takes `count` and `number`.
    SummaryHidden,
    /// Layer files of the material libraries that are missing or unreadable; takes `count` and
    /// `number`.
    SummaryMissingLayers,
    /// The heading of the report of the parts that were not resolved in full; takes `number`.
    ReportHeading,
    /// Shown when every part was resolved.
    ReportAll,
    /// A part left out of the file; takes `path`, `name` and `reason`.
    ReportSkipped,
    /// A part kept with a note; takes `path`, `name` and `reason`.
    ReportNote,
}

impl AssemblyExportText {
    /// Every text, for checks of the Fluent files.
    pub(crate) const ALL: [Self; 64] = [
        Self::FormHeading,
        Self::FormGlb,
        Self::FormPackage,
        Self::FormPackageNote,
        Self::FolderHeading,
        Self::ChooseFolder,
        Self::FolderNone,
        Self::PackageReplaces,
        Self::PackageNotAPackage,
        Self::PackageNoName,
        Self::RunningTitlePackage,
        Self::WritingPackage,
        Self::SummaryTitlePackage,
        Self::SummaryPackageCounts,
        Self::SummaryPackageTextures,
        Self::SetupTitle,
        Self::SetupTitleInterior,
        Self::ActionInterior,
        Self::Entity,
        Self::Container,
        Self::InteriorHeading,
        Self::Interior,
        Self::Changes,
        Self::ChangesNone,
        Self::PicksHeading,
        Self::PicksHint,
        Self::PicksNote,
        Self::PicksInvalid,
        Self::LodHeading,
        Self::Lod,
        Self::MaterialsHeading,
        Self::MaterialsColours,
        Self::MaterialsTextures,
        Self::TextureMipHeading,
        Self::TextureMip,
        Self::VertexColours,
        Self::FileHeading,
        Self::ChooseFile,
        Self::FileNone,
        Self::FileFilter,
        Self::Start,
        Self::RunningTitle,
        Self::RunningTitleInterior,
        Self::Resolving,
        Self::Reading,
        Self::ReadingInterior,
        Self::Writing,
        Self::SummaryTitle,
        Self::SummaryTitleInterior,
        Self::SummaryInterior,
        Self::SummaryInteriorOpened,
        Self::SummaryCancelledTitle,
        Self::SummaryCancelled,
        Self::SummaryFile,
        Self::SummaryMismatch,
        Self::SummaryCounts,
        Self::SummaryImages,
        Self::SummaryFallbacks,
        Self::SummaryHidden,
        Self::SummaryMissingLayers,
        Self::ReportHeading,
        Self::ReportAll,
        Self::ReportSkipped,
        Self::ReportNote,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one arm per text")]
    pub const fn id(self) -> &'static str {
        match self {
            Self::FormHeading => "assembly-export-form-heading",
            Self::FormGlb => "assembly-export-form-glb",
            Self::FormPackage => "assembly-export-form-package",
            Self::FormPackageNote => "assembly-export-form-package-note",
            Self::FolderHeading => "assembly-export-folder-heading",
            Self::ChooseFolder => "assembly-export-choose-folder",
            Self::FolderNone => "assembly-export-folder-none",
            Self::PackageReplaces => "assembly-export-package-replaces",
            Self::PackageNotAPackage => "assembly-export-package-not-a-package",
            Self::PackageNoName => "assembly-export-package-no-name",
            Self::RunningTitlePackage => "assembly-export-running-title-package",
            Self::WritingPackage => "assembly-export-writing-package",
            Self::SummaryTitlePackage => "assembly-export-summary-title-package",
            Self::SummaryPackageCounts => "assembly-export-summary-package-counts",
            Self::SummaryPackageTextures => "assembly-export-summary-package-textures",
            Self::SetupTitle => "assembly-export-title",
            Self::SetupTitleInterior => "assembly-export-title-interior",
            Self::ActionInterior => "assembly-export-action-interior",
            Self::Entity => "assembly-export-entity",
            Self::Container => "assembly-export-container",
            Self::InteriorHeading => "assembly-export-interior-heading",
            Self::Interior => "assembly-export-interior",
            Self::Changes => "assembly-export-changes",
            Self::ChangesNone => "assembly-export-changes-none",
            Self::PicksHeading => "assembly-export-picks-heading",
            Self::PicksHint => "assembly-export-picks-hint",
            Self::PicksNote => "assembly-export-picks-note",
            Self::PicksInvalid => "assembly-export-picks-invalid",
            Self::LodHeading => "assembly-export-lod-heading",
            Self::Lod => "assembly-export-lod",
            Self::MaterialsHeading => "assembly-export-materials-heading",
            Self::MaterialsColours => "assembly-export-materials-colours",
            Self::MaterialsTextures => "assembly-export-materials-textures",
            Self::TextureMipHeading => "assembly-export-texture-mip-heading",
            Self::TextureMip => "assembly-export-texture-mip",
            Self::VertexColours => "assembly-export-vertex-colours",
            Self::FileHeading => "assembly-export-file-heading",
            Self::ChooseFile => "assembly-export-choose-file",
            Self::FileNone => "assembly-export-file-none",
            Self::FileFilter => "assembly-export-file-filter",
            Self::Start => "assembly-export-start",
            Self::RunningTitle => "assembly-export-running-title",
            Self::RunningTitleInterior => "assembly-export-running-title-interior",
            Self::Resolving => "assembly-export-resolving",
            Self::Reading => "assembly-export-reading",
            Self::ReadingInterior => "assembly-export-reading-interior",
            Self::Writing => "assembly-export-writing",
            Self::SummaryTitle => "assembly-export-summary-title",
            Self::SummaryTitleInterior => "assembly-export-summary-title-interior",
            Self::SummaryInterior => "assembly-export-summary-interior",
            Self::SummaryInteriorOpened => "assembly-export-summary-interior-opened",
            Self::SummaryCancelledTitle => "assembly-export-summary-cancelled-title",
            Self::SummaryCancelled => "assembly-export-summary-cancelled",
            Self::SummaryFile => "assembly-export-summary-file",
            Self::SummaryMismatch => "assembly-export-summary-mismatch",
            Self::SummaryCounts => "assembly-export-summary-counts",
            Self::SummaryImages => "assembly-export-summary-images",
            Self::SummaryFallbacks => "assembly-export-summary-fallbacks",
            Self::SummaryHidden => "assembly-export-summary-hidden",
            Self::SummaryMissingLayers => "assembly-export-summary-missing-layers",
            Self::ReportHeading => "assembly-export-report-heading",
            Self::ReportAll => "assembly-export-report-all",
            Self::ReportSkipped => "assembly-export-report-skipped",
            Self::ReportNote => "assembly-export-report-note",
        }
    }
}

impl From<AssemblyExportText> for super::Text {
    fn from(text: AssemblyExportText) -> Self {
        Self::AssemblyExport(text)
    }
}
