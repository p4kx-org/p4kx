// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{self, Display, Error as FormatError, Result as FormatResult, Write};

use crate::budget::Budget;
use crate::error::ToolError;

/// Formats once, admitting each actual written chunk before copying it.
pub(crate) fn display(budget: &mut Budget, value: &impl Display) -> Result<String, ToolError> {
    let mut output = Text {
        budget,
        rendered: String::new(),
        refusal: None,
    };
    let formatted = fmt::write(&mut output, format_args!("{value}"));
    if let Some(error) = output.refusal {
        return Err(error);
    }
    formatted.map_err(|source| ToolError::ResponseTextFormat { source })?;
    Ok(output.rendered)
}

struct Text<'budget> {
    budget: &'budget mut Budget,
    rendered: String,
    refusal: Option<ToolError>,
}

impl Write for Text<'_> {
    fn write_str(&mut self, text: &str) -> FormatResult {
        if self.refusal.is_some() {
            return Err(FormatError);
        }
        if let Err(error) = self.budget.admit_text(text.len()) {
            self.refusal = Some(error);
            return Err(FormatError);
        }
        self.rendered.push_str(text);
        Ok(())
    }
}

#[cfg(test)]
#[path = "budget_text_tests.rs"]
mod tests;
