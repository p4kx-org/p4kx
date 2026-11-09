// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{ArchiveWatch, DataForge, TagTree, Value as SourceValue, ValueError};
use serde_json::Value;

use super::{Failure, Result, ensure};

const WATCH_EVERY_RECORDS: u32 = 512;
const ENTITY_SAMPLE_LIMIT: usize = 32;

fn rows<'a>(catalog: &'a Value, path: &str) -> Result<&'a Vec<Value>> {
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("required-rows"))
}

fn payload<'a>(row: &'a Value, field: &str) -> Result<&'a Value> {
    let carrier = row
        .get(field)
        .ok_or_else(|| Failure::check("required-carrier"))?;
    ensure(carrier["state"] == "present", "present-carrier")?;
    carrier
        .get("value")
        .ok_or_else(|| Failure::check("required-value"))
}

fn indexed(rows: &[Value]) -> Result<BTreeMap<u32, &Value>> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let index = payload(row, "recordIndex")?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| Failure::check("physical-index"))?;
        ensure(
            indexed.insert(index, row).is_none(),
            "unique-physical-index",
        )?;
    }
    Ok(indexed)
}

pub(super) fn definitions(
    forge: &DataForge<'_>,
    catalog: &Value,
    watch: &ArchiveWatch,
) -> Result<(usize, usize)> {
    let definitions = indexed(rows(catalog, "/definitions/tags/rows")?)?;
    let databases = indexed(rows(catalog, "/declarations/tagRoots/rows")?)?;
    let mut counts = (0, 0);
    for record in forge.records() {
        if record.index().is_multiple_of(WATCH_EVERY_RECORDS) {
            watch
                .check()
                .map_err(|error| Failure::source("archive-change", error))?;
        }
        let kind = record
            .type_name()
            .map_err(|error| Failure::source("record-type", error))?;
        let (table, count) = match kind {
            "Tag" => (&definitions, &mut counts.0),
            "TagDatabase" => (&databases, &mut counts.1),
            _ => continue,
        };
        let Ok(tree) = record.tree() else {
            ensure(
                !table.contains_key(&record.index()),
                "failed-source-has-no-row",
            )?;
            ensure(
                rows(catalog, "/exportHealth/failedRecords")?
                    .iter()
                    .any(|row| row["recordIndex"].as_u64() == Some(u64::from(record.index()))),
                "failed-source-health",
            )?;
            continue;
        };
        let row = table
            .get(&record.index())
            .ok_or_else(|| Failure::check("readable-source-row"))?;
        ensure(
            payload(row, "guid")?.as_str() == Some(tree.guid().to_string().as_str()),
            "source-guid",
        )?;
        ensure(
            payload(row, "recordPath")?.as_str() == Some(tree.record_path()),
            "source-path",
        )?;
        if kind == "Tag" {
            scalar(tree.field("tagName"), row, "tagName")?;
            scalar(tree.field("legacyGUID"), row, "legacyGuid")?;
            array(tree.field("children"), row, "children")?;
        } else {
            array(tree.field("tags"), row, "tags")?;
        }
        *count += 1;
    }
    ensure(
        counts.0 == definitions.len() && counts.1 == databases.len(),
        "exact-readable-cardinality",
    )?;
    ensure(counts.0 > 0, "nonempty-installed-taxonomy")?;
    Ok(counts)
}

pub(super) fn entities(forge: &DataForge<'_>, catalog: &Value) -> Result<usize> {
    let declarations = rows(catalog, "/declarations/entityTags/rows")?;
    let mut count = 0;
    for row in declarations.iter().take(ENTITY_SAMPLE_LIMIT) {
        let index = payload(row, "recordIndex")?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| Failure::check("entity-index"))?;
        let record = forge
            .record(index)
            .ok_or_else(|| Failure::check("entity-source"))?;
        let tree = record
            .tree()
            .map_err(|error| Failure::source("entity-tree", error))?;
        ensure(
            tree.type_name() == "EntityClassDefinition",
            "entity-owner-type",
        )?;
        ensure(
            payload(row, "guid")?.as_str() == Some(tree.guid().to_string().as_str()),
            "entity-owner-guid",
        )?;
        array(tree.field("tags"), row, "tags")?;
        count += 1;
    }
    ensure(count > 0, "nonempty-entity-sample")?;
    Ok(count)
}

