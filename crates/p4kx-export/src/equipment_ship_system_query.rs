// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;

use crate::catalog_v3::catalog_item_components::{self, ItemEquipmentFamily};
use crate::record_observation::RecordObservation;

pub use super::ComponentOwner;
pub use crate::catalog_v3::catalog_quantum_drive::{
    QuantumBoost, QuantumDrive, QuantumHeat, QuantumProfile,
};
pub use crate::catalog_v3::catalog_shield_generator::{ShieldGenerator, ShieldRange, ShieldStun};

struct ShieldGeneratorRows;

impl Projector for ShieldGeneratorRows {
    type Row<'source> = ShieldGenerator<'source>;
    const FAMILY: Family = Family::ItemEquipment(ItemEquipmentFamily::ShieldGenerators);

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_item_components::read_shield_generators(tree, projection, admission, cancel)
    }
}

struct QuantumDriveRows;

impl Projector for QuantumDriveRows {
    type Row<'source> = QuantumDrive<'source>;
    const FAMILY: Family = Family::ItemEquipment(ItemEquipmentFamily::QuantumDrives);

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_item_components::read_quantum_drives(tree, projection, admission, cancel)
    }
}

record_query::unlocalized!(
    ShieldGeneratorRows,
    ShieldGenerator,
    /// Authored shield generator components and their per-value provenance.
    ShieldGeneratorSnapshot,
    with_shield_generators,
    with_shield_generators_from
);

record_query::unlocalized!(
    QuantumDriveRows,
    QuantumDrive,
    /// Authored quantum drive components and their per-value provenance.
    QuantumDriveSnapshot,
    with_quantum_drives,
    with_quantum_drives_from
);
