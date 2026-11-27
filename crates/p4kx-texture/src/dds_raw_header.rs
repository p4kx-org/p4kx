// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::{ByteReader, ParseError};

use crate::{DdsDx10Fields, DdsLegacyFields, DdsPixelFormatFields, TextureError};

const MAGIC: &[u8; 4] = b"DDS ";
const MAGIC_LEN: usize = MAGIC.len();
const HEADER_SIZE: u32 = 124;
const LEGACY_LEN: usize = HEADER_SIZE as usize;
const DX10_LEN: usize = 20;
const DX10_FOUR_CC: [u8; 4] = *b"DX10";
pub(crate) const MAX_HEADER_LEN: usize = MAGIC_LEN + LEGACY_LEN + DX10_LEN;

/// A structurally complete DDS header retaining every authored word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdsRawHeader {
    magic_present: bool,
    legacy: DdsLegacyFields,
    dx10: Option<DdsDx10Fields>,
}

impl DdsRawHeader {
    /// Reads a 124-byte legacy header and its declared DX10 extension, with optional DDS magic.
    ///
    /// # Errors
    /// Returns non-DDS, truncated-header or unsupported header-size errors without topology validation.
    pub fn parse(bytes: &[u8]) -> Result<Self, TextureError> {
        let magic_present = match bytes.get(..MAGIC_LEN) {
            Some(start) if start == MAGIC => true,
            Some(start) if start == HEADER_SIZE.to_le_bytes() => false,
            _ => return Err(TextureError::NotDds),
        };
        let magic_len = if magic_present { MAGIC_LEN } else { 0 };
        let body = bytes.get(magic_len..).unwrap_or_default();
        if body
            .get(..4)
            .is_some_and(|size| size != HEADER_SIZE.to_le_bytes())
        {
            return Err(TextureError::InvalidHeader("the header size is not 124"));
        }
        let truncated = |needed| TextureError::TruncatedHeader {
            len: bytes.len() as u64,
            needed: (magic_len + needed) as u64,
        };
        let mut reader = ByteReader::new(body);
        let legacy = read_legacy(&mut reader).map_err(|_| truncated(LEGACY_LEN))?;
        let dx10 = if legacy.pixel_format.four_cc == DX10_FOUR_CC {
            Some(read_dx10(&mut reader).map_err(|_| truncated(LEGACY_LEN + DX10_LEN))?)
        } else {
            None
        };
        Ok(Self {
            magic_present,
            legacy,
            dx10,
        })
    }

    /// Returns whether the four DDS magic bytes were present.
    #[must_use]
    pub const fn magic_present(&self) -> bool {
        self.magic_present
    }

    /// Returns the complete authored legacy fields.
    #[must_use]
    pub const fn legacy(&self) -> &DdsLegacyFields {
        &self.legacy
    }

    /// Returns the declared DX10 extension without interpreting unknown values.
    #[must_use]
    pub const fn dx10(&self) -> Option<&DdsDx10Fields> {
        self.dx10.as_ref()
    }

    /// Returns the bytes occupied by this header, including optional magic and extension.
    #[must_use]
    pub const fn header_len(&self) -> usize {
        let magic = if self.magic_present { MAGIC_LEN } else { 0 };
        let dx10 = if self.dx10.is_some() { DX10_LEN } else { 0 };
        magic + LEGACY_LEN + dx10
    }
}

fn read_legacy(reader: &mut ByteReader<'_>) -> Result<DdsLegacyFields, ParseError> {
    let size = reader.read_u32()?;
    let flags = reader.read_u32()?;
    let height = reader.read_u32()?;
    let width = reader.read_u32()?;
    let pitch_or_linear_size = reader.read_u32()?;
    let depth = reader.read_u32()?;
    let mip_map_count = reader.read_u32()?;
    let mut reserved1 = [0; 11];
    for word in &mut reserved1 {
        *word = reader.read_u32()?;
    }
    let pixel_format = DdsPixelFormatFields {
        size: reader.read_u32()?,
        flags: reader.read_u32()?,
        four_cc: reader.read_u32()?.to_le_bytes(),
        rgb_bit_count: reader.read_u32()?,
        r_bit_mask: reader.read_u32()?,
        g_bit_mask: reader.read_u32()?,
        b_bit_mask: reader.read_u32()?,
        a_bit_mask: reader.read_u32()?,
    };
    Ok(DdsLegacyFields {
        size,
        flags,
        height,
        width,
        pitch_or_linear_size,
        depth,
        mip_map_count,
        reserved1,
        pixel_format,
        caps: reader.read_u32()?,
        caps2: reader.read_u32()?,
        caps3: reader.read_u32()?,
        caps4: reader.read_u32()?,
        reserved2: reader.read_u32()?,
    })
}

fn read_dx10(reader: &mut ByteReader<'_>) -> Result<DdsDx10Fields, ParseError> {
    Ok(DdsDx10Fields {
        dxgi_format: reader.read_u32()?,
        resource_dimension: reader.read_u32()?,
        misc_flag: reader.read_u32()?,
        array_size: reader.read_u32()?,
        misc_flags2: reader.read_u32()?,
    })
}
