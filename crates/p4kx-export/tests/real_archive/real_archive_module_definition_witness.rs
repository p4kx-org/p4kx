// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, StructValue, Value as Original};
use serde_json::{Value, json};

use super::receipts::{self, Receipt};
use super::{Result, SOURCE};
use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_item_durability::Trace;

pub(super) fn header(receipt: &Receipt<'_>, tree: &RecordTree<'_>, row: &Value) -> Result {
    let trace = receipt.trace(tree);
    for (name, expected, role) in [
        ("guid", json!(tree.guid().to_string()), "payload"),
        ("recordIndex", json!(tree.record_index()), "selector"),
        ("sourceType", json!(SOURCE), "selector"),
    ] {
        let cell = field(row, name)?;
        assert!(
            consumer::present(cell)? == &expected,
            "exact original module {name}"
        );
        trace.header(cell, name, role)?;
    }
    Ok(())
}

pub(super) fn verify(
    forge: &DataForge<'_>,
    receipt: &Receipt<'_>,
    tree: &RecordTree<'_>,
    row: &Value,
) -> Result {
    header(receipt, tree, row)?;
    let trace = receipt.trace(tree);
    let source = tree.root();
    receipts::scalar(&trace, row, "module", source.field("module"))?;
    owned(
        &trace,
        source.field("moduleType"),
        field(row, "moduleType")?,
        |source, row| module_type(forge, &trace, source, row),
    )?;
    for (name, compare) in [
        ("locationEntities", location as Compare),
        ("properties", property),
        ("spawnedEntities", spawn),
        ("declaredVariables", variable),
    ] {
        slots(
            &trace,
            source.field(name),
            field(row, name)?,
            |source, row| compare(forge, &trace, source, row),
        )?;
    }
    Ok(())
}

type Compare =
    for<'source> fn(&DataForge<'_>, &Trace<'_, 'source>, &StructValue<'source>, &Value) -> Result;

fn slots<'source>(
    trace: &Trace<'_, 'source>,
    source: Option<&Original<'source>>,
    row: &Value,
    compare: impl Fn(&StructValue<'source>, &Value) -> Result,
) -> Result {
    if let Some(raw @ Original::Array(_)) = source {
        trace.check(row, raw, "selector")?;
    }
    verify_array(source, row, |raw, cell| owned(trace, raw, cell, &compare))
}

fn owned<'source>(
    trace: &Trace<'_, 'source>,
    source: Option<&Original<'source>>,
    row: &Value,
    compare: impl FnOnce(&StructValue<'source>, &Value) -> Result,
) -> Result {
    let Some(raw @ Original::Struct(data)) = source else {
        return verify_state(source, row);
    };
    let payload = consumer::present(row)?;
    if let Some(kind) = payload.get("sourceType") {
        assert!(
            consumer::present(kind)?.as_str() == Some(data.type_name()),
            "exact original subtype"
        );
        trace.part(kind, raw, "$type", "selector")?;
    }
    trace.part(row, raw, "$type", "selector")?;
    compare(data, payload)
}

fn module_type<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> Result {
    match source.type_name() {
        "ModuleDeclarationType_Beacon" => kind(row, "beacon"),
        "ModuleDeclarationType_Location" => {
            kind(row, "location")?;
            receipts::scalar(trace, row, "isPersistent", source.field("isPersistent"))
        }
        "ModuleDeclarationType_Mission" => {
            kind(row, "mission")?;
            receipts::scalar(
                trace,
                row,
                "pausesLocationNpcAutospawns",
                source.field("pausesLocationNPCAutospawns"),
            )?;
            record_links(forge, SOURCE, source, row, "requiredLocationModules")?;
            verify_link(
                forge,
                "LocationResourceSlot",
                source.field("requiredResourceSlot"),
                field(row, "requiredResourceSlot")?,
            )
        }
        _ => kind(row, "authored"),
    }
}

fn spawn<'source>(
    _forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> Result {
    for name in ["debugName", "spawnImmediately"] {
        receipts::scalar(trace, row, name, source.field(name))?;
    }
    if source.type_name() == "ModuleSpawnedEntities_NPC" {
        kind(row, "npc")?;
        receipts::scalar(
            trace,
            row,
            "spawnDescriptionPropertyName",
            source.field("spawnDescriptionPropertyName"),
        )
    } else {
        kind(row, "authored")
    }
}

fn location<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> Result {
    for name in ["debugName", "requiredForInit", "optional"] {
        receipts::scalar(trace, row, name, source.field(name))?;
    }
    owned(
        trace,
        source.field("acquiringStrategy"),
        field(row, "acquiringStrategy")?,
        |source, row| strategy(trace, source, row),
    )?;
    if source.type_name() == "ModuleLocationEntities_Static" {
        kind(row, "static")?;
        record_links(
            forge,
            "LocationEntityDeclaration",
            source,
            row,
            "entityDeclarations",
        )
    } else {
        kind(row, "authored")
    }
}

fn strategy<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> Result {
    match source.type_name() {
        "ModuleLocationEntitiesAcquiringStrategy_ExactCount" => {
            kind(row, "exactCount")?;
            receipts::scalar(trace, row, "count", source.field("count"))
        }
        "ModuleLocationEntitiesAcquiringStrategy_AllSuitable" => {
            kind(row, "allSuitable")?;
            for name in ["minCount", "maxCount"] {
                receipts::scalar(trace, row, name, source.field(name))?;
            }
            Ok(())
        }
        _ => kind(row, "authored"),
    }
}

fn property<'source>(
    _forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> Result {
    for name in ["missionVariableName", "extendedTextToken"] {
        receipts::scalar(trace, row, name, source.field(name))?;
    }
    owned(
        trace,
        source.field("value"),
        field(row, "value")?,
        |_, _| Ok(()),
    )
}

fn variable<'source>(
    _forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> Result {
    receipts::scalar(
        trace,
        row,
        "subsumptionVariableName",
        source.field("subsumptionVariableName"),
    )?;
    let name = match source.type_name() {
        "VariableDeclaration_LocationEntities" => "locationEntities",
        "VariableDeclaration_SpawnedEntities" => "spawnedEntities",
        _ => return kind(row, "authored"),
    };
    kind(row, name)?;
    verify_array(source.field(name), field(row, name)?, |original, row| {
        let Some(Original::Reference(target)) = original else {
            return verify_state(original, row);
        };
        let payload = consumer::present(row)?;
        let index = consumer::present(field(payload, "targetRecordIndex")?)?;
        let guid = consumer::present(field(payload, "recordGuid")?)?;
        assert!(
            index.as_u64() == Some(u64::from(target.record_index()))
                && guid.as_str() == Some(&target.guid().to_string()),
            "exact original WeakPointer owner"
        );
        if let Some(location) = target.location() {
            assert!(
                payload
                    .pointer("/target/location/value")
                    .and_then(Value::as_str)
                    == Some(&location.to_string()),
                "exact original WeakPointer instance position"
            );
        }
        Ok(())
    })
}

fn record_links(
    forge: &DataForge<'_>,
    target: &str,
    source: &StructValue<'_>,
    row: &Value,
    name: &str,
) -> Result {
    verify_array(source.field(name), field(row, name)?, |original, row| {
        verify_link(forge, target, original, row)
    })
}

fn kind(row: &Value, kind: &str) -> Result {
    assert_eq!(
        field(row, "kind")?.as_str(),
        Some(kind),
        "original typed module variant"
    );
    Ok(())
}
