// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, StructValue, TreeBuilder, Value as Original,
    ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, Rows, require_state};
use crate::real_archive_item_durability as proof;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_fuel_configurations_match_physical_source_and_resolved_ship_ports() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options).map_err(|_| "build fuel catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "decode catalog")?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let tanks = components::component_rows(&catalog, "/configurations/fuelTanks/rows")?;
    let intakes = components::component_rows(&catalog, "/configurations/fuelIntakes/rows")?;
    let resources =
        components::component_rows(&catalog, "/configurations/resourceContainers/rows")?;
    compare_rows(&forge, &tanks, "SCItemFuelTankParams", |source, row| {
        for field in ["hydrogenMaxFlowMultiplier", "quantumMaxFlowMultiplier"] {
            single(source, field, components::cell(row, field)?)?;
        }
        Ok(())
    })?;
    compare_rows(&forge, &intakes, "SCItemFuelIntakeParams", |source, row| {
        for field in ["fuelPushRate", "minimumRate"] {
            single(source, field, components::cell(row, field)?)?;
        }
        Ok(())
    })?;
    compare_rows(&forge, &resources, "ResourceContainer", compare_container)?;
    compare_ship_instances(&catalog, &tanks, &intakes, &resources)?;
    watch
        .check()
        .map_err(|_| "Archive changed during fuel verification")?;
    for (fields, slots) in [
        (
            "hydrogenMaxFlowMultiplier, quantumMaxFlowMultiplier",
            tanks.len(),
        ),
        ("fuelPushRate, minimumRate", intakes.len()),
        (
            "capacity, defaultComposition.entry, defaultComposition.weight, defaultCompositionFillFactor",
            resources.len(),
        ),
    ] {
        crate::real_archive_verification::print_line(&format!(
            "fuel configurations: {fields} checked across {slots} component slots"
        ));
    }
    Ok(())
}

fn compare_rows<'source>(
    forge: &'source DataForge<'source>,
    rows: &Rows<'_>,
    kind: &str,
    mut compare: impl FnMut(&StructValue<'source>, &Value) -> TestResult,
) -> TestResult {
    let mut trees = TreeBuilder::new();
    for (&(owner, slot), row) in rows {
        let record = forge.record(owner).ok_or("physical fuel owner Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read physical fuel owner")?;
        let Some(Original::Array(values)) = tree.field("Components") else {
            return Err("physical fuel owner Components".into());
        };
        let Some(Original::Struct(component)) = values.get(slot) else {
            return Err("original fuel component slot".into());
        };
        if component.type_name() != kind
            || proof::text(row, "/sourceType/value")? != kind
            || proof::text(row, "/ownerGuid/value")? != tree.guid().to_string()
        {
            return Err("fuel component identity differs".into());
        }
        compare(component, row)?;
    }
    Ok(())
}

fn compare_container(source: &StructValue<'_>, row: &Value) -> TestResult {
    compare_capacity(source.field("capacity"), components::cell(row, "capacity")?)?;
    single(
        source,
        "defaultCompositionFillFactor",
        components::cell(row, "defaultCompositionFillFactor")?,
    )?;
    let cell = components::cell(row, "defaultComposition")?;
    let Some(original) = source.field("defaultComposition") else {
        return require_state(cell, "absent");
    };
    let Original::Array(original) = original else {
        return Err("original composition array".into());
    };
    let slots = proof::rows(cell, "/value")?;
    if original.len() != slots.len() {
        return Err("physical composition cardinality differs".into());
    }
    for (original, slot) in original.iter().zip(slots) {
        match original {
            Original::Null => require_state(slot, "null")?,
            Original::Struct(original) => {
                let value = at(slot, "/value")?;
                compare_reference(original.field("entry"), components::cell(value, "entry")?)?;
                single(original, "weight", components::cell(value, "weight")?)?;
            }
            _ => return Err("unreadable original composition slot".into()),
        }
    }
    Ok(())
}

fn compare_capacity(original: Option<&Original<'_>>, cell: &Value) -> TestResult {
    let unit = match original {
        None => return require_state(cell, "absent"),
        Some(Original::Null) => return require_state(cell, "null"),
        Some(Original::Struct(unit)) => unit,
        _ => return Err("unreadable original capacity unit".into()),
    };
    let value = at(cell, "/value")?;
    if proof::text(value, "/sourceType/value")? != unit.type_name() {
        return Err("physical capacity unit differs".into());
    }
    match unit.type_name() {
        "SBaseCargoUnit" => Ok(()),
        "SCentiCargoUnit" => integer(unit, "centiSCU", value),
        "SMicroCargoUnit" => integer(unit, "microSCU", value),
        "SStandardCargoUnit" => single(
            unit,
            "standardCargoUnits",
            components::cell(value, "standardCargoUnits")?,
        ),
        _ if proof::text(value, "/kind")? == "unknownUnit" => Ok(()),
        _ => Err("unknown authored unit was narrowed to a known unit".into()),
    }
}

