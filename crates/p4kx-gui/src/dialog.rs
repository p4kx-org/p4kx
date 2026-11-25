// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_export::{CATALOG_FILE, DIAGNOSTIC_CATALOG_FILE, MARKER_CATALOG_FILE};
use crate::{ComparisonSide, QueryExportFormat};

/// A dialog the host shows for the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// Picks an Archive to open.
    OpenArchive,
    /// Picks a DataForge file such as `Game2.dcb` to open.
    OpenDataForge,
    /// Picks a folder to add as a manual entry.
    AddFolder,
    /// Picks an Archive to add as a manual entry.
    AddArchive,
    /// Picks the folder of an Extraction.
    ExtractionFolder,
    /// Picks the folder of files to convert.
    ConversionInput,
    /// Picks the folder the converted files are written to.
    ConversionOutput,
    /// Picks the Archive of one side of the Comparison.
    ComparisonArchive(ComparisonSide),
    /// Picks the file the Comparison report is saved to.
    ComparisonReport,
    /// Picks the file the Query result is exported to.
    QueryExport(QueryExportFormat),
    /// Picks the folder of a Record export.
    RecordExportFolder,
    /// Picks the file the P4KX catalog is written to.
    CatalogFile,
    /// Picks the file of a catalog with full source proofs.
    FullCatalogFile,
    /// Picks the file of an explicitly requested diagnostic P4KX catalog.
    DiagnosticCatalogFile,
    /// Picks the GLB file a model is exported to.
    ModelFile,
    /// Picks the GLB file an Entity is exported to.
    AssemblyFile,
    /// Picks the folder an Entity is exported to as a Blender package.
    AssemblyPackage,
    /// Picks the folder of a sound export.
    SoundsFolder,
    /// Picks the CHF file of a head.
    CharacterChf,
    /// Picks the GLB file a Character is exported to.
    CharacterFile,
    /// Picks the folder a Character is exported to as a Blender package.
    CharacterPackage,
    /// Picks the PNG or SVG file a UI picture is exported to.
    UiFile,
}

impl Dialog {
    pub(crate) const fn catalog_file_name(self) -> Option<&'static str> {
        match self {
            Self::CatalogFile => Some(MARKER_CATALOG_FILE),
            Self::FullCatalogFile => Some(CATALOG_FILE),
            Self::DiagnosticCatalogFile => Some(DIAGNOSTIC_CATALOG_FILE),
            _ => None,
        }
    }

    pub(crate) const fn is_parent_output(self) -> bool {
        self.catalog_file_name().is_some()
            || matches!(
                self,
                Self::ComparisonReport
                    | Self::QueryExport(_)
                    | Self::ModelFile
                    | Self::AssemblyFile
                    | Self::AssemblyPackage
                    | Self::CharacterFile
                    | Self::CharacterPackage
                    | Self::UiFile
            )
    }

    pub(crate) const fn is_package_output(self) -> bool {
        matches!(self, Self::AssemblyPackage | Self::CharacterPackage)
    }

    pub(crate) const fn is_glb_output(self) -> bool {
        matches!(
            self,
            Self::ModelFile | Self::AssemblyFile | Self::CharacterFile
        )
    }

    pub(crate) const fn is_setup_output(self) -> bool {
        self.is_glb_output() || self.is_package_output()
    }
}
