// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{
    BankMediaSourceFields, BankMediaSourceTable, HircObject, MediaStorage, SoundSource,
};
use p4kx_extraction::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

use super::walk_banks::Loaded;
use super::{SoundVariant, SourceKind};

/// The original source evidence for the public terminal fields of a sound variant.
#[derive(Debug)]
#[non_exhaustive]
pub struct TerminalMediaOrigins {
    /// The plug-in selector and, when applicable, the declared media ID.
    pub wem_id: FieldOrigin,
    /// The declared source ID, including sources that have no WEM file.
    pub declared_source_id: FieldOrigin,
    /// The plug-in selector and, when used, the declared storage byte.
    pub source_kind: FieldOrigin,
    /// The original source flags and the bank directory path used for localisation.
    pub localised: FieldOrigin,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct TerminalSourceTable {
    bank: ObservedEntryBinding,
    kind: Range<u64>,
    version: Option<Range<u64>>,
    table: BankMediaSourceTable,
}

impl TerminalSourceTable {
    pub(super) fn capture(
        archive: &Archive,
        owner: &Loaded<'_>,
        object: &HircObject,
    ) -> Option<Self> {
        let table = owner.bank.media_source_fields(object).ok()??;
        let entry = archive.entries().get(owner.soundbank.entry_position())?;
        Some(Self {
            bank: archive.observed_entry(entry)?.binding(),
            kind: decoded_range(object.kind_byte_range()),
            version: owner.bank.version_byte_range().map(decoded_range),
            table,
        })
    }

    fn origins(
        &self,
        archive: &Archive,
        uses: &SourceUses,
        variant: &SoundVariant<'_>,
    ) -> Result<TerminalMediaOrigins, SourceEvidenceError> {
        let entry = self.entry(archive);
        let row = self.table.sources.get(variant.source_ordinal);
        let (Some(entry), Some(row)) = (entry, row) else {
            return Ok(unknown(uses));
        };
        let Some(plugin) = &row.plugin_id else {
            return Ok(unknown(uses));
        };
        let complete = self.version.is_some() && entry.ancestors().all(|item| item.is_some());
        let scope = uses.observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let mut common = self.selectors(&scope)?;
        let (namespace, site) = self.source_coordinates(variant.source_ordinal);
        let word = |field, bytes, role| register(&scope, namespace, field, bytes, site, role);
        common.push(word(
            "pluginId",
            plugin.range.clone(),
            SourceUseRole::Selector,
        )?);

        let mut for_id = common.clone();
        if let Some(id) = &row.source_id {
            for_id.push(word("sourceId", id.range.clone(), SourceUseRole::Payload)?);
        }
        let declared_source_id = uses.origin(&for_id, complete && row.source_id.is_some());
        let wem_id = if variant.wem_id().is_some() {
            declared_source_id.clone()
        } else {
            uses.origin(&common, complete)
        };
        let mut localisation_uses = common.clone();
        let localised_complete = localisation(
            variant.source(),
            row,
            &scope,
            namespace,
            site,
            &mut localisation_uses,
        )?;
        let reads_storage = reads_storage(variant.source_kind());
        if let Some(storage) = row.storage_code.as_ref().filter(|_| reads_storage) {
            common.push(word(
                "storageCode",
                storage.range.clone(),
                SourceUseRole::Conversion,
            )?);
        }
        let kind_complete = complete && (!reads_storage || row.storage_code.is_some());
        Ok(TerminalMediaOrigins {
            wem_id,
            declared_source_id,
            source_kind: uses.origin(&common, kind_complete),
            localised: uses.origin(&localisation_uses, complete && localised_complete),
        })
    }

    const fn source_coordinates(&self, ordinal: usize) -> (&'static str, Option<u64>) {
        if self.table.source_count.is_some() {
            ("bnkMusicTrackSource", Some(ordinal as u64))
        } else {
            ("bnkSoundMediaSource", None)
        }
    }

    fn entry<'archive>(&self, archive: &'archive Archive) -> Option<ObservedEntry<'archive>> {
        let entry =
            archive.observed_entry(archive.entries().get(self.bank.entry_index() as usize)?)?;
        self.bank.is_same_entry(&entry.binding()).then_some(entry)
    }

    fn selectors(&self, scope: &SourceScope) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut selectors = vec![register(
            scope,
            "bnkHircObject",
            "typeCode",
            self.kind.clone(),
            None,
            SourceUseRole::Selector,
        )?];
        if let Some(version) = &self.version {
            selectors.push(register(
                scope,
                "bnkBkhd",
                "version",
                version.clone(),
                None,
                SourceUseRole::Selector,
            )?);
        }
        if let Some(count) = &self.table.source_count {
            selectors.push(register(
                scope,
                "bnkMusicTrack",
                "sourceCount",
                count.range.clone(),
                None,
                SourceUseRole::Selector,
            )?);
        }
        Ok(selectors)
    }
}

impl SoundVariant<'_> {
    /// Captures the HIRC inputs for the WEM ID, source kind and localisation status.
    ///
    /// Missing or unbound proof is marked unknown. Use a path-source resolution to retain it.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot register the observed uses.
    pub fn terminal_media_origins(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<TerminalMediaOrigins, SourceEvidenceError> {
        self.path_sources
            .last()
            .and_then(|path| path.terminal.as_deref())
            .map_or_else(
                || Ok(unknown(uses)),
                |terminal| terminal.origins(archive, uses, self),
            )
    }
}

fn unknown(uses: &SourceUses) -> TerminalMediaOrigins {
    let origin = uses.origin(&[], false);
    TerminalMediaOrigins {
        wem_id: origin.clone(),
        declared_source_id: origin.clone(),
        source_kind: origin.clone(),
        localised: origin,
    }
}

fn localisation(
    source: &SoundSource,
    row: &BankMediaSourceFields,
    scope: &SourceScope,
    namespace: &str,
    ordinal: Option<u64>,
    uses: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    if !source.has_media_file() {
        return Ok(true);
    }
    let Some(storage) = &row.storage_code else {
        return Ok(false);
    };
    uses.push(register(
        scope,
        namespace,
        "storageCode",
        storage.range.clone(),
        ordinal,
        SourceUseRole::Selector,
    )?);
    if matches!(source.storage, MediaStorage::Other(_)) {
        return Ok(true);
    }
    if let Some(flags) = &row.source_bits {
        uses.push(register(
            scope,
            namespace,
            "sourceBits",
            flags.range.clone(),
            ordinal,
            SourceUseRole::Conversion,
        )?);
    }
    if !source.is_language_specific() {
        let directory = scope.for_scope(InputScope::ArchiveDirectoryMetadata);
        uses.push(directory.at(
            SourceLocation::Metadata {
                field: "path".to_owned(),
            },
            SourceUseRole::Conversion,
        )?);
    }
    Ok(row.source_bits.is_some())
}

const fn reads_storage(kind: SourceKind) -> bool {
    match kind {
        SourceKind::Embedded
        | SourceKind::PrefetchStreamed
        | SourceKind::Streamed
        | SourceKind::Other(_) => true,
        SourceKind::Plugin | SourceKind::External | SourceKind::Midi => false,
    }
}

const fn decoded_range(range: Range<usize>) -> Range<u64> {
    range.start as u64..range.end as u64
}

fn register(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    bytes: Range<u64>,
    ordinal: Option<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    let mut indices = vec![bytes.start, bytes.end];
    indices.extend(ordinal);
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices,
            field: field.to_owned(),
        },
        role,
    )
}
