// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::{Guid, Record};
use p4kx_extraction::{DataForgeOrigin, EntryOrigin, SourceLocation};
use p4kx_model::{AuthoredAttributeKind, MaterialLibrary, TintPalette};

/// The physical DataForge Record selected by a palette lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteRecordSelection {
    /// The Record index in the DataForge supplied to the provider.
    pub index: u32,
    /// The GUID of the selected Record.
    pub guid: Guid,
}

impl PaletteRecordSelection {
    pub(super) const fn of(record: &Record<'_>) -> Self {
        Self {
            index: record.index(),
            guid: record.guid(),
        }
    }
}

/// The selected value of a palette lookup and its physical Record coordinate.
#[derive(Debug)]
pub struct PaletteRead<T> {
    /// The value supplied by the palette provider.
    pub value: T,
    /// The Record that supplied the value when the provider can prove it.
    pub record: Option<PaletteRecordSelection>,
}

impl<T> PaletteRead<T> {
    pub(super) const fn unobserved(value: T) -> Self {
        Self {
            value,
            record: None,
        }
    }
}

/// The Records used to select and read one palette.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaletteRecords {
    /// The Record selected by an original palette path lookup.
    pub selector: Option<PaletteRecordSelection>,
    /// The Record whose payload supplied the palette.
    pub palette: Option<PaletteRecordSelection>,
}

/// The authored choice that supplied the tint palette to the placement.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PaletteSelection {
    /// A named palette chosen through the placement's authored geometry.
    Named {
        /// The original Record and field, when the named choice was read with a coordinate.
        location: Option<SourceLocation>,
    },
    /// The original MTL root's default-palette field selected the payload.
    MaterialDefault {
        /// The actual material Entry through which the default was selected.
        library: Option<Arc<EntryOrigin>>,
    },
    /// A SOC inclusion named a palette through the DataForge path lookup.
    IncludedPath,
    /// A SOC entity directly named an original palette by GUID.
    SocEntity,
}

/// The observed selection, Record coordinates and physical source of a rendered palette.
#[derive(Debug, Clone)]
pub struct PaletteSource {
    pub(crate) records: PaletteRecords,
    pub(crate) forge_origin: Option<Arc<DataForgeOrigin>>,
    pub(crate) named_origin: Option<Arc<DataForgeOrigin>>,
    pub(crate) material_default_location: Option<SourceLocation>,
    pub(crate) selection: PaletteSelection,
    pub(crate) ancestor: Option<usize>,
}

impl PaletteSource {
    /// Returns the path-lookup selector and palette payload Record coordinates.
    #[must_use]
    pub const fn records(&self) -> PaletteRecords {
        self.records
    }

    /// Returns the DataForge supplying the observed palette Records.
    #[must_use]
    pub fn forge_origin(&self) -> Option<&DataForgeOrigin> {
        self.forge_origin.as_deref()
    }

    /// Returns the DataForge supplying a named geometry selector, independently of the palette payload.
    #[must_use]
    pub fn named_origin(&self) -> Option<&DataForgeOrigin> {
        self.named_origin.as_deref()
    }

    /// Returns the authored root declaration that supplied the material's default palette path.
    #[must_use]
    pub const fn material_default_location(&self) -> Option<&SourceLocation> {
        self.material_default_location.as_ref()
    }

    /// Returns the authored selection which chose this palette.
    #[must_use]
    pub const fn selection(&self) -> &PaletteSelection {
        &self.selection
    }

    /// Returns the original supplying part when the palette is inherited.
    #[must_use]
    pub const fn inherited_from(&self) -> Option<usize> {
        self.ancestor
    }
}

pub(crate) fn material_default_location(library: &MaterialLibrary) -> Option<SourceLocation> {
    let root = library.authored.root.as_ref()?;
    let position = root.authored.attributes.iter().position(|attribute| {
        attribute.name == "DefaultPalette"
            && matches!(
                attribute.kind,
                AuthoredAttributeKind::Attribute { namespace: None }
            )
            && library.default_palette.as_deref() == Some(attribute.value.as_str())
    })?;
    Some(SourceLocation::Subresource {
        namespace: "materialXmlDeclaration".into(),
        indices: vec![position as u64],
        field: "DefaultPalette".into(),
    })
}

#[derive(Debug, Clone)]
pub(crate) struct ChosenPalette {
    pub(crate) palette: Arc<TintPalette>,
    pub(crate) source: PaletteSource,
}
