// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{RecordTree, StructValue, Value, ValueError};
use p4kx_extraction::{SourceLocation, SourceValue};

/// Type and subtype selected from an Entity's original `AttachDef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemClassification {
    record: Option<u32>,
    component: Option<usize>,
    item_type: Field,
    subtype: Field,
    scope: Scope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Components,
    Definition,
    Field,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    value: SourceValue<String>,
    invalid_offset: Option<u32>,
}

impl Field {
    const fn of(value: SourceValue<String>) -> Self {
        Self {
            value,
            invalid_offset: None,
        }
    }
}

impl ItemClassification {
    pub(crate) fn read(
        tree: &RecordTree<'_>,
        attachable: Option<(usize, &StructValue<'_>)>,
    ) -> Self {
        let (component, item_type, subtype, scope) = if let Some((position, component)) = attachable
        {
            let (item_type, subtype, scope) = definition(component.field("AttachDef"));
            (Some(position), item_type, subtype, scope)
        } else {
            let state = match tree.field("Components") {
                None | Some(Value::Null) => SourceValue::Absent,
                Some(Value::Array(values))
                    if !values.iter().any(|value| matches!(value, Value::Error(_))) =>
                {
                    SourceValue::Absent
                }
                Some(Value::Array(_) | Value::Error(_)) => SourceValue::Unreadable,
                Some(_) => SourceValue::Invalid,
            };
            (
                None,
                Field::of(state.clone()),
                Field::of(state),
                Scope::Components,
            )
        };
        Self {
            record: Some(tree.record_index()),
            component,
            item_type,
            subtype,
            scope,
        }
    }

    pub(crate) const fn unresolved() -> Self {
        Self {
            record: None,
            component: None,
            item_type: Field::of(SourceValue::Unreadable),
            subtype: Field::of(SourceValue::Unreadable),
            scope: Scope::Unresolved,
        }
    }

    /// Returns the exact original Type value and disposition.
    #[must_use]
    pub const fn item_type(&self) -> &SourceValue<String> {
        &self.item_type.value
    }

    /// Returns the exact original `SubType` value and disposition.
    #[must_use]
    pub const fn subtype(&self) -> &SourceValue<String> {
        &self.subtype.value
    }

    /// Returns the authored coordinate establishing Type or its disposition.
    #[must_use]
    pub fn item_type_location(&self) -> Option<SourceLocation> {
        self.location("Type", &self.item_type)
    }

    /// Returns the authored coordinate establishing `SubType` or its disposition.
    #[must_use]
    pub fn subtype_location(&self) -> Option<SourceLocation> {
        self.location("SubType", &self.subtype)
    }

    pub(crate) const fn component(&self) -> Option<usize> {
        self.component
    }

    pub(crate) fn item_type_text(&self) -> &str {
        text(&self.item_type.value)
    }

    pub(crate) fn subtype_text(&self) -> &str {
        text(&self.subtype.value)
    }

    fn location(&self, member: &str, field: &Field) -> Option<SourceLocation> {
        let record_index = self.record?;
        if matches!(field.value, SourceValue::Unreadable) {
            return Some(SourceLocation::Subresource {
                namespace: "dataForgeStringOffset".into(),
                indices: vec![
                    u64::from(record_index),
                    self.component? as u64,
                    u64::from(field.invalid_offset?),
                ],
                field: format!("AttachDef.{member}"),
            });
        }
        let field = match self.scope {
            Scope::Components => "Components".to_owned(),
            Scope::Definition => format!("Components[{}].AttachDef", self.component?),
            Scope::Field if matches!(field.value, SourceValue::Absent) => {
                format!("Components[{}].AttachDef", self.component?)
            }
            Scope::Field => format!("Components[{}].AttachDef.{member}", self.component?),
            Scope::Unresolved => return None,
        };
        Some(SourceLocation::DataForge {
            record_index,
            field,
        })
    }
}

fn definition(value: Option<&Value<'_>>) -> (Field, Field, Scope) {
    let state = match value {
        Some(Value::Struct(fields)) => {
            return (
                field(fields.field("Type")),
                field(fields.field("SubType")),
                Scope::Field,
            );
        }
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        Some(_) => SourceValue::Invalid,
    };
    (
        Field::of(state.clone()),
        Field::of(state),
        Scope::Definition,
    )
}

fn field(value: Option<&Value<'_>>) -> Field {
    let value = match value {
        Some(Value::Error(ValueError::InvalidString { offset, .. })) => {
            // DataForge decodes the original offset slot before trying to read its text.
            return Field {
                value: SourceValue::Unreadable,
                invalid_offset: Some(*offset),
            };
        }
        Some(Value::String(text) | Value::Locale(text) | Value::Enum(text)) => {
            SourceValue::Present((*text).to_owned())
        }
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        Some(_) => SourceValue::Invalid,
    };
    Field::of(value)
}

fn text(value: &SourceValue<String>) -> &str {
    match value {
        SourceValue::Present(value) => value,
        _ => "",
    }
}

impl crate::Part {
    /// Returns the selected classification of an Entity part.
    #[must_use]
    pub const fn classification(&self) -> Option<&ItemClassification> {
        self.classification.as_ref()
    }
}

impl crate::Assembly {
    /// Returns the original DataForge payload supplying the selected Entity records.
    #[must_use]
    pub fn forge_origin(&self) -> Option<&p4kx_extraction::DataForgeOrigin> {
        self.forge_origin.as_deref()
    }
}
