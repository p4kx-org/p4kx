// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{PAINT_ALPHA_RED, SHIP_MISSING, XML_DIR, builder, paint, port, ship, vehicle_xml};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VehicleLookupInput {
    Missing,
    Present { modification: &'static str },
    Malformed,
    Encrypted,
    Recovered,
}

pub(crate) fn write_vehicle_lookup_archive(
    directory: &Path,
    input: VehicleLookupInput,
) -> Result<PathBuf, Box<dyn Error>> {
    let modification = match input {
        VehicleLookupInput::Present { modification } => modification,
        _ => "ExampleModification",
    };
    let forge = builder()
        .record(paint(PAINT_ALPHA_RED, "FixturePaint", "DirectAlpha", ""))
        .record(ship(
            SHIP_MISSING,
            "ACAS_Missing",
            "scripts/entities/vehicles/implementations/xml/acas_missing.xml",
            modification,
            vec![port("", "DirectAlpha", "Paints")],
        ))
        .build();
    let path = directory.join("Data.p4k");
    let xml = format!("{XML_DIR}ACAS_Missing.xml");
    let second = match input {
        VehicleLookupInput::Missing | VehicleLookupInput::Recovered => {
            TestEntry::stored("Data/Unrelated.xml", b"<Unrelated/>")
        }
        VehicleLookupInput::Present { .. } => TestEntry::stored(xml, b"<Vehicle/>"),
        VehicleLookupInput::Malformed => TestEntry::stored(xml, b"CryXmlB\0not really"),
        VehicleLookupInput::Encrypted => TestEntry::stored(xml, vehicle_xml()).encrypted(),
    };
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge))
        .entry(second)
        .build();
    if input == VehicleLookupInput::Recovered {
        crate::export_health_recovery::remove_directory_record_and_end(&mut bytes)?;
    }
    std::fs::write(&path, bytes)?;
    Ok(path)
}
