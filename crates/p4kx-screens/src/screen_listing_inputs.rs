// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::screen_asset_inputs::ScreenAssetInputs;
use crate::screens::EntityScreens;

/// One original owned value in a physical DataForge Record.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ValueInput {
    /// The owner Record's physical index.
    pub record_index: u32,
    /// The source value's zero-based canonical preorder position.
    pub ordinal: usize,
}

/// Original fields selected when reading the target canvas.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CanvasInput {
    /// The listed canvas was read from the Reference's actual target.
    Read {
        /// The physical Record that supplied the listed canvas.
        record_index: u32,
        /// Whether its original width supplied the result.
        width_read: bool,
        /// Whether its original height supplied the result.
        height_read: bool,
    },
    /// The target could not be read and the listing used the GUID.
    Unreadable,
    /// The actual listing lookup could not be matched to the Reference's admitted target.
    Unproved,
}

/// Original inputs of one screen listing row.
#[derive(Debug)]
#[non_exhaustive]
pub struct ScreenInput {
    /// The physical Record of the resolved Entity supplying its copied metadata.
    pub entity_record: Option<u32>,
    /// The original owned canvas Reference in the scanned Entity Record.
    pub reference: ValueInput,
    /// The physical Record admitted by that original Reference.
    pub target_record: u32,
    /// Whether the selected GUID was stored in the body rather than obtained through a pointer.
    pub stored_guid: bool,
    /// The actual target admitted by the original Reference and read by the listing.
    pub canvas: CanvasInput,
    /// The observed inputs of each canvas, asset and widget group listed below this screen.
    pub assets: ScreenAssetInputs,
    /// Whether the row belongs to the root Entity.
    pub is_root: bool,
}

/// Original inputs of one movie listing row.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MovieInput {
    /// The physical Record of the resolved Entity supplying its copied metadata.
    pub entity_record: Option<u32>,
    /// The original owned moviePath in the scanned Entity Record.
    pub movie_path: ValueInput,
    /// Whether the row belongs to the root Entity.
    pub is_root: bool,
}

/// Logical DataForge inputs selected by the listing in output order.
#[derive(Debug, Default)]
pub struct EntityScreenInputs {
    pub(crate) entity_record: Option<u32>,
    pub(crate) brand_style_record: Option<u32>,
    pub(crate) screens: Vec<ScreenInput>,
    pub(crate) movies: Vec<MovieInput>,
}

impl EntityScreenInputs {
    /// Returns the root Entity's physical Record position.
    #[must_use]
    pub const fn entity_record(&self) -> Option<u32> {
        self.entity_record
    }

    /// Returns the physical HUD style Record chosen by the brand check.
    #[must_use]
    pub const fn brand_style_record(&self) -> Option<u32> {
        self.brand_style_record
    }

    /// Returns the original inputs in emitted screen row order.
    #[must_use]
    pub fn screens(&self) -> &[ScreenInput] {
        &self.screens
    }

    /// Returns the original inputs in emitted movie row order.
    #[must_use]
    pub fn movies(&self) -> &[MovieInput] {
        &self.movies
    }
}

/// An unchanged listing paired with the original logical inputs selected while computing it.
#[derive(Debug)]
pub struct ObservedEntityScreens {
    pub(crate) listing: EntityScreens,
    pub(crate) inputs: EntityScreenInputs,
}

impl ObservedEntityScreens {
    /// Returns the emitted screen listing.
    #[must_use]
    pub const fn listing(&self) -> &EntityScreens {
        &self.listing
    }

    /// Returns the selected DataForge input addresses.
    #[must_use]
    pub const fn inputs(&self) -> &EntityScreenInputs {
        &self.inputs
    }
}
