// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_blender::{MANIFEST_FILE, PackageError, check_output};
use p4kx_extraction::WriteProtectionError;

use crate::installation::refusal_of;

/// What an Export of an Entity writes: one GLB file, or a Blender package folder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssemblyExportForm {
    /// One GLB file.
    #[default]
    Glb,
    /// A Blender package: a folder with `scene.json`, one GLB file per mesh and the textures.
    Package,
}

/// Why a folder cannot receive a Blender package.
#[derive(Debug, Clone)]
pub enum PackageProblem {
    /// The output violates a protection or identity boundary.
    Protected(WriteProtectionError),
    /// The folder exists and holds something other than an earlier package or nothing, so it is
    /// not replaced.
    NotAPackage,
    /// The path names no folder.
    NoFolderName,
}

/// What an export writes and where: the form, and the GLB file or the package folder chosen for
/// it with what the dialog tells about it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Output {
    form: AssemblyExportForm,
    file: Option<PathBuf>,
    refusal: Option<WriteProtectionError>,
    target: Option<PackageTarget>,
}

impl Output {
    pub(crate) fn new(package: bool) -> Self {
        Self {
            form: if package {
                AssemblyExportForm::Package
            } else {
                AssemblyExportForm::Glb
            },
            ..Self::default()
        }
    }

    pub(crate) const fn form(&self) -> AssemblyExportForm {
        self.form
    }

    pub(crate) const fn set_form(&mut self, form: AssemblyExportForm) {
        self.form = form;
    }

    pub(crate) fn package_folder(&self) -> Option<&Path> {
        self.target.as_ref().map(PackageTarget::folder)
    }

    pub(crate) fn set_package_folder(&mut self, folder: PathBuf) {
        self.target = Some(PackageTarget::new(folder));
    }

    pub(crate) fn package_problem(&self) -> Option<&PackageProblem> {
        self.target.as_ref().and_then(PackageTarget::problem)
    }

    pub(crate) fn package_replaces(&self) -> bool {
        self.target.as_ref().is_some_and(PackageTarget::replaces)
    }

    pub(crate) fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    pub(crate) fn set_file(&mut self, file: PathBuf) {
        let file = if file.extension().is_none() {
            file.with_extension("glb")
        } else {
            file
        };
        self.refusal = refusal_of(file.parent());
        self.file = Some(file);
    }

    pub(crate) fn refusal(&self) -> Option<&WriteProtectionError> {
        if self.form == AssemblyExportForm::Package {
            match self.package_problem() {
                Some(PackageProblem::Protected(refused)) => Some(refused),
                _ => None,
            }
        } else {
            self.refusal.as_ref()
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        match self.form {
            AssemblyExportForm::Glb => self.file.is_some() && self.refusal.is_none(),
            AssemblyExportForm::Package => {
                self.target.is_some() && self.package_problem().is_none()
            }
        }
    }

    /// Returns the path the export writes, the GLB file or the package folder of the form, when
    /// the choice is ready.
    pub(crate) fn path(&self) -> Option<PathBuf> {
        if !self.is_ready() {
            return None;
        }
        match self.form {
            AssemblyExportForm::Glb => self.file.clone(),
            AssemblyExportForm::Package => self.package_folder().map(Path::to_path_buf),
        }
    }

    /// Returns the folder the chosen file or package folder lies in, else `fallback`.
    pub(crate) fn folder_or(&self, fallback: Option<&Path>) -> Option<PathBuf> {
        match self.form {
            AssemblyExportForm::Glb => self.file(),
            AssemblyExportForm::Package => self.package_folder(),
        }
        .and_then(Path::parent)
        .or(fallback)
        .map(Path::to_path_buf)
    }
}

/// The folder a Blender package is written to and what the writer makes of it.
#[derive(Debug, Clone)]
pub(crate) struct PackageTarget {
    folder: PathBuf,
    problem: Option<PackageProblem>,
    replaces: bool,
}

impl PackageTarget {
    #[expect(
        clippy::match_same_arms,
        reason = "an unreadable folder is reported by the export, not by the dialog"
    )]
    pub(super) fn new(folder: PathBuf) -> Self {
        let problem = match check_output(&folder) {
            Err(PackageError::Refused(refused)) => Some(PackageProblem::Protected(refused)),
            Err(PackageError::NotAPackage { .. }) => Some(PackageProblem::NotAPackage),
            Err(PackageError::NoFolderName { .. }) => Some(PackageProblem::NoFolderName),
            Err(PackageError::Write { .. }) => None,
            Ok(()) | Err(_) => None,
        };
        let replaces = problem.is_none() && folder.join(MANIFEST_FILE).is_file();
        Self {
            folder,
            problem,
            replaces,
        }
    }

    pub(super) fn folder(&self) -> &Path {
        &self.folder
    }

    pub(super) const fn problem(&self) -> Option<&PackageProblem> {
        self.problem.as_ref()
    }

    pub(super) const fn replaces(&self) -> bool {
        self.replaces
    }
}