fn integer(unit: &StructValue<'_>, field: &str, value: &Value) -> TestResult {
    let Some(Original::Int32(expected)) = unit.field(field) else {
        return Err("source capacity Int32".into());
    };
    let cell = components::cell(value, field)?;
    if at(cell, "/value")?.as_i64() != Some(i64::from(*expected)) {
        return Err("stored capacity Int32 differs".into());
    }
    require_state(cell, "present")
}

fn single(source: &StructValue<'_>, field: &str, cell: &Value) -> TestResult {
    match source.field(field) {
        None => require_state(cell, "absent"),
        Some(Original::Single(expected)) => {
            let bits = format!("{:08x}", expected.to_bits());
            if proof::text(cell, "/value/rawBits")? != bits {
                return Err("stored Single bits differ".into());
            }
            require_state(cell, "present")
        }
        _ => Err("unreadable original Single".into()),
    }
}

fn compare_reference(original: Option<&Original<'_>>, cell: &Value) -> TestResult {
    let (guid, status) = match original {
        None => return require_state(cell, "absent"),
        Some(Original::Null) => return require_state(cell, "null"),
        Some(Original::Reference(reference)) => (reference.guid(), "present"),
        Some(Original::Error(ValueError::DanglingReference { guid })) => (*guid, "dangling"),
        _ => return Err("unreadable original composition Reference".into()),
    };
    if proof::text(cell, "/value/guid/value")? != guid.to_string()
        || proof::text(cell, "/value/status")? != status
    {
        return Err("original composition Reference differs".into());
    }
    Ok(())
}

fn compare_ship_instances(
    catalog: &Value,
    tanks: &Rows<'_>,
    intakes: &Rows<'_>,
    resources: &Rows<'_>,
) -> TestResult {
    let intake_owners: BTreeSet<_> = intakes.keys().map(|&(owner, _)| owner).collect();
    let resources = tank_resources(tanks, resources);
    for ship in proof::rows(catalog, "/loadouts")? {
        if proof::text(ship, "/entityKind")? != "ship" {
            continue;
        }
        let mut stack: Vec<_> = proof::rows(ship, "/ports")?.iter().collect();
        let mut attached_tanks = BTreeSet::new();
        let mut attached_resources = BTreeSet::new();
        let mut intake = false;
        while let Some(port) = stack.pop() {
            if at(port, "/dockedVehicle")?.as_bool() == Some(true) {
                if !proof::rows(port, "/ports")?.is_empty() {
                    return Err("a docked ship contributed nested loadout placements".into());
                }
                continue;
            }
            stack.extend(proof::rows(port, "/ports")?);
            let Some(owner) = port
                .pointer("/sourceConstraints/attachedItem/targetRecordIndex/value")
                .and_then(Value::as_u64)
            else {
                continue;
            };
            let owner = u32::try_from(owner)?;
            if let Some(guids) = resources.get(&owner).filter(|guids| !guids.is_empty()) {
                attached_tanks.insert(owner);
                attached_resources.extend(guids.iter().copied());
            }
            intake |= intake_owners.contains(&owner);
        }
        if attached_tanks.len() >= 2 && attached_resources.len() >= 2 && intake {
            return Ok(());
        }
    }
    Err(
        "configured build needs a ship with two tanks, distinct authored resources and an intake"
            .into(),
    )
}

fn tank_resources<'row>(
    tanks: &Rows<'_>,
    resources: &Rows<'row>,
) -> BTreeMap<u32, BTreeSet<&'row str>> {
    let owners: BTreeSet<_> = tanks.keys().map(|&(owner, _)| owner).collect();
    let mut references = BTreeMap::<_, BTreeSet<_>>::new();
    for (&(owner, _), row) in resources {
        if !owners.contains(&owner) {
            continue;
        }
        let guids = references.entry(owner).or_default();
        for (array, link) in [
            ("/inclusiveResources/value", "/value"),
            ("/defaultComposition/value", "/value/entry/value"),
        ] {
            let values = row.pointer(array).and_then(Value::as_array);
            guids.extend(values.into_iter().flatten().filter_map(|slot| {
                let reference = slot.pointer(link)?;
                if reference.get("status")?.as_str()? != "present" {
                    return None;
                }
                reference.pointer("/guid/value")?.as_str()
            }));
        }
    }
    references
}

fn at<'row>(value: &'row Value, path: &str) -> TestResult<&'row Value> {
    value
        .pointer(path)
        .ok_or_else(|| "required fuel configuration value".into())
}
