// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_texture::{
    DdsAlphaAssociation, DdsAlphaTransform, DdsPngEncoding, DdsTransfer, RgbaImage, TextureError,
};
use thiserror::Error;

use crate::texture_origins::{TextureComponents, TextureReadId, TextureReadUse};

/// Why a texture could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TextureLoadError {
    /// No such texture exists.
    #[error("the texture does not exist")]
    NotFound,
    /// The texture exists but cannot be read, decoded or used.
    #[error("{0}")]
    Unreadable(String),
}

/// A decoded texture.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LoadedTexture {
    /// The pixels, 8-bit RGBA, any alpha tail merged into the alpha channel.
    pub image: RgbaImage,
    /// Whether the alpha channel holds data, from an alpha tail or from the texture's own format,
    /// rather than the opaque value a format without alpha decodes to.
    pub has_alpha: bool,
    declared_transfer: DdsTransfer,
    alpha: Option<DdsAlphaInput>,
    encoding: Option<DdsPngEncoding>,
    read: Option<TextureReadId>,
    fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DdsAlphaInput {
    association: DdsAlphaAssociation,
    opacity: bool,
    source: AlphaSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AlphaSource {
    Embedded,
    Split { original: Option<Vec<u8>> },
}

impl LoadedTexture {
    /// Creates a texture from `image`, whose alpha channel holds data when `has_alpha` is set.
    #[must_use]
    pub const fn new(image: RgbaImage, has_alpha: bool) -> Self {
        Self {
            image,
            has_alpha,
            declared_transfer: DdsTransfer::Unknown,
            alpha: None,
            encoding: None,
            read: None,
            fallback: false,
        }
    }

    /// Records the source declaration and the transfer of the decoded RGB samples.
    #[must_use]
    pub const fn with_dds_transfer(
        mut self,
        declared: DdsTransfer,
        encoding: DdsPngEncoding,
    ) -> Self {
        self.declared_transfer = declared;
        self.encoding = Some(encoding);
        self
    }

    /// Records the authored DDS association and whether selected samples can carry opacity.
    #[must_use]
    pub fn with_dds_alpha(mut self, association: DdsAlphaAssociation, opacity: bool) -> Self {
        self.alpha = Some(DdsAlphaInput {
            association,
            opacity,
            source: AlphaSource::Embedded,
        });
        self
    }

    /// Records that a separate DDS supplied output alpha, retaining replaced embedded opacity.
    ///
    /// # Errors
    /// Returns an error if the supplied channel does not contain one value per image texel.
    pub fn with_split_alpha(mut self, original: Option<Vec<u8>>) -> Result<Self, TextureError> {
        if let Some(original) = &original {
            let expected = self.image.pixels().len() / 4;
            if original.len() != expected {
                return Err(TextureError::MipLength {
                    expected: expected as u64,
                    actual: original.len() as u64,
                });
            }
        }
        if let Some(alpha) = &mut self.alpha {
            alpha.source = AlphaSource::Split { original };
        }
        Ok(self)
    }

    pub(crate) const fn opacity_transform(&self) -> DdsAlphaTransform {
        match &self.alpha {
            Some(DdsAlphaInput {
                source: AlphaSource::Split { original: None },
                ..
            })
            | None => DdsAlphaTransform::Preserve,
            Some(alpha) => {
                DdsAlphaTransform::for_opacity(alpha.association, self.decoded_transfer())
            }
        }
    }

    pub(crate) fn straight_texel(&self, pixel: [u8; 4], index: usize) -> [u8; 4] {
        let opacity = self.original_opacity().map_or(pixel[3], |alpha| {
            alpha.get(index).copied().unwrap_or(pixel[3])
        });
        self.opacity_transform()
            .texel_with_opacity(self.decoded_transfer(), pixel, opacity)
    }

    fn original_opacity(&self) -> Option<&[u8]> {
        match self.alpha.as_ref().map(|alpha| &alpha.source) {
            Some(AlphaSource::Split { original }) => original.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn colour_image(&self) -> Result<Cow<'_, RgbaImage>, TextureError> {
        let transform = self.opacity_transform();
        if transform == DdsAlphaTransform::Preserve {
            return Ok(Cow::Borrowed(&self.image));
        }
        let mut image = self.image.clone();
        let transfer = self.decoded_transfer();
        if let Some(opacity) = self.original_opacity() {
            transform.image_with_opacity(transfer, &mut image, opacity)?;
        } else {
            transform.image(transfer, &mut image);
        }
        Ok(Cow::Owned(image))
    }

    pub(crate) const fn uses_opacity(&self) -> bool {
        let Some(alpha) = &self.alpha else {
            return self.has_alpha;
        };
        alpha.opacity
            && (matches!(alpha.source, AlphaSource::Split { .. })
                || !matches!(
                    alpha.association,
                    DdsAlphaAssociation::Opaque | DdsAlphaAssociation::Custom
                ))
    }

    /// Returns the encoding of the decoded RGB samples.
    #[must_use]
    pub const fn decoded_transfer(&self) -> DdsTransfer {
        match self.encoding {
            Some(encoding) => encoding.transfer,
            None => DdsTransfer::Unknown,
        }
    }

    /// Binds the decoded value to the loader's physical read evidence.
    #[must_use]
    pub const fn with_read(mut self, read: TextureReadId) -> Self {
        self.read = Some(read);
        self
    }

    const fn with_fallback(mut self) -> Self {
        self.fallback = true;
        self
    }

    pub(crate) const fn contribution(&self, components: TextureComponents) -> TextureReadUse {
        TextureReadUse {
            read: self.read,
            components,
            fallback: self.fallback,
            declared_transfer: self.declared_transfer,
            transfer: None,
            alpha_association: match &self.alpha {
                Some(alpha) => Some(alpha.association),
                None => None,
            },
            alpha_transform: DdsAlphaTransform::Preserve,
        }
    }

    pub(crate) const fn colour_contribution(
        &self,
        components: TextureComponents,
    ) -> TextureReadUse {
        let mut contribution = self.contribution(components);
        contribution.transfer = self.encoding;
        contribution.alpha_transform = self.opacity_transform();
        contribution
    }
}

impl PartialEq for LoadedTexture {
    fn eq(&self, other: &Self) -> bool {
        self.image == other.image
            && self.has_alpha == other.has_alpha
            && self.declared_transfer == other.declared_transfer
            && self.encoding == other.encoding
            && self.alpha == other.alpha
    }
}

impl Eq for LoadedTexture {}

/// Gives the GLB writer the textures of the materials, wherever they are stored.
pub trait TextureLoader: Sync {
    /// Loads mip `mip`, 0 being the largest, of the texture at `path`. A `mip` beyond the
    /// texture's last one selects the last.
    ///
    /// `path` is relative to the `Data` folder, with `/` as separator and the extension `.dds`; it
    /// has no empty, `.` or `..` component.
    ///
    /// # Errors
    ///
    /// Returns [`TextureLoadError::NotFound`] when there is no such texture and
    /// [`TextureLoadError::Unreadable`] when it cannot be used.
    fn load(&self, path: &str, mip: u32) -> Result<LoadedTexture, TextureLoadError>;

    /// Returns the width and height [`TextureLoader::load`] would give for mip `mip` of the
    /// texture at `path`, without decoding it, or `None` when the loader cannot tell that
    /// cheaply. The writer uses it to leave out an image that would exceed the budget before
    /// decoding anything.
    ///
    /// # Errors
    ///
    /// Returns the errors [`TextureLoader::load`] would return for the same texture.
    fn dimensions(&self, _path: &str, _mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        Ok(None)
    }
}

/// A loader without textures: every texture is unreadable.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NoTextures;

impl TextureLoader for NoTextures {
    fn load(&self, _path: &str, _mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        Err(TextureLoadError::Unreadable(
            "no texture source was given".to_owned(),
        ))
    }
}

/// Loads the texture a path is mapped to when the texture at the path does not exist.
pub(crate) struct WithFallbacks<'a> {
    pub(crate) inner: &'a dyn TextureLoader,
    pub(crate) own: &'a std::collections::HashMap<String, String>,
}

impl TextureLoader for WithFallbacks<'_> {
    fn load(&self, path: &str, mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        match self.inner.load(path, mip) {
            Err(TextureLoadError::NotFound) => self
                .own
                .get(path)
                .map_or(Err(TextureLoadError::NotFound), |own| {
                    self.inner.load(own, mip).map(LoadedTexture::with_fallback)
                }),
            other => other,
        }
    }

    fn dimensions(&self, path: &str, mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        match self.inner.dimensions(path, mip) {
            Err(TextureLoadError::NotFound) => self
                .own
                .get(path)
                .map_or(Err(TextureLoadError::NotFound), |own| {
                    self.inner.dimensions(own, mip)
                }),
            other => other,
        }
    }
}
