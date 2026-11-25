// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Common source bindings for a soundbank's embedded WEM list.

use p4kx_audio::EmbeddedWemSource;

use crate::bank_field_evidence::BankFieldEvidence;
use crate::wem_payload_evidence::{
    DidxWord, EvidenceError, PayloadField, embedded_extent, put, range_use,
};
use crate::{
    Bank, EmbeddedWem, FieldOrigin, FieldOrigins, SectionId, SourceInventory, SourceLocation,
    SourceScope, SourceUse, SourceUseRole,
};

#[path = "bank_wem_read.rs"]
mod read;

pub use read::{BankWemRead, BankWemReadMode};

const WORD_BYTES: u64 = size_of::<u32>() as u64;

/// Physical source of one observed bank listing.
pub struct BankWemEvidence<'bank> {
    evidence: BankFieldEvidence<'bank>,
}

impl<'bank> BankWemEvidence<'bank> {
    /// Binds an observed soundbank to a fresh evidence inventory.
    ///
    /// # Errors
    /// Returns an error when the original source cannot be recorded.
    pub fn new(bank: &'bank Bank) -> Result<Self, EvidenceError> {
        Ok(Self {
            evidence: BankFieldEvidence::new(bank)?,
        })
    }

    /// Binds the stored name and BKHD fields at the listing root.
    ///
    /// # Errors
    /// Returns an error for missing original ranges or invalid output addresses.
    pub fn header(&self, root: &str, fields: &mut FieldOrigins) -> Result<(), EvidenceError> {
        self.evidence.header(root, fields)
    }

    /// Returns the directory, DIDX and DATA facts that determine the list.
    ///
    /// # Errors
    /// Returns an error when the original contributors cannot be recorded.
    pub fn membership(&self) -> Result<FieldOrigin, EvidenceError> {
        let mut decoded_end = 0;
        let mut didx = None;
        let mut data = None;
        for (tag, body) in self.evidence.bank.soundbank().source_section_bodies() {
            decoded_end = body.end;
            match tag {
                SectionId::DIDX => didx = Some(body),
                SectionId::DATA => data = Some(body),
                _ => {}
            }
        }
        let routing = self.evidence.scope.at(
            SourceLocation::Subresource {
                namespace: "bnkSectionDirectory".into(),
                indices: vec![decoded_end],
                field: "headerScan".into(),
            },
            SourceUseRole::Selector,
        )?;
        let mut membership = vec![routing];
        if let (Some(didx), Some(data)) = (didx, data) {
            membership.push(range_use(
                &self.evidence.scope,
                "bnkDidx",
                "triplets",
                didx,
                SourceUseRole::Conversion,
            )?);
            membership.push(range_use(
                &self.evidence.scope,
                "bnkData",
                "bodyLength",
                data.start - WORD_BYTES..data.start,
                SourceUseRole::Selector,
            )?);
        }
        Ok(self
            .evidence
            .uses
            .origin(&membership, self.evidence.complete))
    }

    /// Binds one selected physical WEM and the fields its caller emits.
    ///
    /// # Errors
    /// Returns an error if the original coordinates or output addresses cannot be recorded.
    pub fn row(
        &self,
        parent: &str,
        read: &BankWemRead<'_>,
        selection: &[(&str, PayloadField)],
        fields: &mut FieldOrigins,
    ) -> Result<(), EvidenceError> {
        let wem = read.media();
        self.check_member(wem)?;
        let triplet = wem.source().triplet_offset();
        for (name, field) in [("id", DidxWord::Id), ("size", DidxWord::Length)] {
            let word = field.bind(&self.evidence.scope, triplet, SourceUseRole::Payload)?;
            put(
                fields,
                parent,
                name,
                self.evidence.uses.origin(&[word], self.evidence.complete),
            )?;
        }
        read.payload_plan().record(
            &self.evidence.uses,
            &self.evidence.scope,
            self.evidence.complete,
            parent,
            fields,
            selection,
        )
    }

