// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_assembly::{Assembly, Interior, Issue, LoadoutChanges};
use p4kx_blender::{Issue as PackageIssue, PackageSummary};
use p4kx_extraction::GlbSummary;

use crate::PortMismatch;
use crate::model_info::printable;

/// A part of the Assembly that could not be resolved or done in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartIssue {
    /// The path of the part's port from the Entity, port names joined by `/`; empty for the
    /// Entity itself.
    pub path: String,
    /// The class name of the part's Entity.
    pub name: String,
    /// What went wrong.
    pub reason: String,
    /// Whether the part, or its geometry, is left out of the file.
    pub skipped: bool,
}

impl PartIssue {
    pub(crate) fn of(issue: &Issue) -> Self {
        Self {
            path: printable(issue.path()),
            name: printable(issue.name()),
            reason: printable(&issue.kind().to_string()),
            skipped: issue.kind().skips(),
        }
    }

    pub(crate) fn of_package(issue: &PackageIssue) -> Self {
        Self {
            path: printable(&issue.path),
            name: printable(&issue.name),
            reason: printable(&issue.reason),
            skipped: issue.skipped,
        }
    }
}

/// What the Interior of an export holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteriorFacts {
    /// The object containers the Entity names that were opened, or, for a container exported on
    /// its own, the containers that were opened, the root and every nested one counted.
    pub containers: usize,
    /// Whether `containers` counts the object containers of an Entity rather than the containers
    /// opened for a container on its own.
    pub of_entity: bool,
    /// The parts of the containers: the containers themselves, their `.soc` files, models, props
    /// and lights.
    pub parts: usize,
    /// The parts with a model of their own, readable or not.
    pub model_parts: usize,
    /// The parts whose model was read.
    pub modelled_parts: usize,
    /// The lights.
    pub lights: usize,
}

/// What a finished Export as GLB wrote and what it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyExportSummary {
    file: PathBuf,
    name: String,
    lod: u32,
    parts: usize,
    modelled: usize,
    written: GlbSummary,
    issues: Vec<PartIssue>,
    missing_layers: usize,
    excluded: usize,
    replaced: usize,
    interior: Option<InteriorFacts>,
    package: Option<PackageSummary>,
    mismatches: Vec<PortMismatch>,
}

impl AssemblyExportSummary {
    pub(crate) fn of_package(
        (folder, name): (PathBuf, String),
        assembly: &Assembly,
        (package, changes): (PackageSummary, &LoadoutChanges),
    ) -> Self {
        let issues = package.issues.iter().map(PartIssue::of_package).collect();
        Self {
            issues,
            package: Some(package),
            ..Self::new((folder, name), assembly, (GlbSummary::default(), changes))
        }
    }

    pub(crate) fn new(
        (file, name): (PathBuf, String),
        assembly: &Assembly,
        (written, changes): (GlbSummary, &LoadoutChanges),
    ) -> Self {
        Self {
            file,
            name,
            lod: assembly.lod(),
            parts: assembly.parts().len(),
            modelled: assembly.modelled_parts(),
            written,
            issues: assembly.issues().iter().map(PartIssue::of).collect(),
            missing_layers: assembly.unresolved_layers().count(),
            excluded: changes.exclusions().len(),
            replaced: changes.replacements().len(),
            interior: assembly.interior().map(|interior| InteriorFacts {
                containers: interior.containers().len(),
                of_entity: true,
                parts: interior.parts(),
                model_parts: interior.model_parts(),
                modelled_parts: interior.modelled_parts(),
                lights: interior.lights(),
            }),
            package: None,
            mismatches: Vec::new(),
        }
    }

    pub(crate) fn of_interior(
        (file, name): (PathBuf, String),
        interior: &Interior,
        written: GlbSummary,
    ) -> Self {
        Self {
            file,
            name,
            lod: interior.lod(),
            parts: interior.parts().len(),
            modelled: interior.modelled_parts(),
            written,
            issues: interior.issues().iter().map(PartIssue::of).collect(),
            missing_layers: interior.unresolved_layers().count(),
            excluded: 0,
            replaced: 0,
            interior: Some(InteriorFacts {
                containers: interior.containers(),
                of_entity: false,
                parts: interior.parts().len(),
                model_parts: interior.model_parts(),
                modelled_parts: interior.modelled_parts(),
                lights: interior.lights(),
            }),
            package: None,
            mismatches: Vec::new(),
        }
    }

    pub(crate) fn of_interior_package(
        (folder, name): (PathBuf, String),
        interior: &Interior,
        package: PackageSummary,
    ) -> Self {
        let issues = package.issues.iter().map(PartIssue::of_package).collect();
        Self {
            issues,
            package: Some(package),
            ..Self::of_interior((folder, name), interior, GlbSummary::default())
        }
    }

    pub(crate) fn with_mismatches(mut self, mismatches: Vec<PortMismatch>) -> Self {
        self.mismatches = mismatches;
        self
    }

    /// Returns the replacements that were written although their Entity does not fit the port.
    #[must_use]
    pub fn mismatches(&self) -> &[PortMismatch] {
        &self.mismatches
    }

    /// Returns what the Blender package holds, or `None` when the export wrote a GLB file.
    #[must_use]
    pub const fn package(&self) -> Option<&PackageSummary> {
        self.package.as_ref()
    }

    /// Returns what the Interior holds, or `None` when the export has none.
    #[must_use]
    pub const fn interior(&self) -> Option<&InteriorFacts> {
        self.interior.as_ref()
    }

    /// Returns the GLB file that was written.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the class name of the Entity.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the level of detail that was exported.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Returns how many parts the Assembly holds.
    #[must_use]
    pub const fn parts(&self) -> usize {
        self.parts
    }

    /// Returns how many parts have a model.
    #[must_use]
    pub const fn modelled_parts(&self) -> usize {
        self.modelled
    }

    /// Returns what the writer counted.
    #[must_use]
    pub const fn written(&self) -> &GlbSummary {
        &self.written
    }

    /// Returns the parts that could not be resolved or done in full, in the order the Assembly lists them.
    #[must_use]
    pub fn issues(&self) -> &[PartIssue] {
        &self.issues
    }

    /// Returns how many parts the report lists as left out of the file.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.issues.iter().filter(|issue| issue.skipped).count()
    }

    /// Returns how many layer files of the material libraries are missing or unreadable; their
    /// materials keep the colours of their own file.
    #[must_use]
    pub const fn missing_layers(&self) -> usize {
        self.missing_layers
    }

    /// Returns how many ports the changes left out.
    #[must_use]
    pub const fn excluded(&self) -> usize {
        self.excluded
    }

    /// Returns how many ports hold another Entity because of the changes.
    #[must_use]
    pub const fn replaced(&self) -> usize {
        self.replaced
    }
}
