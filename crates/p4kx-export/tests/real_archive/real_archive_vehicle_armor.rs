// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, ExportDiagnosticSeverity, Kind, build_catalog_from,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, StructValue, TreeBuilder,
    Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_item_components::{self as components, owned, require_state, single};
use crate::real_archive_item_durability as proof;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const TABLE: &str = "/configurations/vehicleArmor/rows";
const ARMOR: &str = "SCItemVehicleArmorParams";
const FILTER: &str = "libs/foundry/records/entities/scitem/ships/armor/*";
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
    let filter = Filter::new(FILTER);
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let expected = original_slots(&forge, &filter, &watch)?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build scoped vehicle armor catalog")?;
    for diagnostic in built.summary().export_health().diagnostics() {
        if diagnostic.severity != ExportDiagnosticSeverity::Information {
            assert!(
                diagnostic.kind.is_some_and(|kind| kind != Kind::Items),
                "blocking vehicle armour source diagnostic: {:?}",
                diagnostic.code
            );
            crate::real_archive_verification::print_line(&format!(
                "scoped vehicle armour catalog also reports {:?} {:?}: {}",
                diagnostic.kind, diagnostic.code, diagnostic.count
            ));
        }
    }
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let origins = Origins::read(&catalog, entry.origin())?;
    compare_rows(&forge, &catalog, &origins, &expected)?;
    watch
        .check()
        .map_err(|_| "Archive changed during vehicle armour verification")?;
    crate::real_archive_verification::print_line(&format!(
        "vehicle armour: {} physical signature, multiplier, penetration and deflection scopes checked",
        expected.len()
    ));
    Ok(())
}

fn original_slots(
    forge: &DataForge<'_>,
    filter: &Filter,
    watch: &ArchiveWatch,
) -> TestResult<BTreeSet<(u32, usize)>> {
    let rule = Kind::Items.rule().ok_or("selected item rule")?;
    let probe = forge
        .pointer_probe("Components", ARMOR)
        .ok_or("original vehicle armour type declaration")?;
    let mut trees = TreeBuilder::new();
    let mut selected = BTreeSet::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during armour source selection")?;
        if !components::selected_item(&record, &rule, filter)?
            || !components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original vehicle armour owner")?;
        for (slot, component) in slots(&tree)?.iter().enumerate() {
            if matches!(component, Original::Struct(component) if component.type_name() == ARMOR) {
                selected.insert((record.index(), slot));
            }
        }
    }
    assert!(
        !selected.is_empty(),
        "configured build contains original vehicle armour slots"
    );
    Ok(selected)
}

fn slots<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> TestResult<&'tree [Original<'source>]> {
    let Some(Original::Array(entries)) = tree.field("Components") else {
        return Err("original vehicle armour owner Components".into());
    };
    Ok(entries)
}

fn compare_rows(
    forge: &DataForge<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
    expected: &BTreeSet<(u32, usize)>,
) -> TestResult {
    let rows = components::component_rows(catalog, TABLE)?;
    assert!(
        rows.keys().eq(expected),
        "every selected original vehicle armour slot is exported exactly once"
    );
    let mut trees = TreeBuilder::new();
    for ((owner, slot), row) in rows {
        let record = forge.record(owner).ok_or("original armour owner Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original armour owner")?;
        let original = slots(&tree)?.get(slot).ok_or("original armour slot")?;
        let Original::Struct(source) = original else {
            return Err("original vehicle armour component slot".into());
        };
        assert!(
            source.type_name() == ARMOR
                && proof::text(row, "/sourceType/value")? == source.type_name()
                && proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
            "original armour component identity"
        );
        let locations = Locations::read(tree.root());
        let ordinal = locations.ordinal(original)?;
        origins.occurrence(
            components::cell(row, "componentOrdinal")?,
            owner,
            ordinal,
            "$value",
        )?;
        origins.occurrence(
            components::cell(row, "sourceType")?,
            owner,
            ordinal,
            "$type",
        )?;
        for name in [
            "signalInfrared",
            "signalElectromagnetic",
            "signalCrossSection",
        ] {
            if let Some(value) = source.field(name) {
                origins.occurrence(
                    components::cell(row, name)?,
                    owner,
                    locations.ordinal(value)?,
                    "$value",
                )?;
            }
        }
        compare_armor(source, row)?;
    }
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
