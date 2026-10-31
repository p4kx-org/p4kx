// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Which fields of a widget a binding sets at rest and which values they take.

use p4kx_dataforge::Guid;

use super::Rest;
use super::graph::FieldKind;
use super::state::PARAMETER;
use crate::style::GeometryModifiers;

/// Reads the tag a node gave: a tag node's tag, or the empty text of a string parameter, which
/// names none.
pub(super) fn tag_of(value: Rest) -> Result<Option<Guid>, String> {
    match value {
        Rest::Tag(tag) => Ok(tag),
        Rest::Text(text) if text.is_empty() => Ok(None),
        _ => Err("a tag that only the game names at run time".to_owned()),
    }
}

/// Returns whether a binding of the field `field` of the kind `kind` is evaluated at rest.
pub(super) fn is_evaluated(kind: FieldKind, field: &str) -> bool {
    match (kind, field) {
        (FieldKind::Other, _) => false,
        (FieldKind::Boolean, "Instantiated" | "IsActive") | (FieldKind::Number, "Alpha") => true,
        (FieldKind::Number | FieldKind::Integer, field) if GeometryModifiers::has_field(field) => {
            true
        }
        (_, field) => field.starts_with(PARAMETER),
    }
}

/// Returns why `value` cannot be the value of the layout field `field`; `None` for any other
/// field and for a value it takes.
pub(super) fn layout_problem(field: &str, value: &Rest) -> Option<&'static str> {
    if !GeometryModifiers::has_field(field) {
        return None;
    }
    match value {
        Rest::Num(number) if GeometryModifiers::takes(field, *number) => None,
        Rest::Num(_) => Some("a number the layout field does not take"),
        _ => Some("a value that is not a number"),
    }
}
