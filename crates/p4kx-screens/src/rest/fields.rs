// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Which fields of a widget a binding sets at rest and which values they take.

use p4kx_dataforge::Guid;

use super::Rest;
use super::eval::{Eval, TEXT_FIELD};
use super::graph::{Binding, CANVAS_FIELD, FieldKind};
use super::state::{AtRest, BindingOmission, PARAMETER, STATE_TAG};
use crate::report::ContentOmissionCode;
use crate::style::GeometryModifiers;
use crate::text::is_placeholder;

/// Reads the tag a node gave: a tag node's tag, or the empty text of a string parameter, which
/// names none.
fn tag_of(value: Rest) -> Result<Option<Guid>, String> {
    match value {
        Rest::Tag(tag) => Ok(tag),
        Rest::Text(text) if text.is_empty() => Ok(None),
        _ => Err("a tag that only the game names at run time".to_owned()),
    }
}

/// Returns whether a binding of the field `field` of the kind `kind` is evaluated at rest.
fn is_evaluated(kind: FieldKind, field: &str) -> bool {
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
fn layout_problem(field: &str, value: &Rest) -> Option<&'static str> {
    if !GeometryModifiers::has_field(field) {
        return None;
    }
    match value {
        Rest::Num(number) if GeometryModifiers::takes(field, *number) => None,
        Rest::Num(_) => Some("a number the layout field does not take"),
        _ => Some("a value that is not a number"),
    }
}

impl AtRest {
    pub(super) fn omit(
        &mut self,
        eval: &Eval<'_>,
        binding: &Binding,
        code: ContentOmissionCode,
        detail: Option<String>,
    ) {
        let variable = eval.direct_variable(binding.input);
        let source = variable
            .filter(|path| !path.trim().is_empty())
            .map(str::to_owned);
        self.omissions.push(BindingOmission {
            operation: binding.operation,
            variable: variable.and(binding.input),
            widget: binding.widget,
            field: binding.field.clone(),
            source,
            code,
            detail,
        });
    }

    fn bind_text(&mut self, eval: &mut Eval<'_>, binding: &Binding) -> Result<(), String> {
        if !(binding.field.starts_with(PARAMETER) || binding.field == TEXT_FIELD) {
            self.omit(eval, binding, ContentOmissionCode::UnsupportedField, None);
            return Ok(());
        }
        self.text_bound.insert(binding.widget);
        let Rest::Text(text) = eval.input(binding.input, 0)? else {
            return Err("a value that is not a text".to_owned());
        };
        let explicit = if let Some(path) = eval.direct_variable(binding.input) {
            if path.trim().is_empty() {
                self.omit(eval, binding, ContentOmissionCode::SourceMissing, None);
                return Ok(());
            }
            if eval.variable(path).is_none() {
                self.omit(eval, binding, ContentOmissionCode::VariableMissing, None);
                return Ok(());
            }
            true
        } else {
            false
        };
        if !is_placeholder(&text) || (explicit && text.is_empty()) {
            self.localise(binding.widget, &binding.field, text);
        }
        Ok(())
    }

    pub(super) fn apply_binding(
        &mut self,
        eval: &mut Eval<'_>,
        binding: &Binding,
    ) -> Result<(), String> {
        match (binding.kind, binding.field.as_str()) {
            (FieldKind::Localised, _) => self.bind_text(eval, binding)?,
            (FieldKind::Text, CANVAS_FIELD) => {
                self.canvases
                    .insert(binding.widget, eval.canvas(binding.input, 0)?);
            }
            (FieldKind::Text, field) if field.ends_with(STATE_TAG) => {
                self.tag(binding.widget, tag_of(eval.input(binding.input, 0)?)?);
            }
            (kind, field) if is_evaluated(kind, field) => {
                let value = eval.input(binding.input, 0)?;
                if let Some(why) = layout_problem(field, &value) {
                    return Err(why.to_owned());
                }
                let value = match (kind, value) {
                    (FieldKind::Integer, Rest::Num(number)) if field.starts_with(PARAMETER) => {
                        Rest::Integer(number)
                    }
                    (_, value) => value,
                };
                self.set(binding.widget, field, value)?;
            }
            _ => self.omit(eval, binding, ContentOmissionCode::UnsupportedField, None),
        }
        Ok(())
    }
}
