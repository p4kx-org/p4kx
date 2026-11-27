// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the image preview of a texture, looked up in the Fluent file of the current
/// language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageText {
    /// The preview tab that shows a texture as an image.
    Tab,
    /// The label of the mip selector.
    Mip,
    /// The key of the pixel format.
    Format,
    /// The key of the size of the largest mip.
    Size,
    /// The key of the number of mips.
    Mips,
    /// The key of the number of cubemap faces.
    Faces,
    /// The key of the number of array elements or volume slices.
    Layers,
    /// The width and height of a mip; takes `width` and `height`.
    Dimensions,
    /// Shown for a value a texture does not have, such as the faces of a plain texture.
    None,
    /// The accessible name of the shown mip; takes `level`, `width` and `height`.
    Shown,
    /// Shown while a mip is decoded; takes `level` and `path`.
    Decoding,
    /// Shown for a format the preview cannot decode; takes `format` and `path`.
    Unsupported,
    /// Shown when the texture or a mip cannot be decoded; takes `path` and `reason`.
    Failed,
    /// Shown when every mip is larger than the preview can show; takes `path`.
    Unshowable,
    /// The rendered pixels of the selected mip.
    Pixels,
    /// Source encryption includes a proved encrypted input.
    OriginEncrypted,
    /// All contributing inputs are proved unencrypted.
    OriginPlain,
    /// Available evidence does not establish the encryption origin.
    OriginUnknown,
}

impl ImageText {
    /// Every text of the image preview.
    pub(crate) const ALL: [Self; 18] = [
        Self::Tab,
        Self::Mip,
        Self::Format,
        Self::Size,
        Self::Mips,
        Self::Faces,
        Self::Layers,
        Self::Dimensions,
        Self::None,
        Self::Shown,
        Self::Decoding,
        Self::Unsupported,
        Self::Failed,
        Self::Unshowable,
        Self::Pixels,
        Self::OriginEncrypted,
        Self::OriginPlain,
        Self::OriginUnknown,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tab => "preview-tab-image",
            Self::Mip => "image-mip",
            Self::Format => "image-format",
            Self::Size => "image-size",
            Self::Mips => "image-mips",
            Self::Faces => "image-faces",
            Self::Layers => "image-layers",
            Self::Dimensions => "image-dimensions",
            Self::None => "image-none",
            Self::Shown => "image-shown",
            Self::Decoding => "image-decoding",
            Self::Unsupported => "image-unsupported",
            Self::Failed => "image-failed",
            Self::Unshowable => "image-unshowable",
            Self::Pixels => "image-pixels",
            Self::OriginEncrypted => "image-origin-encrypted",
            Self::OriginPlain => "image-origin-plain",
            Self::OriginUnknown => "image-origin-unknown",
        }
    }
}
