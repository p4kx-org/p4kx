// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceUseRole, SourceValue, StructValue, Value};

use crate::catalog_read::original::{OriginalValueEdge, OriginalValueFact, OriginalValuePart};
use crate::source_value_catalog::{self, SourceNode, SourcePayload};
use crate::template_reference_index::TemplateReferences;
use crate::template_source::{OriginalFieldPosition, TemplateSource, TemplateSourceRoot};
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn container<'source>(
    members: &TemplateMembers<'_, '_, '_, 'source>,
    fact: &OriginalValueFact<'_, '_, 'source>,
) -> Result<Option<SourceNode<'source>>, CatalogError> {
    let fields = members.fields();
    let payload = match fact.value() {
        Value::Struct(source) => struct_payload(members, fact, source)?,
        Value::Array(array) => {
            let mut elements = Vec::with_capacity(array.len());
            for index in 0..array.len() {
                let child = fields.child(
                    Some(fact.ordinal()),
                    OriginalValueEdge::Element {
                        ordinal: u64::try_from(index)
                            .map_err(|_| ExportAccountingError::CounterOverflow)?,
                    },
                )?;
                elements.push(members.value_ref(child)?);
            }
            SourcePayload::Array { elements }
        }
        _ => return Ok(None),
    };
    Ok(Some(SourceNode {
        value: fact
            .bind(
                OriginalValuePart::Value,
                SourceValue::Present(payload),
                SourceUseRole::Payload,
                true,
            )?
            .into_cell(fields.sources()),
        original_error: None,
    }))
}

fn struct_payload<'source>(
    members: &TemplateMembers<'_, '_, '_, 'source>,
    fact: &OriginalValueFact<'_, '_, 'source>,
    source: &StructValue<'source>,
) -> Result<SourcePayload<'source>, CatalogError> {
    let fields = members.fields();
    let source_type = fact
        .bind(
            OriginalValuePart::ConcreteType,
            SourceValue::Present(source.type_name()),
            SourceUseRole::Selector,
            true,
        )?
        .into_cell(fields.sources());
    let mut children = Vec::with_capacity(source.fields().len());
    for index in 0..source.fields().len() {
        let child = fields.child(
            Some(fact.ordinal()),
            OriginalValueEdge::Field {
                ordinal: u64::try_from(index)
                    .map_err(|_| ExportAccountingError::CounterOverflow)?,
            },
        )?;
        children.push(source_value_catalog::read_field(
            child,
            members.value_ref(child)?,
            fields.sources(),
        )?);
    }
    Ok(SourcePayload::Struct {
        source_type,
        fields: children,
    })
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    mut meaning: impl FnMut(
        &'fields OriginalValueFact<'scope, 'read, 'source>,
        SourceNode<'source>,
    ) -> Result<
        crate::template_source::TemplateSourceValue<'source>,
        CatalogError,
    >,
) -> Result<TemplateSource<'source>, CatalogError> {
    let sources = members.fields().sources();
    let retained_roots = members.roots();
    let mut roots = Vec::with_capacity(retained_roots.len());
    for fact in retained_roots {
        let OriginalValueEdge::Field { ordinal } = fact.edge() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        roots.push(TemplateSourceRoot {
            source_position: OriginalFieldPosition {
                parent_ordinal: fact.parent_ordinal().map(DecimalU64::new),
                field_index: DecimalU64::new(ordinal),
            },
            field: source_value_catalog::read_field(fact, members.value_ref(fact)?, sources)?,
        });
    }
    let mut values = Vec::with_capacity(members.value_count());
    for fact in members.values() {
        members.fields().check_cancel()?;
        let raw = match container(members, fact)? {
            Some(raw) => raw,
            None => references.raw(fact)?,
        };
        values.push(meaning(fact, raw)?);
    }
    Ok(TemplateSource { roots, values })
}
