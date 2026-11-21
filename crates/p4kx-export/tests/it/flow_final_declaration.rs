// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole};
use serde_json::{Value, json};

use crate::flow_final_origin::Evidence;
use crate::flow_final_original::Declaration;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;

pub(crate) fn check(
    field: &Value,
    source: &Declaration,
    evidence: &Evidence<'_>,
    ordinal: u64,
) -> TestResult {
    let values = [
        (
            "name",
            json!(source.name),
            "$fieldName",
            SourceUseRole::Payload,
        ),
        (
            "declaration/dataType",
            json!(label(source.data_type)),
            "$dataType",
            SourceUseRole::Selector,
        ),
        (
            "declaration/isArray",
            json!(source.is_array),
            "$isArray",
            SourceUseRole::Selector,
        ),
        (
            "declaration/target",
            json!(source.target),
            "$target",
            SourceUseRole::Selector,
        ),
    ];
    for (path, expected, part, role) in values {
        let cell = field
            .pointer(&format!("/{path}"))
            .ok_or("independent sourced original field metadata")?;
        assert_eq!(
            raw::present(cell)?,
            &expected,
            "exact decoder declaration fact {path}"
        );
        evidence.value(cell, ordinal, part, role)?;
    }
    Ok(())
}

const fn label(value: DataType) -> &'static str {
    match value {
        DataType::Bool => "bool",
        DataType::Int8 => "int8",
        DataType::Int16 => "int16",
        DataType::Int32 => "int32",
        DataType::Int64 => "int64",
        DataType::UInt8 => "uint8",
        DataType::UInt16 => "uint16",
        DataType::UInt32 => "uint32",
        DataType::UInt64 => "uint64",
        DataType::String => "string",
        DataType::Single => "single",
        DataType::Double => "double",
        DataType::Locale => "locale",
        DataType::Guid => "guid",
        DataType::Enum => "enum",
        DataType::Class => "class",
        DataType::StrongPointer => "strongPointer",
        DataType::WeakPointer => "weakPointer",
        DataType::Reference => "reference",
    }
}
