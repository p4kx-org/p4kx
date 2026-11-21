// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, IniEncoding, InputScope, Language, SourceInput,
    SourceScope, SourceUse, SourceUses,
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
    locales: CatalogLocaleOrigins,
}

#[derive(Default)]
pub(crate) struct CatalogLocaleOrigins {
    scopes: BTreeMap<Language, CatalogLocaleInput>,
    directory: Option<SourceUse>,
    directory_complete: bool,
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
        Self::from_inputs(
            archive,
            entry,
            Some(texts),
            SourceUses::with_compact_coordinates(),
        )
    }

    pub(crate) fn for_archive_records(
        archive: &Archive,
        entry: &DataForgeEntry,
    ) -> Result<Self, CatalogError> {
        Self::for_archive_records_with_uses(archive, entry, SourceUses::with_compact_coordinates())
    }

    pub(crate) fn for_archive_records_with_uses(
        archive: &Archive,
        entry: &DataForgeEntry,
        uses: SourceUses,
    ) -> Result<Self, CatalogError> {
        if !entry.origin().matches_archive(archive) {
            return Err(CatalogError::SourceOriginMismatch);
        }
        Self::from_inputs(archive, entry, None, uses)
    }

    fn from_inputs(
        archive: &Archive,
        entry: &DataForgeEntry,
        texts: Option<&TextSources>,
        uses: SourceUses,
    ) -> Result<Self, CatalogError> {
        if matches!(entry.origin(), DataForgeOrigin::Archive(_))
            && !entry.origin().matches_archive(archive)
        {
            return Err(CatalogError::SourceOriginMismatch);
        }
        let input = match entry.origin() {
            DataForgeOrigin::Archive(origin) => SourceInput::Archive(origin),
            DataForgeOrigin::File(origin) => SourceInput::File(origin),
            _ => SourceInput::Unknown,
        };
        let dataforge = uses.observe(input, InputScope::Payload)?;
        let entry_frame = Arc::new(EntryFrame::new(archive, &uses));
        let mut origins = Self {
            uses,
            entry_frame,
            dataforge,
            locales: CatalogLocaleOrigins::default(),
            health_texts: HealthTextCapture::new(),
        };
        if let Some(texts) = texts {
            origins.attach_texts(archive, texts)?;
        }
        Ok(origins)
    }

    pub(crate) fn attach_texts(
        &mut self,
        archive: &Archive,
        texts: &TextSources,
    ) -> Result<(), CatalogError> {
        if !self.locales.scopes.is_empty() || self.locales.directory.is_some() {
            return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.locales = CatalogLocaleOrigins::read(archive, texts, &self.uses)?;
        Ok(())
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

    pub(crate) const fn locale_origins(&self) -> &CatalogLocaleOrigins {
        &self.locales
    }

    pub(crate) fn locale(&self, language: Language) -> Option<&CatalogLocaleInput> {
        self.locales.get(language)
    }

    pub(crate) const fn locale_directory(&self) -> Option<&SourceUse> {
        self.locales.directory()
    }

    pub(crate) const fn locale_directory_complete(&self) -> bool {
        self.locales.directory_complete()
    }
}

impl CatalogLocaleOrigins {
    pub(crate) fn read(
        archive: &Archive,
        texts: &TextSources,
        uses: &SourceUses,
    ) -> Result<Self, CatalogError> {
        let mut scopes = BTreeMap::new();
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
                scopes.insert(
                    language,
                    CatalogLocaleInput {
                        scope: uses.observe(input, InputScope::Payload)?,
                        decoded,
                    },
                );
            }
        }
        let directory = if scopes.len() < Language::ALL.len() {
            uses.observe_root_directory_paths(archive)?
        } else {
            None
        };
        let directory_complete =
            directory.is_some() && crate::export_archive_health::inventory(archive).is_none();
        Ok(Self {
            scopes,
            directory,
            directory_complete,
        })
    }

    pub(crate) fn get(&self, language: Language) -> Option<&CatalogLocaleInput> {
        self.scopes.get(&language)
    }

    pub(crate) const fn directory(&self) -> Option<&SourceUse> {
        self.directory.as_ref()
    }

    pub(crate) const fn directory_complete(&self) -> bool {
        self.directory_complete
    }
}
