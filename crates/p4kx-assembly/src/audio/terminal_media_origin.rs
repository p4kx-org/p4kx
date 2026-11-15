// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_archive::{Archive, ObservedEntryBinding};
use p4kx_audio::{BankMediaSourceTable, HircObject};
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
    /// The plug-in selector and, when used, the declared storage byte.
    pub source_kind: FieldOrigin,
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
        let entry = archive
            .entries()
            .get(self.bank.entry_index() as usize)
            .and_then(|entry| archive.observed_entry(entry))
            .filter(|entry| self.bank.is_same_entry(&entry.binding()));
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
        let site = self
            .table
            .source_count
            .as_ref()
            .map(|_| variant.source_ordinal as u64);
        let namespace = if site.is_some() {
            "bnkMusicTrackSource"
        } else {
            "bnkSoundMediaSource"
        };
        let word = |field, bytes, role| register(&scope, namespace, field, bytes, site, role);
        common.push(word(
            "pluginId",
            plugin.range.clone(),
            SourceUseRole::Selector,
        )?);

        let mut for_id = common.clone();
        let id_complete = match (variant.wem_id(), &row.source_id) {
            (Some(_), Some(id)) => {
                for_id.push(word("sourceId", id.range.clone(), SourceUseRole::Payload)?);
                complete
            }
            (Some(_), None) => false,
            (None, _) => complete,
        };
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
            wem_id: uses.origin(&for_id, id_complete),
            source_kind: uses.origin(&common, kind_complete),
        })
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
    /// Captures the HIRC inputs that produced the WEM ID and source kind of this variant.
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
        source_kind: origin,
    }
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
