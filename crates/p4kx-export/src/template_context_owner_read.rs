// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::template_projection::TemplateOwnerProjection;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, CatalogReadFailure};

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    input: &OriginalCatalogInput<'_, '_>,
    ledger: &mut ExportLedger,
    failures: &mut Vec<CatalogReadFailure<'source>>,
    kind: FlowOwnerKind,
) -> Result<Option<TemplateOwnerProjection<'source>>, CatalogError> {
    let Some((_, owner)) = members.fields().field(None, "owner")? else {
        return Ok(None);
    };
    if !matches!(owner.value(), Value::Reference(reference) if reference.location().is_none()) {
        return Ok(None);
    }
    let captured = references.observation(owner)?.class_name(input)?;
    if let Some(error) = captured.error {
        let admission = RecordObservation::capture(input, &captured.record)?;
        ledger.observe(
            admission
                .source(
                    Some(kind.catalog_kind()),
                    ExportSourceRole::Dependency,
                    SourceDisposition::ReadFailed,
                )?
                .diagnosed(crate::export_source_failure::diagnosis(&error)),
        );
        failures.push(CatalogReadFailure::of(&captured.record, &error));
    }
    Ok(Some(TemplateOwnerProjection {
        source: members.value_ref(owner)?,
        class_name: captured.name.into_cell(members.fields().sources()),
    }))
}
