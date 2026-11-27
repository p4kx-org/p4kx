// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses,
};

use crate::report::{ContentOmission, OmissionInput};
use crate::screen_asset_inputs::WidgetInput;

/// The authored fields known to contribute to a Screen diagnostic.
#[derive(Debug)]
#[non_exhaustive]
pub struct ContentOmissionOrigin {
    /// Authored widget type and its physical position.
    pub widget_type: FieldOrigin,
    /// Authored widget name or the declaration supplying its empty default.
    pub widget_name: FieldOrigin,
    /// Original widget field or binding operation that supplied the field name.
    pub field: FieldOrigin,
    /// The original authored runtime target or variable path, when present.
    pub source: Option<FieldOrigin>,
    /// The known contributions to the omission decision.
    pub decision: FieldOrigin,
}

impl ContentOmissionOrigin {
    pub(crate) fn capture(
        omission: &ContentOmission,
        scope: &SourceScope,
        registry: &SourceUses,
    ) -> Result<Self, SourceEvidenceError> {
        let widget = omission.widget_input;
        let kind = scope.at(
            SourceLocation::Subresource {
                namespace: "dataforgeWidgetType".into(),
                indices: vec![u64::from(widget.record_index), widget.slot as u64],
                field: widget.section.field_name().into(),
            },
            SourceUseRole::Selector,
        )?;
        let widget_type = registry.origin(std::slice::from_ref(&kind), true);
        let widget_name = if omission.widget.is_empty() {
            widget_type.clone()
        } else {
            let name = original(scope, widget, widget_field(widget, "name"))?;
            registry.origin(&[name], true)
        };
        let (field_use, source_use) = authored(omission, scope)?;
        let field = registry.origin(std::slice::from_ref(&field_use), true);
        let source = omission
            .source
            .as_ref()
            .and(source_use.as_ref())
            .map(|used| registry.origin(std::slice::from_ref(used), true));
        let inputs: Vec<_> = [kind, field_use].into_iter().chain(source_use).collect();
        Ok(Self {
            widget_type,
            widget_name,
            field,
            source,
            decision: registry.origin(&inputs, false),
        })
    }
}

fn authored(
    omission: &ContentOmission,
    scope: &SourceScope,
) -> Result<(SourceUse, Option<SourceUse>), SourceEvidenceError> {
    let widget = omission.widget_input;
    match omission.input {
        OmissionInput::WidgetField(name) => {
            let field = original(scope, widget, widget_field(widget, name))?;
            let source = omission.source.as_ref().map(|_| field.clone());
            Ok((field, source))
        }
        OmissionInput::Binding {
            operation,
            variable,
        } => {
            let field = original(scope, widget, format!("operations[{operation}].field"))?;
            let source = variable
                .map(|at| original(scope, widget, format!("operations[{at}].binding")))
                .transpose()?;
            Ok((field, source))
        }
    }
}

fn widget_field(widget: WidgetInput, name: &str) -> String {
    format!("{}[{}].{name}", widget.section.field_name(), widget.slot)
}

fn original(
    scope: &SourceScope,
    widget: WidgetInput,
    field: String,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::DataForge {
            record_index: widget.record_index,
            field,
        },
        SourceUseRole::Selector,
    )
}
