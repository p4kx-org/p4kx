// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_dataforge::{
    DataForge, Guid, ObservedResolution, PositionedRecord, RecordJsonBytes, RecordJsonSource,
    RecordJsonUse,
};

use crate::origin_companion;
use crate::resolved_texts::{LocalizationLookupEvidence, RecordLocalization};
use crate::{
    DataForgeEntry, DataForgeOrigin, FieldOrigin, InputScope, OriginArtifactBinding,
    OriginOutputAddress, RecordJsonLocation, SourceEvidenceError, SourceInput, SourceInventory,
    SourceScope, SourceUse, SourceUses, TagPathOrigins,
};

pub(crate) struct RecordFieldReceipt {
    bytes: Vec<u8>,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    inventory: SourceInventory,
}

impl RecordFieldReceipt {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn describe(
        &self,
        out: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        origin_companion::write_origin_companion(
            out,
            binding,
            &self.inventory,
            self.fields
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

pub(super) struct Capture {
    uses: SourceUses,
    original: RecordOriginal,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl Capture {
    pub(super) fn from_record(
        input: &DataForgeEntry,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
    ) -> io::Result<Self> {
        let original = forge
            .record(record.tree().record_index())
            .filter(|original| record.tree().is_from(original))
            .and_then(|_| input.origin_of(forge))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    SourceEvidenceError::ForeignEvidence,
                )
            })?;
        let uses = SourceUses::with_compact_use_refs();
        let original = RecordOriginal::observe(&uses, original)?;
        Ok(Self {
            uses,
            original,
            fields: Vec::new(),
        })
    }

    pub(super) fn field(
        &mut self,
        address: OriginOutputAddress,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        complete: bool,
    ) -> io::Result<()> {
        let (contributors, complete) = self.original.bind(sites, complete)?;
        self.bound_field(address, &contributors, complete)
    }

    pub(super) const fn uses(&self) -> &SourceUses {
        &self.uses
    }

    pub(super) fn tagged_field(
        &mut self,
        address: OriginOutputAddress,
        selectors: &[RecordJsonUse],
        selection_complete: bool,
        tags: &TagPathOrigins<'_>,
        guid: Guid,
    ) -> io::Result<()> {
        let (mut contributors, mut complete) = self.original.bind(
            selectors.iter().map(RecordJsonLocation::of_use),
            selection_complete,
        )?;
        complete &= tags
            .append(guid, &mut contributors)
            .map_err(io::Error::other)?;
        self.bound_field(address, &contributors, complete)
    }

    pub(super) fn localized_field(
        &mut self,
        address: OriginOutputAddress,
        selectors: &[RecordJsonUse],
        selection_complete: bool,
        localization: &RecordLocalization<'_>,
        observation: &ObservedResolution<'_>,
    ) -> io::Result<()> {
        let key = selectors
            .iter()
            .rfind(|input| {
                matches!(
                    input.source,
                    RecordJsonSource::Bytes {
                        kind: RecordJsonBytes::Text,
                        ..
                    }
                )
            })
            .and_then(RecordJsonLocation::of_use);
        self.localized_value(
            address,
            selectors.iter().map(RecordJsonLocation::of_use),
            key.as_ref(),
            selection_complete,
            localization.capture_lookup(observation).as_ref(),
        )
    }

    pub(super) fn localized_value(
        &mut self,
        address: OriginOutputAddress,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        key: Option<&RecordJsonLocation>,
        complete: bool,
        lookup: Option<&LocalizationLookupEvidence<'_>>,
    ) -> io::Result<()> {
        let (contributors, complete) = self
            .original
            .bind_lookup(sites, key, complete, lookup, &self.uses)?;
        self.bound_field(address, &contributors, complete)
    }

    fn bound_field(
        &mut self,
        address: OriginOutputAddress,
        contributors: &[SourceUse],
        complete: bool,
    ) -> io::Result<()> {
        self.fields.try_reserve(1).map_err(io::Error::other)?;
        self.fields
            .push((address, self.uses.origin(contributors, complete)));
        Ok(())
    }

    pub(super) fn finish(self, bytes: Vec<u8>) -> io::Result<RecordFieldReceipt> {
        let inventory = self.uses.freeze().map_err(io::Error::other)?;
        Ok(RecordFieldReceipt {
            bytes,
            fields: self.fields,
            inventory,
        })
    }
}

pub(crate) struct RecordOriginal {
    scope: SourceScope,
    complete: bool,
}

impl RecordOriginal {
    pub(crate) fn observe(uses: &SourceUses, origin: &DataForgeOrigin) -> io::Result<Self> {
        let (source, complete) = match origin {
            DataForgeOrigin::Archive(entry) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            DataForgeOrigin::File(file) => (SourceInput::File(file), true),
            DataForgeOrigin::Unknown => (SourceInput::Unknown, false),
        };
        let scope = uses
            .observe(source, InputScope::Payload)
            .map_err(io::Error::other)?;
        Ok(Self { scope, complete })
    }

    pub(super) fn bind_lookup(
        &self,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        key: Option<&RecordJsonLocation>,
        complete: bool,
        lookup: Option<&LocalizationLookupEvidence<'_>>,
        uses: &SourceUses,
    ) -> io::Result<(Vec<SourceUse>, bool)> {
        let (mut contributors, complete) = self.bind(sites, complete)?;
        let Some(lookup) = lookup else {
            return Ok((contributors, false));
        };
        let authored_key = key.and_then(|key| {
            contributors.iter().find(|source| {
                source.description().location() == &key.location && source.role() == key.role
            })
        });
        let mut additional = Vec::new();
        let lookup_complete = lookup
            .append_uses(uses, authored_key, &mut additional)
            .map_err(io::Error::other)?;
        contributors
            .try_reserve(additional.len())
            .map_err(io::Error::other)?;
        contributors.extend(additional);
        Ok((contributors, complete && lookup_complete))
    }

    pub(crate) fn bind(
        &self,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        complete: bool,
    ) -> io::Result<(Vec<SourceUse>, bool)> {
        let mut complete = self.complete && complete;
        let mut contributors = Vec::new();
        for site in sites {
            let Some(site) = site else {
                complete = false;
                continue;
            };
            contributors.try_reserve(1).map_err(io::Error::other)?;
            contributors.push(
                self.scope
                    .at(site.location, site.role)
                    .map_err(io::Error::other)?,
            );
        }
        Ok((contributors, complete))
    }
}
