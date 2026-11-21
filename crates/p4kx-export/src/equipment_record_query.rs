// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

macro_rules! unlocalized {
    ($projector:ty, $row:ident, $(#[$doc:meta])+ $snapshot:ident, $with:ident, $from:ident) => {
        $(#[$doc])+
        pub type $snapshot<'source> = super::EquipmentSnapshot<'source, $row<'source>>;
        super::record_query::unlocalized!(@functions $projector, $snapshot, $with, $from);
    };
    (owned $projector:ty, $row:ident, $(#[$doc:meta])+ $snapshot:ident, $with:ident, $from:ident) => {
        $(#[$doc])+
        pub type $snapshot<'source> = super::EquipmentSnapshot<'source, $row>;
        super::record_query::unlocalized!(@functions $projector, $snapshot, $with, $from);
    };
    (@functions $projector:ty, $snapshot:ident, $with:ident, $from:ident) => {

        /// Reads canonical rows by physical owner.
        ///
        /// # Errors
        /// Returns input, source-evidence, Archive-change or cancellation errors.
        pub fn $with<T>(
            archive: &p4kx_extraction::Archive,
            selection: crate::catalog_queries::CatalogRecordSelection,
            cancel: Option<&p4kx_extraction::CancelToken>,
            inspect: impl FnOnce($snapshot<'_>) -> T,
        ) -> Result<T, crate::CatalogError> {
            super::read::with_archive::<$projector, T>(
                archive,
                None,
                selection,
                crate::catalog_queries::CatalogLanguageSelection::Selected(&[]),
                cancel,
                inspect,
            )
        }

        /// Supplies the same rows from an already decoded Entry of this Archive.
        ///
        /// # Errors
        /// Returns query errors or a source-origin mismatch for a foreign Entry.
        pub fn $from<T>(
            archive: &p4kx_extraction::Archive,
            entry: &p4kx_extraction::DataForgeEntry,
            selection: crate::catalog_queries::CatalogRecordSelection,
            cancel: Option<&p4kx_extraction::CancelToken>,
            inspect: impl FnOnce($snapshot<'_>) -> T,
        ) -> Result<T, crate::CatalogError> {
            super::read::with_archive::<$projector, T>(
                archive,
                Some(entry),
                selection,
                crate::catalog_queries::CatalogLanguageSelection::Selected(&[]),
                cancel,
                inspect,
            )
        }

        impl crate::catalog_queries::CatalogQuerySession<'_> {
            /// Reads canonical rows through this Archive session.
            ///
            /// # Errors
            /// Returns canonical query, Archive-change or cancellation errors.
            pub fn $with<T>(
                &self,
                selection: crate::catalog_queries::CatalogRecordSelection,
                cancel: Option<&p4kx_extraction::CancelToken>,
                inspect: impl FnOnce($snapshot<'_>) -> T,
            ) -> Result<T, crate::CatalogError> {
                self.parsed(cancel)?.$with(selection, cancel, inspect)
            }
        }

        impl crate::catalog_queries::ParsedCatalogQueries<'_> {
            /// Reads canonical rows from this parsed DataForge.
            ///
            /// # Errors
            /// Returns canonical query, Archive-change or cancellation errors.
            pub fn $with<T>(
                &self,
                selection: crate::catalog_queries::CatalogRecordSelection,
                cancel: Option<&p4kx_extraction::CancelToken>,
                inspect: impl FnOnce($snapshot<'_>) -> T,
            ) -> Result<T, crate::CatalogError> {
                self.project(cancel, |archive, entry, forge| {
                    super::read::with_parsed::<$projector, T>(
                        archive, entry, forge, selection, &[], cancel, inspect,
                    )
                })
            }
        }
    };
}

pub(super) use unlocalized;
