// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, StructValue, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, owned, require_state, single};
use crate::real_archive_item_durability as proof;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const TABLE: &str = "/configurations/facilities/rows";
const SEAT: &str = "SCItemSeatParams";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn seat_facility_definitions_match_the_original_physical_components() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build seat facility catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let (seats, ejections) = trace(&entry, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during seat facility verification")?;
    if seats == 0 {
        return Err("configured build contains no traced seat component".into());
    }
    crate::real_archive_verification::print_line(&format!(
        "seat facilities: {seats} physical seats and {ejections} stored ejection scopes checked",
    ));
    Ok(())
}

fn trace(entry: &DataForgeEntry, catalog: &Value) -> TestResult<(usize, usize)> {
    let rows = components::component_rows(catalog, TABLE)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut trees = TreeBuilder::new();
    let mut seats = 0;
    let mut ejections = 0;
    for ((owner, slot), row) in rows {
        if row["kind"] != "seat" {
            continue;
        }
        let record = forge
            .record(owner)
            .ok_or("original facility owner Record")?;
        let tree = trees.build(&record).map_err(|_| "read facility owner")?;
        let Some(Original::Array(entries)) = tree.field("Components") else {
            return Err("original facility Components".into());
        };
        let Some(Original::Struct(seat)) = entries.get(slot) else {
            return Err("original seat component slot".into());
        };
        if seat.type_name() != SEAT
            || proof::text(row, "/sourceType/value")? != SEAT
            || proof::text(row, "/ownerGuid/value")? != tree.guid().to_string()
        {
            return Err("original seat component identity differs".into());
        }
        compare_seat(seat, row)?;
        seats += 1;
        ejections += usize::from(matches!(seat.field("ejection"), Some(Original::Struct(_))));
    }
    Ok((seats, ejections))
}

fn compare_seat(seat: &StructValue<'_>, row: &Value) -> TestResult {
    scalars(
        seat,
        row,
        &[
            "seatType",
            "controlTemplate",
            "transitionTemplate",
            "setYawPitchLimits",
        ],
    )?;
    singles(seat, row, &["minYaw", "maxYaw", "minPitch", "maxPitch"])?;
    owned(seat, row, "actorAttachment", |actor, row| {
        scalars(actor, row, &["type", "boneName", "stance"])
    })?;
    owned(seat, row, "ejection", |ejection, row| {
        singles(
            ejection,
            row,
            &[
                "maxLinearVelocity",
                "maxLinearAcceleration",
                "maxAngularVelocity",
                "maxAngularAcceleration",
                "ejectionLoopTime",
            ],
        )?;
        owned(ejection, row, "offsetAngles", |angles, row| {
            singles(angles, row, &["x", "y", "z"])
        })
    })
}

fn singles(source: &StructValue<'_>, row: &Value, fields: &[&str]) -> TestResult {
    for field in fields {
        single(source, field, components::cell(row, field)?)?;
    }
    Ok(())
}

fn scalars(source: &StructValue<'_>, row: &Value, fields: &[&str]) -> TestResult {
    for field in fields {
        let cell = components::cell(row, field)?;
        let same = match source.field(field) {
            None => {
                require_state(cell, "absent")?;
                continue;
            }
            Some(Original::Enum(expected) | Original::String(expected)) => {
                proof::text(cell, "/value")? == *expected
            }
            Some(Original::Bool(expected)) => {
                cell.get("value").and_then(Value::as_bool) == Some(*expected)
            }
            _ => return Err("unreadable original seat scalar".into()),
        };
        if !same {
            return Err(format!("original seat field {field} differs").into());
        }
        require_state(cell, "present")?;
    }
    Ok(())
}
