// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::sound::{WemOriginField, WemOriginPlan};
use p4kx_extraction::{
    EmbeddedWem, EntryOrigin, FieldOrigin, InputScope, ObservedEntry, SectionId, Soundbank,
    SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};

use super::wem_header_provenance::WemHeaderEvidenceError;

#[path = "bank_wem_outcome.rs"]
mod wem_outcome;
pub(super) use wem_outcome::BankWemInspection;

#[cfg(test)]
#[path = "soundbank_provenance_tests.rs"]
mod tests;

const WORD_BYTES: u64 = size_of::<u32>() as u64;

/// Origins of the facts in the path line of a directly opened soundbank.
#[derive(Debug)]
#[non_exhaustive]
pub struct SoundbankSummaryProvenance {
    /// Stored path in the owning Archive directory.
    pub path: FieldOrigin,
    /// Uncompressed size in the owning Archive directory.
    pub size: FieldOrigin,
    /// Number of distinct embedded WEM IDs admitted by the bank parser.
    pub wem_count: FieldOrigin,
    inventory: SourceInventory,
}

impl SoundbankSummaryProvenance {
    /// Returns the physical source and original field coordinates for the summary.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}

/// Direct fields of the physical DIDX entry supplying an embedded WEM.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSoundIdentityProvenance {
    /// WEM ID stored in the selected DIDX triplet.
    pub id: FieldOrigin,
    /// Length stored in that triplet.
    pub size: FieldOrigin,
}

/// Original inputs used to display one embedded WEM row.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSoundProvenance {
    /// Media ID and stored length of the selected physical triplet.
    pub identity: BankSoundIdentityProvenance,
    /// Detected codec or the status displayed in its place.
    pub codec: FieldOrigin,
    /// Playing duration or the reason it is missing.
    pub duration: FieldOrigin,
    inventory: SourceInventory,
}

impl BankSoundProvenance {
    /// Returns the physical source and original field coordinates for this row.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    pub(super) fn capture(
        wem: &EmbeddedWem<'_>,
        inspection: &BankWemInspection,
        source: Option<&EntryOrigin>,
    ) -> Result<Self, WemHeaderEvidenceError> {
        let uses = SourceUses::new();
        let source = SoundbankSource::from_origin(&uses, source)?;
        let placement = source.embedded(wem);
        let (codec, duration) = placement.row_facts(inspection)?;
        Ok(Self {
            identity: placement.identity()?,
            codec,
            duration,
            inventory: uses.freeze()?,
        })
    }
}

/// Binds bank and embedded WEM decisions to one observed bank input.
pub(super) struct SoundbankSource<'uses> {
    uses: &'uses SourceUses,
    payload: SourceScope,
    complete: bool,
}

impl<'uses> SoundbankSource<'uses> {
    pub(super) fn observe(
        uses: &'uses SourceUses,
        source: Option<ObservedEntry<'_>>,
    ) -> Result<Self, SourceEvidenceError> {
        let complete = source.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        Self::register(
            uses,
            source.map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            complete,
        )
    }

    pub(super) fn from_origin(
        uses: &'uses SourceUses,
        source: Option<&EntryOrigin>,
    ) -> Result<Self, SourceEvidenceError> {
        let complete = source.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        Self::register(
            uses,
            source.map_or(SourceInput::Unknown, SourceInput::Archive),
            complete,
        )
    }

    fn register(
        uses: &'uses SourceUses,
        input: SourceInput<'_>,
        complete: bool,
    ) -> Result<Self, SourceEvidenceError> {
        let payload = uses.observe(input, InputScope::Payload)?;
        Ok(Self {
            uses,
            payload,
            complete,
        })
    }

    pub(super) fn summary<B: AsRef<[u8]>>(
        &self,
        bank: &Soundbank<B>,
    ) -> Result<SoundbankSummaryProvenance, SourceEvidenceError> {
        Ok(SoundbankSummaryProvenance {
            path: self.directory_field("path")?,
            size: self.directory_field("uncompressed_size")?,
            wem_count: self.origin(&self.membership(bank)?),
            inventory: self.uses.freeze()?,
        })
    }

