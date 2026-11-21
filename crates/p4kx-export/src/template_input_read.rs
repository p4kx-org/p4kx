// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::template_input::{TemplateInput, TemplateInputs};
use crate::template_source_members::TemplateMembers;
use crate::template_source_refs::DeclarationBinding;

pub(crate) fn read(
    members: &TemplateMembers<'_, '_, '_, '_>,
) -> Result<TemplateInputs, CatalogError> {
    Ok(TemplateInputs {
        not_for_release: one(members, "notForRelease")?,
        work_in_progress: one(members, "workInProgress")?,
        display_info: one(members, "contractDisplayInfo")?,
        contract_class: one(members, "contractClass")?,
        objective_tokens: one(members, "objectiveTokens")?,
        partial_reward_payout: one(members, "partialRewardPayout")?,
        modifiers: one(members, "modifiers")?,
        owner: one(members, "owner")?,
        start_comms: one(members, "contractStartCommsNotification")?,
        end_comms: one(members, "contractEndCommsNotifications")?,
    })
}

fn one(
    members: &TemplateMembers<'_, '_, '_, '_>,
    name: &str,
) -> Result<TemplateInput, CatalogError> {
    let fields = members.fields();
    let Some((_, fact)) = fields.field(None, name)? else {
        return Ok(TemplateInput {
            declaration: DeclarationBinding::Absent {
                origin: fields.absent(None)?,
            },
            selected_struct: None,
        });
    };
    Ok(TemplateInput {
        declaration: DeclarationBinding::Present {
            field: members.field_ref(fact)?,
        },
        selected_struct: fields
            .first_struct(fact)?
            .map(|value| members.value_ref(value))
            .transpose()?,
    })
}
