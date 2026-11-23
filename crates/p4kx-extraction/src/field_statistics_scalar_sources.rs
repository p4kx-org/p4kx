// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_dataforge::query::QueryByteUse;
use p4kx_dataforge::{
    DataType, FieldStatisticsInput, FieldStatisticsScalarRead, FieldStatisticsValue, ScalarSource,
};

use super::{Binder, payload, property_target, storage};
use crate::query_source_evidence::QuerySourceSite;
use crate::{SourceEvidenceError, SourceUseRole};

impl Binder<'_> {
    pub(super) fn operand(
        &mut self,
        input: &FieldStatisticsInput,
        value: &FieldStatisticsValue,
    ) -> Result<(), SourceEvidenceError> {
        let text = match value {
            FieldStatisticsValue::Empty(text) | FieldStatisticsValue::Nonempty { text, .. } => {
                text.as_ref()
            }
            FieldStatisticsValue::Structural | FieldStatisticsValue::Unreadable
                if matches!(
                    storage(input),
                    Some(DataType::StrongPointer | DataType::WeakPointer)
                ) =>
            {
                None
            }
            _ => return Ok(()),
        };
        self.scalar_operand(input, text)
    }

    pub(super) fn scalar_operand(
        &mut self,
        input: &FieldStatisticsInput,
        text: Option<&Range<u64>>,
    ) -> Result<(), SourceEvidenceError> {
        self.bytes(payload(input), QueryByteUse::Value, SourceUseRole::Payload)?;
        self.bytes(text, QueryByteUse::Text, SourceUseRole::Payload)
    }

    pub(super) fn scalar_read(
        &mut self,
        input: &FieldStatisticsInput,
        read: FieldStatisticsScalarRead,
    ) -> Result<(), SourceEvidenceError> {
        for original in read.sources() {
            if original == ScalarSource::EnumCount
                && let Some(target) = property_target(input)
            {
                self.sites
                    .decoder_uses
                    .push(self.scope.at(target, SourceUseRole::Selector)?);
            }
            if let Some(site) = QuerySourceSite::of(original.into(), SourceUseRole::Selector) {
                let role = site.role;
                let bound = site.bind(self.scope)?;
                if role == SourceUseRole::Attempted {
                    self.sites.attempts.push(bound);
                } else {
                    self.sites.decoder_uses.push(bound);
                }
            }
        }
        Ok(())
    }
}
