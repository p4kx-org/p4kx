// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::catalog_read::original::OriginalValueFact;
use crate::template_meaning_fields;
use crate::template_source_meaning::TriggerMeaning;
use crate::template_source_members::TemplateMembers;

pub(crate) fn read<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<TriggerMeaning, CatalogError> {
    let fields = members.fields();
    Ok(TriggerMeaning {
        description_field: crate::template_field_binding::read(fields, fact, "description")?,
        note_interpretation: template_meaning_fields::note(fields, fact)?,
        condition_field: crate::template_field_binding::read(fields, fact, "condition")?,
        condition: fields
            .selected_struct(fact, "condition")?
            .map(|child| members.value_ref(child))
            .transpose()?,
        action_field: crate::template_field_binding::read(fields, fact, "action")?,
        action: fields
            .selected_struct(fact, "action")?
            .map(|child| members.value_ref(child))
            .transpose()?,
    })
}
