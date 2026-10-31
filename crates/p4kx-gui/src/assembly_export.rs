// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod job;
mod summary;
mod target;

use std::path::{Path, PathBuf};

pub(crate) use job::stop::{Observed, Stoppable, StoppableTextures};
pub use job::{AssemblyExport, AssemblyExportProgress, AssemblyExportStage, AssemblyExportStatus};
pub(crate) use job::{Request, Subject, failure_of, info_of, package_failure, start};
use p4kx_assembly::{ChangeError, LoadoutChanges, LoadoutPicks};
use p4kx_extraction::{Guid, WriteRefused};
use p4kx_settings::{AssemblyExportSettings, ModelMaterials};
pub use summary::{AssemblyExportSummary, InteriorFacts, PartIssue};
pub(crate) use target::Output;
pub use target::{AssemblyExportForm, PackageProblem};

/// The levels of detail the Export of an Entity as GLB offers.
pub const ASSEMBLY_LODS: [u32; 4] = [0, 1, 2, 3];

/// The material modes the Export of an Entity as GLB offers.
pub const ASSEMBLY_MATERIALS: [ModelMaterials; 2] =
    [ModelMaterials::Colours, ModelMaterials::Textures];

/// The choices of an Export as GLB before it starts.
///
/// The subject is an Entity with the changes made to its Loadout and whether its Interior comes
/// with it, or an Interior container on its own; the other choices are the level of detail, the
/// materials and the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyExportSetup {
    entity: Option<Guid>,
    container: Vec<String>,
    name: String,
    changes: LoadoutChanges,
    picks_text: String,
    picks_error: Option<ChangeError>,
    interior: bool,
    has_interior: bool,
    lod: u32,
    materials: ModelMaterials,
    texture_mip: u32,
    vertex_colours: bool,
    folder: Option<PathBuf>,
    output: Output,
}

impl AssemblyExportSetup {
    pub(crate) fn new(
        (guid, name): (Guid, &str),
        (changes, has_interior): (&LoadoutChanges, bool),
        settings: &AssemblyExportSettings,
    ) -> Self {
        Self {
            entity: Some(guid),
            container: Vec::new(),
            name: name.to_owned(),
            changes: changes.clone(),
            picks_text: changes.picks().to_string(),
            interior: settings.interior,
            has_interior,
            ..Self::with_settings(settings)
        }
    }

    pub(crate) fn of_container(
        (chain, name): (Vec<String>, &str),
        settings: &AssemblyExportSettings,
    ) -> Self {
        Self {
            entity: None,
            container: chain,
            name: name.to_owned(),
            changes: LoadoutChanges::default(),
            interior: settings.interior,
            has_interior: false,
            ..Self::with_settings(settings)
        }
    }

