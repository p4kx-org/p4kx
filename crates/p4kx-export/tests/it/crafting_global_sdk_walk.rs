// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::crafting::{CraftingGlobalConfiguration, CraftingGlobalSnapshot};
use serde_json::Value;

use crate::canonical_header::{originals, wire::Wire};
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::{self as common, TestResult};

pub(super) fn snapshot(snapshot: &CraftingGlobalSnapshot<'_>, catalog: &Value) -> TestResult {
    for row in snapshot.rows() {
        let index = row
            .source()
            .record_index()
            .value()
            .ok_or("physical global index")?;
        let expected = common::at(catalog, "/configurations/craftingGlobal/rows")?
            .as_array()
            .ok_or("canonical crafting-global table")?
            .iter()
            .find(|row| {
                row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(*index))
            })
            .ok_or("canonical physical crafting-global row")?;
        configuration(row, Wire::new(expected, snapshot.source_inventory()))?;
    }
    Ok(())
}

fn configuration(row: &CraftingGlobalConfiguration<'_>, wire: Wire<'_>) -> TestResult {
    resource_consumer_walk::visit_header(row.source(), wire)?;
    wire.field("defaultBlueprintSelection")?.cell(
        row.default_blueprint_selection(),
        |value, wire| {
            wire.field("sourceType")?.leaf(value.source_type())?;
            wire.field("blueprintRecords")?
                .cell(value.blueprint_records(), |slots, wire| {
                    wire.array(slots, |slot, wire| originals::visit_record(slot, wire))
                })?;
            originals::visit_arena(value.additional_fields(), wire.field("additionalFields")?)
        },
    )?;
    wire.field("dismantleBlacklistResources")?
        .cell(row.dismantle_blacklist_resources(), |slots, wire| {
            wire.array(slots, |slot, wire| originals::visit_record(slot, wire))
        })?;
    wire.field("dismantleBlacklistEntityClasses")?
        .cell(row.dismantle_blacklist_entity_classes(), |slots, wire| {
            wire.array(slots, |slot, wire| originals::visit_record(slot, wire))
        })?;
    originals::visit_arena(row.additional_fields(), wire.field("additionalFields")?)
}
