// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::template_issues::Unknowns;
use crate::template_property::PropertyKind;
use crate::template_property_kind::PropertyValueKind;
use crate::template_property_plan::SelectedProperty;
use crate::template_property_projection::TemplatePropertyProjection;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    selected: &[SelectedProperty<'fields, 'scope, 'read, 'source>],
    unknowns: &Unknowns,
) -> Result<Vec<TemplatePropertyProjection>, CatalogError> {
    selected
        .iter()
        .map(|property| one(members, property, unknowns))
        .collect()
}

fn one<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    property: &SelectedProperty<'fields, 'scope, 'read, 'source>,
    unknowns: &Unknowns,
) -> Result<TemplatePropertyProjection, CatalogError> {
    let fields = members.fields();
    let Value::Struct(source) = property.property.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let kind = property.objective.and_then(|_| {
        unknowns.parse(
            "objective property",
            Some(source.type_name()),
            PropertyKind::parse,
        )
    });
    let value_kind = property
        .value
        .map(|fact| match fact.value() {
            Value::Struct(source) => Ok(PropertyValueKind::read(source.type_name())),
            _ => Err(ExportAccountingError::ConflictingSourceDatum),
        })
        .transpose()?
        .flatten();
    let caller = |name| -> Result<_, CatalogError> {
        fields
            .field(Some(property.property), name)?
            .map(|(_, fact)| members.value_ref(fact))
            .transpose()
    };
    Ok(TemplatePropertyProjection {
        source: members.value_ref(property.property)?,
        objective: property
            .objective
            .map(|fact| members.value_ref(fact))
            .transpose()?,
        naming: members.value_ref(property.naming)?,
        kind,
        variable: crate::template_declaration_binding::field(
            members,
            Some(property.naming),
            "missionVariableName",
        )?,
        token: crate::template_declaration_binding::field(
            members,
            Some(property.naming),
            "extendedTextToken",
        )?,
        value: property
            .value
            .map(|fact| members.value_ref(fact))
            .transpose()?,
        value_kind,
        contract_property: if property.objective.is_some() {
            caller("property")?
        } else {
            None
        },
        linked_output: if property.objective.is_some() {
            caller("linkedOutput")?
        } else {
            None
        },
    })
}
