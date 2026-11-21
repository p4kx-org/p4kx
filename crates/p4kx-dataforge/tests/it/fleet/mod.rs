// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic DataForge of ships whose References lead to makers, planets and shields, some
//! of them null, dangling or of an unexpected type, and a helper to read the cells of a Query.

use std::error::Error;
use std::num::NonZeroUsize;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const AEGIS: u128 = 10;
pub(crate) const TERRA: u128 = 20;
pub(crate) const BUBBLE: u128 = 30;
pub(crate) const MISSING: u128 = 99;

fn reference_to(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Planet")
                .field("name", TestType::String)
                .field("mass", TestType::Single),
        )
        .structure(
            TestStruct::new("Manufacturer")
                .field("name", TestType::String)
                .field("home", reference_to("Planet")),
        )
        .structure(TestStruct::new("Component").field("name", TestType::String))
        .structure(
            TestStruct::new("Shield")
                .parent("Component")
                .field("hp", TestType::Single),
        )
        .structure(TestStruct::new("Crew").field("role", TestType::String))
        .structure(
            TestStruct::new("Dock")
                .field("name", TestType::String)
                .field("pilot", TestType::WeakPointer("Crew".to_owned())),
        )
        .structure(
            TestStruct::new("Ship")
                .field("name", TestType::String)
                .field("maker", reference_to("Manufacturer"))
                .field("shield", reference_to("Component"))
                .array("escorts", reference_to("Ship"))
                .field("pilot", TestType::WeakPointer("Crew".to_owned()))
                .field("plain", TestType::Reference)
                .field("mass", TestType::Single),
        )
}

fn ship(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "Ship",
        format!("Ship.{name}"),
        format!("libs/ships/{}.xml", name.to_ascii_lowercase()),
    )
    .value("name", TestValue::text(name))
}

pub(crate) fn database() -> Vec<u8> {
    schema()
        .instance(
            "ace",
            TestInstance::new("Crew").value("role", TestValue::text("ace")),
        )
        .record(
            TestRecord::new(TERRA, "Planet", "Planet.Terra", "libs/planets/terra.xml")
                .value("name", TestValue::text("Terra"))
                .value("mass", TestValue::Single(5.5)),
        )
        .record(
            TestRecord::new(
                AEGIS,
                "Manufacturer",
                "Manufacturer.Aegis",
                "libs/makers/aegis.xml",
            )
            .value("name", TestValue::text("Aegis"))
            .value("home", TestValue::Reference(TERRA)),
        )
        .record(
            TestRecord::new(BUBBLE, "Shield", "Shield.Bubble", "libs/shields/bubble.xml")
                .value("name", TestValue::text("Bubble"))
                .value("hp", TestValue::Single(900.0)),
        )
        .record(
            ship(1, "Alpha")
                .value("maker", TestValue::Reference(AEGIS))
                .value("shield", TestValue::Reference(BUBBLE))
                .value(
                    "escorts",
                    TestValue::Array(vec![
                        TestValue::Reference(2),
                        TestValue::Reference(MISSING),
                        TestValue::Null,
                    ]),
                )
                .value("pilot", TestValue::pointer("ace"))
                .value("mass", TestValue::Single(100.0)),
        )
        .record(
            ship(2, "Beta")
                .value("escorts", TestValue::Array(vec![TestValue::Reference(1)]))
                .value("mass", TestValue::Single(50.0)),
        )
        .record(
            ship(3, "Gamma")
                .value("maker", TestValue::Reference(MISSING))
                .value("shield", TestValue::Reference(AEGIS)),
        )
        .record(dock(40, "Planet", TestValue::pointer("Planet.Terra")))
        .record(dock(
            41,
            "Lost",
            TestValue::Raw(vec![0, 0, 0, 0, 99, 0, 0, 0]),
        ))
        .build()
}

/// A Dock whose weak pointer `pilot` names an instance of another struct or none at all.
fn dock(guid: u128, name: &str, pilot: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        "Dock",
        format!("Dock.{name}"),
        format!("libs/docks/{}.xml", name.to_ascii_lowercase()),
    )
    .value("name", TestValue::text(name))
    .value("pilot", pilot)
}

/// Returns per row the record name, the cells as text, `-` for an empty cell, and the number of
/// unresolved References.
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
            rows.push(format!(
                "{name}: {} ({} unresolved)",
                cells.join(" | "),
                row.unresolved_references()
            ));
        }
    }
    Ok(rows)
}
