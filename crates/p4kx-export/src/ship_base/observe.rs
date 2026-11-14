// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutFiles, LoadoutReadObserver, LoadoutResolver};
use p4kx_extraction::{CanonicalReadObserver, DataForge, TreeBuilder};

use super::{CargoReader, ShipReader};

impl<'f> ShipReader<'f> {
    /// Creates a fresh reader that observes resolver and cargo canonical reads.
    #[must_use]
    pub fn with_observed_entities<O>(
        entities: Arc<Entities<'f>>,
        files: &'f dyn LoadoutFiles,
        observer: Arc<O>,
    ) -> Self
    where
        O: LoadoutReadObserver + CanonicalReadObserver + 'f,
    {
        let forge = entities.forge();
        Self {
            resolver: LoadoutResolver::with_observed_entities(entities, observer.clone())
                .with_files(files)
                .stopping_at_docked_vehicles(),
            trees: TreeBuilder::with_read_observer(observer),
            cargo: CargoReader::new(forge),
        }
    }
}

impl<'f> ShipReader<'f> {
    /// Creates a reader over `forge` that reads the vehicle implementations and XML Loadouts from
    /// `files`. Docked vehicles in a Loadout stay in their port, marked, and are not expanded.
    #[must_use]
    pub fn new(forge: &'f DataForge<'f>, files: &'f dyn LoadoutFiles) -> Self {
        Self::with_entities(Arc::new(Entities::new(forge)), files)
    }

    /// Creates a reader like [`new`](Self::new) over the DataForge of `entities`, sharing its
    /// index with every reader built from the same `Arc`, so that readers working in parallel
    /// walk the Records once.
    #[must_use]
    pub fn with_entities(entities: Arc<Entities<'f>>, files: &'f dyn LoadoutFiles) -> Self {
        let forge = entities.forge();
        Self {
            resolver: LoadoutResolver::with_entities(entities)
                .with_files(files)
                .stopping_at_docked_vehicles(),
            trees: TreeBuilder::new(),
            cargo: CargoReader::new(forge),
        }
    }
}
