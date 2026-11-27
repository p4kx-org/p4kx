// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForgeEntry, ProvenanceMode, SourceUses};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::texts::TextSources;

#[derive(Clone, Copy)]
pub(crate) struct CatalogQuerySource<'query> {
    entry: &'query DataForgeEntry,
    provenance_mode: ProvenanceMode,
}

impl<'query> CatalogQuerySource<'query> {
    pub(crate) const fn new(
        entry: &'query DataForgeEntry,
        provenance_mode: ProvenanceMode,
    ) -> Self {
        Self {
            entry,
            provenance_mode,
        }
    }

    pub(crate) const fn entry(self) -> &'query DataForgeEntry {
        self.entry
    }

    pub(crate) const fn provenance_mode(self) -> ProvenanceMode {
        self.provenance_mode
    }

    pub(crate) fn registry(self) -> SourceUses {
        SourceUses::with_provenance_mode(self.provenance_mode)
    }

    pub(crate) fn records(self, archive: &Archive) -> Result<CatalogOrigins, CatalogError> {
        CatalogOrigins::for_archive_records_with_provenance_mode(
            archive,
            self.entry,
            self.provenance_mode,
        )
    }

    pub(crate) fn with_texts(
        self,
        archive: &Archive,
        texts: &TextSources,
    ) -> Result<CatalogOrigins, CatalogError> {
        CatalogOrigins::new_with_provenance_mode(archive, self.entry, texts, self.provenance_mode)
    }
}