    pub(super) const fn embedded(&self, wem: &EmbeddedWem<'_>) -> EmbeddedWemEvidence<'_> {
        let source = wem.source();
        EmbeddedWemEvidence {
            bank: self,
            triplet: source.triplet_offset(),
            data_body: source.data_body_offset(),
            bytes: source.byte_range(),
        }
    }

    fn membership<B: AsRef<[u8]>>(
        &self,
        bank: &Soundbank<B>,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut decoded_end = 0;
        let mut didx = None;
        let mut data = None;
        for (tag, body) in bank.source_section_bodies() {
            decoded_end = body.end;
            match tag {
                SectionId::DIDX => didx = Some(body),
                SectionId::DATA => data = Some(body),
                _ => {}
            }
        }
        let mut inputs = vec![self.payload.at(
            SourceLocation::Subresource {
                namespace: "bnkSectionDirectory".into(),
                indices: vec![decoded_end],
                field: "headerScan".into(),
            },
            SourceUseRole::Selector,
        )?];
        if let (Some(didx), Some(data)) = (didx, data) {
            inputs.push(self.range("bnkDidx", "triplets", didx, SourceUseRole::Conversion)?);
            inputs.push(self.data_length(data.start)?);
        }
        Ok(inputs)
    }

    fn origin(&self, inputs: &[SourceUse]) -> FieldOrigin {
        self.uses.origin(inputs, self.complete)
    }

    fn directory_field(&self, field: &str) -> Result<FieldOrigin, SourceEvidenceError> {
        let used = self
            .payload
            .for_scope(InputScope::ArchiveDirectoryMetadata)
            .at(
                SourceLocation::Metadata {
                    field: field.into(),
                },
                SourceUseRole::Payload,
            )?;
        Ok(self.origin(&[used]))
    }

    fn data_length(&self, data_body: u64) -> Result<SourceUse, SourceEvidenceError> {
        self.range(
            "bnkData",
            "bodyLength",
            data_body - WORD_BYTES..data_body,
            SourceUseRole::Selector,
        )
    }

    fn range(
        &self,
        namespace: &str,
        field: &str,
        bytes: Range<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.payload.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: vec![bytes.start, bytes.end],
                field: field.into(),
            },
            role,
        )
    }
}

/// Binds the physical DIDX and DATA selection of one admitted WEM.
pub(super) struct EmbeddedWemEvidence<'bank> {
    bank: &'bank SoundbankSource<'bank>,
    triplet: u64,
    data_body: u64,
    bytes: Range<u64>,
}

impl EmbeddedWemEvidence<'_> {
    pub(super) fn identity(&self) -> Result<BankSoundIdentityProvenance, SourceEvidenceError> {
        use SourceUseRole::Payload;

        let id = self.didx(DidxField::Id, Payload)?;
        let size = self.didx(DidxField::Length, Payload)?;
        Ok(BankSoundIdentityProvenance {
            id: self.bank.origin(&[id]),
            size: self.bank.origin(&[size]),
        })
    }

    pub(super) fn selectors(&self) -> Result<[SourceUse; 4], SourceEvidenceError> {
        use SourceUseRole::Selector;

        Ok([
            self.didx(DidxField::Id, Selector)?,
            self.didx(DidxField::Offset, Selector)?,
            self.didx(DidxField::Length, Selector)?,
            self.bank.data_length(self.data_body)?,
        ])
    }

    pub(super) fn riff_selection(&self) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut inputs = Vec::from(self.selectors()?);
        inputs.push(self.riff_word(0, "signature")?);
        inputs.push(self.riff_word(1, "declaredSize")?);
        Ok(inputs)
    }

    pub(super) fn field(
        &self,
        plan: &WemOriginPlan<'_>,
        field: WemOriginField,
        selection: &[SourceUse],
    ) -> Result<FieldOrigin, WemHeaderEvidenceError> {
        let mut inputs = selection.to_vec();
        for site in plan.field(field)? {
            inputs.push(site.bind_embedded(&self.bank.payload, self.bytes.clone())?);
        }
        Ok(self.bank.origin(&inputs))
    }

    fn riff_word(&self, word: u64, field: &str) -> Result<SourceUse, SourceEvidenceError> {
        let start = self.bytes.start + word * WORD_BYTES;
        self.bank.range(
            "bnkWemRiffHeader",
            field,
            start..start + WORD_BYTES,
            SourceUseRole::Conversion,
        )
    }

    pub(super) fn sampled_bytes(
        &self,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.bank
            .range("bnkEmbeddedWem", "metadataRead", self.bytes.clone(), role)
    }

    fn didx(
        &self,
        field: DidxField,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        let (word, field) = match field {
            DidxField::Id => (0, "mediaId"),
            DidxField::Offset => (1, "dataOffset"),
            DidxField::Length => (2, "dataLength"),
        };
        let start = self.triplet + word * WORD_BYTES;
        self.bank
            .range("bnkDidx", field, start..start + WORD_BYTES, role)
    }
}

#[derive(Clone, Copy)]
enum DidxField {
    Id,
    Offset,
    Length,
}
