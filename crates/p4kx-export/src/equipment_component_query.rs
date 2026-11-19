// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry};

use super::EquipmentSnapshot;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::component_sources::PhysicalComponent;

macro_rules! equipment_component_query {
    ($family:ty, $row:ident, $snapshot:ident, $with:ident, $from:ident) => {
        /// Authored component rows and the scope of their original evidence.
        pub type $snapshot<'source> = EquipmentSnapshot<'source, $row<'source>>;

        /// Reads the selected physical components from the Archive.
        ///
        /// # Errors
        /// Returns source evidence, Archive-change or cancellation errors.
        pub fn $with<T>(
            archive: &Archive,
            selection: CatalogRecordSelection,
            cancel: Option<&CancelToken>,
            inspect: impl FnOnce($snapshot<'_>) -> T,
        ) -> Result<T, CatalogError> {
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
                .$with(selection, cancel, inspect)
        }

        /// Reads the selected components using an already decoded Entry from this Archive.
        ///
        /// # Errors
        /// Returns query errors or a source-origin mismatch for a foreign Entry.
        pub fn $from<T>(
            archive: &Archive,
            entry: &DataForgeEntry,
            selection: CatalogRecordSelection,
            cancel: Option<&CancelToken>,
            inspect: impl FnOnce($snapshot<'_>) -> T,
        ) -> Result<T, CatalogError> {
            CatalogQuerySession::from_entry(
                archive,
                entry,
                CatalogLanguageSelection::Selected(&[]),
                cancel,
            )?
            .$with(selection, cancel, inspect)
        }

        impl CatalogQuerySession<'_> {
            /// Reads the selected physical components through this Archive session.
            ///
            /// # Errors
            /// Returns query errors, Archive change or cancellation.
            pub fn $with<T>(
                &self,
                selection: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
                inspect: impl FnOnce($snapshot<'_>) -> T,
            ) -> Result<T, CatalogError> {
                self.parsed(cancel)?.$with(selection, cancel, inspect)
            }
        }

        impl ParsedCatalogQueries<'_> {
            /// Reads the selected physical components from this parsed DataForge.
            ///
            /// # Errors
            /// Returns query errors, Archive change or cancellation.
            pub fn $with<T>(
                &self,
                selection: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
                inspect: impl FnOnce($snapshot<'_>) -> T,
            ) -> Result<T, CatalogError> {
                self.project(cancel, |archive, entry, forge| {
                    super::component_query::with_parsed::<$family, T>(
                        archive, entry, forge, selection, cancel, inspect,
                    )
                })
            }
        }
    };
}

pub(super) use equipment_component_query;

pub(super) fn with_parsed<F: PhysicalComponent, T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl for<'source> FnOnce(EquipmentSnapshot<'source, F::Row<'source>>) -> T,
) -> Result<T, CatalogError> {
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let (table, coverage) = F::query(forge, &input, selection, cancel)?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(EquipmentSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        public_names: Vec::new(),
        game_texts: Vec::new(),
        requested_languages: Vec::new(),
        available_languages: Vec::new(),
        localization_notes: Vec::new(),
        source_inventory,
        coverage,
        dependency_coverage: (),
    }))
}
