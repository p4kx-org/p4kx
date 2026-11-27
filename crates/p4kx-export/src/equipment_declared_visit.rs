// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::owner_visit::EntityScope;
use super::rows::{Family, Projector, ReadOptions};
use super::{EquipmentSnapshot, EquipmentVisitSummary, ItemAttachmentComponent};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection, ItemPortContainer};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries};
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

const OWNER_KINDS: &[Kind] = &[Kind::Ships, Kind::Items];

/// Physical Ship or Item identity with its authored port and attachment declarations.
#[derive(Debug)]
pub struct DeclaredEquipment<'source> {
    source: CanonicalRecordHeader<'source>,
    kind: Kind,
    port_containers: Vec<ItemPortContainer<'source>>,
    attachments: Vec<ItemAttachmentComponent<'source>>,
}

impl<'source> DeclaredEquipment<'source> {
    /// Borrows the original physical Record identity.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Returns the original owner's canonical catalog kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// Borrows authored port containers in component order.
    #[must_use]
    pub fn port_containers(&self) -> &[ItemPortContainer<'source>] {
        &self.port_containers
    }

    /// Borrows authored attachment constraints in component order.
    #[must_use]
    pub fn attachments(&self) -> &[ItemAttachmentComponent<'source>] {
        &self.attachments
    }
}

/// One selected owner's declarations, localization and source inventory.
pub type DeclaredEquipmentSnapshot<'source> =
    EquipmentSnapshot<'source, DeclaredEquipment<'source>>;

/// Visits declared equipment one physical owner at a time.
/// Each callback receives one Ship or Item row, even when it has no declarations.
///
/// # Errors
/// Returns source, cancellation or callback errors.
pub fn visit_declared_equipment<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&DeclaredEquipmentSnapshot<'_>) -> Result<(), E>,
) -> Result<EquipmentVisitSummary, E> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .visit_declared_equipment(selection, cancel, visit)
}

/// Visits declared equipment using a decoded Entry from the same Archive.
///
/// # Errors
/// Returns the errors of [`visit_declared_equipment`] or an origin mismatch.
pub fn visit_declared_equipment_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&DeclaredEquipmentSnapshot<'_>) -> Result<(), E>,
) -> Result<EquipmentVisitSummary, E> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .visit_declared_equipment(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits declared equipment using this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`visit_declared_equipment`].
    pub fn visit_declared_equipment<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&DeclaredEquipmentSnapshot<'_>) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.parsed(cancel)?
            .visit_declared_equipment(selection, cancel, visit)
    }
}

pub(crate) struct DeclaredEquipmentAvailability {
    pub(crate) ports: crate::catalog_queries::DatasetAvailability,
    pub(crate) attachments: crate::catalog_queries::DatasetAvailability,
}

impl ParsedCatalogQueries<'_> {
    pub(crate) fn declared_equipment_availability(
        &self,
        cancel: Option<&CancelToken>,
    ) -> Result<DeclaredEquipmentAvailability, CatalogError> {
        self.project(cancel, |_, _, forge| {
            Ok(DeclaredEquipmentAvailability {
                ports: Family::ItemPorts.availability(forge, 0),
                attachments: Family::Attachments.availability(forge, 0),
            })
        })
    }

    /// Visits declared equipment in physical Record order using this parsed DataForge.
    /// Commit staged consumer changes after successful traversal and coverage review.
    ///
    /// # Errors
    /// Returns the errors of [`visit_declared_equipment`].
    pub fn visit_declared_equipment<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        mut visit: impl FnMut(&DeclaredEquipmentSnapshot<'_>) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.visit_equipment_owners::<Declarations, E>(
            selection,
            EntityScope::Kinds(OWNER_KINDS),
            cancel,
            |_, snapshot| visit(&snapshot),
        )
    }
}

struct Declarations;

impl Projector for Declarations {
    type Row<'source> = DeclaredEquipment<'source>;
    const FAMILY: Family = Family::DeclaredEquipment;

    fn read<'source>(
        _forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let kind = Kind::by_record(tree.record_path(), tree.type_name())
            .filter(|kind| OWNER_KINDS.contains(kind))
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let source = CanonicalRecordHeader::read(tree, projection.original().origins())?;
        let read = catalog_entity_configurations::read_declared_equipment(
            tree,
            projection,
            admission,
            &options.alignment,
            cancel,
        )?;
        Ok(read.map(|rows| {
            let (port_containers, attachments) = rows.into_declarations();
            vec![DeclaredEquipment {
                source,
                kind,
                port_containers,
                attachments,
            }]
        }))
    }
}
