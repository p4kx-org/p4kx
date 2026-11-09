// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_source::MaterialSourceLimits;
use crate::mtl_source_error::MaterialReadError;
use crate::{Material, MaterialError, MaterialLibrary, MaterialReadObservation, mtl_source_read};
impl MaterialLibrary {
    /// Reads the `.mtl` file `bytes`, text XML or `CryXml`, of the library `path`.
    ///
    /// A file with a `SubMaterials` element is a list: the material ID is the position of the
    /// sub-material, and an entry that is no `Material` element counts as a position and reads
    /// as the engine's default material. A file without is one material that serves every ID. Numbers that are
    /// missing, malformed or not finite read as white diffuse, a dim dielectric specular, no
    /// emission, full opacity and a shininess of 10.
    ///
    /// # Errors
    ///
    /// Returns a typed encoding/XML/format or input-limit error when the source cannot be parsed,
    /// [`MaterialError::Xml`] when the text is malformed,
    /// [`MaterialError::NotMaterial`] when its root is no `Material`, and
    /// [`MaterialError::TooMany`] when it holds more sub-materials or texture slots than the
    /// limits allow.
    pub fn parse(path: &str, bytes: &[u8]) -> Result<Self, MaterialError> {
        mtl_source_read::parse(path, bytes)
    }

    /// Reads bounded authored syntax while preserving the checkpoint's original error.
    ///
    /// # Errors
    /// Returns a typed material/input limit error or the original caller interruption.
    pub fn parse_with<Interrupted>(
        path: &str,
        bytes: &[u8],
        limits: MaterialSourceLimits,
        checkpoint: impl FnMut() -> Result<(), Interrupted>,
    ) -> Result<Self, MaterialReadError<Interrupted>> {
        Self::parse_with_observer(path, bytes, limits, checkpoint, |_| Ok(()))
    }

    /// Reads bounded material syntax and reports original sites used by interpreted fields.
    ///
    /// Observations borrow only during the callback. Callers retain contributions only after a
    /// successful parse; observer errors use the same interruption path as the checkpoint.
    ///
    /// # Errors
    /// Returns the original material/input-limit error or caller interruption.
    pub fn parse_with_observer<Interrupted>(
        path: &str,
        bytes: &[u8],
        limits: MaterialSourceLimits,
        checkpoint: impl FnMut() -> Result<(), Interrupted>,
        observer: impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    ) -> Result<Self, MaterialReadError<Interrupted>> {
        mtl_source_read::read_observed(path, bytes, limits, checkpoint, observer)
    }

    /// Returns the material of `id`, or `None` when the library has no sub-material at that
    /// position and the engine falls back to its default material.
    #[must_use]
    pub fn material(&self, id: u16) -> Option<&Material> {
        if self.single {
            self.materials.first()
        } else {
            self.materials.get(usize::from(id))
        }
    }
}
