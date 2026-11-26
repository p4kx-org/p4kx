// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Record counts and the edge cases of the schema drift.

use std::error::Error;

use p4kx_comparison::{
    DataForge, PropertyChange, SchemaChangeKind, SchemaDiff, SideCounts, compare_schema,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn diff(old: &DataForgeBuilder, new: &DataForgeBuilder) -> Result<SchemaDiff, Box<dyn Error>> {
    let (old, new) = (old.build(), new.build());
    Ok(compare_schema(
        &DataForge::open(&old)?,
        &DataForge::open(&new)?,
    ))
}

fn record(guid: u128, type_name: &str) -> TestRecord {
    TestRecord::new(guid, type_name, "r", format!("libs/{guid}.xml"))
}

fn wheels(old: bool) -> DataForgeBuilder {
    let wheel = |name: &str| TestValue::Struct(TestInstance::new("Wheel").named(name));
    let car = TestStruct::new("Car").array("wheels", TestType::StrongPointer("Wheel".to_owned()));
    let wheel_struct = if old {
        TestStruct::new("Wheel").field("grip", TestType::Int32)
    } else {
        TestStruct::new("Wheel").field("grip", TestType::Single)
    };
    DataForgeBuilder::new()
        .structure(wheel_struct)
        .structure(car)
        .record(
            record(0x1000_0000_0000_4000_8000_0000_0000_0001, "Car").value(
                "wheels",
                TestValue::Array(vec![wheel("a"), wheel("b"), wheel("c")]),
            ),
        )
}

#[test]
fn a_struct_that_only_strong_pointers_hold_has_no_records() -> TestResult {
    let diff = diff(&wheels(true), &wheels(false))?;

    let wheel = diff
        .structs
        .iter()
        .find(|change| change.name == "Wheel")
        .ok_or("Wheel")?;
    assert_eq!(wheel.change, SchemaChangeKind::Changed);
    assert_eq!(
        (wheel.records.old, wheel.records.new),
        (0, 0),
        "three instances of Wheel, but none is a Record"
    );
    assert_eq!(
        wheel.mapped_instances,
        Some(SideCounts { old: 3, new: 3 }),
        "pointer targets still contribute to the direct mapping count"
    );
    Ok(())
}

fn traversal_types(include_nested: bool) -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .structure(TestStruct::new("Route").field("enabled", TestType::Bool))
        .structure(TestStruct::new("Waypoint").field("weight", TestType::Int32))
        .structure(
            TestStruct::new("Panel").field("target", TestType::StrongPointer("Route".to_owned())),
        );
    let panel = record(1, "Panel");
    if include_nested {
        builder
            .structure(
                TestStruct::new("TraversalRoute")
                    .parent("Route")
                    .field("targetPath", TestType::WeakPointer("Waypoint".to_owned()))
                    .field("nextTarget", TestType::StrongPointer("Route".to_owned())),
            )
            .structure(TestStruct::new("UnusedRange").parent("Route"))
            .record(panel.value(
                "target",
                TestValue::Struct(TestInstance::new("TraversalRoute")),
            ))
    } else {
        builder.record(panel)
    }
}

#[test]
fn an_added_or_removed_nested_struct_reports_its_own_declarations() -> TestResult {
    let earlier = traversal_types(false);
    let later = traversal_types(true);
    for (diff, kind, instances) in [
        (
            diff(&earlier, &later)?,
            SchemaChangeKind::Added,
            SideCounts { old: 0, new: 1 },
        ),
        (
            diff(&later, &earlier)?,
            SchemaChangeKind::Removed,
            SideCounts { old: 1, new: 0 },
        ),
    ] {
        let [traversal, unused] = diff.structs.as_slice() else {
            return Err("two one-sided structures".into());
        };
        assert_eq!(traversal.name, "TraversalRoute", "canonical struct order");
        assert_eq!(traversal.change, kind, "one-sided struct change");
        assert_eq!(
            traversal.records,
            SideCounts::default(),
            "no GUID-addressed Record"
        );
        assert_eq!(
            traversal.mapped_instances,
            Some(instances),
            "one nested mapped instance"
        );
        assert_eq!(unused.name, "UnusedRange", "empty derived declaration");
        assert!(
            unused.properties.is_empty(),
            "inherited properties stay excluded"
        );
        assert_eq!(
            unused.mapped_instances,
            Some(SideCounts::default()),
            "the empty mapping count is explicit"
        );
        check_declarations(&traversal.properties, kind)?;
        let counts = diff.counts.properties;
        let (added, removed) = if kind == SchemaChangeKind::Added {
            (2, 0)
        } else {
            (0, 2)
        };
        assert_eq!(
            (counts.added, counts.removed, counts.changed),
            (added, removed, 0),
            "only actual declarations count"
        );
    }
    Ok(())
}

