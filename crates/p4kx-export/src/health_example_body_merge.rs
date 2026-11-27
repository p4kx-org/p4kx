// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::ExampleBody;
use crate::ExportAccountingError;

impl ExampleBody {
    pub(super) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        match (&mut *self, other) {
            (Self::Header(recipe), Self::Header(other)) => recipe.merge(other),
            (Self::Loadout(receipt), Self::Loadout(other)) => receipt.merge(other),
            (Self::RequestedLoadout(receipt), Self::RequestedLoadout(other)) => {
                receipt.merge(other)
            }
            (Self::LoadoutTarget(receipt), Self::LoadoutTarget(other)) => receipt.merge(other),
            (
                Self::Technical {
                    subject,
                    field,
                    evidence,
                },
                Self::Technical {
                    subject: other_subject,
                    field: other_field,
                    evidence: other_evidence,
                },
            ) => {
                if *field != other_field || *evidence != other_evidence {
                    return Err(ExportAccountingError::ConflictingSourceDatum);
                }
                subject.merge(other_subject)
            }
            (Self::Absent(subject), Self::Absent(other))
            | (Self::GuidHeader(subject), Self::GuidHeader(other)) => subject.merge(other),
            (
                Self::Captured {
                    frame,
                    field,
                    source_type,
                    target_guid,
                },
                Self::Captured {
                    frame: other_frame,
                    field: other_field,
                    source_type: other_type,
                    target_guid: other_guid,
                },
            ) => {
                let mut original = frame.as_ref().clone();
                original.merge(other_frame.as_ref().clone())?;
                field.merge(other_field)?;
                source_type.merge(other_type)?;
                target_guid.merge(other_guid)?;
                *frame = Arc::new(original);
                Ok(())
            }
            _ => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
}
