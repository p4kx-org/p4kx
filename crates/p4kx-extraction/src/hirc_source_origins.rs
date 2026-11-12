// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{BankHircField, BankHircObject, BankObservedHeader};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::bank::Bank;
use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_scope::{SourceInput, SourceScope};
use crate::source_use::{InputScope, SourceLocation, SourceUse, SourceUseRole};
use crate::source_uses::SourceUses;
use crate::source_value::{SourceValue, SourcedValue};

use super::SourceFamily;

pub(super) struct ObjectOrigins {
    uses: SourceUses,
    scope: SourceScope,
    complete: bool,
    family: SourceFamily,
}

impl ObjectOrigins {
    pub(super) fn new(bank: &Bank, family: SourceFamily) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let input = bank.observed_input();
        let complete = !matches!(input, SourceInput::Unknown);
        let scope = uses.observe(input, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            complete,
            family,
        })
    }

    pub(super) fn field<T: Copy>(
        &self,
        field: &BankHircField<T>,
        label: &'static str,
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        let used = self.site(
            self.family.namespace(),
            label,
            &field.range,
            SourceUseRole::Payload,
        )?;
        Ok(self.value(field.value, &[used]))
    }

    pub(super) fn optional<T: Copy>(
        &self,
        field: Option<&BankHircField<T>>,
        label: &'static str,
    ) -> Result<Option<SourcedValue<T>>, SourceEvidenceError> {
        field.map(|field| self.field(field, label)).transpose()
    }

    pub(super) fn derived<T>(
        &self,
        value: T,
        extent: &Range<u64>,
        label: &'static str,
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        let used = self.site(
            self.family.namespace(),
            label,
            extent,
            SourceUseRole::Conversion,
        )?;
        Ok(self.value(value, &[used]))
    }

    pub(super) fn range(
        &self,
        extent: &Range<u64>,
        label: &'static str,
    ) -> Result<SourcedValue<StateRange>, SourceEvidenceError> {
        let used = self.site(
            self.family.namespace(),
            label,
            extent,
            SourceUseRole::Selector,
        )?;
        Ok(self.value(StateRange::new(extent), &[used]))
    }

    pub(super) fn version(
        &self,
        header: &BankObservedHeader,
    ) -> Result<SourcedValue<u32>, SourceEvidenceError> {
        let start = header.section().body_offset();
        let range = start..start + size_of::<u32>() as u64;
        let used = self.site("bnkBkhd", "version", &range, SourceUseRole::Payload)?;
        Ok(self.value(header.header().version, &[used]))
    }

    pub(super) fn coverage<T>(
        &self,
        value: T,
        header: &BankObservedHeader,
        object: &BankHircObject,
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        let start = header.section().body_offset();
        let version = self.site(
            "bnkBkhd",
            "version",
            &(start..start + size_of::<u32>() as u64),
            SourceUseRole::Selector,
        )?;
        let body = self.site(
            self.family.namespace(),
            self.family.extent_label(),
            &(object.header_offset()..object.body_end_exclusive()),
            SourceUseRole::Conversion,
        )?;
        Ok(self.value(value, &[version, body]))
    }

    fn site(
        &self,
        namespace: &'static str,
        label: &'static str,
        extent: &Range<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices: vec![extent.start, extent.end],
                field: label.to_owned(),
            },
            role,
        )
    }

    fn value<T>(&self, value: T, uses: &[SourceUse]) -> SourcedValue<T> {
        self.uses
            .value(SourceValue::Present(value), uses, self.complete)
    }
}

impl Serialize for ObjectOrigins {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.uses
            .freeze()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}
