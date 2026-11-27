// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::owner_visit::EntityScope;
use super::rows::{Family, Projector, ReadOptions};
use super::weapon_combat_read::{self, CombinedWeaponOwner};
use super::{EquipmentSnapshot, EquipmentVisitSummary};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::ParsedCatalogQueries;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::record_observation::RecordObservation;

impl ParsedCatalogQueries<'_> {
    pub(crate) fn visit_combined_weapons<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(
            u32,
            EquipmentSnapshot<'source, CombinedWeaponOwner<'source>>,
        ) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.visit_equipment_owners::<CombinedWeapons, E>(
            selection,
            EntityScope::Items(None),
            cancel,
            visit,
        )
    }
}

struct CombinedWeapons;

impl Projector for CombinedWeapons {
    type Row<'source> = CombinedWeaponOwner<'source>;
    const FAMILY: Family = Family::ProjectileAmmo;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        weapon_combat_read::read_combined(forge, tree, projection, admission, cancel)
    }
}
