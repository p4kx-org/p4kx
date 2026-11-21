// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::fit::Fit;
use super::layout::Cell;
use super::spec::{Horizontal, Vertical};
use crate::paint::Area;
use crate::still_inputs::StillTextInput;

#[derive(Debug)]
pub(crate) struct Prepared {
    pub(super) font: usize,
    pub(super) size: f32,
    pub(super) cells: Vec<Cell>,
    pub(super) line_spacing: f32,
    pub(super) horizontal: Horizontal,
    pub(super) vertical: Vertical,
    pub(super) wrap: bool,
    pub(super) height_input: Option<f32>,
    pub(super) fit: Option<Fit>,
    pub(crate) input: Option<StillTextInput>,
}

impl Prepared {
    /// Returns whether shaping the text again in `area` would give this result: the height is
    /// the one a size taken from the height was counted from, and the width still holds a fitted
    /// text.
    pub(super) fn holds_for(&self, area: Area) -> bool {
        let height = self
            .height_input
            .is_none_or(|height| height.to_bits() == area.height.to_bits());
        let width = self.fit.as_ref().is_none_or(|fit| {
            fit.width.to_bits() == area.width.to_bits() || (!fit.shrunk && fit.widest <= area.width)
        });
        height && width
    }
}
