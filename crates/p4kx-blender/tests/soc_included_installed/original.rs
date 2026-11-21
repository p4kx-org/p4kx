// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::Chunk;

use super::Check;

pub(super) const NAME_BYTES: usize = 256;

pub(super) struct Original<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) header: &'a Chunk,
    pub(super) ordinal: usize,
    pub(super) layout: u32,
    pub(super) object_range: Range<usize>,
    pub(super) name_ranges: [Range<usize>; 3],
}

impl<'a> Original<'a> {
    pub(super) fn read(bytes: &'a [u8], header: &'a Chunk, ordinal: usize) -> Check<Self> {
        let model_count = usize::try_from(word32(bytes, 4)?)?;
        let models = 8..8 + model_count.checked_mul(NAME_BYTES).ok_or("model size")?;
        let counts = models.end;
        let material_count = usize::from(u16::from_le_bytes(word(bytes, counts)?));
        let palette_count = usize::from(u16::from_le_bytes(word(bytes, counts + 2)?));
        let materials = counts + 4..counts + 4 + material_count * NAME_BYTES;
        let palettes = materials.end..materials.end + palette_count * NAME_BYTES;
        let layout = word32(bytes, palettes.end)?;
        let object_start = palettes.end + 32;
        let declared = usize::try_from(word32(bytes, object_start - 4)?)?;
        let object_end = object_start
            .checked_add(declared)
            .ok_or("object table length")?;
        let object_range = object_start..object_end;
        bytes
            .get(object_range.clone())
            .ok_or("original object table")?;
        Ok(Self {
            bytes,
            header,
            ordinal,
            layout,
            object_range,
            name_ranges: [models, materials, palettes],
        })
    }

    pub(super) fn placement_rows(&self) -> Check<Vec<(usize, Range<usize>)>> {
        let stride = match self.layout {
            8..=10 => 176,
            12 => 184,
            13 => 192,
            14 => 200,
            15 => 208,
            _ => return Ok(Vec::new()),
        };
        let mut offset = self.object_range.start;
        let mut placements = Vec::new();
        let mut ordinal = 0;
        while offset < self.object_range.end {
            let kind = word32(self.bytes, offset)?;
            let length = match kind {
                1 => stride,
                16 => 136,
                _ => return Ok(Vec::new()),
            };
            let end = offset + length;
            if end > self.object_range.end {
                return Err("partial original record".into());
            }
            if kind == 1 {
                placements.push((ordinal, offset..end));
            }
            offset = end;
            ordinal += 1;
        }
        Ok(placements)
    }
}

pub(super) fn word<const N: usize>(bytes: &[u8], offset: usize) -> Check<[u8; N]> {
    bytes
        .get(offset..offset + N)
        .ok_or("source word outside chunk")?
        .try_into()
        .map_err(|_| "incorrect original word width".into())
}

fn word32(bytes: &[u8], offset: usize) -> Check<u32> {
    Ok(u32::from_le_bytes(word(bytes, offset)?))
}
