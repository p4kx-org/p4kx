// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_gltf::{LoadedTexture, TextureLoadError, TextureLoader};
use p4kx_texture::{DdsPngEncoding, DdsTransfer, DxgiFormat, TextureFormat};

use crate::path_index::PathIndex;
use crate::texture_preview::ObservedPreviewImage;
use crate::{ConversionError, DdsSourceInfo, Texture};

#[path = "model_texture_dds.rs"]
mod dds;
#[path = "model_texture_origins.rs"]
mod origin;

pub use dds::{ModelDdsRefusal, ObservedModelDds};
pub use origin::{ArchiveTextureReads, TextureTransferUses};

/// The most pixels one texture of a model may hold: 8192 × 8192.
pub const MAX_MODEL_TEXTURE_PIXELS: u64 = 1 << 26;

const DXGI_WITH_ALPHA: &[u32] = &[
    1, 2, 3, 4, 9, 10, 11, 12, 13, 14, 23, 24, 25, 27, 28, 29, 30, 31, 32, 65, 73, 74, 75, 76, 77,
    78, 86, 87, 90, 91, 97, 98, 99, 115,
];

/// Loads the textures of a model's materials from the Entries of an Archive.
pub struct ArchiveTextures<'i, 'a> {
    index: &'i PathIndex<'a>,
}

impl<'i, 'a> ArchiveTextures<'i, 'a> {
    /// Loads textures from the Entries `index` is over.
    #[must_use]
    pub const fn new(index: &'i PathIndex<'a>) -> Self {
        Self { index }
    }
}

/// One decoded model image with its actual DDS metadata and source inputs.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedModelTexture<'a> {
    /// Decoded image and its actual header and pixel spans.
    pub decoded: ObservedPreviewImage<'a>,
    /// Complete raw and normalized source header metadata.
    pub metadata: DdsSourceInfo,
    /// Actual zero-based mip after the model loader's size negotiation.
    pub level: u32,
    /// Whether the alpha header reduced the requested mip index.
    pub alpha_limits_level: bool,
    /// Whether the decoded alpha channel carries data.
    pub has_alpha: bool,
    /// Whether the selected alpha channel is available for packed shader data.
    pub has_packed_alpha: bool,
    /// Embedded opacity of a premultiplied colour surface before a separate alpha merge.
    pub embedded_opacity: Option<Vec<u8>>,
    /// Original effective transfer and the encoding of the decoded RGB samples.
    pub transfer: (DdsTransfer, DdsPngEncoding),
}

struct OpenTexture<'a> {
    texture: Texture<'a>,
    level: u32,
    size: (u32, u32),
    alpha_limits_level: bool,
}

impl<'a> ArchiveTextures<'_, 'a> {
    /// Loads a model texture and captures the header and mip used by that same decode.
    ///
    /// # Errors
    /// Returns the normal model texture lookup, geometry or decode error.
    pub fn load_observed(
        &self,
        path: &str,
        mip: u32,
    ) -> Result<ObservedModelTexture<'a>, TextureLoadError> {
        let opened = self.open(path, mip)?;
        let texture = &opened.texture;

        let (decoded, embedded_opacity) = texture
            .decode_model_mip_observed(opened.level)
            .map_err(|error| TextureLoadError::Unreadable(describe(&error)))?;
        let format = texture.header().format();
        let bc1_uses_alpha = matches!(format, TextureFormat::Dxgi(DxgiFormat(70..=72)))
            && decoded
                .image()
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != u8::MAX);
        let has_packed_alpha = texture.has_alpha_tail() || format_has_alpha(format);
        let has_alpha = has_packed_alpha || bc1_uses_alpha;
        let transfer = (
            decoded.inputs.source_transfer(),
            decoded
                .inputs
                .png_encoding()
                .map_err(|error| TextureLoadError::Unreadable(describe(&error)))?,
        );
        Ok(ObservedModelTexture {
            decoded,
            metadata: texture.source_metadata(),
            level: opened.level,
            alpha_limits_level: opened.alpha_limits_level,
            has_alpha,
            has_packed_alpha,
            embedded_opacity,
            transfer,
        })
    }

    /// Opens the texture at `path` and returns it with the mip `mip` selects and that mip's
    /// size, after checking it is one image within the pixel limit.
    fn open(&self, path: &str, mip: u32) -> Result<OpenTexture<'a>, TextureLoadError> {
        let entry = archive_path(path);
        let texture = Texture::from_index(self.index, &entry).map_err(|error| match error {
            ConversionError::UnknownEntry(_) => TextureLoadError::NotFound,
            other => TextureLoadError::Unreadable(describe(&other)),
        })?;
        let header = texture.header();
        if header.surfaces() > 1 || header.depth() > 1 {
            return Err(TextureLoadError::Unreadable(
                "the texture is a cubemap, array or volume, not one image".to_owned(),
            ));
        }
        let tail = texture
            .alpha_tail()
            .transpose()
            .map_err(|error| TextureLoadError::Unreadable(describe(&error)))?;
        let color_last = header.mip_count().saturating_sub(1);
        let alpha_last = tail
            .as_ref()
            .map(|tail| tail.header().mip_count().saturating_sub(1));
        let last = alpha_last.map_or(color_last, |alpha| color_last.min(alpha));
        let level = mip.min(last);
        let alpha_limits_level = alpha_last.is_some_and(|alpha| alpha < color_last && mip > alpha);
        let halved = |full: u32| (full >> level).max(1);
        let size = (halved(header.width()), halved(header.height()));
        if let Some(tail) = &tail {
            let tail_size = (
                halved(tail.header().width()),
                halved(tail.header().height()),
            );
            if tail_size != size {
                return Err(TextureLoadError::Unreadable(format!(
                    "the alpha tail is {}x{} at this mip, the texture {}x{}",
                    tail_size.0, tail_size.1, size.0, size.1
                )));
            }
        }
        let pixels = u64::from(size.0) * u64::from(size.1);
        if pixels > MAX_MODEL_TEXTURE_PIXELS {
            return Err(TextureLoadError::Unreadable(format!(
                "the texture has {pixels} pixels at this mip, more than the limit of \
                 {MAX_MODEL_TEXTURE_PIXELS}; use a higher --texture-mip"
            )));
        }
        Ok(OpenTexture {
            texture,
            level,
            size,
            alpha_limits_level,
        })
    }
}

impl TextureLoader for ArchiveTextures<'_, '_> {
    fn load(&self, path: &str, mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        let observed = self.load_observed(path, mip)?;
        let (declared, encoding) = observed.transfer;
        Ok(
            LoadedTexture::new(observed.decoded.into_image(), observed.has_packed_alpha)
                .with_dds_transfer(declared, encoding),
        )
    }

    fn dimensions(&self, path: &str, mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        self.open(path, mip).map(|opened| Some(opened.size))
    }
}

fn archive_path(path: &str) -> String {
    format!("Data\\{}", path.replace('/', "\\"))
}

/// Returns whether pixels of `format` have an alpha channel that holds data.
fn format_has_alpha(format: TextureFormat) -> bool {
    match format {
        TextureFormat::Dxgi(dxgi) => DXGI_WITH_ALPHA.contains(&dxgi.0),
        TextureFormat::L8A8 => true,
        _ => false,
    }
}

/// Returns the message of `error` and of its sources, separated by colons.
fn describe(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}
