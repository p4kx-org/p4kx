// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Guid;

use crate::contract::Blueprint;

/// Clears blueprint category links to selected development leftovers or unreadable Records.
pub(crate) fn unlink_categories(blueprints: &mut [Blueprint<'_>], missing: &HashSet<Guid>) {
    for blueprint in blueprints {
        if blueprint
            .category_guid
            .is_some_and(|category| missing.contains(&category.0))
        {
            blueprint.category_guid = None;
        }
    }
}
