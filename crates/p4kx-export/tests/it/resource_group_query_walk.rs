// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    ResourceGroupDefinition, ResourceGroupMembers, ResourceGroupParent,
};
use p4kx_extraction::SourceInventory;
use serde_json::Value;

use crate::canonical_header::wire::{TestResult, Wire};
use crate::catalog_resources::resource_consumer_walk::{visit_header, visit_reference};

pub(super) fn definition(
    row: &ResourceGroupDefinition<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    visit_header(row.header(), wire)?;
    wire.field("name")?.leaf(row.name())?;
    wire.field("description")?.leaf(row.description())?;
    wire.field("defaultThumbnailPath")?
        .leaf(row.default_thumbnail_path())?;
    wire.field("defaultThumbnailPathSVG")?
        .leaf(row.default_thumbnail_path_svg())
}

pub(super) fn members(
    row: &ResourceGroupMembers,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    wire.field("guid")?
        .cell(row.guid(), |guid, wire| wire.raw(&guid.get().to_string()))?;
    wire.field("recordIndex")?.leaf(row.record_index())?;
    wire.field("groups")?.cell(row.groups(), |slots, wire| {
        wire.array(slots, visit_reference)
    })?;
    wire.field("resources")?
        .cell(row.resources(), |slots, wire| {
            wire.array(slots, visit_reference)
        })
}

pub(super) fn parent(
    row: &ResourceGroupParent,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    wire.field("guid")?
        .cell(row.guid(), |guid, wire| wire.raw(&guid.get().to_string()))?;
    wire.field("recordIndex")?.leaf(row.record_index())?;
    wire.field("parent")?.cell(row.parent(), |parent, wire| {
        let Some(reference) = parent else {
            return wire.raw(&Option::<u8>::None);
        };
        visit_reference(reference, wire)
    })
}
