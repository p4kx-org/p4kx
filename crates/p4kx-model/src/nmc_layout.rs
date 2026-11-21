// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::{ByteReader, ParseError};

use crate::nmc::NODE_LEN;
use crate::reader::to_usize;

pub(crate) const HEADER_BYTES: usize = 32;
const FOOTER_BYTES: usize = 32;

pub(crate) struct NmcLayout<'a> {
    pub(crate) records: &'a [u8],
    pub(crate) material_bytes: &'a [u8],
    pub(crate) material_start: u64,
    pub(crate) names: &'a [u8],
    pub(crate) names_start: u64,
    pub(crate) properties: &'a [u8],
    pub(crate) properties_start: u64,
    pub(crate) declared_property_bytes: u32,
}

impl<'a> NmcLayout<'a> {
    pub(crate) fn read(bytes: &'a [u8]) -> Result<Self, ParseError> {
        let mut reader = ByteReader::new(bytes);
        reader.skip(4)?;
        let count = to_usize(reader.read_u32()?);
        reader.skip(4)?;
        let other_indices = to_usize(reader.read_u32()?);
        let material_indices = to_usize(reader.read_u32()?);
        let names_len = to_usize(reader.read_u32()?);
        let declared_property_bytes = reader.read_u32()?;
        reader.skip(4)?;
        let records = reader.read_bytes(count.saturating_mul(NODE_LEN))?;
        reader.skip(FOOTER_BYTES)?;
        reader.skip(other_indices.saturating_mul(2))?;
        let material_start = reader.offset();
        let material_bytes = reader.read_bytes(material_indices.saturating_mul(2))?;
        let names_start = reader.offset();
        let names = reader.read_bytes(names_len)?;
        let properties_start = reader.offset();
        let n = to_usize(declared_property_bytes).min(reader.remaining());
        let properties = reader.read_bytes(n)?;
        Ok(Self {
            records,
            material_bytes,
            material_start,
            names,
            names_start,
            properties,
            properties_start,
            declared_property_bytes,
        })
    }

    pub(crate) const fn names_range(&self) -> Range<u64> {
        self.names_start..self.properties_start
    }

    pub(crate) const fn properties_range(&self) -> Range<u64> {
        self.properties_start..self.properties_start + self.properties.len() as u64
    }
}

pub(crate) fn name_line(entry: &[u8]) -> &[u8] {
    entry
        .split(|&byte| byte == b'\n')
        .next()
        .unwrap_or_default()
}
