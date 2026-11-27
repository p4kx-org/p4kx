// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_gfx::{RasterOptions, RgbaImage};

use super::Composed;

impl Composed {
    /// Returns the tracked glyph range and actual lookup key of a widget's painted text.
    pub(crate) fn text_items_in(
        &self,
        owner: &Range<usize>,
    ) -> Option<(&Range<usize>, Option<&str>)> {
        self.inputs.text_items.iter().find_map(|(range, input)| {
            (range.start >= owner.start && range.end <= owner.end)
                .then_some((range, input.localization_key.as_deref()))
        })
    }

    pub(crate) fn rasterize_without_items(
        &self,
        options: &RasterOptions,
        removed: Range<usize>,
    ) -> Result<RgbaImage, &'static str> {
        if self.scene.items.get(removed.clone()).is_none() {
            return Err("the traced item range is outside the scene");
        }
        let mut scene = self.scene.clone();
        drop(scene.items.drain(removed));
        scene
            .rasterize(&self.bitmaps, options)
            .map(|raster| raster.image)
            .map_err(|_| "cannot rasterize the scene without the selected items")
    }
}
