// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::{ByteReader, ParseError};

use crate::error::{CryXmlError, Table};
use crate::format::{ByteOrder, Signature};

pub(crate) const NODE_LEN: usize = 28;
pub(crate) const ATTRIBUTE_LEN: usize = 8;
const CHILD_LEN: usize = 4;

/// The header of a `CryXml` file and the byte slices of its tables.
pub(crate) struct Header<'a> {
    pub(crate) byte_order: ByteOrder,
    pub(crate) nodes: &'a [u8],
    pub(crate) attributes: &'a [u8],
    pub(crate) children: &'a [u8],
    pub(crate) strings: &'a [u8],
    places: [(Table, usize, usize); 4],
}

impl<'a> Header<'a> {
    /// Reads the header behind `signature`, detects the byte order and locates every table.
    pub(crate) fn read(bytes: &'a [u8], signature: Signature) -> Result<Self, CryXmlError> {
        let mut reader = ByteReader::new(bytes);
        reader.skip(signature.len())?;
        let size_field: [u8; 4] = array(&mut reader)?;
        let (byte_order, size) = byte_order(size_field, bytes.len())?;
        let data = bytes.get(..size).unwrap_or(bytes);
        let mut fields = Reader { reader, byte_order };
        let mut places = [(Table::Nodes, 0, 0); 4];
        let mut next = 0;
        let mut table = |table: Table, entry_len: usize| -> Result<&'a [u8], CryXmlError> {
            let position = fields.u32()?;
            let count = fields.u32()?;
            let found = slice(data, table, position, count, entry_len)?;
            if let Some(place) = places.get_mut(next) {
                let start = usize::try_from(position).unwrap_or(usize::MAX);
                *place = (table, start, found.len());
            }
            next += 1;
            Ok(found)
        };
        let nodes = table(Table::Nodes, NODE_LEN)?;
        let attributes = table(Table::Attributes, ATTRIBUTE_LEN)?;
        let children = table(Table::Children, CHILD_LEN)?;
        let strings = table(Table::Strings, 1)?;
        Ok(Self {
            byte_order,
            nodes,
            attributes,
            children,
            strings,
            places,
        })
    }

    pub(crate) const fn render_starts(&self) -> [usize; 3] {
        let [(_, nodes, _), (_, attributes, _), _, (_, strings, _)] = self.places;
        [nodes, attributes, strings]
    }

    /// Checks that no two tables share a byte, so that what is derived from them cannot exceed a
    /// small multiple of the file size.
    ///
    /// # Errors
    ///
    /// Returns [`CryXmlError::TablesOverlap`] for the first two tables that overlap.
    pub(crate) fn ensure_disjoint(&self) -> Result<(), CryXmlError> {
        for (index, &(first, start, len)) in self.places.iter().enumerate() {
            for &(second, other_start, other_len) in self.places.iter().skip(index + 1) {
                let overlaps = len > 0
                    && other_len > 0
                    && start < other_start.saturating_add(other_len)
                    && other_start < start.saturating_add(len);
                if overlaps {
                    return Err(CryXmlError::TablesOverlap { first, second });
                }
            }
        }
        Ok(())
    }
}

/// Picks the byte order whose declared size matches the input, or else one that fits into it.
fn byte_order(field: [u8; 4], available: usize) -> Result<(ByteOrder, usize), CryXmlError> {
    let little = u32::from_le_bytes(field);
    let big = u32::from_be_bytes(field);
    let fits = |size: u32| usize::try_from(size).ok().filter(|&size| size <= available);
    let exact = |size: u32| fits(size).filter(|&size| size == available);
    exact(little)
        .map(|size| (ByteOrder::Little, size))
        .or_else(|| exact(big).map(|size| (ByteOrder::Big, size)))
        .or_else(|| fits(little).map(|size| (ByteOrder::Little, size)))
        .or_else(|| fits(big).map(|size| (ByteOrder::Big, size)))
        .ok_or_else(|| CryXmlError::Truncated {
            declared: u64::from(little.min(big)),
            available: available as u64,
        })
}

fn slice(
    data: &[u8],
    table: Table,
    position: u32,
    count: u32,
    entry_len: usize,
) -> Result<&[u8], CryXmlError> {
    let start = usize::try_from(position).ok();
    let len = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(entry_len));
    start
        .zip(len)
        .and_then(|(start, len)| Some(start..start.checked_add(len)?))
        .and_then(|range| data.get(range))
        .ok_or(CryXmlError::TableOutOfBounds {
            table,
            position,
            count,
        })
}

/// A reader of integers in the byte order of one `CryXml` file.
pub(crate) struct Reader<'a> {
    reader: ByteReader<'a>,
    byte_order: ByteOrder,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8], byte_order: ByteOrder) -> Self {
        Self {
            reader: ByteReader::new(bytes),
            byte_order,
        }
    }

    pub(crate) fn u16(&mut self) -> Result<u16, ParseError> {
        let bytes = array(&mut self.reader)?;
        Ok(match self.byte_order {
            ByteOrder::Little => u16::from_le_bytes(bytes),
            ByteOrder::Big => u16::from_be_bytes(bytes),
        })
    }

    pub(crate) fn u32(&mut self) -> Result<u32, ParseError> {
        let bytes = array(&mut self.reader)?;
        Ok(match self.byte_order {
            ByteOrder::Little => u32::from_le_bytes(bytes),
            ByteOrder::Big => u32::from_be_bytes(bytes),
        })
    }
}

fn array<const N: usize>(reader: &mut ByteReader<'_>) -> Result<[u8; N], ParseError> {
    let bytes = reader.read_bytes(N)?;
    let mut array = [0; N];
    array.copy_from_slice(bytes);
    Ok(array)
}
