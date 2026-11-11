// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, Value};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};
use crate::catalog_v3::tag_catalog::{ContextualTagPath, TagPathLookup, TagPathMeaning};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_projection::{TemplateTagInterpretation, TemplateTagProjection};
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_tag_cache::TemplateTagCache;
use std::cell::RefCell;

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    kind: FlowOwnerKind,
) -> Result<Vec<TemplateTagProjection>, CatalogError> {
    members
        .values()
        .iter()
        .filter(|fact| {
            let name = fact.field().name();
            tag_field(name)
                || (matches!(kind, FlowOwnerKind::MissionBrokerEntry) && name == "missionTags")
        })
        .filter(|fact| !matches!(fact.value(), Value::Struct(_) | Value::Array(_)))
        .map(|fact| one(members, references, tags, fact))
        .collect()
}

fn one<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
) -> Result<TemplateTagProjection, CatalogError> {
    let source = members.value_ref(fact)?;
    if matches!(
        fact.value(),
        Value::Reference(_) | Value::Error(p4kx_extraction::ValueError::DanglingReference { .. })
    ) {
        let sourced = tags
            .lookup(references, fact)?
            .map(|datum| datum.into_cell(members.fields().sources()));
        let (interpretation, path) = match sourced {
            TagPathLookup::Ready { meaning, value } => (
                match meaning {
                    TagPathMeaning::Path => TemplateTagInterpretation::Path,
                    TagPathMeaning::GuidFallback => TemplateTagInterpretation::GuidFallback,
                    TagPathMeaning::Inapplicable => TemplateTagInterpretation::Inapplicable,
                    TagPathMeaning::Unresolved => TemplateTagInterpretation::Unresolved,
                },
                value,
            ),
            TagPathLookup::Unavailable { value } => (TemplateTagInterpretation::Unavailable, value),
        };
        return Ok(TemplateTagProjection {
            source,
            interpretation,
            path: path?,
        });
    }
    let (interpretation, value, role, complete) = match fact.value() {
        Value::Error(_) => (
            TemplateTagInterpretation::Unreadable,
            SourceValue::Unreadable,
            SourceUseRole::Attempted,
            false,
        ),
        Value::Null => (
            TemplateTagInterpretation::Null,
            SourceValue::Absent,
            SourceUseRole::Selector,
            true,
        ),
        _ => (
            TemplateTagInterpretation::WrongShape,
            SourceValue::Absent,
            SourceUseRole::Selector,
            true,
        ),
    };
    Ok(TemplateTagProjection {
        source,
        interpretation,
        path: ContextualTagPath::immediate(
            fact.bind(OriginalValuePart::Value, value, role, complete)?
                .into_cell(members.fields().sources()),
        ),
    })
}

fn tag_field(name: &str) -> bool {
    matches!(
        name,
        "communicationTags"
            | "missionPhaseIdentifierTag"
            | "instanceTagIdentifier"
            | "ocTagsToSearchInstanceEnter"
            | "tags"
            | "positiveTags"
            | "negativeTags"
            | "markupTags"
            | "entityTags"
            | "resourceTags"
            | "tagFilters"
            | "spawnTags"
    )
}
