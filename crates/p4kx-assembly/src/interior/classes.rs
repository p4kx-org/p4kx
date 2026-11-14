// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use p4kx_dataforge::{DataForge, Guid};
use p4kx_extraction::DataForgeEntry;

use crate::entities::is_entity;
use crate::spec::read_geometry;
use crate::{ForgePalettes, Geometry, PaletteProvider};

type Geometries = HashMap<Guid, Option<Arc<Geometry>>>;

/// What an Interior asks of the DataForge: the geometry a class Record names and the tint
/// palettes.
pub(super) trait ClassLookup {
    fn geometry(&self, guid: Guid) -> Option<Arc<Geometry>>;

    fn palettes(&self) -> &dyn PaletteProvider;
}

/// The DataForge an [`Interior`](crate::Interior) looks the class Records of its entities up in.
///
/// With it, entities that name only a class Record (`EntityClassGUID`), such as doors, panels and
/// cabinets, are placed as props with the geometry, material library and tint palette of that
/// Record, and the tint palettes the `.soc` file chooses are applied. It remembers the geometry
/// of each Record it read, so the containers of an Entity that share it read it once.
#[derive(Debug)]
pub struct InteriorForge<'f> {
    forge: &'f DataForge<'f>,
    palettes: ForgePalettes<'f>,
    geometries: Mutex<Geometries>,
}

impl<'f> InteriorForge<'f> {
    /// Looks the class Records up in `forge`.
    #[must_use]
    pub fn new(forge: &'f DataForge<'f>) -> Self {
        Self {
            forge,
            palettes: ForgePalettes::new(forge),
            geometries: Mutex::new(HashMap::new()),
        }
    }

    /// Binds palette reads to the Entry that supplied this exact DataForge.
    #[must_use]
    pub fn with_entry(mut self, entry: &'f DataForgeEntry) -> Self {
        self.palettes = self.palettes.with_entry(entry);
        self
    }

    fn read(&self, guid: Guid) -> Option<Arc<Geometry>> {
        let record = self.forge.record_by_guid(guid)?;
        if !is_entity(&record) {
            return None;
        }
        read_geometry(&record.tree().ok()?).map(Arc::new)
    }
}

impl ClassLookup for InteriorForge<'_> {
    fn geometry(&self, guid: Guid) -> Option<Arc<Geometry>> {
        let Ok(mut known) = self.geometries.lock() else {
            return self.read(guid);
        };
        known.entry(guid).or_insert_with(|| self.read(guid)).clone()
    }

    fn palettes(&self) -> &dyn PaletteProvider {
        &self.palettes
    }
}
