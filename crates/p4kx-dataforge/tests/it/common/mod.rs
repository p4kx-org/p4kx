// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The synthetic DataForge the condition tests run over, and helpers to list the selected Records.

use std::error::Error;
use std::num::NonZeroUsize;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) struct Item {
    pub(crate) path: &'static str,
    pub(crate) mass: f32,
    pub(crate) power: f64,
    pub(crate) count: i32,
    pub(crate) big: u64,
    pub(crate) name: &'static str,
    pub(crate) size: &'static str,
    pub(crate) id: u128,
    pub(crate) active: bool,
}

pub(crate) const ITEMS: [Item; 3] = [
    Item {
        path: "a",
        mass: 0.1,
        power: 1.5,
        count: 10,
        big: u64::MAX,
        name: "Avenger",
        size: "Large",
        id: 1,
        active: true,
    },
    Item {
        path: "b",
        mass: 2.5,
        power: f64::NAN,
        count: 3,
        big: 0,
        name: "aurora",
        size: "Small",
        id: 2,
        active: false,
    },
    Item {
        path: "c",
        mass: -1.0,
        power: -0.0,
        count: -7,
        big: 1,
        name: "Say \"hi\"",
        size: "Small",
        id: 0xffff_0000_0000_0000_0000_0000_0000_0000,
        active: true,
    },
];

pub(crate) fn database() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(
            TestStruct::new("Item")
                .field("mass", TestType::Single)
                .field("power", TestType::Double)
                .field("count", TestType::Int32)
                .field("big", TestType::UInt64)
                .field("name", TestType::String)
                .field("label", TestType::Locale)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("id", TestType::Guid)
                .field("active", TestType::Bool),
        )
        .structure(TestStruct::new("Gadget").field("name", TestType::UInt32))
        .structure(
            TestStruct::new("Broken")
                .field("count", TestType::Int32)
                .declared_size(99),
        );
    for (index, item) in (1_u128..).zip(ITEMS) {
        builder = builder.record(
            TestRecord::new(
                index,
                "Item",
                format!("Item.{}", item.path),
                format!("libs/items/{}.xml", item.path),
            )
            .value("mass", TestValue::Single(item.mass))
            .value("power", TestValue::Double(item.power))
            .value("count", TestValue::Int32(item.count))
            .value("big", TestValue::UInt64(item.big))
            .value("name", TestValue::text(item.name))
            .value("label", TestValue::text(format!("@item_{}", item.path)))
            .value("size", TestValue::text(item.size))
            .value("id", TestValue::Guid(item.id))
            .value("active", TestValue::Bool(item.active)),
        );
    }
    builder
        .record(
            TestRecord::new(10, "Gadget", "Gadget.g", "libs/gadgets/g.xml")
                .value("name", TestValue::UInt32(42)),
        )
        .record(TestRecord::new(
            11,
            "Broken",
            "Broken.x",
            "libs/broken/x.xml",
        ))
        .build()
}

/// Returns the last part of each selected record path, `!` behind a Record that failed.
pub(crate) fn names(forge: &DataForge<'_>, text: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    let mut names = Vec::new();
    for page in run.pages(NonZeroUsize::MIN.saturating_add(63)) {
        for row in page {
            let name = row.record_path().rsplit('/').next().unwrap_or_default();
            let failed = if row.cells().is_err() { "!" } else { "" };
            names.push(format!("{name}{failed}"));
        }
    }
    Ok(names)
}

pub(crate) fn assert_selects(forge: &DataForge<'_>, cases: &[(&str, &[&str])]) -> TestResult {
    for (text, expected) in cases {
        assert_eq!(names(forge, text)?, *expected, "{text}");
    }
    Ok(())
}
