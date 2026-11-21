// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use crate::CatalogError;
use crate::catalog_read::original::OriginalFlowFields;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_plan::TemplatePlan;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;

/// The canonical selection and retained arena of one original mission owner.
pub(crate) struct PreparedOwner<'fields, 'scope, 'read, 'source> {
    pub(crate) plan: TemplatePlan<'fields, 'scope, 'read, 'source>,
    pub(crate) members: TemplateMembers<'fields, 'scope, 'read, 'source>,
    pub(crate) references: RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
}

impl<'fields, 'scope, 'read, 'source> PreparedOwner<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        capture: &'fields TemplateFieldCapture<'_, 'scope, 'read, 'source>,
        owner: FlowOwnerKind,
    ) -> Result<Self, CatalogError> {
        let mut references = RefCell::new(TemplateReferences::new(fields));
        let plan = TemplatePlan::read(fields, references.get_mut(), owner)?;
        let members = TemplateMembers::new(fields, plan.roots(), owner)?
            .with_warning_capture(capture.example_body());
        Ok(Self {
            plan,
            members,
            references,
        })
    }
}
