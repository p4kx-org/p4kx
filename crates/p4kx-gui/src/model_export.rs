// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod job;
mod summary;

use std::path::{Path, PathBuf};

use p4kx_extraction::{AnimationChoice, WriteProtectionError};
use p4kx_settings::{ModelExportSettings, ModelMaterials};

pub use job::{ModelExport, ModelExportProgress, ModelExportStage, ModelExportStatus};
pub(crate) use job::{Request, start};
pub use summary::{
    AnimationNote, AnimationNoteKind, MaterialProblem, MissingTexture, ModelExportFailure,
    ModelExportSummary,
};

use crate::installation::refusal_of;
use crate::{AnimationCount, LodInfo, ModelInfo};

/// The material modes the Export as GLB dialog offers.
pub const MODEL_MATERIALS: [ModelMaterials; 2] =
    [ModelMaterials::Colours, ModelMaterials::Textures];

/// The texture mip levels the Export as GLB dialog offers: the full size and three halvings.
pub const MODEL_TEXTURE_MIPS: [u32; 4] = [0, 1, 2, 3];

/// The choices of an Export as GLB before it starts: the level of detail, the materials, the
/// animations and the file.
#[derive(Debug, Clone)]
pub struct ModelExportSetup {
    entry: u32,
    entry_path: String,
    name: String,
    lods: Vec<LodInfo>,
    lod: u32,
    materials: ModelMaterials,
    texture_mip: u32,
    animations: bool,
    animation_count: AnimationCount,
    vertex_colours: bool,
    folder: Option<PathBuf>,
    file: Option<PathBuf>,
    refusal: Option<WriteProtectionError>,
}

impl ModelExportSetup {
    pub(crate) fn new(
        (entry, entry_path): (u32, &str),
        info: &ModelInfo,
        settings: &ModelExportSettings,
    ) -> Self {
        let lods = info.lods.clone();
        let lod = if lods.iter().any(|lod| lod.level == settings.lod) {
            settings.lod
        } else {
            lods.first().map_or(0, |lod| lod.level)
        };
        Self {
            entry,
            entry_path: entry_path.to_owned(),
            name: info.name.clone(),
            lods,
            lod,
            materials: settings.materials,
            texture_mip: nearest_mip(settings.texture_mip),
            animations: settings.animations,
            animation_count: info.animations,
            vertex_colours: settings.vertex_colours,
            folder: settings.folder.clone(),
            file: None,
            refusal: None,
        }
    }

    /// Returns the id of the model Entry.
    #[must_use]
    pub const fn entry(&self) -> u32 {
        self.entry
    }

    /// Returns the file name of the model.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the levels of detail the model has, the most detailed first.
    #[must_use]
    pub fn lods(&self) -> &[LodInfo] {
        &self.lods
    }

    /// Returns the level of detail that is exported.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Chooses the level of detail; a level the model lacks is ignored.
    pub fn set_lod(&mut self, lod: u32) {
        if self.lods.iter().any(|known| known.level == lod) {
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
        if MODEL_TEXTURE_MIPS.contains(&mip) {
            self.texture_mip = mip;
        }
    }

    /// Returns how many animations the model has that an export writes by default; 0 when the
    /// count is not known.
    #[must_use]
    pub const fn animation_count(&self) -> usize {
        match self.animation_count {
            AnimationCount::Counted(count) => count,
            _ => 0,
        }
    }

    /// Returns whether the setup offers to write the animations: the model has some, or it has
    /// more files next to it than the preview counts.
    #[must_use]
    pub const fn animations_offered(&self) -> bool {
        matches!(
            self.animation_count,
            AnimationCount::Counted(1..) | AnimationCount::TooMany
        )
    }

    /// Returns whether the animations found for the model are written.
    #[must_use]
    pub const fn animations(&self) -> bool {
        self.animations
    }

    /// Chooses whether the animations found for the model are written.
    pub const fn set_animations(&mut self, written: bool) {
        self.animations = written;
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

    /// Returns the name the file dialog proposes: the model's name with `.glb`.
    #[must_use]
    pub fn proposed_name(&self) -> String {
        let stem = self
            .name
            .rsplit_once('.')
            .map_or(&*self.name, |(stem, _)| stem);
        format!("{stem}.glb")
    }

    /// Returns the chosen file, or `None` while none is chosen.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// Chooses the file to write; a name without extension gets `.glb`.
    pub fn set_file(&mut self, file: PathBuf) {
        let file = if file.extension().is_none() {
            file.with_extension("glb")
        } else {
            file
        };
        self.refusal = refusal_of(file.parent());
        self.file = Some(file);
    }

    /// Returns why the chosen file cannot be written, when it lies inside a Star Citizen
    /// installation.
    #[must_use]
    pub const fn refusal(&self) -> Option<&WriteProtectionError> {
        self.refusal.as_ref()
    }

    /// Returns whether the setup can start: a file is chosen outside every installation.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.file.is_some() && self.refusal.is_none()
    }

    pub(crate) fn request(&self) -> Option<Request> {
        if !self.is_ready() {
            return None;
        }
        Some(Request {
            entry_path: self.entry_path.clone(),
            file: self.file.clone()?,
            lod: self.lod,
            materials: self.materials,
            texture_mip: self.texture_mip,
            animations: (self.animations && self.animations_offered())
                .then_some(AnimationChoice::All),
            vertex_colours: self.vertex_colours,
            #[cfg(feature = "test-hooks")]
            pause: crate::hold::Pause::default(),
        })
    }

    pub(crate) fn settings(&self) -> ModelExportSettings {
        ModelExportSettings {
            lod: self.lod,
            materials: self.materials,
            texture_mip: self.texture_mip,
            animations: self.animations,
            vertex_colours: self.vertex_colours,
            folder: self
                .file
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
                .or_else(|| self.folder.clone()),
        }
    }
}

fn nearest_mip(mip: u32) -> u32 {
    MODEL_TEXTURE_MIPS.last().map_or(0, |&last| mip.min(last))
}
