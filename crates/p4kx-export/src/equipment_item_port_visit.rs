// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::owner_visit::EntityScope;
use super::rows::{Family, Projector, ReadOptions};
use super::{EquipmentSnapshot, EquipmentVisitSummary};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogPublicName, CatalogRecordSelection, ItemPortContainer,
    ItemPortSnapshot,
};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries};
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations;
use crate::record_observation::RecordObservation;

/// Visits original item ports and their evidence one readable physical Record at a time.
/// Commit caller-owned changes only after successful completion and review of its coverage.
///
/// # Errors
/// Returns input, origin, cancellation or caller errors.
pub fn visit_item_ports<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(u32, &ItemPortSnapshot<'_>) -> Result<(), E>,
) -> Result<EquipmentVisitSummary, E> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .visit_item_ports(selection, cancel, visit)
}

/// Visits item ports using a previously decoded entry from the same Archive.
///
/// # Errors
/// Returns the errors of [`visit_item_ports`], or an origin mismatch for a foreign entry.
pub fn visit_item_ports_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(u32, &ItemPortSnapshot<'_>) -> Result<(), E>,
) -> Result<EquipmentVisitSummary, E> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .visit_item_ports(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits item ports using this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`visit_item_ports`].
    pub fn visit_item_ports<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(u32, &ItemPortSnapshot<'_>) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.parsed(cancel)?
            .visit_item_ports(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits item ports using this view's parsed DataForge and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`visit_item_ports`].
    pub fn visit_item_ports<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        mut visit: impl FnMut(u32, &ItemPortSnapshot<'_>) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.visit_equipment_owners::<ItemPorts, E>(
            selection,
            EntityScope::All,
            cancel,
            |index, snapshot| visit(index, &port_snapshot(snapshot)),
        )
    }
}

struct ItemPorts;

impl Projector for ItemPorts {
    type Row<'source> = ItemPortContainer<'source>;

    const FAMILY: Family = Family::ItemPorts;

    fn read<'source>(
        _forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_item_ports_scoped(
            tree,
            projection,
            admission,
            &options.alignment,
            cancel,
        )
    }
}

fn port_snapshot<'source>(
    snapshot: EquipmentSnapshot<'source, ItemPortContainer<'source>>,
) -> ItemPortSnapshot<'source> {
    let EquipmentSnapshot {
        build,
        availability,
        rows,
        public_names,
        requested_languages,
        available_languages,
        localization_notes,
        source_inventory,
        coverage,
        ..
    } = snapshot;
    ItemPortSnapshot {
        build,
        availability,
        rows,
        public_names: public_names
            .into_iter()
            .map(CatalogPublicName::from)
            .collect(),
        requested_languages,
        available_languages,
        localization_notes,
        source_inventory,
        coverage,
    }
}
