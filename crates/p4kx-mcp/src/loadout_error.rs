// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutError, Target};

use crate::budget::Budget;
use crate::budget_text;
use crate::error::ToolError;
use crate::loadout_shapes::ErrorShape;

pub(crate) fn shape(
    error: &LoadoutError,
    target: Option<&Target>,
    text: &mut Budget,
) -> Result<ErrorShape, ToolError> {
    let (reference, class_name) = match target {
        Some(Target::Reference(guid)) => (Some(budget_text::display(text, guid)?), None),
        Some(Target::ClassName(name)) => (None, Some(text.text(name)?)),
        _ => (None, None),
    };
    Ok(ErrorShape {
        kind: error.kind(),
        reason: budget_text::display(text, error)?,
        reference,
        class_name,
    })
}
