// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, Guid, Record, StructValue};
use p4kx_extraction::{DataForgeEntry, DataForgeOrigin};
use p4kx_model::{TintEntry, TintPalette, srgb8_to_linear};
use thiserror::Error;

use crate::read::{as_struct, struct_field, text, unsigned};

#[path = "palette_source.rs"]
mod source;

pub(crate) use source::{ChosenPalette, material_default_location};
pub use source::{
    PaletteRead, PaletteRecordSelection, PaletteRecords, PaletteSelection, PaletteSource,
};

const TINT_PALETTE_TREE: &str = "TintPaletteTree";
/// The fields of the palette root that hold the colours of the red, green and blue masks of a
/// stencil map.
const DECAL_COLOURS: [&str; 3] = ["decalColorR", "decalColorG", "decalColorB"];

/// Why a [`PaletteProvider`] cannot supply a palette.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum PaletteFailure {
    /// There is no such Record.
    #[error("there is no such palette")]
    Missing,
    /// The Record exists but is no readable tint palette.
    #[error("{reason}")]
    Unreadable {
        /// What is wrong.
        reason: String,
    },
}

impl PaletteFailure {
    /// Returns a stable code for the kind of failure, `missing` or `unreadable`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable { .. } => "unreadable",
        }
    }
}

/// Supplies the tint palettes an Assembly is drawn with.
pub trait PaletteProvider {
    /// Reads the `TintPaletteTree` Record `guid`.
    ///
    /// # Errors
    ///
    /// Returns [`PaletteFailure::Missing`] when there is no such Record and
    /// [`PaletteFailure::Unreadable`] when it is no readable tint palette.
    fn by_guid(&self, guid: Guid) -> Result<TintPalette, PaletteFailure>;

    /// Returns the GUID of the `TintPaletteTree` Record at the record path `path`, as a material
    /// library names its default palette; ignores case and the kind of separator.
    ///
    /// # Errors
    ///
    /// Returns [`PaletteFailure::Missing`] when no tint palette is at the path.
    fn guid_at(&self, path: &str) -> Result<Guid, PaletteFailure>;

    /// Returns the verified DataForge supplying this provider's palette Records.
    fn forge_origin(&self) -> Option<&DataForgeOrigin> {
        None
    }

    /// Reads a palette and its selected DataForge Record coordinate.
    ///
    /// # Errors
    /// As [`Self::by_guid`].
    fn by_guid_observed(&self, guid: Guid) -> Result<PaletteRead<TintPalette>, PaletteFailure> {
        self.by_guid(guid).map(PaletteRead::unobserved)
    }

    /// Selects a palette GUID and the actual Record that supplied it.
    ///
    /// # Errors
    /// As [`Self::guid_at`].
    fn guid_at_observed(&self, path: &str) -> Result<PaletteRead<Guid>, PaletteFailure> {
        self.guid_at(path).map(PaletteRead::unobserved)
    }
}

/// Reads tint palettes from the `TintPaletteTree` Records of a DataForge.
#[derive(Debug, Clone, Copy)]
pub struct ForgePalettes<'f> {
    forge: &'f DataForge<'f>,
    origin: Option<&'f DataForgeOrigin>,
}

impl<'f> ForgePalettes<'f> {
    /// Reads palettes from `forge`.
    #[must_use]
    pub const fn new(forge: &'f DataForge<'f>) -> Self {
        Self {
            forge,
            origin: None,
        }
    }

    /// Binds the provider to an Entry that supplied this exact opened DataForge.
    #[must_use]
    pub fn with_entry(mut self, entry: &'f DataForgeEntry) -> Self {
        self.origin = entry.origin_of(self.forge);
        self
    }
}

impl PaletteProvider for ForgePalettes<'_> {
    fn forge_origin(&self) -> Option<&DataForgeOrigin> {
        self.origin
    }

    fn by_guid(&self, guid: Guid) -> Result<TintPalette, PaletteFailure> {
        self.by_guid_observed(guid).map(|read| read.value)
    }

    fn guid_at(&self, path: &str) -> Result<Guid, PaletteFailure> {
        self.guid_at_observed(path).map(|read| read.value)
    }

    fn by_guid_observed(&self, guid: Guid) -> Result<PaletteRead<TintPalette>, PaletteFailure> {
        let record = self
            .forge
            .record_by_guid(guid)
            .ok_or(PaletteFailure::Missing)?;
        let value = read_record(&record)?;
        Ok(PaletteRead {
            value,
            record: Some(PaletteRecordSelection::of(&record)),
        })
    }

    fn guid_at_observed(&self, path: &str) -> Result<PaletteRead<Guid>, PaletteFailure> {
        self.forge
            .records_at_path(path)
            .find(|record| {
                record
                    .type_name()
                    .is_ok_and(|name| name == TINT_PALETTE_TREE)
            })
            .map(|record| PaletteRead {
                value: record.guid(),
                record: Some(PaletteRecordSelection::of(&record)),
            })
            .ok_or(PaletteFailure::Missing)
    }
}

fn unreadable(reason: impl Into<String>) -> PaletteFailure {
    PaletteFailure::Unreadable {
        reason: reason.into(),
    }
}

fn read_record(record: &Record<'_>) -> Result<TintPalette, PaletteFailure> {
    let type_name = record
        .type_name()
        .map_err(|error| unreadable(error.to_string()))?;
    if type_name != TINT_PALETTE_TREE {
        return Err(unreadable(format!("the Record is a {type_name}")));
    }
    let tree = record
        .tree()
        .map_err(|error| unreadable(error.to_string()))?;
    let root = tree
        .field("root")
        .and_then(as_struct)
        .ok_or_else(|| unreadable("the palette has no root"))?;
    let entry = |name: &str| {
        struct_field(root, name)
            .and_then(read_entry)
            .ok_or_else(|| unreadable(format!("the palette has no readable {name}")))
    };
    let glass = struct_field(root, "glassColor")
        .and_then(read_colour)
        .ok_or_else(|| unreadable("the palette has no readable glassColor"))?;
    let palette = TintPalette::new(
        text(root, "name"),
        [entry("entryA")?, entry("entryB")?, entry("entryC")?],
        glass,
    );
    let decal = DECAL_COLOURS.map(|name| struct_field(root, name).and_then(read_colour));
    Ok(match decal {
        [Some(red), Some(green), Some(blue)] => palette.with_decal([red, green, blue]),
        _ => palette,
    })
}

fn read_entry(entry: &StructValue<'_>) -> Option<TintEntry> {
    let glossiness = unsigned(entry, "glossiness")?;
    Some(TintEntry {
        tint: read_colour(struct_field(entry, "tintColor")?)?,
        specular: read_colour(struct_field(entry, "specColor")?)?,
        glossiness: f32::from(u16::try_from(glossiness).ok()?),
    })
}

fn read_colour(colour: &StructValue<'_>) -> Option<[f32; 3]> {
    let channel = |name: &str| -> Option<f32> {
        let byte = u8::try_from(unsigned(colour, name)?).ok()?;
        Some(srgb8_to_linear(byte))
    };
    Some([channel("r")?, channel("g")?, channel("b")?])
}
