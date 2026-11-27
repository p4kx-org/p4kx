// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "resource_group_query_api.rs"]
mod api;
#[path = "resource_group_query_direct.rs"]
mod direct;
#[path = "resource_group_query_parent_api.rs"]
mod parent_api;
#[path = "resource_group_query_parents.rs"]
mod parents;
#[path = "resource_group_query_request.rs"]
mod request;
#[path = "resource_group_query_snapshot.rs"]
mod snapshot;

pub use crate::catalog_v3::resource_group_catalog::{
    ResourceGroupDefinition, ResourceGroupMembers, ResourceGroupParent,
};
pub use api::{with_resource_groups, with_resource_groups_from};
pub use parent_api::{with_resource_group_parents, with_resource_group_parents_from};
pub use snapshot::{ResourceGroupParentSnapshot, ResourceGroupSnapshot};

#[cfg(test)]
#[path = "resource_group_query_tests.rs"]
mod tests;
