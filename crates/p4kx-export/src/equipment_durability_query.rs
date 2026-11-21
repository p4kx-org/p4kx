// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_durability::DurabilityComponent;
use crate::catalog_v3::catalog_item_components::{self, ItemEquipmentFamily};
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::catalog_durability::{Degradation, Distortion, Durability};

macro_rules! projector {
    ($name:ident, $source:lifetime, $row:ty, $family:ident, $read:ident) => {
        struct $name;

        impl Projector for $name {
            type Row<$source> = $row;
            const FAMILY: Family = Family::ItemEquipment(ItemEquipmentFamily::Durability(
                DurabilityComponent::$family,
            ));

            fn read<'source>(
                _: &'source DataForge<'source>,
                tree: &RecordTree<'source>,
                projection: &CatalogProjection<'_, 'source>,
                admission: &RecordObservation<'_, 'source>,
                _: &ReadOptions<'_>,
                cancel: Option<&CancelToken>,
            ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
                catalog_item_components::$read(tree, projection, admission, cancel)
            }
        }
    };
}

projector!(HealthRows, 'source, Durability<'source>, Health, read_health);
projector!(DegradationRows, 'source, Degradation, Degradation, read_degradation);
projector!(DistortionRows, 'source, Distortion, Distortion, read_distortion);

record_query::unlocalized!(
    HealthRows,
    Durability,
    /// Physical Health components and their original source evidence.
    DurabilitySnapshot,
    with_durability,
    with_durability_from
);

record_query::unlocalized!(
    owned DegradationRows,
    Degradation,
    /// Physical degradation components and their original source evidence.
    DegradationSnapshot,
    with_degradation,
    with_degradation_from
);

record_query::unlocalized!(
    owned DistortionRows,
    Distortion,
    /// Physical distortion components and their original source evidence.
    DistortionSnapshot,
    with_distortion,
    with_distortion_from
);
