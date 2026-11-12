// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, StructValue, TreeBuilder, Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, owned, require_state, single};
use crate::real_archive_item_durability as proof;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const TABLE: &str = "/configurations/vehicleArmor/rows";
const DAMAGE: [(&str, &str); 6] = [
    ("DamagePhysical", "physical"),
    ("DamageEnergy", "energy"),
    ("DamageDistortion", "distortion"),
    ("DamageThermal", "thermal"),
    ("DamageBiochemical", "biochemical"),
    ("DamageStun", "stun"),
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn vehicle_armor_tuning_matches_the_original_physical_components() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build vehicle armor catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let rows = components::component_rows(&catalog, TABLE)?;
    if rows.is_empty() {
        return Err("configured build contains no traced vehicle armour".into());
    }
    let mut trees = TreeBuilder::new();
    for (&(owner, slot), row) in &rows {
        let record = forge.record(owner).ok_or("original armour owner Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original armour owner")?;
        let Some(Original::Array(entries)) = tree.field("Components") else {
            return Err("original armour Components".into());
        };
        let Some(Original::Struct(source)) = entries.get(slot) else {
            return Err("original vehicle armour component slot".into());
        };
        if source.type_name() != "SCItemVehicleArmorParams"
            || proof::text(row, "/sourceType/value")? != source.type_name()
            || proof::text(row, "/ownerGuid/value")? != tree.guid().to_string()
        {
            return Err("original armour component identity differs".into());
        }
        compare_armor(source, row)?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during vehicle armour verification")?;
    crate::real_archive_verification::print_line(&format!(
        "vehicle armour: {} physical signature, multiplier, penetration and deflection scopes checked",
        rows.len()
    ));
    Ok(())
}

fn compare_armor(source: &StructValue<'_>, row: &Value) -> TestResult {
    for field in [
        "signalInfrared",
        "signalElectromagnetic",
        "signalCrossSection",
    ] {
        single(source, field, components::cell(row, field)?)?;
    }
    owned(source, row, "damageMultiplier", compare_multiplier)?;
    owned(source, row, "armorPenetrationResistance", |source, row| {
        single(
            source,
            "basePenetrationReduction",
            components::cell(row, "basePenetrationReduction")?,
        )?;
        owned(source, row, "penetrationAbsorptionForType", compare_damage)
    })?;
    owned(source, row, "armorDeflection", |source, row| {
        owned(source, row, "deflectionValue", compare_damage)?;
        owned(source, row, "healthCurve", compare_curve)
    })
}

fn compare_multiplier(source: &StructValue<'_>, row: &Value) -> TestResult {
    match source.type_name() {
        "DamageBase" if row["kind"] == "base" => Ok(()),
        "DamageInfo" if row["kind"] == "info" => compare_damage(source, row),
        "DamageParams" if row["kind"] == "parameters" => {
            single(source, "damageTotal", components::cell(row, "damageTotal")?)?;
            compare_macro(
                source.field("damageMacro"),
                components::cell(row, "damageMacro")?,
            )
        }
        _ if row["kind"] == "unknown" => {
            let roots = proof::rows(row, "/additionalFields/roots")?;
            if roots.len() != source.fields().len() {
                return Err("unknown authored damage field inventory differs".into());
            }
            for (field, root) in source.fields().iter().zip(roots) {
                if proof::text(root, "/name/value")? != field.name() {
                    return Err("unknown authored damage field order differs".into());
                }
            }
            Ok(())
        }
        _ => Err("original damage multiplier kind differs".into()),
    }
}

fn compare_macro(original: Option<&Original<'_>>, cell: &Value) -> TestResult {
    let (guid, status) = match original {
        None => return require_state(cell, "absent"),
        Some(Original::Null) => return require_state(cell, "null"),
        Some(Original::Reference(reference)) => (reference.guid(), "present"),
        Some(Original::Error(ValueError::DanglingReference { guid })) => (*guid, "dangling"),
        Some(Original::Error(ValueError::UnexpectedReferenceType { guid, .. })) => {
            (*guid, "unexpectedTargetType")
        }
        _ => return Err("unreadable original damage Macro".into()),
    };
    if proof::text(cell, "/value/value/guid/value")? != guid.to_string()
        || proof::text(cell, "/value/value/status")? != status
    {
        return Err("original damage Macro reference differs".into());
    }
    Ok(())
}

fn compare_damage(source: &StructValue<'_>, row: &Value) -> TestResult {
    for (source_field, field) in DAMAGE {
        single(source, source_field, components::cell(row, field)?)?;
    }
    Ok(())
}

fn compare_curve(source: &StructValue<'_>, row: &Value) -> TestResult {
    let use_lut = components::cell(row, "useLut")?;
    match source.field("useLUT") {
        None => require_state(use_lut, "absent")?,
        Some(Original::Bool(value))
            if use_lut.get("value").and_then(Value::as_bool) == Some(*value) =>
        {
            require_state(use_lut, "present")?;
        }
        _ => return Err("original Bezier useLUT differs".into()),
    }
    let points = components::cell(row, "points")?;
    let source = match source.field("points") {
        None => return require_state(points, "absent"),
        Some(Original::Null) => return require_state(points, "null"),
        Some(Original::Array(values)) => values,
        _ => return Err("unreadable original Bezier point array".into()),
    };
    let slots = proof::rows(points, "/value")?;
    if source.len() != slots.len() {
        return Err("original Bezier point count differs".into());
    }
    for (point, slot) in source.iter().zip(slots) {
        match point {
            Original::Null => require_state(slot, "null")?,
            Original::Struct(point) => {
                let output = slot.get("value").ok_or("original Bezier point")?;
                for field in ["x", "y"] {
                    single(point, field, components::cell(output, field)?)?;
                }
            }
            _ => return Err("unreadable original Bezier point".into()),
        }
    }
    Ok(())
}
