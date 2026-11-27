// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::resource_group_v3_fixture as fixture;

#[path = "resource_group_query_failures.rs"]
mod failures;
#[path = "resource_group_query_parity.rs"]
mod parity;
#[path = "resource_group_query_scope.rs"]
mod scope;
#[path = "resource_group_query_selection.rs"]
mod selection;
#[path = "resource_group_query_texts.rs"]
mod texts;
#[path = "resource_group_query_walk.rs"]
mod walk;
