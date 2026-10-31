// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::picture::{CUSTOM_SHAPE, ICON};
use crate::style::{Modifiers, Subject, resolve_part};
use crate::text::{TEXT, TEXT_FIELD};
use crate::walk::Walk;
use crate::widget::Widget;

/// Returns the type of the widget a widget of the type `type_name` holds and draws with: the text
/// of a text field, the shape of an icon.
fn part_type(type_name: &str) -> Option<&'static str> {
    match type_name {
        TEXT_FIELD => Some(TEXT),
        ICON => Some(CUSTOM_SHAPE),
        _ => None,
    }
}

impl Walk<'_, '_, '_, '_> {
    /// Resolves the entries that select the part a widget holds, with the widget as its parent:
    /// those that require the type of the part and those that name no type and select by the
    /// parent. `None` for a widget that holds no part.
    pub(crate) fn inner_part(&mut self, widget: &Widget, outer: &Subject) -> Option<Modifiers> {
        let part = Subject::of(part_type(&widget.type_name)?, &[]);
        self.above.push(outer.clone());
        let found = resolve_part(
            &self.sheets,
            (&part, &self.above),
            self.tags,
            &mut self.budget,
        );
        self.above.pop();
        Some(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_field_holds_a_text_and_an_icon_a_custom_shape() {
        assert_eq!(part_type(TEXT_FIELD), Some(TEXT));
        assert_eq!(part_type(ICON), Some(CUSTOM_SHAPE));
        assert_eq!(part_type(CUSTOM_SHAPE), None);
    }
}
