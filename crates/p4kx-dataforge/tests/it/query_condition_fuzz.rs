// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Random `where` conditions over a synthetic DataForge compile or fail with a span inside the
//! Query, and running them never panics.

#[path = "query_condition_fuzz_failure.rs"]
mod failure;

use std::error::Error;
use std::num::NonZeroUsize;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn database() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(
            TestStruct::new("Item")
                .field("mass", TestType::Single)
                .field("count", TestType::Int64)
                .field("name", TestType::String)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("id", TestType::Guid)
                .field("active", TestType::Bool)
                .array("tags", TestType::String),
        )
        .structure(
            TestStruct::new("Gadget")
                .field("name", TestType::UInt8)
                .field("raw", TestType::String),
        );
    for index in 0..64_u8 {
        builder = builder.record(item(index)).record(gadget(index));
    }
    builder.build()
}

fn item(index: u8) -> TestRecord {
    let mass = if index == 7 {
        f32::NAN
    } else {
        f32::from(index) - 20.0
    };
    let size = if index.is_multiple_of(2) {
        "Small"
    } else {
        "Large"
    };
    TestRecord::new(
        u128::from(index) + 1,
        "Item",
        format!("Item.R{index}"),
        format!("libs/items/r{index:02}.xml"),
    )
    .value("mass", TestValue::Single(mass))
    .value("count", TestValue::Int64(i64::from(index) * -1_000_000_007))
    .value("name", TestValue::text(format!("Name {index}")))
    .value("size", TestValue::text(size))
    .value("id", TestValue::Guid(u128::from(index) << 64))
    .value("active", TestValue::Bool(index.is_multiple_of(3)))
    .value(
        "tags",
        TestValue::Array(
            (0..index % 4)
                .map(|tag| TestValue::text(format!("name {tag}")))
                .collect(),
        ),
    )
}

fn gadget(index: u8) -> TestRecord {
    let raw = if index.is_multiple_of(5) {
        TestValue::Raw(vec![0xff; 4])
    } else {
        TestValue::text("ok")
    };
    TestRecord::new(
        u128::from(index) + 1000,
        "Gadget",
        format!("Gadget.R{index}"),
        format!("libs/gadgets/g{index:02}.xml"),
    )
    .value("name", TestValue::UInt8(index))
    .value("raw", raw)
}

struct Random(u64);

impl Random {
    const fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        let index = usize::try_from(self.next() % items.len() as u64).unwrap_or(0);
        items.get(index).copied().unwrap_or_default()
    }
}

/// The paths conditions compare: fields of both types, unknown ones and paths into the array.
const FIELDS: [&str; 17] = [
    "mass",
    "count",
    "name",
    "size",
    "id",
    "active",
    "tags",
    "raw",
    "nope",
    "Mass",
    "tags[0]",
    "tags[*]",
    "tags[9]",
    "tags[Item]",
    "mass[0]",
    "tags.x",
    "name.x",
];

fn condition(random: &mut Random, depth: usize) -> String {
    let operators = [
        "=",
        "!=",
        "<",
        "<=",
        ">",
        ">=",
        "contains",
        "starts with",
        "like",
    ];
    let literals = [
        "0",
        "-5",
        "1.5",
        "1e40",
        "1e400",
        "-9223372036854775809",
        "340282366920938463463374607431768211456",
        "\"Small\"",
        "\"name 1*\"",
        "\"*1?\"",
        "\"\"",
        "\"00000000-0000-0001-0000-000000000000\"",
        "true",
        "false",
    ];
    let mut text = String::new();
    for part in 0..=random.next() % 3 {
        if part > 0 {
            text += random.pick(&[" and ", " or "]);
        }
        if random.next().is_multiple_of(5) {
            text += "not ";
        }
        if depth < 3 && random.next().is_multiple_of(4) {
            text.push('(');
            text += &condition(random, depth + 1);
            text.push(')');
            continue;
        }
        for (index, words) in [&FIELDS[..], &operators, &literals].into_iter().enumerate() {
            if index > 0 {
                text.push(' ');
            }
            text += random.pick(words);
        }
    }
    text
}

#[test]
fn random_conditions_compile_or_fail_inside_the_query_and_run() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let mut random = Random(0x5851_f42d_4c95_7f2d);
    let (mut compiled_count, mut failed_count) = (0, 0);
    for round in 0..3_000 {
        let selection = ["from Item", "from Gadget", "at \"libs\""][round % 3];
        let text = format!("{selection} where {}", condition(&mut random, 0));
        let Ok(query) = Query::parse(&text) else {
            continue;
        };
        match query.compile(&forge) {
            Ok(compiled) => {
                compiled_count += 1;
                match compiled.run() {
                    Ok(run) => {
                        let rows: usize = run
                            .pages(NonZeroUsize::MIN.saturating_add(9))
                            .map(|page| page.len())
                            .sum();
                        assert_eq!(rows, run.len(), "{text}");
                        assert!(rows <= 128, "{text}");
                    }
                    Err(error) => failure::check(&forge, &query, &error, &text)?,
                }
            }
            Err(error) => {
                failed_count += 1;
                assert!(error.span().start() > selection.len(), "{text}: {error:?}");
                assert!(error.span().end() <= text.len(), "{text}: {error:?}");
                assert!(!error.marked(&text).to_string().is_empty(), "{text}");
            }
        }
    }
    assert!(compiled_count > 100, "{compiled_count} compiled");
    assert!(failed_count > 100, "{failed_count} failed");
    Ok(())
}
