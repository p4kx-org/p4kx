// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::owner_visit::{EntityScope, EquipmentVisitSummary};
use super::rows::{Attachments, Bounds};
use super::snapshot::{AttachmentConstraintSnapshot, ItemBoundsSnapshot};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

macro_rules! configuration_visitor {
    ($visit:ident, $visit_from:ident, $snapshot:ident, $projector:ident, $label:literal) => {
        #[doc = concat!("Visits physical owners of ", $label, " in Record order.")]
        /// Stage changes until traversal succeeds and its coverage meets the caller's requirements.
        ///
        /// # Errors
        ///
        /// Returns input, Archive-change, cancellation or visitor errors.
        pub fn $visit<E: From<CatalogError>>(
            archive: &Archive,
            selection: CatalogRecordSelection,
            cancel: Option<&CancelToken>,
            visit: impl for<'source> FnMut(u32, &$snapshot<'source>) -> Result<(), E>,
        ) -> Result<EquipmentVisitSummary, E> {
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
                .$visit(selection, cancel, visit)
        }

        #[doc = concat!("Visits ", $label, " from the same Archive's decoded DataForge Entry.")]
        ///
        /// # Errors
        ///
        /// Returns query or visitor errors, including origin mismatch for a foreign Entry.
        pub fn $visit_from<E: From<CatalogError>>(
            archive: &Archive,
            entry: &DataForgeEntry,
            selection: CatalogRecordSelection,
            cancel: Option<&CancelToken>,
            visit: impl for<'source> FnMut(u32, &$snapshot<'source>) -> Result<(), E>,
        ) -> Result<EquipmentVisitSummary, E> {
            CatalogQuerySession::from_entry(
                archive,
                entry,
                CatalogLanguageSelection::Selected(&[]),
                cancel,
            )?
            .$visit(selection, cancel, visit)
        }

        impl CatalogQuerySession<'_> {
            #[doc = concat!("Visits ", $label, " through this Archive session.")]
            ///
            /// # Errors
            ///
            /// Returns query or visitor errors.
            pub fn $visit<E: From<CatalogError>>(
                &self,
                selection: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
                visit: impl for<'source> FnMut(u32, &$snapshot<'source>) -> Result<(), E>,
            ) -> Result<EquipmentVisitSummary, E> {
                self.parsed(cancel)?.$visit(selection, cancel, visit)
            }
        }

        impl ParsedCatalogQueries<'_> {
            #[doc = concat!("Visits ", $label, " using this parsed original DataForge.")]
            ///
            /// # Errors
            ///
            /// Returns query or visitor errors.
            pub fn $visit<E: From<CatalogError>>(
                &self,
                selection: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
                mut visit: impl for<'source> FnMut(u32, &$snapshot<'source>) -> Result<(), E>,
            ) -> Result<EquipmentVisitSummary, E> {
                self.visit_equipment_owners::<$projector, E>(
                    selection,
                    EntityScope::All,
                    cancel,
                    |index, snapshot| visit(index, &snapshot),
                )
            }
        }
    };
}

configuration_visitor!(
    visit_item_bounds,
    visit_item_bounds_from,
    ItemBoundsSnapshot,
    Bounds,
    "item bounds"
);
configuration_visitor!(
    visit_attachment_constraints,
    visit_attachment_constraints_from,
    AttachmentConstraintSnapshot,
    Attachments,
    "attachment constraints"
);
