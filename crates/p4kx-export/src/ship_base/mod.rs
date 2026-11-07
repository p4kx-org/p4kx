// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The base data of ships: hull mass, hull hit points, cargo capacity, size and default Loadout,
//! each value with the file and field it was read from (ADR-0061, `docs/ship-base-data.md`).
//!
//! [`build_ship_base`] reads every ship of an Archive in parallel and returns a
//! [`BuiltShipBase`] that writes as JSON or NDJSON. [`ShipReader`] reads one ship at a time and is
//! the library API for catalog kinds built on the same values.

mod cargo;
mod model;
mod read;
mod select;
mod write;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_channels::PublicPatches;
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Filter, Record};
use rayon::prelude::*;
use serde_json::Value;

use crate::ArchiveKeyUse;
use crate::build::source;
use crate::guid_text::CatalogGuid;
use crate::{CatalogFailure, CatalogPhase, CatalogProgress};

pub(crate) use model::ShipRead;
pub use model::{
    Cargo, CargoGrid, Hull, HullThreshold, Implementation, LoadoutPort, Mass, MassBasis,
    PartialPartValue, ShipBase, Source, SourceKind, SourceShare, Sourced, UnreadablePartValue,
    VehicleKind,
};
pub use read::ShipReader;
pub use write::ShipBaseError;

/// The version of the ship base data format, raised for a breaking change.
pub const SHIP_BASE_VERSION: u32 = 2;

/// The settings of one ship base export.
#[derive(Clone, Copy)]
pub struct ShipBaseOptions<'a> {
    /// Selects only the ships whose record path matches.
    pub filter: Option<&'a Filter>,
    /// The export time the file states.
    pub exported_at: SystemTime,
    /// Stops the export once cancelled.
    pub cancel: Option<&'a CancelToken>,
    /// Called from any worker thread while ships are read; calls may overtake each other.
    pub progress: Option<&'a (dyn Fn(CatalogProgress) + Sync)>,
    /// The public patches the file's source states for the Build; `None` states none.
    pub public_patches: Option<&'a PublicPatches>,
}

impl ShipBaseOptions<'_> {
    /// Returns options for every ship, exported at `exported_at`, without cancellation, progress
    /// or public patches.
    #[must_use]
    pub const fn new(exported_at: SystemTime) -> Self {
        Self {
            filter: None,
            exported_at,
            cancel: None,
            progress: None,
            public_patches: None,
        }
    }
}

impl std::fmt::Debug for ShipBaseOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShipBaseOptions")
            .field("filter", &self.filter)
            .field("exported_at", &self.exported_at)
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("public_patches", &self.public_patches)
            .finish()
    }
}

/// The base data of the ships of one Archive.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltShipBase {
    ships: Vec<ShipBase>,
    failures: Vec<CatalogFailure>,
    skipped: usize,
    source: Value,
    pub(crate) archive_key: ArchiveKeyUse,
    filter: Option<String>,
}

impl BuiltShipBase {
    /// Returns the ships, ordered by the text of their GUID.
    #[must_use]
    pub fn ships(&self) -> &[ShipBase] {
        &self.ships
    }

    /// Returns the ships that could not be read; they are left out.
    #[must_use]
    pub fn failures(&self) -> &[CatalogFailure] {
        &self.failures
    }

    /// Returns how many ship Records were left out as development leftovers.
    #[must_use]
    pub const fn skipped(&self) -> usize {
        self.skipped
    }

    /// Returns whether no ship failed and no Filter narrowed the selection.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.filter.is_none()
    }
}

/// Reads the base data of every ship of `archive`.
///
/// A ship is an Entity Record under `entities/spaceships` or `entities/groundvehicles`, the
/// selection of the catalog's `ships` kind, without the development leftovers
/// ([`EXCLUDED_TOKENS`](crate::EXCLUDED_TOKENS), and for ships also
/// [`SHIP_EXCLUDED_TOKENS`](crate::SHIP_EXCLUDED_TOKENS)). Ships are read in parallel by worker-local
/// [`ShipReader`]s. A ship that cannot be read is left out and listed in
/// [`BuiltShipBase::failures`].
///
/// # Errors
///
/// Returns [`ShipBaseError::Load`] or [`ShipBaseError::DataForge`] when the DataForge cannot be
/// read and [`ShipBaseError::Cancelled`] once the [`CancelToken`] of `options` is cancelled.
pub fn build_ship_base(
    archive: &Archive,
    options: &ShipBaseOptions<'_>,
) -> Result<BuiltShipBase, ShipBaseError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(ShipBaseError::Cancelled);
    }
    let entry = DataForgeEntry::read(archive).map_err(ShipBaseError::Load)?;
    build_ship_base_from(archive, &entry, options)
}

/// Reads the base data of every ship like [`build_ship_base`] from `entry`, the DataForge Entry
/// already read from `archive`.
///
/// # Errors
///
/// As [`build_ship_base`], except that the DataForge Entry is not read here.
pub fn build_ship_base_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &ShipBaseOptions<'_>,
) -> Result<BuiltShipBase, ShipBaseError> {
    let forge = entry.open().map_err(ShipBaseError::DataForge)?;
    let files = ArchiveFiles::new(archive);
    let records: Vec<Record<'_>> = forge.records().collect();
    let selection = select::ships(&records, options);
    let total = selection.ships.len();
    let done = AtomicUsize::new(0);
    let entities = Arc::new(Entities::new(&forge));
    let read: Vec<Option<Result<ShipBase, CatalogFailure>>> = selection
        .ships
        .par_iter()
        .map_init(
            || ShipReader::with_entities(Arc::clone(&entities), &files),
            |reader, record| {
                if options.cancel.is_some_and(CancelToken::is_cancelled) {
                    return None;
                }
                let result = reader.read(record);
                let finished = done.fetch_add(1, Ordering::Relaxed).saturating_add(1);
                if let Some(progress) = options.progress {
                    progress(CatalogProgress {
                        phase: CatalogPhase::Records,
                        records_done: finished,
                        records_total: total,
                    });
                }
                Some(result)
            },
        )
        .collect();
    let mut ships = Vec::with_capacity(read.len());
    let mut failures = selection.failures;
    for result in read {
        match result.ok_or(ShipBaseError::Cancelled)? {
            Ok(ship) => ships.push(ship),
            Err(failure) => failures.push(failure),
        }
    }
    ships.sort_by_key(|ship| CatalogGuid(ship.guid).text());
    failures.sort_by(|a, b| a.record_path.cmp(&b.record_path).then(a.guid.cmp(&b.guid)));
    let archive_key = ArchiveKeyUse::of(archive);
    let source = serde_json::to_value(source(
        archive,
        options.exported_at,
        options.public_patches,
        archive_key,
    ))?;
    Ok(BuiltShipBase {
        ships,
        failures,
        skipped: selection.skipped,
        source,
        archive_key,
        filter: options.filter.map(|filter| filter.as_str().to_owned()),
    })
}
