// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, IniEncoding, InputScope, Language, SourceInput,
    SourceScope, SourceUses,
};

use crate::CatalogError;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::{EntryFrame, HealthTextCapture};
use crate::texts::TextSources;

/// Source scopes captured from the actual successfully read catalog inputs.
pub(crate) struct CatalogOrigins {
    pub(crate) uses: SourceUses,
    pub(crate) dataforge: SourceScope,
    health_texts: HealthTextCapture,
    entry_frame: Arc<EntryFrame>,
    locales: BTreeMap<Language, CatalogLocaleInput>,
}

/// One original locale input and its recorded Entry-decoding contribution.
pub(crate) struct CatalogLocaleInput {
    scope: SourceScope,
    decoded: bool,
}

impl CatalogLocaleInput {
    pub(crate) const fn scope(&self) -> &SourceScope {
        &self.scope
    }

    pub(crate) const fn decoded(&self) -> bool {
        self.decoded
    }
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
                let text = entry.text();
                let decoded = text.is_lossy()
                    || matches!(
                        text.encoding(),
                        IniEncoding::Utf8Bom | IniEncoding::Utf16Le | IniEncoding::Utf16Be
                    );
                locales.insert(
                    language,
                    CatalogLocaleInput {
                        scope: uses.observe(input, InputScope::Payload)?,
                        decoded,
                    },
                );
            }
        }
        let entry_frame = Arc::new(EntryFrame::new(archive, &uses));
        Ok(Self {
            uses,
            entry_frame,
            dataforge,
            locales,
            health_texts: HealthTextCapture::new(),
        })
    }

    pub(crate) const fn record(&self, record_index: u32) -> CatalogRecordContext<'_> {
        CatalogRecordContext::new(&self.uses, &self.dataforge, record_index)
    }

    pub(crate) const fn entry_frame(&self) -> &Arc<EntryFrame> {
        &self.entry_frame
    }

    pub(crate) const fn health_texts(&self) -> &HealthTextCapture {
        &self.health_texts
    }

    pub(crate) fn locale(&self, language: Language) -> Option<&CatalogLocaleInput> {
        self.locales.get(&language)
    }
}
