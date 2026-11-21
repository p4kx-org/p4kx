// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree};

use super::projectile_ammo_read;
use super::{AmmoBinding, AmmoContainer, WeaponMagazineOutcome, WeaponMagazineResolution};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_item_components::{self, resolution::MagazineDependencies};
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;

/// The two canonical rows of an admitted off-owner magazine container.
#[derive(Debug)]
#[non_exhaustive]
pub struct LinkedMagazineSources<'source> {
    /// The authored container configuration.
    pub container: AmmoContainer<'source>,
    /// The authored ammunition slot binding of the same physical component.
    pub binding: AmmoBinding<'source>,
}

/// The observed primary resolution and out-of-selection magazine rows of one weapon.
#[derive(Debug)]
#[non_exhaustive]
pub struct WeaponMagazineDependencyOwner<'source> {
    /// The original weapon owner and observed primary magazine route.
    pub resolution: WeaponMagazineResolution,
    /// Admitted out-of-selection container and binding pairs in physical slot order.
    pub dependencies: Vec<LinkedMagazineSources<'source>>,
    /// Body and field completeness of the reached magazine.
    pub dependency_coverage: CatalogRecordCoverage,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    filter: Option<&Filter>,
) -> Result<ScopedSourceRead<Vec<WeaponMagazineDependencyOwner<'source>>>, CatalogError> {
    let retained = MagazineDependencies::new(filter);
    let mut read = catalog_item_components::read_magazine_resolution(
        forge,
        tree,
        projection,
        admission,
        cancel,
        Some(&retained),
    )?;
    let mut ledger = ExportLedger::default();
    let dependencies = retained
        .take(&mut ledger)?
        .map(|row| LinkedMagazineSources {
            container: row.container,
            binding: row.binding,
        })
        .collect();
    let mut coverage = projectile_ammo_read::magazine_coverage(ledger)?;
    if let Some(outcome) = read.value.as_ref().and_then(|row| row.outcome().value()) {
        match outcome {
            WeaponMagazineOutcome::LinkedTargetUnreadable {
                magazine_record_index,
                ..
            } => {
                coverage
                    .unreadable_bodies
                    .extend(magazine_record_index.value().copied());
            }
            WeaponMagazineOutcome::LinkedContainerUndetermined {
                magazine_record_index,
            } => {
                coverage
                    .incomplete_fields
                    .extend(magazine_record_index.value().copied());
            }
            _ => {}
        }
    }
    coverage.incomplete_fields.sort_unstable();
    coverage.incomplete_fields.dedup();
    read.selection_complete &=
        coverage.unreadable_bodies.is_empty() && coverage.incomplete_fields.is_empty();
    Ok(read.map(|route| {
        route
            .map(|resolution| WeaponMagazineDependencyOwner {
                resolution,
                dependencies,
                dependency_coverage: coverage,
            })
            .into_iter()
            .collect()
    }))
}
