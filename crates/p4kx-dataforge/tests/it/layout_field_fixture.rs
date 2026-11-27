// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::layout_field_raw::{self as raw, Tables};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(crate) const BASE: &str = "WideBase";
pub(crate) const DERIVED: &str = "WideDerived";
const GUID: u128 = 0x7777_7777_7777_7777_7777_7777_7777_7777;

pub(crate) fn field_name(index: usize, own: usize) -> String {
    if index < own {
        format!("base-{index}")
    } else {
        format!("own-{}", index - own)
    }
}

pub(crate) const fn value(index: usize, own: usize) -> u8 {
    if index == 0 {
        11
    } else if index + 1 == own {
        22
    } else if index == own {
        33
    } else if index + 1 == 2 * own {
        44
    } else {
        0
    }
}

pub(crate) fn database(own: usize, padding: usize) -> TestResult<Vec<u8>> {
    let base = structure(BASE, 0, own, own, None)?;
    let child = structure(DERIVED, own, own, 2 * own + padding, Some(BASE))?;
    let mut record = TestRecord::new(
        GUID,
        DERIVED,
        "WideDerived.Source",
        "source/wide-fields.xml",
    );
    for index in [0, own - 1, own, 2 * own - 1] {
        record = record.value(field_name(index, own), TestValue::UInt8(value(index, own)));
    }
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(base)
        .structure(child)
        .record(record)
        .build();
    let tables = Tables::read(&bytes)?;
    raw::write_half(&mut bytes, tables.records + 34, 0)?;
    Ok(bytes)
}

fn structure(
    name: &str,
    first: usize,
    count: usize,
    size: usize,
    parent: Option<&str>,
) -> TestResult<TestStruct> {
    let mut structure = TestStruct::new(name).declared_size(u32::try_from(size)?);
    if let Some(parent) = parent {
        structure = structure.parent(parent);
    }
    for index in first..first + count {
        structure = structure.field(field_name(index, count), TestType::UInt8);
    }
    Ok(structure)
}
