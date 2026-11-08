// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, InputScope, Language, SourceInput, SourceScope,
    SourceUses,
};

use crate::CatalogError;
use crate::resource_value_read::ResourceContext;
use crate::texts::TextSources;

/// Source scopes captured from the actual successfully read catalog inputs.
pub(crate) struct CatalogOrigins {
    pub(crate) uses: SourceUses,
    pub(crate) dataforge: SourceScope,
    locales: BTreeMap<Language, SourceScope>,
}

impl CatalogOrigins {
    pub(crate) fn new(
        archive: &Archive,
        entry: &DataForgeEntry,
        texts: &TextSources,
    ) -> Result<Self, CatalogError> {
        if matches!(entry.origin(), DataForgeOrigin::Archive(_))
            && !entry.origin().matches_archive(archive)
        {
            return Err(CatalogError::SourceOriginMismatch);
        }
        let uses = SourceUses::new();
        let input = match entry.origin() {
            DataForgeOrigin::Archive(origin) => SourceInput::Archive(origin),
            DataForgeOrigin::File(origin) => SourceInput::File(origin),
            _ => SourceInput::Unknown,
        };
        let dataforge = uses.observe(input, InputScope::Payload)?;
        let mut locales = BTreeMap::new();
        for language in Language::ALL {
            if let Some(entry) = texts.entry(language) {
                let input = entry
                    .origin()
                    .map_or(SourceInput::Unknown, SourceInput::Archive);
                locales.insert(language, uses.observe(input, InputScope::Payload)?);
            }
        }
        Ok(Self {
            uses,
            dataforge,
            locales,
        })
    }

    pub(crate) const fn resource(&self, record_index: u32) -> ResourceContext<'_> {
        ResourceContext::new(&self.uses, &self.dataforge, record_index)
    }

    pub(crate) fn locale(&self, language: Language) -> Option<&SourceScope> {
        self.locales.get(&language)
    }
}
