// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[path = "resource_group_records.rs"]
mod records;

use records::LATER;
pub(crate) use records::{CHILD, FIRST, MISSING, OTHER, graph, group, references, schema};

pub(crate) fn clean() -> DataForgeBuilder {
    schema()
        .record(group(FIRST, "included_first", references(&[CHILD])))
        .record(group(CHILD, "included_child", references(&[])))
}

pub(crate) fn fault(before: bool) -> DataForgeBuilder {
    let mut invalid = 1_u32.to_le_bytes().to_vec();
    invalid.extend_from_slice(&u32::MAX.to_le_bytes());
    let broken = group(LATER, "included_broken", TestValue::Raw(invalid));
    let winner = group(FIRST, "included_winner", references(&[CHILD]));
    let builder = if before {
        schema().record(broken).record(winner)
    } else {
        schema().record(winner).record(broken)
    };
    builder.record(group(CHILD, "included_child", references(&[])))
}

pub(crate) fn build(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_folder, archive) = common::open(database, origin)?;
    let mut options = common::options();
    options.publication_mode = mode;
    options.filter = filter;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

pub(crate) fn diagnostic(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )
}

pub(crate) fn rows<'a>(catalog: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    catalog
        .pointer(&format!("/{path}/rows"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing {path} rows").into())
}

pub(crate) fn row<'a>(catalog: &'a Value, path: &str, index: u64) -> TestResult<&'a Value> {
    rows(catalog, path)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index))
        .ok_or_else(|| format!("missing physical {path} row {index}").into())
}

pub(crate) fn unreadable_first_path() -> TestResult<Vec<u8>> {
    let path = "libs/foundry/records/resourcetypedatabase/header_corrupt_marker.xml";
    let source = TestRecord::new(LATER, "ResourceTypeGroup", "ResourceTypeGroup.Early", path);
    let mut bytes = schema()
        .record(source)
        .record(group(FIRST, "included_winner", references(&[CHILD])))
        .record(group(CHILD, "included_child", references(&[])))
        .build();
    let position = {
        let mut matching = bytes
            .windows(path.len())
            .enumerate()
            .filter_map(|(index, value)| (value == path.as_bytes()).then_some(index));
        let index = matching.next().ok_or("the synthetic path token")?;
        if matching.next().is_some() {
            return Err("the synthetic path token must be unique".into());
        }
        index
    };
    *bytes.get_mut(position).ok_or("the synthetic path byte")? = 0xff;
    Ok(bytes)
}
