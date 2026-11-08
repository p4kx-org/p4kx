// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::FusedIterator;
use std::ops::Range;
use std::path::{Path, PathBuf};

use p4kx_texture::{DdsHeader, TextureError};
use schemars::JsonSchema;
use serde::Serialize;

use crate::texture_names::suffixed_path;
use crate::{CUBE_FACE_NAMES, CubemapLayout};

const CUBE_FACES: u32 = 6;
const CROSS_COLUMNS: u32 = 4;
const CROSS_ROWS: u32 = 3;

/// The encoding requested for a converted image, without a decoder-availability claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextureImageFormat {
    /// Eight-bit RGBA PNG.
    Png,
    /// Linear 32-bit floating-point RGB or RGBA OpenEXR.
    Exr,
}

impl TextureImageFormat {
    /// Returns the image format selected by ordinary Extraction.
    #[must_use]
    pub const fn for_header(header: &DdsHeader) -> Self {
        if header.format().is_high_range() {
            Self::Exr
        } else {
            Self::Png
        }
    }

    /// Returns the conventional filename extension.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Exr => "exr",
        }
    }

    /// Returns the stored number of bits per channel.
    #[must_use]
    pub const fn bit_depth(self) -> u8 {
        match self {
            Self::Png => 8,
            Self::Exr => 32,
        }
    }
}

/// The source face represented by one image, or the existing combined cross layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "index",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum TextureImageFace {
    /// A texture with no independently named face.
    Plane,
    /// DDS positive X.
    PositiveX,
    /// DDS negative X.
    NegativeX,
    /// DDS positive Y.
    PositiveY,
    /// DDS negative Y.
    NegativeY,
    /// DDS positive Z.
    PositiveZ,
    /// DDS negative Z.
    NegativeZ,
    /// A face ordinal whose orientation the normalized header does not establish.
    Indexed(#[schemars(range(max = u32::MAX))] u32),
    /// One horizontal cross containing all six source faces.
    Cross,
}

impl TextureImageFace {
    const fn of(index: u32, faces: u32, cross: bool) -> Self {
        if cross {
            return Self::Cross;
        }
        match (index, faces) {
            (_, 1) => Self::Plane,
            (0, CUBE_FACES) => Self::PositiveX,
            (1, CUBE_FACES) => Self::NegativeX,
            (2, CUBE_FACES) => Self::PositiveY,
            (3, CUBE_FACES) => Self::NegativeY,
            (4, CUBE_FACES) => Self::PositiveZ,
            (5, CUBE_FACES) => Self::NegativeZ,
            _ => Self::Indexed(index),
        }
    }
}

/// One planned image in storage order; planning does not read, decode or publish bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextureImage {
    /// Actual selected mip, with zero denoting the largest.
    pub level: u32,
    /// Original array element, also zero for a non-array texture.
    pub layer: u32,
    /// Source face or combined cross identity.
    pub face: TextureImageFace,
    /// Original volume slice at this mip, also zero for a non-volume texture.
    pub slice: u32,
    /// Actual output image width, including a requested cross layout.
    pub width: u32,
    /// Actual output image height, including a requested cross layout.
    pub height: u32,
    /// Requested output encoding.
    pub format: TextureImageFormat,
    /// Existing image suffix before the caller's extension.
    pub suffix: String,
}

impl TextureImage {
    /// Returns the image path beside the caller's requested output, preserving its extension.
    #[must_use]
    pub fn path(&self, output: &Path) -> PathBuf {
        suffixed_path(output, &self.suffix)
    }
}

/// The shared image naming and topology plan used by PNG, OpenEXR and ordinary Extraction.
#[derive(Debug, Clone)]
pub struct TextureImagePlan {
    header: DdsHeader,
    level: u32,
    format: TextureImageFormat,
    faces: u32,
    slices: u32,
    cross: bool,
    width: u32,
    height: u32,
    remaining: Range<u32>,
}

impl TextureImagePlan {
    /// Plans one valid mip without decoding or changing the caller's chosen image format.
    ///
    /// # Errors
    ///
    /// Returns an absent-mip error or the existing image-size overflow error for a cross.
    pub fn new(
        header: &DdsHeader,
        level: u32,
        layout: CubemapLayout,
        format: TextureImageFormat,
    ) -> Result<Self, TextureError> {
        if level >= header.mip_count() {
            return Err(TextureError::NoSuchMip {
                level,
                mip_count: header.mip_count(),
            });
        }
        let cross = layout == CubemapLayout::Cross && header.faces() == CUBE_FACES;
        let faces = if cross { 1 } else { header.faces() };
        let slices = header.slices(level);
        let mut width = (header.width() >> level).max(1);
        let mut height = (header.height() >> level).max(1);
        if cross {
            let overflow = || TextureError::InvalidHeader("the texture size overflows");
            width = width.checked_mul(CROSS_COLUMNS).ok_or_else(overflow)?;
            height = height.checked_mul(CROSS_ROWS).ok_or_else(overflow)?;
        }
        Ok(Self {
            header: *header,
            level,
            format,
            faces,
            slices,
            cross,
            width,
            height,
            remaining: 0..header.array_size() * faces * slices,
        })
    }

    fn suffix(&self, layer: u32, face: u32, slice: u32) -> String {
        use std::fmt::Write as _;

        fn numbered(suffix: &mut String, name: &str, index: u32, count: u32) {
            use std::fmt::Write as _;
            let width = count.saturating_sub(1).to_string().len();
            let _written = write!(suffix, "_{name}{index:0width$}");
        }

        let mut suffix = String::new();
        if self.header.array_size() > 1 {
            numbered(&mut suffix, "layer", layer, self.header.array_size());
        }
        if self.faces > 1 {
            match usize::try_from(face)
                .ok()
                .and_then(|index| CUBE_FACE_NAMES.get(index))
            {
                Some(name) if self.header.faces() == CUBE_FACES => {
                    let _written = write!(suffix, "_{name}");
                }
                _ => numbered(&mut suffix, "face", face, self.faces),
            }
        }
        if self.slices > 1 {
            numbered(&mut suffix, "slice", slice, self.slices);
        }
        suffix
    }
}

impl Iterator for TextureImagePlan {
    type Item = TextureImage;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.remaining.next()?;
        let layer = index / (self.faces * self.slices);
        let face = (index / self.slices) % self.faces;
        let slice = index % self.slices;
        Some(TextureImage {
            level: self.level,
            layer,
            face: TextureImageFace::of(face, self.header.faces(), self.cross),
            slice,
            width: self.width,
            height: self.height,
            format: self.format,
            suffix: self.suffix(layer, face, slice),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}

impl ExactSizeIterator for TextureImagePlan {}
impl FusedIterator for TextureImagePlan {}
