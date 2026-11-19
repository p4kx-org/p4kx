// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::slice;

use p4kx_extraction::{
    IniEncoding, InputScope, Language, LocalizationEntry, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_origins::CatalogLocaleOrigins;
use crate::schema_fields::every_field_required;
use crate::texts::Texts;
use crate::{CatalogError, ExportAccountingError};

/// The document's language selection and fallback policy.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogLanguagePolicy {
    selection: LanguageSelection,
    fallback: LanguageFallback,
}

impl CatalogLanguagePolicy {
    pub(crate) const AVAILABLE_SUPPORTED: Self = Self {
        selection: LanguageSelection::AvailableSupported,
        fallback: LanguageFallback::None,
    };
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum LanguageSelection {
    AvailableSupported,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum LanguageFallback {
    None,
}

/// The observed input and diagnostics of one selected language.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CatalogLanguageSource<'source> {
    path: SourcedValue<&'source str>,
    encoding: SourcedValue<SourceIniEncoding>,
    lossy_decoding: SourcedValue<bool>,
    skipped_malformed_lines: SourcedValue<usize>,
}

impl<'source> CatalogLanguageSource<'source> {
    /// Borrows the original localization Entry path.
    #[must_use]
    pub const fn path(&self) -> &SourcedValue<&'source str> {
        &self.path
    }

    /// Borrows the encoding detected on the original Entry.
    #[must_use]
    pub const fn encoding(&self) -> &SourcedValue<SourceIniEncoding> {
        &self.encoding
    }

    /// Borrows the original Entry's decoding-loss observation.
    #[must_use]
    pub const fn lossy_decoding(&self) -> &SourcedValue<bool> {
        &self.lossy_decoding
    }

    /// Borrows the number of malformed lines skipped during indexing.
    #[must_use]
    pub const fn skipped_malformed_lines(&self) -> &SourcedValue<usize> {
        &self.skipped_malformed_lines
    }
}

/// Encoding observed while decoding a localization Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum SourceIniEncoding {
    /// UTF-8 without a byte-order mark.
    Utf8,
    /// UTF-8 with a byte-order mark.
    Utf8Bom,
    /// Little-endian UTF-16.
    Utf16Le,
    /// Big-endian UTF-16.
    Utf16Be,
    /// An encoding outside the supported encoding vocabulary.
    Unknown,
}

impl From<IniEncoding> for SourceIniEncoding {
    fn from(original: IniEncoding) -> Self {
        match original {
            IniEncoding::Utf8 => Self::Utf8,
            IniEncoding::Utf8Bom => Self::Utf8Bom,
            IniEncoding::Utf16Le => Self::Utf16Le,
            IniEncoding::Utf16Be => Self::Utf16Be,
            _ => Self::Unknown,
        }
    }
}

/// Canonical localization Entry observations keyed by supported language code.
pub type CatalogLanguageSources<'source> =
    BTreeMap<&'static str, SourcedValue<CatalogLanguageSource<'source>>>;

pub(crate) fn language_sources<'source>(
    texts: &Texts<'source>,
    registry: &SourceUses,
    origins: &CatalogLocaleOrigins,
    languages: &[Language],
) -> Result<CatalogLanguageSources<'source>, CatalogError> {
    let mut sources = BTreeMap::new();
    for &language in languages {
        let entry = match (texts.source_entry(language), origins.get(language)) {
            (Some(entry), Some(input)) => {
                let skipped = texts
                    .skipped_lines(language)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                present(registry, input.scope(), entry, skipped)?
            }
            (None, None) => {
                let survey = origins.directory();
                registry.value(
                    SourceValue::Absent,
                    survey.map_or(&[], slice::from_ref),
                    origins.directory_complete(),
                )
            }
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        sources.insert(language.code(), entry);
    }
    Ok(sources)
}

fn present<'source>(
    registry: &SourceUses,
    scope: &SourceScope,
    entry: &'source LocalizationEntry,
    skipped: usize,
) -> Result<SourcedValue<CatalogLanguageSource<'source>>, CatalogError> {
    let complete = entry
        .origin()
        .is_some_and(|origin| origin.ancestors().all(|ancestor| ancestor.is_some()));
    let path = scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        SourceUseRole::Payload,
    )?;
    let encoding = observation(scope, "encoding")?;
    let lossy = observation(scope, "lossyDecoding")?;
    let malformed_lines = observation(scope, "skippedMalformedLines")?;
    let source = CatalogLanguageSource {
        path: registry.value(
            SourceValue::Present(entry.path()),
            slice::from_ref(&path),
            complete,
        ),
        encoding: registry.value(
            SourceValue::Present(entry.text().encoding().into()),
            slice::from_ref(&encoding),
            complete,
        ),
        lossy_decoding: registry.value(
            SourceValue::Present(entry.text().is_lossy()),
            slice::from_ref(&lossy),
            complete,
        ),
        skipped_malformed_lines: registry.value(
            SourceValue::Present(skipped),
            slice::from_ref(&malformed_lines),
            complete,
        ),
    };
    Ok(registry.value(
        SourceValue::Present(source),
        &[path, encoding, lossy, malformed_lines],
        complete,
    ))
}

fn observation(scope: &SourceScope, field: &str) -> Result<SourceUse, CatalogError> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "globalIni".into(),
            indices: Vec::new(),
            field: field.into(),
        },
        SourceUseRole::Conversion,
    )?)
}
