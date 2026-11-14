// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, Guid, StructValue, TreeBuilder,
    Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_item_components::{self as components, Rows, require_state};
use crate::real_archive_item_durability as proof;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[path = "fuel_ship_witness.rs"]
mod ship_witness;
#[path = "fuel_source_witness.rs"]
mod witness;

use ship_witness::{FuelKind, ShipWitness};
use witness::Witness;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_fuel_configurations_match_physical_source_and_resolved_ship_ports() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let ship = ShipWitness::select(&archive, &forge)?;
    let mut hydrogen = BTreeSet::new();
    let mut quantum = BTreeSet::new();
    for (&owner, &(kind, record_path)) in &ship.owners {
        let catalog = scoped_catalog(&archive, &entry, record_path)?;
        let origins = Origins::read(&catalog, entry.origin())?;
        let references = compare_owner(&forge, &catalog, &origins, owner, kind)?;
        match kind {
            FuelKind::Hydrogen => hydrogen.extend(references),
            FuelKind::Quantum => quantum.extend(references),
            FuelKind::Intake => {}
        }
    }
    assert!(
        !hydrogen.is_empty() && !quantum.is_empty() && hydrogen.is_disjoint(&quantum),
        "authored hydrogen and quantum tank resources retain distinct identities"
    );
    let catalog = scoped_catalog(&archive, &entry, ship.record_path)?;
    ship.compare(&catalog)?;

    watch
        .check()
        .map_err(|_| "Archive changed during fuel verification")?;
    Ok(())
}

fn scoped_catalog(archive: &Archive, entry: &DataForgeEntry, scope: &str) -> TestResult<Value> {
    let filter = Filter::new(scope);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built =
        build_catalog_from(archive, entry, &options).map_err(|_| "build scoped fuel catalog")?;
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "decode scoped fuel catalog")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn compare_owner(
    forge: &DataForge<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
    owner: u32,
    kind: FuelKind,
) -> TestResult<BTreeSet<Guid>> {
    let (table, source_type, fields): (_, _, &[_]) = match kind {
        FuelKind::Hydrogen | FuelKind::Quantum => (
            "/configurations/fuelTanks/rows",
            "SCItemFuelTankParams",
            &["hydrogenMaxFlowMultiplier", "quantumMaxFlowMultiplier"],
        ),
        FuelKind::Intake => (
            "/configurations/fuelIntakes/rows",
            "SCItemFuelIntakeParams",
            &["fuelPushRate", "minimumRate"],
        ),
    };
    let mut rows = components::component_rows(catalog, table)?;
    rows.retain(|(index, _), _| *index == owner);
    let mut checked = compare_rows(forge, origins, &rows, source_type, |source, row| {
        for field in fields {
            single(source, field, components::cell(row, field)?)?;
        }
        Ok(())
    })?;
    let mut references = BTreeSet::new();
    if kind != FuelKind::Intake {
        let mut resources =
            components::component_rows(catalog, "/configurations/resourceContainers/rows")?;
        resources.retain(|(index, _), _| *index == owner);
        checked += compare_rows(
            forge,
            origins,
            &resources,
            "ResourceContainer",
            compare_container,
        )?;
        for row in resources.values() {
            references.extend(resource_references(row)?);
        }
    }
    crate::real_archive_verification::print_line(&format!(
        "fuel configurations: {kind:?}, {checked} exact original field origins"
    ));
    Ok(references)
}

fn compare_rows<'source>(
    forge: &'source DataForge<'source>,
    origins: &Origins<'_>,
    rows: &Rows<'_>,
    kind: &str,
    mut compare: impl FnMut(&StructValue<'source>, &Value) -> TestResult,
) -> TestResult<usize> {
    assert!(
        !rows.is_empty(),
        "the installed witness needs original {kind} slots"
    );
    let mut checked = 0;
    let mut trees = TreeBuilder::new();
    for (&(owner, slot), row) in rows {
        let record = forge.record(owner).ok_or("physical fuel owner Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read physical fuel owner")?;
        let Some(Original::Array(values)) = tree.field("Components") else {
            return Err("physical fuel owner Components".into());
        };
        let Some(original @ Original::Struct(component)) = values.get(slot) else {
            return Err("original fuel component slot".into());
        };
        if component.type_name() != kind
            || proof::text(row, "/sourceType/value")? != kind
            || proof::text(row, "/ownerGuid/value")? != tree.guid().to_string()
        {
            return Err("fuel component identity differs".into());
        }
        let witness = Witness {
            owner,
            locations: Locations::read(tree.root()),
            origins,
        };
        witness.occurrence(
            original,
            components::cell(row, "componentOrdinal")?,
            "$value",
        )?;
        witness.occurrence(original, components::cell(row, "sourceType")?, "$type")?;
        checked += witness.compare(forge, component, row)?;
        compare(component, row)?;
    }
    Ok(checked)
}

fn compare_container(source: &StructValue<'_>, row: &Value) -> TestResult {
    compare_capacity(source.field("capacity"), components::cell(row, "capacity")?)?;

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

fn resource_references(row: &Value) -> TestResult<BTreeSet<Guid>> {
    let mut references = BTreeSet::new();
    for (array, link) in [
        ("/inclusiveResources/value", "/value"),
        ("/defaultComposition/value", "/value/entry/value"),
    ] {
        let values = row.pointer(array).and_then(Value::as_array);
        for slot in values.into_iter().flatten() {
            let Some(reference) = slot.pointer(link) else {
                continue;
            };
            if proof::text(reference, "/status")? == "present" {
                references.insert(proof::text(reference, "/guid/value")?.parse()?);
            }
        }
    }
    Ok(references)
}

fn at<'row>(value: &'row Value, path: &str) -> TestResult<&'row Value> {
    value
        .pointer(path)
        .ok_or_else(|| "required fuel configuration value".into())
}
