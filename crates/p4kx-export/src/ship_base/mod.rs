// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The base data of ships: hull mass, hull hit points, cargo capacity, size and default Loadout,
//! each value with the file and field it was read from (ADR-0061, `docs/ship-base-data.md`).
//!
//! [`build_ship_base`] reads every ship of an Archive in parallel and returns a
//! [`BuiltShipBase`] that writes as JSON or NDJSON. [`ShipReader`] reads one ship at a time and is
//! the library API for catalog kinds built on the same values.

mod cargo;
mod evidence;
mod model;
mod read;
mod select;
mod write;

use std::time::SystemTime;

use p4kx_channels::PublicPatches;
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, DataForgeLoadError, Filter, Record, SourceInventory,
    SourceInventorySelection,
};
use serde_json::Value;

use crate::ArchiveKeyUse;
use crate::build::source;
use crate::guid_text::CatalogGuid;
use crate::{CatalogFailure, CatalogProgress};

pub(crate) use cargo::Observation as ShipCargoObservation;
pub use cargo::{ShipCargoMountSource, ShipCargoSource};
pub use evidence::ShipBaseEvidence;
pub(crate) use model::ShipRead;
pub use model::{
    Cargo, CargoGrid, Hull, HullThreshold, Implementation, LoadoutPort, Mass, MassBasis,
    PartialPartValue, ShipBase, Source, SourceKind, SourceShare, Sourced, UnreadablePartValue,
    VehicleKind,
};
pub use read::ShipReader;
pub(crate) use read::VehicleKindPath;
#[cfg(test)]
pub(crate) use read::VehicleMeasurements;
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
    evidence: Vec<ShipBaseEvidence>,
    source_inventory: SourceInventory,
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

    /// Iterates each written row with its physical identity and immediate field origins.
    pub fn rows_with_evidence(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ShipBase, &ShipBaseEvidence)> {
        self.ships.iter().zip(&self.evidence)
    }

    /// Selects the original source-use closure of all emitted vehicle rows.
    ///
    /// # Errors
    /// Returns an error if an emitted source reference cannot be verified.
    pub fn provenance(&self) -> Result<SourceInventorySelection<'_>, ShipBaseError> {
        let references = self.evidence.iter().flat_map(ShipBaseEvidence::references);
        Ok(self.source_inventory.select_uses(references)?)
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
    let cancel = options.cancel;
    let entry =
        DataForgeEntry::read_until(archive, &|| cancel.is_some_and(CancelToken::is_cancelled))
            .map_err(|error| match error {
                DataForgeLoadError::Cancelled => ShipBaseError::Cancelled,
                other => ShipBaseError::Load(other),
            })?;
    build_ship_base_from(archive, &entry, options)
}

/// Reads the base data of every ship like [`build_ship_base`] from `entry`, the DataForge Entry
/// already read from `archive`.
///
/// # Errors
///
/// As [`build_ship_base`], except that the DataForge Entry is not read here.
/// An Entry bound to another original Archive is rejected.
pub fn build_ship_base_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &ShipBaseOptions<'_>,
) -> Result<BuiltShipBase, ShipBaseError> {
    let forge = entry.open().map_err(ShipBaseError::DataForge)?;
    let records: Vec<Record<'_>> = forge.records().collect();
    let selection = select::ships(&records, options);
    let observed =
        crate::build::standalone_ships::read(archive, entry, &forge, &selection.ships, options)?;
    let mut rows = observed.rows;
    rows.sort_by_key(|(ship, _)| CatalogGuid(ship.guid).text());
    let (ships, evidence) = rows.into_iter().unzip();
    let mut failures = selection.failures;
    failures.extend(observed.failures);
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
        evidence,
        source_inventory: observed.provenance,
        failures,
        skipped: selection.skipped,
        source,
        archive_key,
        filter: options.filter.map(|filter| filter.as_str().to_owned()),
    })
}
