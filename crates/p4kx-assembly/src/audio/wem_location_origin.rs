// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_extraction::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

use super::{AudioIndex, SoundVariant, WemLocation};

const WORD_BYTES: u64 = size_of::<u32>() as u64;

/// The original metadata used to report the selected WEM location.
#[derive(Debug)]
#[non_exhaustive]
pub struct WemLocationOrigins {
    /// The evidence for the physical location kind.
    pub kind: FieldOrigin,
    /// The original Archive directory path.
    pub path: FieldOrigin,
    /// The stored language folder and its indexed spelling.
    pub language: FieldOrigin,
    /// The original directory size or DIDX length.
    pub len: FieldOrigin,
    /// The original DIDX offset when an embedded location was selected.
    pub offset: Option<FieldOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WemLocationSource {
    entry: ObservedEntryBinding,
    placement: Placement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Placement {
    Embedded { triplet: u64, data_body: u64 },
    Loose { language: LooseLanguage },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LooseLanguage {
    SelectedPath,
    Canonical(ObservedEntryBinding),
    Unknown,
}

impl WemLocationSource {
    pub(super) fn capture(
        index: &AudioIndex,
        archive: &Archive,
        location: &WemLocation<'_>,
        triplet: Option<u64>,
    ) -> Option<Self> {
        let (position, placement) = match *location {
            WemLocation::Soundbank { soundbank, .. } => (
                soundbank.entry_position(),
                Placement::Embedded {
                    triplet: triplet?,
                    data_body: soundbank.media_data_body?,
                },
            ),
            WemLocation::Entry {
                entry, language, ..
            } => (
                entry as usize,
                Placement::Loose {
                    language: LooseLanguage::capture(index, archive, entry, language),
                },
            ),
        };
        Some(Self {
            entry: index.indexed_entry(archive, position)?.binding(),
            placement,
        })
    }

    fn origins(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<WemLocationOrigins, SourceEvidenceError> {
        let embedded = matches!(self.placement, Placement::Embedded { .. });
        let Some(entry) = entry(archive, &self.entry) else {
            return Ok(WemLocationOrigins::unknown(uses, embedded));
        };
        let has_ancestry = !self.entry.has_unknown_ancestry();
        let directory = uses.observe(
            SourceInput::ArchiveView(entry),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let path = metadata(&directory, "path", SourceUseRole::Payload)?;
        let path = uses.origin(&[path], has_ancestry);
        let language_path = metadata(&directory, "path", SourceUseRole::Conversion)?;
        let (mut language_uses, mut complete) = (vec![language_path], has_ancestry);
        if let Placement::Loose { language } = &self.placement {
            complete &= language.add_spelling(archive, uses, &mut language_uses)?;
        }
        let language = uses.origin(&language_uses, complete);
        let placement = self.placement.origins(&directory, uses, has_ancestry)?;
        Ok(WemLocationOrigins {
            kind: placement.kind,
            path,
            language,
            len: placement.len,
            offset: placement.offset,
        })
    }
}

struct PlacementOrigins {
    kind: FieldOrigin,
    len: FieldOrigin,
    offset: Option<FieldOrigin>,
}

impl Placement {
    fn origins(
        &self,
        directory: &SourceScope,
        uses: &SourceUses,
        complete: bool,
    ) -> Result<PlacementOrigins, SourceEvidenceError> {
        let Self::Embedded { triplet, data_body } = *self else {
            let kind = metadata(directory, "path", SourceUseRole::Selector)?;
            let len = metadata(directory, "uncompressed_size", SourceUseRole::Payload)?;
            return Ok(PlacementOrigins {
                kind: uses.origin(&[kind], complete),
                len: uses.origin(&[len], complete),
                offset: None,
            });
        };
        let payload = directory.for_scope(InputScope::Payload);
        let header = data_body - 2 * WORD_BYTES;
        let mut selectors = vec![word(
            &payload,
            "bnkDidx",
            "mediaId",
            triplet,
            SourceUseRole::Selector,
        )?];
        for (ordinal, field) in ["tag", "bodyLength"].into_iter().enumerate() {
            selectors.push(word(
                &payload,
                "bnkSection",
                field,
                header + ordinal as u64 * WORD_BYTES,
                SourceUseRole::Selector,
            )?);
        }
        let offset = word(
            &payload,
            "bnkDidx",
            "dataOffset",
            triplet + WORD_BYTES,
            SourceUseRole::Payload,
        )?;
        let len = word(
            &payload,
            "bnkDidx",
            "dataLength",
            triplet + 2 * WORD_BYTES,
            SourceUseRole::Payload,
        )?;
        Ok(PlacementOrigins {
            kind: uses.origin(&selectors, complete),
            len: uses.origin(&[len], complete),
            offset: Some(uses.origin(&[offset], complete)),
        })
    }
}

impl LooseLanguage {
    fn capture(
        index: &AudioIndex,
        archive: &Archive,
        position: u32,
        language: Option<&str>,
    ) -> Self {
        let Some(language) = language else {
            return Self::SelectedPath;
        };
        match index.language_source(archive, language) {
            Some(spelling) if spelling.entry_index() == position => Self::SelectedPath,
            Some(spelling) => Self::Canonical(spelling.binding()),
            None => Self::Unknown,
        }
    }

    fn add_spelling(
        &self,
        archive: &Archive,
        uses: &SourceUses,
        selected: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        match self {
            Self::SelectedPath => Ok(true),
            Self::Unknown => Ok(false),
            Self::Canonical(binding) => {
                let Some(spelling) = entry(archive, binding) else {
                    return Ok(false);
                };
                let scope = uses.observe(
                    SourceInput::ArchiveView(spelling),
                    InputScope::ArchiveDirectoryMetadata,
                )?;
                selected.push(metadata(&scope, "path", SourceUseRole::Conversion)?);
                Ok(!binding.has_unknown_ancestry())
            }
        }
    }
}

impl WemLocationOrigins {
    fn unknown(uses: &SourceUses, offset: bool) -> Self {
        let unknown = uses.origin(&[], false);
        Self {
            kind: unknown.clone(),
            path: unknown.clone(),
            language: unknown.clone(),
            len: unknown.clone(),
            offset: offset.then_some(unknown),
        }
    }
}

impl SoundVariant<'_> {
    /// Returns the exact physical Entry holding the selected WEM.
    #[must_use]
    pub fn wem_location_entry<'archive>(
        &self,
        archive: &'archive Archive,
    ) -> Option<ObservedEntry<'archive>> {
        entry(archive, &self.location_source.as_ref()?.entry)
    }

    /// Returns the original selected WEM metadata, including each bank-relative DIDX word.
    ///
    /// Use a path-source resolution to retain evidence. An unproved location is unknown.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot register an observed input.
    pub fn wem_location_origins(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<WemLocationOrigins, SourceEvidenceError> {
        self.location_source.as_ref().map_or_else(
            || {
                Ok(WemLocationOrigins::unknown(
                    uses,
                    matches!(self.location, Some(WemLocation::Soundbank { .. })),
                ))
            },
            |source| source.origins(archive, uses),
        )
    }
}

fn entry<'archive>(
    archive: &'archive Archive,
    binding: &ObservedEntryBinding,
) -> Option<ObservedEntry<'archive>> {
    let entry = archive.observed_entry(archive.entries().get(binding.entry_index() as usize)?)?;
    binding.is_same_entry(&entry.binding()).then_some(entry)
}

fn metadata(
    scope: &SourceScope,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Metadata {
            field: field.to_owned(),
        },
        role,
    )
}

fn word(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    start: u64,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices: vec![start, start + WORD_BYTES],
            field: field.to_owned(),
        },
        role,
    )
}
