// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What an animation costs against the budget of values a GLB may hold.

use p4kx_model::Animation;

/// The floats one key writes at most: its time and a rotation (four) or a translation (three).
const VALUES_PER_KEY: usize = 5;

/// Returns the values the bone keys of `animation` write at most.
pub(crate) fn key_cost(animation: &Animation) -> usize {
    animation.key_count().saturating_mul(VALUES_PER_KEY)
}

/// Returns the reason an animation of `cost` values is left out when `remaining` are left; it
/// starts with `limit`, as the issue summary expects.
pub(crate) fn limit_reason(cost: usize, remaining: usize) -> String {
    format!("limit: the animation needs {cost} values, {remaining} of the budget are left")
}
