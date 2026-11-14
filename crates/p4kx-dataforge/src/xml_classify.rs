// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Field;
use crate::xml_text;
const RESERVED: [&str; 3] = ["guid", "path", "type"];

pub(crate) fn classify(fields: &[Field<'_>], attribute: &mut Vec<bool>, order: &mut Vec<usize>) {
    let base = attribute.len();
    attribute.extend(fields.iter().map(|field| {
        field.value().is_scalar()
            && xml_text::is_plain_name(field.name())
            && !RESERVED.contains(&field.name())
    }));
    order.clear();
    order.extend(
        attribute
            .get(base..)
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(index, attribute)| attribute.then_some(index)),
    );
    let name = |index: &usize| fields.get(*index).map(Field::name);
    order.sort_unstable_by(|left, right| name(left).cmp(&name(right)).then(left.cmp(right)));
    for pair in order.windows(2) {
        if let [first, second] = pair
            && name(first) == name(second)
            && let Some(flag) = attribute.get_mut(base + *second)
        {
            *flag = false;
        }
    }
}