    pub(crate) fn raw_payload(
        self,
        wem: &EmbeddedWem<'_>,
    ) -> Result<(SourceInventory, FieldOrigin), EvidenceError> {
        self.check_member(wem)?;
        let placement = wem.source();
        let mut uses = member_selectors(&self.evidence.scope, placement)?;
        uses.push(member_payload(&self.evidence.scope, placement)?);
        let origin = self.evidence.uses.origin(&uses, self.evidence.complete);
        Ok((self.finish()?, origin))
    }

    fn check_member(&self, wem: &EmbeddedWem<'_>) -> Result<(), EvidenceError> {
        let admitted = self
            .evidence
            .bank
            .soundbank()
            .wem(wem.id())
            .ok_or(EvidenceError::InvalidPrefix)?;
        if !std::ptr::eq(admitted.bytes(), wem.bytes())
            || admitted.source().triplet_offset() != wem.source().triplet_offset()
        {
            return Err(EvidenceError::Source(
                crate::SourceEvidenceError::ForeignEvidence,
            ));
        }
        Ok(())
    }

    /// Returns an origin for a tool argument with no original source use.
    #[must_use]
    pub fn request(&self) -> FieldOrigin {
        self.evidence.uses.origin(&[], true)
    }

    /// Freezes the observed physical contributors.
    ///
    /// # Errors
    /// Returns an error if the evidence inventory cannot be constructed.
    pub fn finish(self) -> Result<SourceInventory, EvidenceError> {
        self.evidence.finish()
    }
}

pub(crate) fn member_selectors(
    scope: &SourceScope,
    placement: EmbeddedWemSource,
) -> Result<Vec<SourceUse>, EvidenceError> {
    let triplet = placement.triplet_offset();
    let mut uses = embedded_extent(scope, triplet, placement.data_body_offset())?;
    uses.push(DidxWord::Id.bind(scope, triplet, SourceUseRole::Selector)?);
    Ok(uses)
}

pub(crate) fn member_payload(
    scope: &SourceScope,
    placement: EmbeddedWemSource,
) -> Result<SourceUse, EvidenceError> {
    range_use(
        scope,
        "bnkEmbeddedWem",
        "payload",
        placement.byte_range(),
        SourceUseRole::Payload,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use p4kx_testkit::{TestBank, TestCodec, TestWem};

    use super::{BankWemEvidence, BankWemRead, BankWemReadMode};
    use crate::wem_payload_evidence::{EvidenceError, MetadataField, PayloadField};
    use crate::{Bank, FieldOrigins, SourceEvidenceError};

    fn witness(bank: &Bank) -> Result<BankWemRead<'_>, &'static str> {
        bank.soundbank()
            .wem(4)
            .ok_or("original WEM")
            .map(|wem| BankWemRead::read(wem, BankWemReadMode::AllNonPrefetched))
    }

    #[test]
    fn a_bank_listing_rejects_a_foreign_wem_with_identical_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let selected = folder.path().join("selected.bnk");
        let foreign = folder.path().join("foreign.bnk");
        let bytes = TestBank::new(9)
            .wem(
                4,
                TestWem::pcm(TestCodec::Pcm, 1, 8000, &[0, 1, 2, 3]).bytes(),
            )
            .bytes();
        fs::write(&selected, &bytes)?;
        fs::write(&foreign, &bytes)?;
        let selected = Bank::from_file_observed(&selected)?;
        let foreign = Bank::from_file_observed(&foreign)?;
        let wrong = witness(&foreign)?;
        let right = witness(&selected)?;
        let evidence = BankWemEvidence::new(&selected)?;
        let fields = [
            ("codec", PayloadField::Metadata(MetadataField::Codec)),
            ("error", PayloadField::Status),
        ];
        let mut origins = FieldOrigins::new();
        assert!(
            matches!(
                evidence.row("/wems/0", &wrong, &fields, &mut origins),
                Err(EvidenceError::Source(SourceEvidenceError::ForeignEvidence))
            ),
            "equal IDs, bytes and coordinates do not transfer physical ownership",
        );
        evidence.row("/wems/0", &right, &fields, &mut origins)?;
        assert!(
            evidence.finish()?.use_count() >= 3,
            "the admitted read still binds the DIDX words and parsed codec",
        );
        Ok(())
    }
}
