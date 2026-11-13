// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, PoisonError};

use p4kx_gltf::{LoadedTexture, TextureLoadError, TextureLoader, TextureReadId, TextureReadUse};
use p4kx_texture::{DdsHeaderField, MipByteSpans};

use super::ArchiveTextures;
use crate::texture_preview_png::input::PreviewInputs;
use crate::{SourceEvidenceError, SourceUse, SourceUseRole, SourceUses};

struct Read<'a> {
    level: u32,
    alpha_limits_level: bool,
    inputs: PreviewInputs<'a, MipByteSpans>,
}

/// A texture loader that retains the original spans of its successfully decoded mips.
pub struct ArchiveTextureReads<'i, 'a> {
    textures: ArchiveTextures<'i, 'a>,
    reads: Mutex<Vec<Read<'a>>>,
}

impl<'i, 'a> ArchiveTextures<'i, 'a> {
    /// Retains physical inputs so an export can bind the textures it actually embeds.
    #[must_use]
    pub const fn observed(self) -> ArchiveTextureReads<'i, 'a> {
        ArchiveTextureReads {
            textures: self,
            reads: Mutex::new(Vec::new()),
        }
    }
}

/// Exact header contributors for a GLB texture's transfer values.
#[derive(Debug, Default)]
pub struct TextureTransferUses {
    /// Source words that establish the effective original declaration.
    pub declared: Vec<SourceUse>,
    /// Source words that establish the decoded colour transfer.
    pub decoded: Vec<SourceUse>,
}

impl ArchiveTextureReads<'_, '_> {
    /// Registers the header words for the effective DDS declaration and decoded transfer.
    ///
    /// # Errors
    /// Returns an error if registration was interrupted.
    pub fn transfer_uses(
        &self,
        use_of_texture: TextureReadUse,
        uses: &SourceUses,
    ) -> Result<Option<TextureTransferUses>, SourceEvidenceError> {
        use DdsHeaderField::{CryTransfer, DxgiTransfer, Prefix};
        let Some(id) = use_of_texture.read else {
            return Ok(None);
        };
        let reads = self.reads.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(read) = reads.get(id.index()) else {
            return Ok(None);
        };
        let (header, _) = read.inputs.color_header_evidence(uses)?;
        let mut declaration = header.header_uses(&[Prefix], SourceUseRole::Selector)?;
        declaration
            .extend(header.header_uses(&[DxgiTransfer, CryTransfer], SourceUseRole::Conversion)?);
        let decoded = match use_of_texture.transfer {
            Some(encoding) => header.output_transfer_uses(encoding)?,
            None => Vec::new(),
        };
        drop(reads);
        Ok(Some(TextureTransferUses {
            declared: declaration,
            decoded,
        }))
    }

    /// Registers the original components the GLB writer used for one image input.
    ///
    /// # Errors
    /// Returns an error if registration was interrupted.
    pub fn source_uses(
        &self,
        use_of_texture: TextureReadUse,
        uses: &SourceUses,
    ) -> Result<Option<Vec<SourceUse>>, SourceEvidenceError> {
        let Some(id) = use_of_texture.read else {
            return Ok(None);
        };
        let reads = self.reads.lock().unwrap_or_else(PoisonError::into_inner);
        reads
            .get(id.index())
            .map(|read| {
                read.inputs.glb_uses(
                    uses,
                    read.level,
                    use_of_texture.components,
                    use_of_texture.transfer,
                    read.alpha_limits_level,
                )
            })
            .transpose()
    }
}

impl TextureLoader for ArchiveTextureReads<'_, '_> {
    fn load(&self, path: &str, mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        let opened = self.textures.load_observed(path, mip)?;
        let decoded = opened.decoded;
        let mut reads = self.reads.lock().unwrap_or_else(PoisonError::into_inner);
        let id = TextureReadId::new(reads.len());
        reads.push(Read {
            level: opened.level,
            alpha_limits_level: opened.alpha_limits_level,
            inputs: decoded.inputs,
        });
        drop(reads);
        let (declared, encoding) = opened.transfer;
        Ok(LoadedTexture::new(decoded.image, opened.has_alpha)
            .with_dds_transfer(declared, encoding)
            .with_read(id))
    }

    fn dimensions(&self, path: &str, mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        self.textures.dimensions(path, mip)
    }
}
