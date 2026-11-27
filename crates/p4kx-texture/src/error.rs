// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use thiserror::Error;

use crate::TextureFormat;

/// A failure to read or merge a DDS texture.
#[derive(Debug, Error)]
pub enum TextureError {
    /// The data does not start with a DDS header.
    #[error("not a DDS texture: the header is missing")]
    NotDds,
    /// The data ends inside the DDS header or before the header Entry's declared size.
    #[error("the DDS header is truncated: {len} bytes, {needed} needed")]
    TruncatedHeader {
        /// Bytes available.
        len: u64,
        /// Bytes needed.
        needed: u64,
    },
    /// The DDS header contradicts itself.
    #[error("the DDS header is invalid: {0}")]
    InvalidHeader(&'static str),
    /// The pixel format's size is unknown, so the mips cannot be told apart.
    #[error("the texture format {0} is not supported")]
    UnsupportedFormat(TextureFormat),
    /// The bytes after the header are not a run of the smallest mips.
    #[error("the {tail} bytes after the DDS header are not a whole number of mips")]
    TailMismatch {
        /// Bytes after the header.
        tail: u64,
    },
    /// The 8 bytes after the last mip are not the CryEngine trailer `CExtCEnd`.
    #[error("the 8 bytes after the last mip are not the CryEngine trailer: {found:02x?}")]
    UnknownTrailer {
        /// The bytes found.
        found: [u8; 8],
    },
    /// A sibling part the header needs does not exist.
    #[error("part {index} is missing")]
    MissingPart {
        /// The part's number, the `N` of `.dds.N`.
        index: u32,
    },
    /// A sibling part is shorter than its mip.
    #[error("part {index} is truncated: {actual} of {expected} bytes")]
    TruncatedPart {
        /// The part's number.
        index: u32,
        /// The size of its mip.
        expected: u64,
        /// The bytes it holds.
        actual: u64,
    },
    /// A sibling part is longer than its mip.
    #[error("part {index} is {actual} bytes long, but its mip has {expected}")]
    OversizedPart {
        /// The part's number.
        index: u32,
        /// The size of its mip.
        expected: u64,
        /// The bytes it holds.
        actual: u64,
    },
    /// The header could not be read.
    #[error("cannot read the DDS header")]
    ReadHeader(#[source] io::Error),
    /// A sibling part could not be read.
    #[error("cannot read part {index}")]
    ReadPart {
        /// The part's number.
        index: u32,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The merged texture or the PNG could not be written.
    #[error("cannot write the texture")]
    Write(#[source] io::Error),
    /// The pixel format has no decoder.
    #[error("decoding the texture format {0} is not supported")]
    UnsupportedDecode(TextureFormat),
    /// The pixel format holds no values beyond 0..=1, so it has no high-range decoder.
    #[error("the texture format {0} is not a high-range format")]
    NotHighRange(TextureFormat),
    /// The requested mip level does not exist.
    #[error("mip {level} does not exist: the texture has {mip_count} mips, 0 being the largest")]
    NoSuchMip {
        /// The requested level.
        level: u32,
        /// The number of mips.
        mip_count: u32,
    },
    /// The texture has several images per mip, which cannot be decoded into one image yet.
    #[error("decoding a texture with {images} images per mip is not supported yet")]
    UnsupportedImage {
        /// Cubemap faces times array elements times volume slices.
        images: u64,
    },
    /// The data of a mip is not exactly one image of its size.
    #[error("the mip holds {actual} bytes, but its size needs {expected}")]
    MipLength {
        /// Bytes the size needs.
        expected: u64,
        /// Bytes given.
        actual: u64,
    },
    /// The images given for a sheet are none, of different sizes or in no column.
    #[error("a sheet needs at least one image, all of one size, in at least one column")]
    SheetMismatch,
    /// The images given for a cubemap cross are not six faces of one size.
    #[error("a cubemap cross needs six faces of one size, got {faces} images")]
    NotACube {
        /// The number of images given.
        faces: usize,
    },
    /// The alpha tail has fewer mips than the texture and lacks the requested one.
    #[error("the alpha tail has no mip {level}: it has {mip_count} mips, 0 being the largest")]
    AlphaTailMips {
        /// The requested level.
        level: u32,
        /// The number of mips of the alpha tail.
        mip_count: u32,
    },
    /// The alpha tail's mip is not the size of the colour mip it belongs to.
    #[error(
        "the alpha tail is {width} x {height}, but the texture is {expected_width} x {expected_height}"
    )]
    AlphaTailSize {
        /// Width of the alpha tail's mip.
        width: u32,
        /// Height of the alpha tail's mip.
        height: u32,
        /// Width of the colour mip.
        expected_width: u32,
        /// Height of the colour mip.
        expected_height: u32,
    },
    /// The PNG encoder rejected the image.
    #[error("cannot encode the PNG: {0}")]
    EncodePng(String),
}
