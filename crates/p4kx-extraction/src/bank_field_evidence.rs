// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::wem_payload_evidence::{EvidenceError, put, range_use};
use crate::{
    Bank, FieldOrigins, InputScope, SourceInput, SourceInventory, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

pub(crate) struct BankFieldEvidence<'bank> {
    pub(crate) bank: &'bank Bank,
    pub(crate) uses: SourceUses,
    pub(crate) scope: SourceScope,
    pub(crate) complete: bool,
}

impl<'bank> BankFieldEvidence<'bank> {
    pub(crate) fn new(bank: &'bank Bank) -> Result<Self, EvidenceError> {
        let source = bank.observed_input();
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses.observe(source, InputScope::Payload)?;
        let complete = match source {
            SourceInput::Archive(origin) => origin.ancestors().all(|step| step.is_some()),
            SourceInput::File(_) => true,
            _ => false,
        };
        Ok(Self {
            bank,
            uses,
            scope,
            complete,
        })
    }

    pub(crate) fn header(
        &self,
        root: &str,
        fields: &mut FieldOrigins,
    ) -> Result<(), EvidenceError> {
        let (scope, name) = if matches!(self.bank.observed_input(), SourceInput::File(_)) {
            (InputScope::FileMetadata, "name")
        } else {
            (InputScope::ArchiveDirectoryMetadata, "path")
        };
        let path = self.scope.for_scope(scope).at(
            SourceLocation::Metadata { field: name.into() },
            SourceUseRole::Payload,
        )?;
        put(
            fields,
            root,
            "soundbank",
            self.uses.origin(&[path], self.complete),
        )?;
        let version = self.version_range()?;
        for (output, original, range) in [
            ("version", "version", version.clone()),
            (
                "bank_id",
                "bankId",
                version.end..version.end + size_of::<u32>(),
            ),
        ] {
            let word = self.word("bnkBkhd", original, range, SourceUseRole::Payload)?;
            put(
                fields,
                root,
                output,
                self.uses.origin(&[word], self.complete),
            )?;
        }
        Ok(())
    }

    pub(crate) fn version(&self, role: SourceUseRole) -> Result<SourceUse, EvidenceError> {
        self.word("bnkBkhd", "version", self.version_range()?, role)
    }

    fn version_range(&self) -> Result<Range<usize>, EvidenceError> {
        self.bank
            .soundbank()
            .version_byte_range()
            .ok_or(EvidenceError::MissingHeader)
    }

    pub(crate) fn word(
        &self,
        namespace: &str,
        field: &str,
        range: Range<usize>,
        role: SourceUseRole,
    ) -> Result<SourceUse, EvidenceError> {
        range_use(
            &self.scope,
            namespace,
            field,
            range.start as u64..range.end as u64,
            role,
        )
    }

    pub(crate) fn finish(self) -> Result<SourceInventory, EvidenceError> {
        Ok(self.uses.freeze()?)
    }
}
