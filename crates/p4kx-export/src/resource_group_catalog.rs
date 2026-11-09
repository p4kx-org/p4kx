// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "resource_group_contract.rs"]
mod contract;
#[path = "resource_group_member_read.rs"]
mod members;
#[path = "resource_group_parent_read.rs"]
mod parent;

pub(crate) use contract::{
    ResourceGroup, ResourceGroupDefinition, ResourceGroupMembers, ResourceGroupParent,
};
pub(crate) use members::read as members;
pub(crate) use parent::read as parent;

use p4kx_extraction::DataForge;

use crate::catalog_table::CatalogTable;

pub(crate) struct ResourceGroupTables<'source> {
    pub(crate) definitions: CatalogTable<ResourceGroupDefinition<'source>>,
    pub(crate) members: CatalogTable<ResourceGroupMembers>,
    pub(crate) parents: CatalogTable<ResourceGroupParent>,
}

/// Prepares all group datasets from immutable rows with one shared source-family inventory.
pub(crate) fn tables<'source>(
    forge: &DataForge<'_>,
    rows: Vec<ResourceGroup<'source>>,
    members: Vec<ResourceGroupMembers>,
) -> ResourceGroupTables<'source> {
    let (definitions, parents) = rows
        .into_iter()
        .map(|row| (row.definition, row.parent))
        .unzip();
    let definitions = CatalogTable::new(forge, &["ResourceTypeGroup"], definitions);
    let availability = definitions.availability;
    ResourceGroupTables {
        definitions,
        members: CatalogTable {
            availability,
            rows: members,
        },
        parents: CatalogTable {
            availability,
            rows: parents,
        },
    }
}
