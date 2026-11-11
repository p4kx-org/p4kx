// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::build::MAX_LOD;
use crate::{MaterialRequest, ModelFile};

/// Returns the names of the files next to the model file `name` that reading it can fetch: the
/// geometry companion `<name>m`, and for every level of detail from 1 on its model file and its
/// geometry companion.
#[must_use]
pub fn companion_file_names(name: &str) -> Vec<String> {
    let mut names = vec![format!("{name}m")];
    for level in 1..=MAX_LOD {
        let file = lod_file_name(name, level);
        names.push(format!("{file}m"));
        names.push(file);
    }
    names
}

/// Returns the file name of the model file of level of detail `level` of the model `name`:
/// `<stem>_lod<level>.<extension>`.
pub(crate) fn lod_file_name(name: &str, level: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) => format!("{stem}_lod{level}.{extension}"),
        None => format!("{name}_lod{level}"),
    }
}

/// The failure a [`CompanionResolver`] reports when a companion exists but cannot be read.
pub type CompanionError = Box<dyn Error + Send + Sync>;

/// A file a model asks its resolver for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompanionPath<'a> {
    /// A file in the model's own folder, by file name, e.g. the geometry companion `box.cgfm` of
    /// `box.cgf`.
    Sibling(&'a str),
    /// A validated material request below the logical `Data` root, with `/` as
    /// separator and without the `.mtl` extension, e.g. `Objects/Ships/Gladius/gladius`. The
    /// model has checked that it is a plain path: no empty, `.` or `..` component, no drive.
    Material(&'a str),
}

/// The mip level of a texture whose colour [`CompanionResolver::texture_is_grey`] judges; a texture
/// with fewer levels gives its last.
pub const GREY_PROBE_MIP: u32 = 3;

/// The mean saturation up to which a texture counts as grey.
pub const MAX_GREY_SATURATION: f32 = 0.05;

/// Supplies the companion files of a model, so that reading a model needs no Archive.
pub trait CompanionResolver {
    /// Validates a material name under this resolver's source-path policy.
    ///
    /// # Errors
    /// Returns an error when the name is not admitted by the source-path policy.
    fn material_request(&self, name: &str) -> Result<MaterialRequest, CompanionError> {
        MaterialRequest::new(name)
    }

    /// Returns the bytes of the companion at `path`, or `None` when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns the failure to read a companion that exists.
    fn resolve(&mut self, path: CompanionPath<'_>) -> Result<Option<Vec<u8>>, CompanionError>;

    /// Returns the bytes of a sibling requested for a specific model file role.
    ///
    /// # Errors
    /// Returns the failure to read a sibling that exists.
    fn resolve_model_file(
        &mut self,
        _file: ModelFile,
        name: &str,
    ) -> Result<Option<Vec<u8>>, CompanionError> {
        self.resolve(CompanionPath::Sibling(name))
    }

    /// Returns whether the texture `file`, as a material file names it, holds no colour of its own,
    /// or `None` when the resolver cannot tell, as the default does.
    ///
    /// A texture is grey when the mean saturation of mip [`GREY_PROBE_MIP`] (the last mip when it
    /// has fewer), each pixel weighted by its alpha, is at most [`MAX_GREY_SATURATION`]; the
    /// saturation of a pixel is its largest channel minus its smallest, over its largest.
    fn texture_is_grey(&mut self, _file: &str) -> Option<bool> {
        None
    }
}

impl<F> CompanionResolver for F
where
    F: FnMut(CompanionPath<'_>) -> Result<Option<Vec<u8>>, CompanionError>,
{
    fn resolve(&mut self, path: CompanionPath<'_>) -> Result<Option<Vec<u8>>, CompanionError> {
        self(path)
    }
}
