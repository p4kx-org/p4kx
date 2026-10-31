// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::glb_options::GlbOptions;

/// Which materials get the vertex colours of their mesh as `COLOR_0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Colours {
    /// None.
    Off,
    /// The materials whose colour they are.
    Tints,
    /// Those and the materials whose layer shares they are.
    TintsAndShares,
}

impl Colours {
    /// Returns what `options` ask for.
    pub(crate) const fn of(options: &GlbOptions) -> Self {
        match (options.vertex_colours, options.layer_vertex_colours) {
            (false, _) => Self::Off,
            (true, false) => Self::Tints,
            (true, true) => Self::TintsAndShares,
        }
    }
}