fn state(row: &Value, field: &str, expected: &str) -> Result {
    let carrier = row
        .get(field)
        .ok_or_else(|| Failure::check("required-carrier"))?;
    ensure(
        carrier["state"] == expected && (expected == "present" || carrier["value"].is_null()),
        "source-state",
    )
}

fn scalar(source: Option<&SourceValue<'_>>, row: &Value, field: &str) -> Result {
    match source {
        Some(SourceValue::String(value)) => ensure(
            payload(row, field)?.as_str() == Some(*value),
            "source-string",
        ),
        Some(SourceValue::UInt32(value)) => ensure(
            payload(row, field)?.as_u64() == Some(u64::from(*value)),
            "source-uint32",
        ),
        None => state(row, field, "absent"),
        Some(SourceValue::Null) => state(row, field, "null"),
        Some(SourceValue::Error(_)) => state(row, field, "unreadable"),
        _ => state(row, field, "invalid"),
    }
}

fn array(source: Option<&SourceValue<'_>>, row: &Value, field: &str) -> Result {
    match source {
        Some(SourceValue::Array(values)) => {
            let emitted = payload(row, field)?
                .as_array()
                .ok_or_else(|| Failure::check("array-payload"))?;
            ensure(values.len() == emitted.len(), "array-cardinality")?;
            for (source, carrier) in values.iter().zip(emitted) {
                reference(source, carrier)?;
            }
            Ok(())
        }
        None => state(row, field, "absent"),
        Some(SourceValue::Null) => state(row, field, "null"),
        Some(SourceValue::Error(_)) => state(row, field, "unreadable"),
        _ => state(row, field, "invalid"),
    }
}

fn reference(source: &SourceValue<'_>, carrier: &Value) -> Result {
    let guid = match source {
        SourceValue::Reference(reference) => reference.guid(),
        SourceValue::Error(
            ValueError::DanglingReference { guid }
            | ValueError::UnexpectedReferenceType { guid, .. },
        ) => *guid,
        SourceValue::Null => {
            return ensure(
                carrier["state"] == "null" && carrier["value"].is_null(),
                "null-slot",
            );
        }
        SourceValue::Error(_) => {
            return ensure(
                carrier["state"] == "unreadable" && carrier["value"].is_null(),
                "unreadable-slot",
            );
        }
        _ => {
            return ensure(
                carrier["state"] == "invalid" && carrier["value"].is_null(),
                "invalid-slot",
            );
        }
    };
    ensure(carrier["state"] == "present", "retained-reference")?;
    ensure(
        carrier.pointer("/value/guid/value").and_then(Value::as_str)
            == Some(guid.to_string().as_str()),
        "reference-guid",
    )
}

pub(super) fn paths(tree: &TagTree, catalog: &Value) -> Result {
    let emitted = rows(catalog, "/projections/tagPaths/rows")?;
    ensure(tree.len() == emitted.len(), "path-cardinality")?;
    for (tag, row) in tree.iter().zip(emitted) {
        ensure(
            payload(row, "guid")?.as_str() == Some(tag.guid().to_string().as_str()),
            "path-guid",
        )?;
        ensure(
            payload(row, "path")?.as_str() == Some(tag.path()),
            "path-text",
        )?;
        ensure(
            payload(row, "depth")?.as_u64() == u64::try_from(tag.depth()).ok(),
            "path-depth",
        )?;
        ensure(
            payload(row, "listed")?.as_bool() == Some(tag.is_listed()),
            "path-listing",
        )?;
        let parent = payload(row, "parent")?;
        match tag.parent() {
            None => ensure(parent["type"] == "root", "root-parent")?,
            Some(parent_tag) => ensure(
                parent["type"] == "tag"
                    && parent.pointer("/guid/value").and_then(Value::as_str)
                        == Some(parent_tag.guid().to_string().as_str()),
                "chosen-parent",
            )?,
        }
    }
    Ok(())
}
