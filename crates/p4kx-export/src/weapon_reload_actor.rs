// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, Record, SourceLocation, SourceUseRole, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::template_field_capture::TemplateFieldCapture;

pub(super) const COMPONENT: &str = "SAnimatedCharacterComponentParams";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlayerMotion<'source> {
    #[serde(flatten)]
    pub(super) source: CanonicalRecordHeader<'source>,
    pub(super) components: SourcedValue<Vec<SourcedValue<Option<MotionComponent>>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MotionComponent {
    pub(super) ordinal: SourcedValue<u32>,
    pub(super) action_controller: SourcedValue<String>,
    #[serde(rename = "animDatabase3P")]
    pub(super) anim_database_3p: SourcedValue<String>,
}

pub(super) fn read<'source>(
    candidates: &[Record<'source>],
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<SourcedValue<Vec<PlayerMotion<'source>>>, CatalogError> {
    let source = input.origins().dataforge.at(
        SourceLocation::Metadata {
            field: "entityClassIndex".into(),
        },
        SourceUseRole::Selector,
    )?;
    let sources = &input.origins().uses;
    if candidates.is_empty() {
        return Ok(sources.value(SourceValue::Absent, &[source], true));
    }
    let mut rows = Vec::new();
    for candidate in candidates {
        super::document::check_cancel(cancel)?;
        let Ok(tree) = candidate.tree() else {
            return Ok(sources.value(SourceValue::Unreadable, &[source], false));
        };
        let admission = RecordObservation::capture(input, candidate)?;
        let scope = input.record_scope(candidate, &tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let capture = TemplateFieldCapture::new(&originals, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let observed = Observed::configuration(&originals, input, &capture, &numbers);
        let components =
            observed.declared_owned_array("Components", &mut Vec::new(), |component, notes| {
                if component.source_type()? != COMPONENT {
                    return Ok(None);
                }
                Ok(Some(MotionComponent {
                    ordinal: component.array_ordinal()?,
                    action_controller: component
                        .borrowed_string("actionController", notes)?
                        .map(str::to_owned),
                    anim_database_3p: component
                        .borrowed_string("animDatabase3P", notes)?
                        .map(str::to_owned),
                }))
            })?;
        rows.push(PlayerMotion {
            source: CanonicalRecordHeader::read(&tree, input.origins())?,
            components,
        });
    }
    Ok(sources.value(SourceValue::Present(rows), &[source], true))
}
