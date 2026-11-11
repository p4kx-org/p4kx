// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Record, RecordText, SourceUseRole, SourceValue};

use super::{OriginalDatum, OriginalReferenceObservation, OriginalValuePart};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::record_header_field::RecordHeaderField;

/// The actual selected target and the result of its class-name presentation.
pub(crate) struct OriginalRecordName<'source> {
    pub(crate) record: Record<'source>,
    pub(crate) name: OriginalDatum<&'source str>,
    pub(crate) error: Option<DataForgeError>,
}

impl<'source> OriginalReferenceObservation<'_, '_, '_, 'source> {
    pub(crate) fn class_name(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<OriginalRecordName<'source>, CatalogError> {
        self.caller().check_input(input)?;
        let record = self.record()?;
        let context = input.record_context(&record)?;
        let name = record.stored_name();
        let source_type = record.type_name();
        let mut inputs = vec![
            self.caller()
                .observe(OriginalValuePart::Value, SourceUseRole::Selector)?,
        ];
        inputs.push(context.observe_header(
            RecordHeaderField::RecordName,
            match &name {
                Ok(RecordText::Present(_)) => SourceUseRole::Payload,
                Ok(_) => SourceUseRole::Selector,
                Err(_) => SourceUseRole::Attempted,
            },
        )?);
        inputs.push(context.observe_header(
            RecordHeaderField::SourceType,
            if source_type.is_ok() {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            },
        )?);
        let (value, error) = match (name, source_type) {
            (Ok(name), Ok(source_type)) => {
                inputs.push(
                    context
                        .observe_header(RecordHeaderField::RecordName, SourceUseRole::Conversion)?,
                );
                let name = name.fold(|| "", || "", |name| name);
                (SourceValue::Present(class_name(name, source_type)), None)
            }
            (Err(error), _) | (_, Err(error)) => (SourceValue::Unreadable, Some(error)),
        };
        let complete = error.is_none();
        Ok(OriginalRecordName {
            record,
            name: OriginalDatum::observed(value, inputs, complete),
            error,
        })
    }
}

fn class_name<'source>(name: &'source str, source_type: &str) -> &'source str {
    name.strip_prefix(source_type)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
}
