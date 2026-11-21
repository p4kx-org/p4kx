// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{ROTATION_FLOAT, ROTATION_SMALL_TREE_48, ROTATION_SMALL_TREE_64, bytes_at, f32s};
use crate::CorruptAnimation;

fn small_tree_identity(widths: [u32; 3], bits: u32) -> [u8; 8] {
    let mut raw = 3_u64 << (bits - 2);
    let mut shift = 0;
    for width in widths {
        raw |= (1_u64 << (width - 1)) << shift;
        shift += width;
    }
    raw.to_le_bytes()
}

pub(crate) fn is_canonical_identity(
    data: &[u8],
    start: usize,
    format: u8,
    count: usize,
) -> Result<bool, CorruptAnimation> {
    match format {
        ROTATION_FLOAT => {
            let bytes = bytes_at(data, start, count.saturating_mul(size_of::<[f32; 4]>()))?;
            Ok(f32s::<4>(bytes).all(|[x, y, z, w]| {
                x == 0.0 && y == 0.0 && z == 0.0 && w.abs().to_bits() == 1.0_f32.to_bits()
            }))
        }
        ROTATION_SMALL_TREE_48 => {
            let bytes = bytes_at(data, start, count.saturating_mul(48 / 8))?;
            let [expected @ .., _, _] = small_tree_identity([15; 3], 48);
            Ok(bytes.as_chunks::<6>().0.iter().all(|key| *key == expected))
        }
        ROTATION_SMALL_TREE_64 => {
            let bytes = bytes_at(data, start, count.saturating_mul(64 / 8))?;
            let expected = small_tree_identity([21, 21, 20], 64);
            Ok(bytes.as_chunks::<8>().0.iter().all(|key| *key == expected))
        }
        code => Err(CorruptAnimation::UnknownFormat {
            kind: "rotation",
            code,
        }),
    }
}