fn check_declarations(properties: &[PropertyChange], kind: SchemaChangeKind) -> TestResult {
    let [next, target] = properties else {
        return Err("two directly declared properties".into());
    };
    for (property, name, data_type, reference) in [
        (next, "nextTarget", "StrongPointer", "Route"),
        (target, "targetPath", "WeakPointer", "Waypoint"),
    ] {
        assert_eq!(property.name, name, "property order by name");
        assert_eq!(property.change, kind, "one-sided property change");
        assert!(
            property.aspects.is_empty(),
            "no shared property shape to compare"
        );
        let (present, absent) = if kind == SchemaChangeKind::Added {
            (&property.new, &property.old)
        } else {
            (&property.old, &property.new)
        };
        assert!(absent.is_none(), "no declaration on the other side");
        let shape = present.as_ref().ok_or("original property shape")?;
        assert_eq!(shape.data_type, data_type, "original pointer kind");
        assert_eq!(shape.conversion, 0, "scalar pointer");
        assert_eq!(shape.target.as_deref(), Some(reference), "original target");
    }
    Ok(())
}

fn inherited(extra: bool) -> DataForgeBuilder {
    let base = TestStruct::new("Base").field("size", TestType::Enum("Size".to_owned()));
    let child = TestStruct::new("Child").parent("Base");
    DataForgeBuilder::new()
        .enumeration("Size", if extra { vec!["A", "B"] } else { vec!["A"] })
        .structure(base)
        .structure(child)
        .record(record(0x2000_0000_0000_4000_8000_0000_0000_0001, "Base"))
        .record(record(0x2000_0000_0000_4000_8000_0000_0000_0002, "Child"))
        .record(record(0x2000_0000_0000_4000_8000_0000_0000_0003, "Child"))
}

#[test]
fn an_enum_counts_each_record_once_through_a_parent_and_its_child() -> TestResult {
    let diff = diff(&inherited(false), &inherited(true))?;

    let size = diff
        .enums
        .iter()
        .find(|change| change.name == "Size")
        .ok_or("Size")?;
    assert_eq!((size.records.old, size.records.new), (3, 3));
    Ok(())
}

#[test]
fn a_struct_that_is_its_own_parent_does_not_hang_or_overflow() -> TestResult {
    let looping = |size: u32| {
        (0..size).fold(
            DataForgeBuilder::new().structure(TestStruct::new("Loop").parent("Loop")),
            |builder, index| {
                builder.structure(TestStruct::new(format!("Child{index}")).parent("Loop"))
            },
        )
    };

    let diff = diff(&looping(50), &looping(60))?;

    assert_eq!(diff.counts.structs.added, 10);
    Ok(())
}

#[test]
fn a_name_defined_twice_counts_as_left_out_and_is_a_difference() -> TestResult {
    let single =
        DataForgeBuilder::new().structure(TestStruct::new("Dup").field("a", TestType::Bool));
    let twice = single
        .clone()
        .structure(TestStruct::new("Dup").field("b", TestType::Bool));

    let diff = diff(&single, &twice)?;

    assert_eq!((diff.unreadable.old, diff.unreadable.new), (0, 1));
    assert!(diff.structs.is_empty(), "the first definition wins");
    assert!(diff.has_differences(), "a different number was left out");
    Ok(())
}

#[path = "schema_counts_inheritance.rs"]
mod schema_counts_inheritance;
