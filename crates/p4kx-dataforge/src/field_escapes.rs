// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Field;

/// Appends whether each original field name requires its ordinal escape.
pub(crate) fn append(fields: &[Field<'_>], escaped: &mut Vec<bool>, order: &mut Vec<usize>) {
    let base = escaped.len();
    escaped.extend(fields.iter().map(|field| field.name().starts_with('$')));
    order.clear();
    order.extend(0..fields.len());
    let name = |index: &usize| fields.get(*index).map(Field::name);
    order.sort_unstable_by(|left, right| name(left).cmp(&name(right)).then(left.cmp(right)));
    for pair in order.windows(2) {
        if let [first, second] = pair
            && name(first) == name(second)
            && let Some(flag) = escaped.get_mut(base + *second)
        {
            *flag = true;
        }
    }
}
