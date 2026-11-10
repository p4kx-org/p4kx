// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DdsHeader, RgbaImage, TextureError};

const RGBA: usize = 4;
const CUBE_FACES: u32 = 6;
const CROSS_COLUMNS: u32 = 4;
const CROSS_ROWS: u32 = 3;

/// Returns the number of columns of a sheet of `units` images: the smallest square that holds them.
#[must_use]
pub const fn sheet_columns(units: u64) -> u64 {
    let root = units.isqrt();
    if root * root == units { root } else { root + 1 }
}

impl DdsHeader {
    /// Returns the number of units a preview of mip `level` lays out on one sheet: a full cubemap
    /// is one unit, its cross; every other face, array element and slice is one unit each.
    #[must_use]
    pub const fn preview_units(&self, level: u32) -> u64 {
        let images = self.images(level);
        if self.faces() == CUBE_FACES {
            images / CUBE_FACES as u64
        } else {
            images
        }
    }

    /// Returns the width and height in pixels of the one image a preview shows for mip `level`:
    /// the mip itself, a cubemap's cross, or the sheet that tiles the images of an array, volume
    /// or several cubemaps in [`sheet_columns`] columns. `None` when the level does not exist or
    /// the size overflows.
    #[must_use]
    pub fn preview_size(&self, level: u32) -> Option<[u32; 2]> {
        if level >= self.mip_count() {
            return None;
        }
        let side = |full: u32| full.checked_shr(level).unwrap_or(0).max(1);
        let (columns, rows) = if self.faces() == CUBE_FACES {
            (CROSS_COLUMNS, CROSS_ROWS)
        } else {
            (1, 1)
        };
        let units = self.preview_units(level).max(1);
        let sheet_columns = sheet_columns(units);
        let sheet_rows = units.div_ceil(sheet_columns);
        let sheet_columns = u32::try_from(sheet_columns).ok()?;
        let sheet_rows = u32::try_from(sheet_rows).ok()?;
        Some([
            side(self.width())
                .checked_mul(columns)?
                .checked_mul(sheet_columns)?,
            side(self.height())
                .checked_mul(rows)?
                .checked_mul(sheet_rows)?,
        ])
    }
}

/// Tiles `images` of one size row by row from the top left into a sheet of `columns` columns; the
/// cells without an image are transparent black.
///
/// # Errors
///
/// Returns [`TextureError::SheetMismatch`] when `images` are empty or differ in size or
/// `columns` is zero, and [`TextureError::InvalidHeader`] when the sheet's size overflows.
pub fn tile(images: &[RgbaImage], columns: u32) -> Result<RgbaImage, TextureError> {
    let [first, ..] = images else {
        return Err(TextureError::SheetMismatch);
    };
    let (width, height) = (first.width(), first.height());
    if columns == 0
        || images
            .iter()
            .any(|image| (image.width(), image.height()) != (width, height))
    {
        return Err(TextureError::SheetMismatch);
    }
    let overflow = || TextureError::InvalidHeader("the texture size overflows");
    let count = u64::try_from(images.len()).map_err(|_| overflow())?;
    let columns = u64::from(columns).min(count);
    let rows = count.div_ceil(columns);
    let sheet_width = u64::from(width).checked_mul(columns).ok_or_else(overflow)?;
    let sheet_height = u64::from(height).checked_mul(rows).ok_or_else(overflow)?;
    let sheet_width = u32::try_from(sheet_width).map_err(|_| overflow())?;
    let sheet_height = u32::try_from(sheet_height).map_err(|_| overflow())?;
    let to_usize = |value: u64| usize::try_from(value).map_err(|_| overflow());
    let cell_row = to_usize(u64::from(width))?
        .checked_mul(RGBA)
        .ok_or_else(overflow)?;
    let sheet_row = cell_row
        .checked_mul(to_usize(columns)?)
        .ok_or_else(overflow)?;
    let cell_rows = to_usize(u64::from(height))?;
    let len = sheet_row
        .checked_mul(cell_rows)
        .and_then(|cell_band| cell_band.checked_mul(to_usize(rows).ok()?))
        .ok_or_else(overflow)?;
    let mut pixels = vec![0; len];
    for (index, image) in images.iter().enumerate() {
        let (row, column) = (index / to_usize(columns)?, index % to_usize(columns)?);
        let first_row = row.checked_mul(cell_rows).ok_or_else(overflow)?;
        let start = column.checked_mul(cell_row).ok_or_else(overflow)?;
        let targets = pixels.chunks_exact_mut(sheet_row).skip(first_row);
        for (target, source) in targets.zip(image.pixels().chunks_exact(cell_row)) {
            let cell = target
                .get_mut(start..start.checked_add(cell_row).ok_or_else(overflow)?)
                .ok_or_else(overflow)?;
            cell.copy_from_slice(source);
        }
    }
    RgbaImage::new(sheet_width, sheet_height, pixels)
}
