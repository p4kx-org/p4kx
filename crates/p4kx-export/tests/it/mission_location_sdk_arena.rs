// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    ResourceNumber, SourceArenaOutput, SourceField, SourceNode, SourcePayload, SourceValueRef,
};
use serde_json::{Value, json};

use super::{TestResult, proof::View};

impl View<'_> {
    pub(super) fn arena(&mut self, arena: &SourceArenaOutput<'_>) -> TestResult<Value> {
        check_links(arena)?;
        Ok(json!({
            "roots": arena.roots().iter().map(|field| self.field(field))
                .collect::<TestResult<Vec<_>>>()?,
            "values": arena.values().iter().map(|node| self.node(node))
                .collect::<TestResult<Vec<_>>>()?,
        }))
    }

    fn field(&mut self, field: &SourceField<'_>) -> TestResult<Value> {
        let declaration = field.declaration();
        Ok(json!({
            "name": self.scalar(field.name())?,
            "declaration": {
                "dataType": self.scalar(declaration.data_type())?,
                "isArray": self.scalar(declaration.is_array())?,
                "target": self.scalar(declaration.target())?,
            },
            "value": {"valueIndex": field.value().value_index().to_string()},
        }))
    }

    fn node(&mut self, node: &SourceNode<'_>) -> TestResult<Value> {
        Ok(json!({
            "value": self.cell(node.value(), Self::payload)?,
            "originalError": node.original_error(),
        }))
    }

    fn payload(&mut self, payload: &SourcePayload<'_>) -> TestResult<Value> {
        Ok(match payload {
            SourcePayload::Struct {
                source_type,
                fields,
            } => json!({
                "type":"struct", "sourceType":self.scalar(source_type)?,
                "fields": fields.iter().map(|field| self.field(field)).collect::<TestResult<Vec<_>>>()?,
            }),
            SourcePayload::Array { elements } => json!({
                "type":"array", "elements": elements.iter().map(|value|
                    json!({"valueIndex":value.value_index().to_string()})).collect::<Vec<_>>(),
            }),
            SourcePayload::Boolean { value } => json!({"type":"boolean", "value":value}),
            SourcePayload::Number { value } => json!({"type":"number", "value":value}),
            SourcePayload::String { value } => json!({"type":"string", "value":value}),
            SourcePayload::Locale { value } => json!({"type":"locale", "value":value}),
            SourcePayload::Enum { value } => json!({"type":"enum", "value":value}),
            SourcePayload::Guid { value } => {
                json!({"type":"guid", "value":value.get().to_string()})
            }
            SourcePayload::RecordReference { value } => {
                json!({"type":"recordReference", "value":self.reference(value)?})
            }
            SourcePayload::WeakReference { value } => {
                json!({"type":"weakReference", "value":value})
            }
            _ => {
                return Err(
                    "the consumer must deliberately handle a new authored source payload".into(),
                );
            }
        })
    }
}

fn check_links(arena: &SourceArenaOutput<'_>) -> TestResult {
    for field in arena.roots() {
        require_link(arena, field.value())?;
    }
    for node in arena.values() {
        match node.value().value() {
            Some(SourcePayload::Struct { fields, .. }) => {
                for field in fields {
                    require_link(arena, field.value())?;
                }
            }
            Some(SourcePayload::Array { elements }) => {
                for &reference in elements {
                    require_link(arena, reference)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn require_link(arena: &SourceArenaOutput<'_>, reference: SourceValueRef) -> TestResult {
    arena
        .value(reference)
        .ok_or("an original arena link must resolve in its owner")?;
    Ok(())
}

pub(super) fn assert_int32(arena: &SourceArenaOutput<'_>, expected: i32) -> TestResult {
    let field = arena.roots().first().ok_or("original authored field")?;
    let node = arena
        .value(field.value())
        .ok_or("original authored value")?;
    let Some(SourcePayload::Number {
        value: ResourceNumber::Int32 { value },
    }) = node.value().value()
    else {
        return Err("the original authored value must remain an exact typed Int32".into());
    };
    assert_eq!(
        *value, expected,
        "the consumer reads the authored scalar without a JSON bridge"
    );
    Ok(())
}
