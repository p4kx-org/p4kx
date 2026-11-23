// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUse, SourceUseRole, Value};

use crate::CatalogError;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::digest::{Wide, hash_value_observed};
use crate::variant_evidence::VariantEvidence;

use super::{ATTACHABLE, ComponentKeyRead, PER_ITEM_COMPONENTS, skipped_fields};

pub(crate) fn capture_component_key<'source>(
    tree: &RecordTree<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    status: ComponentKeyRead<'source>,
) -> Result<VariantEvidence, CatalogError> {
    originals.check_tree(tree)?;
    let mut sources = originals.component_selection(None)?.uses;
    if let Some(Value::Array(components)) = tree.field("Components") {
        match status {
            ComponentKeyRead::Ineligible => attachment_inputs(originals, components, &mut sources)?,
            _ => compared_inputs(originals, components, &mut sources)?,
        }
    }
    Ok(VariantEvidence::from_uses(
        originals.sources(),
        &sources,
        !matches!(status, ComponentKeyRead::Unavailable),
    ))
}

fn compared_inputs<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    components: &[Value<'source>],
    sources: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    let mut hasher = Wide::new();
    for value in components {
        let Value::Struct(body) = value else { continue };
        let name = body.type_name();
        if !PER_ITEM_COMPONENTS.contains(&name) {
            let skip = skipped_fields(name);
            hash_value_observed(
                &mut hasher,
                value,
                &|field| skip.contains(&field),
                &mut |value| {
                    let fact = originals.fact(value)?;
                    field_input(originals, fact, sources)
                },
            )?;
        }
    }
    Ok(())
}

fn attachment_inputs<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    components: &[Value<'source>],
    sources: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    let attachment = components
        .iter()
        .find(|value| matches!(value, Value::Struct(body) if body.type_name() == ATTACHABLE));
    let Some(attachment) = attachment else {
        return Ok(());
    };
    let attachment = originals.fact(attachment)?;
    let Some((_, definition)) = originals.field(Some(attachment), "AttachDef")? else {
        sources.push(originals.survey(Some(attachment))?);
        return Ok(());
    };
    field_input(originals, definition, sources)?;
    if matches!(definition.value(), Value::Struct(_)) {
        if let Some((_, kind)) = originals.field(Some(definition), "Type")? {
            field_input(originals, kind, sources)?;
        } else {
            sources.push(originals.survey(Some(definition))?);
        }
    }
    Ok(())
}

fn field_input<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    fact: &OriginalValueFact<'_, '_, 'source>,
    sources: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    if matches!(fact.edge(), OriginalValueEdge::Field { .. }) {
        sources.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
    }
    if fact.observe_unresolved_link(sources)? {
        return Ok(());
    }
    let (part, role) = match fact.value() {
        Value::Error(_) => (OriginalValuePart::Value, SourceUseRole::Attempted),
        Value::Locale(_) => (OriginalValuePart::DataType, SourceUseRole::Selector),
        Value::Struct(_) => {
            sources.push(originals.survey(Some(fact))?);
            (OriginalValuePart::ConcreteType, SourceUseRole::Selector)
        }
        _ => (OriginalValuePart::Value, SourceUseRole::Selector),
    };
    sources.push(fact.observe(part, role)?);
    Ok(())
}
