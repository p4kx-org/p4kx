// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Common source bindings for a soundbank's embedded WEM list.

use crate::wem_payload_evidence::{DidxWord, EvidenceError, PayloadField, put, range_use};
use crate::{
    Bank, FieldOrigin, FieldOrigins, InputScope, SectionId, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUseRole, SourceUses,
};

#[path = "bank_wem_read.rs"]
mod read;

pub use read::{BankWemRead, BankWemReadMode};

const WORD_BYTES: u64 = size_of::<u32>() as u64;

/// Physical source of one observed bank listing.
pub struct BankWemEvidence<'bank> {
    bank: &'bank Bank,
    uses: SourceUses,
    scope: SourceScope,
    complete: bool,
    name_scope: InputScope,
}

impl<'bank> BankWemEvidence<'bank> {
    /// Binds an observed soundbank to a fresh evidence inventory.
    ///
    /// # Errors
    /// Returns an error when the original source cannot be recorded.
    pub fn new(bank: &'bank Bank) -> Result<Self, EvidenceError> {
        let source = bank.observed_input();
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses.observe(source, InputScope::Payload)?;
        let complete = match source {
            SourceInput::Archive(origin) => origin.ancestors().all(|step| step.is_some()),
            SourceInput::File(_) => true,
            _ => false,
        };
        let name_scope = match source {
            SourceInput::File(_) => InputScope::FileMetadata,
            _ => InputScope::ArchiveDirectoryMetadata,
        };
        Ok(Self {
            bank,
            uses,
            scope,
            complete,
            name_scope,
        })
    }

    /// Binds the stored name and BKHD fields at the listing root.
    ///
    /// # Errors
    /// Returns an error for missing original ranges or invalid output addresses.
    pub fn header(&self, root: &str, fields: &mut FieldOrigins) -> Result<(), EvidenceError> {
        let path = self.scope.for_scope(self.name_scope).at(
            SourceLocation::Metadata {
                field: if self.name_scope == InputScope::FileMetadata {
                    "name"
                } else {
                    "path"
                }
                .into(),
            },
            SourceUseRole::Payload,
        )?;
        put(
            fields,
            root,
            "soundbank",
            self.uses.origin(&[path], self.complete),
        )?;
        let version = self
            .bank
            .soundbank()
            .version_byte_range()
            .ok_or(EvidenceError::MissingHeader)?;
        let version = version.start as u64..version.end as u64;
        for (output, original, range) in [
            ("version", "version", version.clone()),
            ("bank_id", "bankId", version.end..version.end + WORD_BYTES),
        ] {
            let word = range_use(
                &self.scope,
                "bnkBkhd",
                original,
                range,
                SourceUseRole::Payload,
            )?;
            put(
                fields,
                root,
                output,
                self.uses.origin(&[word], self.complete),
            )?;
        }
        Ok(())
    }

    /// Returns the directory, DIDX and DATA facts that determine the list.
    ///
    /// # Errors
    /// Returns an error when the original contributors cannot be recorded.
    pub fn membership(&self) -> Result<FieldOrigin, EvidenceError> {
        let mut decoded_end = 0;
        let mut didx = None;
        let mut data = None;
        for (tag, body) in self.bank.soundbank().source_section_bodies() {
            decoded_end = body.end;
            match tag {
                SectionId::DIDX => didx = Some(body),
                SectionId::DATA => data = Some(body),
                _ => {}
            }
        }
        let routing = self.scope.at(
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
                &self.scope,
                "bnkDidx",
                "triplets",
                didx,
                SourceUseRole::Conversion,
            )?);
            membership.push(range_use(
                &self.scope,
                "bnkData",
                "bodyLength",
                data.start - WORD_BYTES..data.start,
                SourceUseRole::Selector,
            )?);
        }
        Ok(self.uses.origin(&membership, self.complete))
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
        let admitted = self
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
        let triplet = wem.source().triplet_offset();
        for (name, field) in [("id", DidxWord::Id), ("size", DidxWord::Length)] {
            let word = field.bind(&self.scope, triplet, SourceUseRole::Payload)?;
            put(
                fields,
                parent,
                name,
                self.uses.origin(&[word], self.complete),
            )?;
        }
        read.payload_plan().record(
            &self.uses,
            &self.scope,
            self.complete,
            parent,
            fields,
            selection,
        )
    }

    /// Returns an origin for a tool argument with no original source use.
    #[must_use]
    pub fn request(&self) -> FieldOrigin {
        self.uses.origin(&[], true)
    }

    /// Freezes the observed physical contributors.
    ///
    /// # Errors
    /// Returns an error if the evidence inventory cannot be constructed.
    pub fn finish(self) -> Result<SourceInventory, EvidenceError> {
        Ok(self.uses.freeze()?)
    }
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
