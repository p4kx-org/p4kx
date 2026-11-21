// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{DataForge, Reference, StructValue, TreeBuilder, Value as Original};
use serde_json::{Value, json};

use super::receipts::Receipt;
use super::{Result, SOURCE, TEMPLATE_KIND, by_index};
use crate::authored_definition_witness::field;
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_item_durability::{Trace, rows};

pub(super) const LOCAL: &str = "ObjectiveHandler_Local";
pub(super) const PLAYER: &str = "ObjectiveHandler_PlayerAttached";
const MODULE_FIELD: &str = "moduleDeclaration";
const HANDLER: &str = "objectiveHandler";

pub(super) struct Caller<'source> {
    index: u32,
    pub(super) path: &'source str,
    types: BTreeSet<&'source str>,
}

impl Caller<'_> {
    pub(super) fn has(&self, kind: &str) -> bool {
        self.types.contains(kind)
    }
}

pub(super) fn select<'source>(forge: &'source DataForge<'source>) -> Result<Vec<Caller<'source>>> {
    let mut builder = TreeBuilder::new();
    let mut callers = Vec::new();
    let mut seen = BTreeSet::new();
    for record in forge
        .records()
        .filter(|record| record.type_name().ok() == Some("ContractTemplate"))
    {
        let path = record.record_path()?;
        let canonical = path.replace('\\', "/").to_ascii_lowercase();
        if !canonical.starts_with("libs/foundry/records/")
            || p4kx_export::is_kind_leftover(TEMPLATE_KIND, path, record.name()?)
        {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|_| "read original objective Record")?;
        let mut types = BTreeSet::new();
        visit_handlers(tree.root(), |raw, handler| {
            if matches!(raw, Original::Reference(target) if target.type_name() == SOURCE) {
                types.insert(handler.type_name());
            }
        });
        if types.iter().any(|kind| !seen.contains(kind)) {
            seen.extend(types.iter().copied());
            callers.push(Caller {
                index: record.index(),
                path,
                types,
            });
        }
        if seen.len() == 2 {
            break;
        }
    }
    Ok(callers)
}

pub(super) fn verify(
    forge: &DataForge<'_>,
    receipt: &Receipt<'_>,
    catalog: &Value,
    caller: &Caller<'_>,
) -> Result<BTreeSet<u32>> {
    let record = forge
        .record(caller.index)
        .ok_or("original objective owner")?;
    let tree = record.tree().map_err(|_| "read original objective owner")?;
    let trace = receipt.trace(&tree);
    let published = consumer::rows(catalog, TEMPLATE_KIND)?;
    let row = by_index(
        published,
        caller.index,
        "the physical objective owner appears once",
    )?;
    let guid = field(row, "guid")?;
    assert!(
        consumer::present(guid)?.as_str() == Some(&record.guid().to_string()),
        "original objective owner GUID"
    );
    trace.header(guid, "guid", "payload")?;
    let inventory = rows(row, "/source/values")?;
    let mut targets = BTreeMap::new();
    let mut result = Ok(());
    visit_handlers(tree.root(), |raw, handler| {
        if result.is_ok() {
            result = compare_link(&trace, inventory, raw, handler).map(|target| {
                if let Some(target) = target {
                    targets.entry(handler.type_name()).or_insert(target);
                }
            });
        }
    });
    result?;
    assert!(
        caller.types.iter().copied().eq(targets.keys().copied()),
        "every selected positive original objective type has a matching module Reference"
    );
    Ok(targets.into_values().collect())
}

fn visit_handlers<'source>(
    tree: &StructValue<'source>,
    mut check: impl FnMut(&Original<'source>, &StructValue<'source>),
) {
    tree.visit_owned_values(|visit| {
        if visit.field_name() == HANDLER
            && let Original::Struct(handler) = visit.value()
            && matches!(handler.type_name(), LOCAL | PLAYER)
            && let Some(raw) = handler.field(MODULE_FIELD)
        {
            check(raw, handler);
        }
    });
}

fn compare_link(
    trace: &Trace<'_, '_>,
    inventory: &[Value],
    raw: &Original<'_>,
    handler: &StructValue<'_>,
) -> Result<Option<u32>> {
    let ordinal = trace.ordinal(raw)?;
    let mut nodes = inventory.iter().filter(|node| {
        node.get("sourceOrdinal")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<usize>().ok())
            == Some(ordinal)
    });
    let node = nodes
        .next()
        .ok_or("original objective module slot in canonical source arena")?;
    assert!(
        nodes.next().is_none(),
        "one raw value per original objective module slot"
    );
    let cell = field(node, "value")?;
    match raw {
        Original::Reference(target) if target.type_name() == SOURCE => {
            let payload = consumer::present(cell)?;
            assert_eq!(
                payload.get("type").and_then(Value::as_str),
                Some("recordReference"),
                "typed module Record link"
            );
            let target_row = field(payload, "value")?;
            verify_target(trace, raw, target_row, target)?;
            trace.check(cell, raw, "payload")?;
            Ok(Some(target.record_index()))
        }
        Original::Null => {
            assert!(
                consumer::source_value(cell)?.is_none()
                    && cell.get("state").and_then(Value::as_str) == Some("null"),
                "the original null module slot stays null"
            );
            trace.check(cell, raw, "payload")?;
            Ok(None)
        }
        Original::Error(_) => {
            assert!(
                consumer::source_value(cell)?.is_none()
                    && node
                        .get("originalError")
                        .is_some_and(|error| !error.is_null()),
                "the original failed module Reference retains its error"
            );
            Ok(None)
        }
        _ => Err(format!(
            "unsupported original objective module storage on {}",
            handler.type_name()
        )
        .into()),
    }
}

fn verify_target(
    trace: &Trace<'_, '_>,
    raw: &Original<'_>,
    row: &Value,
    target: &Reference<'_>,
) -> Result {
    assert_eq!(
        row.get("status").and_then(Value::as_str),
        Some("present"),
        "physical module target exists in source"
    );
    for (name, expected, header) in [
        ("guid", json!(target.guid().to_string()), None),
        (
            "targetRecordIndex",
            json!(target.record_index()),
            Some(("recordIndex", "selector")),
        ),
        (
            "recordPath",
            json!(target.record_path()),
            Some(("fileName", "conversion")),
        ),
        (
            "targetType",
            json!(target.type_name()),
            Some(("sourceType", "selector")),
        ),
    ] {
        let cell = field(row, name)?;
        assert!(
            consumer::present(cell)? == &expected,
            "exact original module target {name}"
        );
        let role = if name == "guid" {
            "payload"
        } else {
            "selector"
        };
        trace.check(cell, raw, role)?;
        if let Some((field, role)) = header {
            trace.header_for(cell, target.record_index(), field, role)?;
        }
    }
    Ok(())
}
