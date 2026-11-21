// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, SourceUses};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::load_error;

pub(super) struct LazyLooseRecords<'source> {
    archive: &'source Archive,
    sources: &'source SourceUses,
    cancel: Option<&'source CancelToken>,
    entry: &'source OnceCell<DataForgeEntry>,
    records: OnceCell<LooseRecords<'source>>,
}

struct LooseRecords<'source> {
    forge: DataForge<'source>,
    origins: CatalogOrigins,
}

impl<'source> LazyLooseRecords<'source> {
    pub(super) const fn new(
        archive: &'source Archive,
        sources: &'source SourceUses,
        cancel: Option<&'source CancelToken>,
        entry: &'source OnceCell<DataForgeEntry>,
    ) -> Self {
        Self {
            archive,
            sources,
            cancel,
            entry,
            records: OnceCell::new(),
        }
    }

    pub(super) fn with_input<T>(
        &self,
        project: impl FnOnce(&OriginalCatalogInput<'_, '_>) -> Result<T, CatalogError>,
    ) -> Result<T, CatalogError> {
        let records = if let Some(records) = self.records.get() {
            records
        } else {
            let entry = DataForgeEntry::read_until(self.archive, &|| {
                self.cancel.is_some_and(CancelToken::is_cancelled)
            })
            .map_err(load_error)?;
            let entry = self.entry.get_or_init(|| entry);
            let forge = entry.open().map_err(CatalogError::DataForge)?;
            let origins = CatalogOrigins::for_archive_records_with_uses(
                self.archive,
                entry,
                self.sources.clone(),
            )?;
            self.records.get_or_init(|| LooseRecords { forge, origins })
        };
        project(&OriginalCatalogInput::new(&records.forge, &records.origins))
    }
}
