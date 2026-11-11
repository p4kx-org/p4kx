// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};
use crate::scoped_location_link::PropertyReferenceStatus;
use crate::template_reference_index::{TargetSelection, TargetSelectionKind, TemplateReferences};
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

/// Keeps the existing strict `MissionProperty` decision alongside its shared raw target cell.
pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &TemplateReferences<'fields, 'scope, 'read, 'source>,
    selection: &TargetSelection<'fields, 'scope, 'read, 'source>,
) -> Result<Option<SourcedValue<PropertyReferenceStatus>>, CatalogError> {
    let (mut inputs, complete) = references.contributions(selection)?;
    let status = match selection.kind() {
        TargetSelectionKind::Absent
        | TargetSelectionKind::Null
        | TargetSelectionKind::WrongShape
        | TargetSelectionKind::WrongStorage
        | TargetSelectionKind::Unreadable => return Ok(None),
        TargetSelectionKind::OutsideOwner => PropertyReferenceStatus::OutsideOwner,
        TargetSelectionKind::RecordTarget | TargetSelectionKind::Unresolved => {
            PropertyReferenceStatus::MissingTarget
        }
        TargetSelectionKind::Instance { owner, field } => {
            let Value::Struct(source) = owner.value() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            inputs.push(owner.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?);
            if source.type_name() == "MissionProperty" {
                match field.map(OriginalValueFact::value) {
                    Some(Value::String(_)) => PropertyReferenceStatus::Resolved,
                    None | Some(Value::Null) => PropertyReferenceStatus::MissingTarget,
                    _ => PropertyReferenceStatus::UnreadableTarget,
                }
            } else {
                PropertyReferenceStatus::UnsupportedTargetType
            }
        }
    };
    Ok(Some(members.fields().sources().value(
        SourceValue::Present(status),
        &inputs,
        complete,
    )))
}
