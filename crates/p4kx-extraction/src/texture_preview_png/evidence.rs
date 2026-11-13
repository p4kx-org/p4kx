// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::DdsHeaderField;

use crate::textures::preview_header::PreviewHeaderEvidence;
use crate::{FieldOrigin, SourceEvidenceError, SourceUse, SourceUseRole, SourceUses};

/// Registered source evidence for the selected color and optional merged alpha texture.
#[derive(Debug)]
#[non_exhaustive]
pub struct PreviewPngEvidence {
    /// The primary texture, including previews that open an alpha DDS directly.
    pub color: PreviewTextureEvidence,
    /// The successfully merged alpha or smoothness tail.
    pub alpha: Option<PreviewTextureEvidence>,
}

impl PreviewPngEvidence {
    /// Registers the selected color and alpha inputs that supplied the preview pixels.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn append_pixel_uses(&self, uses: &mut Vec<SourceUse>) -> Result<(), SourceEvidenceError> {
        use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};
        let fields = [
            Prefix, Format, Width, Height, MipCount, Faces, ArraySize, Depth,
        ];
        for texture in std::iter::once(&self.color).chain(self.alpha.as_ref()) {
            if let Some(selection) = &texture.selection {
                uses.extend(selection.header_uses(&fields, SourceUseRole::Selector)?);
            }
            uses.extend(texture.header_uses(&fields, SourceUseRole::Conversion)?);
            uses.push(texture.header_size_use()?);
            uses.push(texture.mip().clone());
        }
        Ok(())
    }

    /// Returns whether both decoded inputs have their original source bindings.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.color.complete && self.alpha.as_ref().is_none_or(|alpha| alpha.complete)
    }

    /// Registers the stored names used to select the color header and its alpha sibling.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn alpha_lookup_uses(&self) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        std::iter::once(&self.color)
            .chain(self.alpha.as_ref())
            .map(|texture| texture.name_use(SourceUseRole::Selector))
            .collect()
    }
}

/// The observed header and selected mip bytes of one texture in the decoded image.
#[derive(Debug)]
pub struct PreviewTextureEvidence {
    header: PreviewHeaderEvidence,
    mip: SourceUse,
    selection: Option<PreviewHeaderEvidence>,
    complete: bool,
}

impl PreviewTextureEvidence {
    pub(super) const fn new(
        header: PreviewHeaderEvidence,
        mip: SourceUse,
        selection: Option<PreviewHeaderEvidence>,
        complete: bool,
    ) -> Self {
        Self {
            header,
            mip,
            selection,
            complete,
        }
    }

    /// Registers the exact original words used by the requested header fields.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn header_uses(
        &self,
        fields: &[DdsHeaderField],
        role: SourceUseRole,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        self.header.header_uses(fields, role)
    }

    /// Returns the original header evidence.
    #[must_use]
    pub const fn header(&self) -> &PreviewHeaderEvidence {
        &self.header
    }

    /// Returns the selected mip payload that supplied pixels to the preview.
    #[must_use]
    pub const fn mip(&self) -> &SourceUse {
        &self.mip
    }

    /// Registers the original stored filename used for this texture's lookup or display.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn name_use(&self, role: SourceUseRole) -> Result<SourceUse, SourceEvidenceError> {
        self.selection
            .as_ref()
            .unwrap_or(&self.header)
            .name_use(role)
    }

    /// Registers the header Entry length used to locate the selected mip storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn header_size_use(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.selection.as_ref().unwrap_or(&self.header).size_use()
    }

    /// Registers the displayed stored names of the header and its containers.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn path_origin(&self, uses: &SourceUses) -> Result<FieldOrigin, SourceEvidenceError> {
        self.header.path_origin(uses)
    }
}
