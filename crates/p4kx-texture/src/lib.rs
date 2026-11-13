// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads the DDS textures of Star Citizen Archives, which store a texture as a header Entry
//! holding the DDS header and the smallest mips plus one sibling Entry `.dds.N` per larger mip,
//! `.dds.1` holding the smallest of them; a cubemap's part holds that mip of all six faces. An
//! alpha or smoothness tail is a second split texture `.dds.a` with parts `.dds.1a` … `.dds.Na`.
//! Parses legacy and DX10 headers, merges the parts into one standard DDS file through a
//! [`SiblingReader`] the caller supplies, reads and decodes mips to RGBA, BC5 normal maps with
//! their Z reconstructed, merges an alpha tail into the alpha channel, lays cubemap faces out as
//! a cross and the images of arrays and volumes out as a sheet, and writes images as PNG; decodes
//! BC6H and the floating-point formats to floating-point RGB as well and writes that as OpenEXR.

mod alpha;
mod bc;
mod bc6h;
mod bc6h_modes;
mod bc7;
mod bc7_tables;
mod cube;
mod dds_alpha_association;
mod dds_alpha_coverage;
mod dds_alpha_transform;
mod dds_raw_fields;
mod dds_raw_header;
mod dds_transfer;
mod dxgi;
mod error;
mod exr_export;
mod floats;
mod format;
mod hdr;
mod header;
mod header_words;
mod image;
mod merge;
mod mip;
mod mip_dds;
mod mip_source;
mod pixels;
mod png_export;
mod png_transfer;
mod rgtc;
mod sheet;
mod tone_map;

pub use alpha::merge_alpha;
pub use cube::{cube_cross, cube_cross_hdr};
pub use dds_alpha_association::DdsAlphaAssociation;
pub use dds_alpha_coverage::DdsAlphaCoverage;
pub use dds_alpha_transform::DdsAlphaTransform;
pub use dds_raw_fields::{DdsDx10Fields, DdsLegacyFields, DdsPixelFormatFields};
pub use dds_raw_header::DdsRawHeader;
pub use dds_transfer::{DdsTransfer, DdsTransferEvidence};
pub use dxgi::DxgiFormat;
pub use error::TextureError;
pub use exr_export::write_exr;
pub use format::{BlockLayout, TextureFormat};
pub use hdr::{HdrImage, decode_hdr};
pub use header::{DdsHeader, MAX_HEADER_LEN};
pub use header_words::{DdsHeaderField, DdsHeaderWord};
pub use image::{RgbaImage, decode};
pub use merge::{MergeReport, SiblingReader, merge};
pub use mip::{Mip, read_mip, read_mip_with_location};
pub use mip_source::{LocatedMip, MipByteSpans, MipPayloadLocation};
pub use png_export::{write_png, write_png_with_transfer};
pub use png_transfer::{DdsPngEncoding, DdsPngTransform};
pub use sheet::{sheet_columns, tile};
