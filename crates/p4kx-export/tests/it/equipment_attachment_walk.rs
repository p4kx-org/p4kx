// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{ItemAttachment, ItemAttachmentComponent};
use p4kx_export::catalog_queries::{AuthoredRestriction, ResourceNumber};
use p4kx_extraction::SourceInventory;
use serde_json::Value;

use crate::canonical_header::originals;
use crate::canonical_header::wire::Wire;
use crate::commodity_v3_fixture::TestResult;

pub(super) fn row(
    row: &ItemAttachmentComponent<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    wire.field("ownerGuid")?.guid(&row.owner_guid())?;
    wire.field("ownerRecordIndex")?
        .leaf(row.owner_record_index())?;
    wire.field("componentOrdinal")?
        .leaf(row.component_ordinal())?;
    wire.field("definition")?.cell(row.definition(), definition)
}

fn definition(value: &ItemAttachment<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("itemType")?.cell(value.item_type(), label)?;
    wire.field("itemSubtype")?
        .cell(value.item_subtype(), label)?;
    wire.field("size")?.cell(value.size(), number)?;
    wire.field("tags")?.leaf(value.tags())?;
    wire.field("requiredTags")?.leaf(value.required_tags())?;
    let axis = wire.field("attachAxesToAlign")?;
    axis.field("label")?.leaf(value.alignment_axis())?;
    membership(
        axis.field("optionStatus")?,
        value.alignment_axis_option_status(),
    )?;
    let ignored = wire.field("ignoredAttachAxis")?;
    ignored
        .field("label")?
        .leaf(value.ignored_alignment_axis())?;
    membership(
        ignored.field("optionStatus")?,
        value.ignored_alignment_axis_option_status(),
    )?;
    wire.field("attachCheckCenterOfMass")?
        .leaf(value.attach_check_center_of_mass())
}

fn label(value: &AuthoredRestriction<'_, &str>, wire: Wire<'_>) -> TestResult {
    match value {
        AuthoredRestriction::Typed(value) => wire.raw(value),
        AuthoredRestriction::Original(value) => originals::visit_arena(value, wire),
        _ => Err("a new original restriction branch needs a typed traversal".into()),
    }
}

fn number(value: &AuthoredRestriction<'_, ResourceNumber>, wire: Wire<'_>) -> TestResult {
    match value {
        AuthoredRestriction::Typed(value) => wire.number(value),
        AuthoredRestriction::Original(value) => originals::visit_arena(value, wire),
        _ => Err("a new numeric restriction branch needs a typed traversal".into()),
    }
}

fn membership(
    wire: Wire<'_>,
    value: Option<
        &p4kx_extraction::SourcedValue<p4kx_export::catalog_queries::AttachmentAxisOptionStatus>,
    >,
) -> TestResult {
    value.map_or_else(|| wire.raw(&Option::<()>::None), |cell| wire.leaf(cell))
}
