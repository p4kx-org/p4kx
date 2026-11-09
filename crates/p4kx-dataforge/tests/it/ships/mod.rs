// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic DataForge of ships with nested structs, strong pointers, arrays and a polymorphic
//! component array, and a helper to read the cells of a Query.

use std::error::Error;
use std::num::NonZeroUsize;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

fn vector(x: f32, y: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Vector")
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y)),
    )
}

fn weapon(name: &str, damage: f64) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Weapon")
            .value("name", TestValue::text(name))
            .value("damage", TestValue::Double(damage)),
    )
}

fn shield(name: &str, hp: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Shield")
            .value("name", TestValue::text(name))
            .value("hp", TestValue::Single(hp)),
    )
}

fn texts(items: &[&str]) -> TestValue {
    TestValue::Array(items.iter().map(|item| TestValue::text(*item)).collect())
}

pub(crate) fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Vector")
                .field("x", TestType::Single)
                .field("y", TestType::Single),
        )
        .structure(
            TestStruct::new("Component")
                .field("name", TestType::String)
                .array("modes", TestType::Int32),
        )
        .structure(
            TestStruct::new("Weapon")
                .parent("Component")
                .field("damage", TestType::Double),
        )
        .structure(
            TestStruct::new("Shield")
                .parent("Component")
                .field("hp", TestType::Single),
        )
        .structure(
            TestStruct::new("Ship")
                .field("mass", TestType::Single)
                .field("position", TestType::Class("Vector".to_owned()))
                .field("anchor", TestType::StrongPointer("Vector".to_owned()))
                .array(
                    "Components",
                    TestType::StrongPointer("Component".to_owned()),
                )
                .array("tags", TestType::String)
                .array("hardpoints", TestType::Class("Vector".to_owned()))
                .array("links", TestType::Reference),
        )
        .structure(
            TestStruct::new("Station")
                .field("position", TestType::Single)
                .array("tags", TestType::Int32),
        )
        .structure(TestStruct::new("Tiny").parent("Vector").declared_size(4))
}

pub(crate) fn database() -> Vec<u8> {
    schema()
        .record(
            TestRecord::new(1, "Ship", "Ship.Alpha", "libs/ships/alpha.xml")
                .value("mass", TestValue::Single(100.0))
                .value("position", vector(1.5, -2.0))
                .value("anchor", vector(7.0, 8.0))
                .value(
                    "Components",
                    TestValue::Array(vec![
                        weapon("Laser", 250.5),
                        shield("Bubble", 900.0),
                        weapon("Cannon", 1200.0),
                    ]),
                )
                .value("tags", texts(&["fast", "small"]))
                .value(
                    "hardpoints",
                    TestValue::Array(vec![vector(1.0, 2.0), vector(3.0, 4.0)]),
                )
                .value("links", TestValue::Array(vec![TestValue::Reference(2)])),
        )
        .record(
            TestRecord::new(2, "Ship", "Ship.Beta", "libs/ships/beta.xml")
                .value("mass", TestValue::Single(50.0))
                .value("position", vector(0.0, 0.0))
                .value(
                    "Components",
                    TestValue::Array(vec![TestValue::Null, shield("Wall", 10.0)]),
                )
                .value("tags", texts(&["slow"])),
        )
        .record(
            TestRecord::new(3, "Station", "Station.Gamma", "libs/stations/gamma.xml")
                .value("position", TestValue::Single(3.0))
                .value("tags", TestValue::Array(vec![TestValue::Int32(7)])),
        )
        .build()
}

/// Returns per row the record name and the cells as text, `-` for an empty cell.
pub(crate) fn rows(forge: &DataForge<'_>, text: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    let mut rows = Vec::new();
    for page in run.pages(NonZeroUsize::MIN.saturating_add(15)) {
        for row in page {
            let name = row.record_path().rsplit('/').next().unwrap_or_default();
            let cells: Vec<String> = row
                .cells()
                .map_err(|error| format!("{name}: {error}"))?
                .iter()
                .map(|cell| {
                    cell.as_ref()
                        .map_or_else(|| "-".to_owned(), ToString::to_string)
                })
                .collect();
            rows.push(format!("{name}: {}", cells.join(" | ")));
        }
    }
    Ok(rows)
}
