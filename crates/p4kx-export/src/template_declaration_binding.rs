// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::catalog_read::original::OriginalValueFact;
use crate::template_source_members::TemplateMembers;
use crate::template_source_refs::DeclarationBinding;

pub(crate) fn field<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    name: &str,
) -> Result<DeclarationBinding, CatalogError> {
    Ok(match members.fields().field(parent, name)? {
        Some((_, field)) => DeclarationBinding::Present {
            field: members.field_ref(field)?,
        },
        None => DeclarationBinding::Absent {
            origin: members.fields().absent(parent)?,
        },
    })
}