    fn with_settings(settings: &AssemblyExportSettings) -> Self {
        Self {
            entity: None,
            container: Vec::new(),
            name: String::new(),
            changes: LoadoutChanges::default(),
            picks_text: String::new(),
            picks_error: None,
            interior: false,
            has_interior: false,
            lod: nearest(settings.lod, &ASSEMBLY_LODS),
            materials: settings.materials,
            texture_mip: nearest(settings.texture_mip, &crate::MODEL_TEXTURE_MIPS),
            vertex_colours: settings.vertex_colours,
            folder: settings.folder.clone(),
            output: Output::new(settings.package),
        }
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

    /// Returns the class name of the Entity, or the name of the container.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether the setup exports an Interior container on its own rather than an Entity.
    #[must_use]
    pub const fn is_container(&self) -> bool {
        self.entity.is_none()
    }

    /// Returns the changes made to the Loadout, which the export applies; none for a container.
    #[must_use]
    pub const fn changes(&self) -> &LoadoutChanges {
        &self.changes
    }

    /// Returns the text of the Loadout picks, `<group>=<index>` parts separated by commas.
    #[must_use]
    pub fn picks_text(&self) -> &str {
        &self.picks_text
    }

    /// Sets the text of the Loadout picks, which choose the alternatives of the groups of a
    /// generated Loadout (see [`LoadoutPicks`]). A text that is no picks is kept for editing,
    /// leaves the picks as they were and stops the setup from starting.
    pub fn set_picks_text(&mut self, text: impl Into<String>) {
        self.picks_text = text.into();
        self.picks_error = match LoadoutPicks::from_list(&self.picks_text) {
            Ok(picks) => {
                self.changes.set_picks(picks);
                None
            }
            Err(error) => Some(error),
        };
    }

    /// Returns whether the text of the Loadout picks is no picks.
    #[must_use]
    pub const fn picks_invalid(&self) -> bool {
        self.picks_error.is_some()
    }

    /// Returns whether the Entity names object containers, so that the export can carry its
    /// Interior.
    #[must_use]
    pub const fn has_interior(&self) -> bool {
        self.has_interior
    }

    /// Returns whether the Interior of the Entity is exported with the hull.
    #[must_use]
    pub const fn interior(&self) -> bool {
        self.interior && self.has_interior
    }

    /// Chooses whether the Interior of the Entity is exported with the hull; ignored for an
    /// Entity without one and for a container.
    pub const fn set_interior(&mut self, exported: bool) {
        if self.has_interior {
            self.interior = exported;
        }
    }

    /// Returns the level of detail that is exported.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Chooses the level of detail; a level the dialog does not offer is ignored.
    pub fn set_lod(&mut self, lod: u32) {
        if ASSEMBLY_LODS.contains(&lod) {
            self.lod = lod;
        }
    }

    /// Returns how the materials are written.
    #[must_use]
    pub const fn materials(&self) -> ModelMaterials {
        self.materials
    }

    /// Chooses how the materials are written.
    pub const fn set_materials(&mut self, materials: ModelMaterials) {
        self.materials = materials;
    }

    /// Returns the mip level of the textures, 0 for the largest.
    #[must_use]
    pub const fn texture_mip(&self) -> u32 {
        self.texture_mip
    }

    /// Chooses the mip level of the textures; a level the dialog does not offer is ignored.
    pub fn set_texture_mip(&mut self, mip: u32) {
        if crate::MODEL_TEXTURE_MIPS.contains(&mip) {
            self.texture_mip = mip;
        }
    }

    /// Returns whether the vertex colours of the meshes are written.
    #[must_use]
    pub const fn vertex_colours(&self) -> bool {
        self.vertex_colours
    }

    /// Chooses whether the vertex colours of the meshes are written.
    pub const fn set_vertex_colours(&mut self, written: bool) {
        self.vertex_colours = written;
    }

    /// Returns the folder the file dialog starts in, or `None` before the first export.
    #[must_use]
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Returns the name the file dialog proposes: the class name with `.glb`.
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

    /// Returns why the chosen file or package folder cannot be written, when it lies inside a Star
    /// Citizen installation.
    #[must_use]
    pub fn refusal(&self) -> Option<&WriteRefused> {
        self.output.refusal()
    }

    /// Returns whether the setup can start: a file or package folder is chosen that can be
    /// written.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.output.is_ready() && self.picks_error.is_none()
    }

    pub(crate) fn request(&self) -> Option<Request> {
        if self.picks_error.is_some() {
            return None;
        }
        let file = self.output.path()?;
        let subject = self.entity.map_or_else(
            || Subject::Container {
                chain: self.container.clone(),
            },
            |guid| Subject::Entity {
                guid,
                changes: self.changes.clone(),
                interior: self.interior(),
            },
        );
        Some(Request {
            subject,
            name: self.name.clone(),
            form: self.form(),
            file,
            lod: self.lod,
            materials: self.materials,
            texture_mip: self.texture_mip,
            vertex_colours: self.vertex_colours,
            #[cfg(feature = "test-hooks")]
            pause: crate::hold::Pause::default(),
        })
    }

    pub(crate) fn settings(&self) -> AssemblyExportSettings {
        AssemblyExportSettings {
            lod: self.lod,
            materials: self.materials,
            texture_mip: self.texture_mip,
            vertex_colours: self.vertex_colours,
            interior: self.interior,
            package: self.form() == AssemblyExportForm::Package,
            folder: self.output.folder_or(self.folder.as_deref()),
        }
    }
}

fn nearest(value: u32, offered: &[u32]) -> u32 {
    offered
        .iter()
        .copied()
        .filter(|&level| level <= value)
        .max()
        .or_else(|| offered.first().copied())
        .unwrap_or(0)
}
