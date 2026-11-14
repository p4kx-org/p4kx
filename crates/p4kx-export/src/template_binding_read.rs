// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::OriginalValueFact;
use crate::template_reference_index::{TargetSelection, TargetSelectionKind, TemplateReferences};
use crate::template_source_meaning::{
    ObjectiveBinding, ObjectiveInterpretation, PropertyBinding, PropertyInterpretation,
};
use crate::template_source_members::TemplateMembers;

pub(crate) fn objective<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    parent: &'fields OriginalValueFact<'scope, 'read, 'source>,
    caller: &str,
) -> Result<ObjectiveBinding, CatalogError> {
    let selected = references.select(parent, caller, "id")?;
    objective_selected(members, references, &selected)
}

pub(crate) fn objective_selected<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &TemplateReferences<'fields, 'scope, 'read, 'source>,
    selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
) -> Result<ObjectiveBinding, CatalogError> {
    let origin = references.origin(selected)?;
    let field = match selected.kind() {
        TargetSelectionKind::Instance {
            field: Some(field), ..
        } => Some(members.field_ref(field)?),
        _ => None,
    };
    Ok(ObjectiveBinding {
        field,
        interpretation: objective_kind(selected.kind()),
        origin,
    })
}

pub(crate) fn property<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    parent: &'fields OriginalValueFact<'scope, 'read, 'source>,
) -> Result<PropertyBinding, CatalogError> {
    let selected = references.select(parent, "property", "missionVariableName")?;
    property_selected(members, references, &selected)
}

pub(crate) fn property_selected<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &TemplateReferences<'fields, 'scope, 'read, 'source>,
    selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
) -> Result<PropertyBinding, CatalogError> {
    let origin = references.origin(selected)?;
    let field = match selected.kind() {
        TargetSelectionKind::Instance {
            field: Some(field), ..
        } => Some(members.field_ref(field)?),
        _ => None,
    };
    Ok(PropertyBinding {
        field,
        interpretation: property_kind(selected.kind()),
        origin,
    })
}

fn objective_kind(selected: TargetSelectionKind<'_, '_, '_, '_>) -> ObjectiveInterpretation {
    match selected {
        TargetSelectionKind::Absent | TargetSelectionKind::Instance { field: None, .. } => {
            ObjectiveInterpretation::Absent
        }
        TargetSelectionKind::Null => ObjectiveInterpretation::Null,
        TargetSelectionKind::Unreadable => ObjectiveInterpretation::Unreadable,
        TargetSelectionKind::WrongShape => ObjectiveInterpretation::WrongShape,
        TargetSelectionKind::WrongStorage => ObjectiveInterpretation::WrongStorage,
        TargetSelectionKind::OutsideOwner => ObjectiveInterpretation::OutsideOwner,
        TargetSelectionKind::RecordTarget => ObjectiveInterpretation::RecordTarget,
        TargetSelectionKind::Unresolved => ObjectiveInterpretation::Unresolved,
        TargetSelectionKind::Instance {
            field: Some(field), ..
        } => match field.value() {
            Value::Guid(value) if crate::record_fields::nonnil_guid(*value).is_some() => {
                ObjectiveInterpretation::Id
            }
            Value::Guid(_) => ObjectiveInterpretation::Nil,
            Value::Null => ObjectiveInterpretation::Null,
            Value::Error(_) => ObjectiveInterpretation::Unreadable,
            _ => ObjectiveInterpretation::WrongShape,
        },
    }
}

fn property_kind(selected: TargetSelectionKind<'_, '_, '_, '_>) -> PropertyInterpretation {
    match selected {
        TargetSelectionKind::Absent | TargetSelectionKind::Instance { field: None, .. } => {
            PropertyInterpretation::Absent
        }
        TargetSelectionKind::Null => PropertyInterpretation::Null,
        TargetSelectionKind::Unreadable => PropertyInterpretation::Unreadable,
        TargetSelectionKind::WrongShape => PropertyInterpretation::WrongShape,
        TargetSelectionKind::WrongStorage => PropertyInterpretation::WrongStorage,
        TargetSelectionKind::OutsideOwner => PropertyInterpretation::OutsideOwner,
        TargetSelectionKind::RecordTarget => PropertyInterpretation::RecordTarget,
        TargetSelectionKind::Unresolved => PropertyInterpretation::Unresolved,
        TargetSelectionKind::Instance {
            field: Some(field), ..
        } => match field.value() {
            Value::String("") | Value::Enum("") => PropertyInterpretation::Empty,
            Value::String(_) | Value::Enum(_) => PropertyInterpretation::Value,
            Value::Null => PropertyInterpretation::Null,
            Value::Error(_) => PropertyInterpretation::Unreadable,
            _ => PropertyInterpretation::WrongShape,
        },
    }
}
