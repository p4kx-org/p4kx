// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The survey counts pointers in instances, inline structs and pools, and finishes on hostile input.

use std::error::Error;
use std::time::{Duration, Instant};

use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestPool, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const SAMPLE: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const DEADLINE: Duration = Duration::from_secs(5);

fn raw(words: [u32; 2]) -> TestValue {
    TestValue::Raw(words.iter().flat_map(|word| word.to_le_bytes()).collect())
}

fn node() -> TestStruct {
    TestStruct::new("Node")
}

fn weak() -> TestType {
    TestType::WeakPointer("Node".to_owned())
}

#[test]
fn pointers_beyond_16_bits_are_wide() -> TestResult {
    let nodes = 0x1_0001_u128;
    let builder = (0..nodes).fold(
        DataForgeBuilder::new()
            .structure(node())
            .structure(TestStruct::new("Holder").field("far", weak())),
        |builder, index| {
            builder.record(TestRecord::new(
                SAMPLE + index,
                "Node",
                "Node.One",
                "node.xml",
            ))
        },
    );
    let bytes = builder
        .record(
            TestRecord::new(SAMPLE + nodes, "Holder", "Holder.One", "holder.xml")
                .value("far", raw([0, 0x1_0000])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;

    let survey = forge.survey()?;

    assert_eq!(survey.pointers.wide, 1);
    assert_eq!(survey.pointers.narrow, 0);
    assert_eq!(survey.pointers.dangling, 0);
    Ok(())
}

#[test]
fn pointers_inside_an_inline_struct_are_counted() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(node())
        .structure(TestStruct::new("Inner").field("link", weak()))
        .structure(
            TestStruct::new("Outer")
                .field("mark", TestType::Int32)
                .field("inner", TestType::Class("Inner".to_owned()))
                .field("again", TestType::Class("Inner".to_owned())),
        )
        .record(TestRecord::new(SAMPLE, "Outer", "Outer.One", "outer.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let survey = forge.survey()?;

    assert_eq!(survey.pointers.null, 2);
    assert_eq!(survey.unreadable_structs, 0);
    Ok(())
}

#[test]
fn the_pointer_pools_are_counted() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(node())
        .values(TestPool::WeakPointer, 2)
        .values(TestPool::StrongPointer, 1)
        .record(TestRecord::new(SAMPLE, "Node", "Node.One", "node.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let survey = forge.survey()?;

    assert_eq!(survey.pointers.narrow, 3, "zeroed pointers name instance 0");
    assert_eq!(survey.pointers.null, 0);
    Ok(())
}

#[test]
fn nested_zero_size_structs_do_not_blow_up_the_survey() -> TestResult {
    let depth = 64;
    let builder = (0..depth).fold(DataForgeBuilder::new(), |builder, level| {
        let next = format!("S{}", level + 1);
        builder.structure(
            TestStruct::new(format!("S{level}"))
                .declared_size(0)
                .field("left", TestType::Class(next.clone()))
                .field("right", TestType::Class(next)),
        )
    });
    let bytes = builder
        .structure(TestStruct::new(format!("S{depth}")).declared_size(0))
        .record(TestRecord::new(SAMPLE, "S0", "S0.One", "s0.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let started = Instant::now();
    let survey = forge.survey()?;

    assert!(started.elapsed() < DEADLINE, "took {:?}", started.elapsed());
    assert_eq!(survey.pointers.null, 0);
    Ok(())
}

#[test]
fn a_struct_with_too_many_pointers_is_unreadable_not_slow() -> TestResult {
    let inner = (0..40_000).fold(TestStruct::new("Inner"), |inner, index| {
        inner.field(format!("p{index}"), weak())
    });
    let bytes = DataForgeBuilder::new()
        .structure(node())
        .structure(inner)
        .structure(
            TestStruct::new("Outer")
                .field("left", TestType::Class("Inner".to_owned()))
                .field("right", TestType::Class("Inner".to_owned())),
        )
        .record(TestRecord::new(SAMPLE, "Outer", "Outer.One", "outer.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let started = Instant::now();
    let survey = forge.survey()?;

    assert!(started.elapsed() < DEADLINE, "took {:?}", started.elapsed());
    assert_eq!(survey.unreadable_structs, 1);
    Ok(())
}
#[test]
fn a_zero_size_struct_with_a_huge_instance_count_costs_nothing() -> TestResult {
    let mut bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Empty"))
        .record(TestRecord::new(SAMPLE, "Empty", "Empty.One", "empty.xml"))
        .build();
    let mappings = 120 + 16 + 8;
    bytes
        .get_mut(mappings..mappings + 4)
        .ok_or("no mapping")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let forge = DataForge::open(&bytes)?;

    let started = Instant::now();
    let survey = forge.survey()?;

    assert!(started.elapsed() < DEADLINE, "took {:?}", started.elapsed());
    assert_eq!(survey.pointers.null, 0);
    Ok(())
}
