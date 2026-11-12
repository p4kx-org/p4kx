// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, PortDefinitionPosition, PortNode, PortSource};

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
    /// The original attachment route, absent for the root Entity.
    pub attachment_route: Option<usize>,
}

/// Original inputs of one movie listing row.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MovieInput {
    /// The physical Record of the resolved Entity supplying its copied metadata.
    pub entity_record: Option<u32>,
    /// The original owned moviePath in the scanned Entity Record.
    pub movie_path: ValueInput,
    /// The original attachment route, absent for the root Entity.
    pub attachment_route: Option<usize>,
}

/// Logical DataForge inputs selected by the listing in output order.
#[derive(Debug, Default)]
pub struct EntityScreenInputs {
    pub(crate) entity_record: Option<u32>,
    pub(crate) brand_style_record: Option<u32>,
    pub(crate) screens: Vec<ScreenInput>,
    pub(crate) movies: Vec<MovieInput>,
    pub(crate) attachment_routes: Vec<AttachmentInput>,
}

/// The original proven port definitions used by one attachment path.
#[derive(Debug)]
pub struct AttachmentInput {
    ports: Vec<PortDefinitionPosition>,
    complete: bool,
}

impl AttachmentInput {
    pub(crate) fn read(route: &[&PortNode]) -> Self {
        let ports: Vec<_> = route.iter().filter_map(|port| default_port(port)).collect();
        Self {
            complete: route.len() == ports.len(),
            ports,
        }
    }

    /// Returns the known original port definitions in path order.
    #[must_use]
    pub fn ports(&self) -> &[PortDefinitionPosition] {
        &self.ports
    }

    /// Returns whether every segment retained its name and target source.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

fn default_port(port: &PortNode) -> Option<PortDefinitionPosition> {
    let Attachment::Entity(child) = port.attachment() else {
        return None;
    };
    if port.source() != PortSource::PortDefault || child.attachment_target()?.guid() != child.guid()
    {
        return None;
    }
    Some(port.definition()?.position())
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

    /// Returns the actual attachment routes used by emitted rows.
    #[must_use]
    pub fn attachment_routes(&self) -> &[AttachmentInput] {
        &self.attachment_routes
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
