// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_extraction::WriteProtectionError;

use crate::assembly_export::Output;
use crate::{AssemblyExportForm, PackageProblem};

/// The choices of an Export of a Character before it starts: the form, a GLB file or a Blender
/// package folder, and where it is written.
///
/// The level of detail and the materials are those of the view.
#[derive(Debug, Clone)]
pub struct CharacterExportSetup {
    name: String,
    folder: Option<PathBuf>,
    animations: bool,
    output: Output,
}

impl CharacterExportSetup {
    pub(super) fn new(
        name: String,
        folder: Option<PathBuf>,
        (package, animations): (bool, bool),
    ) -> Self {
        Self {
            name,
            folder,
            animations,
            output: Output::new(package),
        }
    }

    /// Returns whether a package carries the animations of the Character's animation folders.
    #[must_use]
    pub const fn animations(&self) -> bool {
        self.animations
    }

    /// Chooses whether a package carries the animations of the Character's animation folders.
    pub const fn set_animations(&mut self, animations: bool) {
        self.animations = animations;
    }

    /// Returns the name of the Character.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns what the export writes.
    #[must_use]
    pub const fn form(&self) -> AssemblyExportForm {
        self.output.form()
    }

    /// Chooses what the export writes.
    pub const fn set_form(&mut self, form: AssemblyExportForm) {
        self.output.set_form(form);
    }

    /// Returns the folder the file dialog starts in, or `None` before the first export.
    #[must_use]
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Returns the name the file dialog proposes: the name of the Character with `.glb`.
    #[must_use]
    pub fn proposed_name(&self) -> String {
        format!("{}.glb", self.name)
    }

    /// Returns the chosen file, or `None` while none is chosen.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        self.output.file()
    }

    /// Chooses the file to write; a name without extension gets `.glb`.
    pub fn set_file(&mut self, file: PathBuf) {
        self.output.set_file(file);
    }

    /// Returns the folder the package is written to, or `None` while none is chosen.
    #[must_use]
    pub fn package_folder(&self) -> Option<&Path> {
        self.output.package_folder()
    }

    /// Chooses the folder of the package: it must be new, empty or hold an earlier package.
    pub fn set_package_folder(&mut self, folder: PathBuf) {
        self.output.set_package_folder(folder);
    }

    /// Returns why the chosen package folder cannot be written, or `None` when it can or none is
    /// chosen.
    #[must_use]
    pub fn package_problem(&self) -> Option<&PackageProblem> {
        self.output.package_problem()
    }

    /// Returns whether the chosen package folder holds an earlier package, which the export
    /// replaces once the new one is complete.
    #[must_use]
    pub fn package_replaces(&self) -> bool {
        self.output.package_replaces()
    }

    /// Returns the protection refusal for the chosen output.
    #[must_use]
    pub fn refusal(&self) -> Option<&WriteProtectionError> {
        self.output.refusal()
    }

    /// Returns whether the setup can start: a file or package folder is chosen that can be
    /// written.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.output.is_ready()
    }

    pub(super) fn path(&self) -> Option<PathBuf> {
        self.output.path()
    }

    pub(super) const fn animations_chosen(&self) -> bool {
        self.animations
    }

    pub(super) fn folder_chosen(&self) -> Option<PathBuf> {
        self.output.folder_or(self.folder.as_deref())
    }
}
