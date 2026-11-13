// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_source::MaterialSourceLimits;
use crate::mtl_source_error::MaterialReadError;
use crate::{
    CompanionResolver, LayerCache, Material, MaterialError, MaterialLibrary,
    MaterialReadObservation, MaterialReading, mtl_source_read,
};
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
    /// [`MaterialError::TooMany`] when it holds more sub-materials than the limit allows.
    pub fn parse(path: &str, bytes: &[u8]) -> Result<Self, MaterialError> {
        mtl_source_read::parse(path, bytes)
    }

    /// Parses a material and optionally resolves its drawn surfaces with the supplied resolver.
    ///
    /// # Errors
    ///
    /// Returns the material parser's original typed error.
    pub fn parse_with_resolver(
        path: &str,
        bytes: &[u8],
        target: Option<String>,
        resolver: &mut dyn CompanionResolver,
        reading: MaterialReading,
        cache: &LayerCache,
    ) -> Result<Self, MaterialError> {
        let mut library = Self::parse(path, bytes)?;
        library.target = target;
        if reading == MaterialReading::Drawn {
            crate::mtl_layers::resolve(&mut library, resolver, cache);
            crate::mtl_relief::check_diffuse_maps(&mut library, resolver);
        }
        Ok(library)
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

    /// Returns whether the source slot contains an explicit non-material entry.
    #[must_use]
    pub fn is_empty_slot(&self, id: u16) -> bool {
        !self.single
            && self
                .authored
                .entries
                .get(usize::from(id))
                .is_some_and(|entry| {
                    entry.disposition == crate::MaterialEntryDisposition::EngineDefault
                })
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
